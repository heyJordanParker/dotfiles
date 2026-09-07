//! Repo-wide scc context, persisted while its filesystem inputs stay unchanged.

use crate::{cache, memo, repo_files};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};

const CACHE_KEY_PREFIX: &str = "repo_context_v5_";
const SCC_ARGS: [&str; 5] = [
    "--format",
    "json",
    "--by-file",
    "--exclude-dir",
    ".tracer-cache",
];

fn empty_payload() -> Payload {
    Payload::default()
}

fn executable_path(name: &str) -> Result<PathBuf> {
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let path = directory.join(name);
        if path
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
            .unwrap_or(false)
        {
            return path
                .canonicalize()
                .with_context(|| format!("resolve {}", path.display()));
        }
    }
    anyhow::bail!("scc executable not found on PATH")
}

fn fingerprint(repo_root: &Path, executable: &Path) -> Result<Option<String>> {
    let Some(files) = repo_files::tracked_files(repo_root, None) else {
        return Ok(None);
    };
    fingerprint_files(executable, &files).map(Some)
}

fn fingerprint_uncached(repo_root: &Path, executable: &Path) -> Result<Option<String>> {
    let Some(files) = repo_files::stamped_files_uncached(repo_root) else {
        return Ok(None);
    };
    fingerprint_files(executable, &files).map(Some)
}

fn fingerprint_files(executable: &Path, files: &repo_files::TrackedFiles) -> Result<String> {
    let executable_metadata =
        fs::metadata(executable).with_context(|| format!("stat {}", executable.display()))?;

    let mut hasher = Sha256::new();
    hasher.update(b"repo-context-filesystem-v5\0");
    hasher.update(executable.as_os_str().as_bytes());
    for value in [
        executable_metadata.mode() as u64,
        executable_metadata.len(),
        executable_metadata.mtime() as u64,
        executable_metadata.mtime_nsec() as u64,
        executable_metadata.ctime() as u64,
        executable_metadata.ctime_nsec() as u64,
        executable_metadata.ino(),
    ] {
        hasher.update(value.to_le_bytes());
    }
    for argument in SCC_ARGS {
        hasher.update(argument.as_bytes());
        hasher.update(b"\0");
    }
    for (path, stamp) in files.iter().zip(&files.stamps) {
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        for value in [
            stamp.mode as u64,
            stamp.size,
            stamp.mtime as u64,
            stamp.mtime_nsec as u64,
            stamp.ctime as u64,
            stamp.ctime_nsec as u64,
            stamp.inode,
        ] {
            hasher.update(value.to_le_bytes());
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

fn compute(repo_root: &Path, executable: &Path) -> Result<Payload> {
    let out = Command::new(executable)
        .args(SCC_ARGS)
        .arg(repo_root)
        .output()
        .context("start scc")?;
    if !out.status.success() {
        anyhow::bail!(
            "scc failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let languages_input: Vec<SccLanguage> =
        serde_json::from_slice(&out.stdout).context("parse scc output")?;
    let root_resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut complexities = Vec::new();
    let mut per_file = BTreeMap::new();
    let mut languages = Vec::new();

    for language_input in languages_input {
        let language = language_input.name;
        languages.push(LanguageRow {
            name: language.clone(),
            count: language_input.count,
            code: language_input.code,
            complexity: language_input.complexity,
        });
        let files = language_input.files;
        for file in files {
            complexities.push(file.complexity);
            let relative = Path::new(&file.location)
                .strip_prefix(&root_resolved)
                .unwrap_or_else(|_| Path::new(&file.location))
                .to_string_lossy()
                .to_string();
            per_file.insert(
                relative,
                FileMetrics {
                    ccn: file.complexity,
                    loc: file.code,
                    language: language.clone(),
                },
            );
        }
    }

    complexities.sort_unstable();
    let p95 = if complexities.is_empty() {
        0
    } else {
        complexities[(((complexities.len() as f64) * 0.95) as i64 - 1).max(0) as usize]
    };
    Ok(Payload {
        available: true,
        summary: Summary {
            total_files: complexities.len() as i64,
            median_file_ccn: median_int(&complexities),
            complexity_p95: p95,
        },
        per_file,
        languages,
    })
}

fn median_int(sorted: &[i64]) -> i64 {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        ((sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0) as i64
    }
}

fn load_or_compute(repo_root: &Path) -> Arc<Payload> {
    static MEMO: memo::Memo<Payload> = OnceLock::new();
    memo::get_or_build(&MEMO, repo_root, || load_or_compute_uncached(repo_root))
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Summary {
    pub total_files: i64,
    pub median_file_ccn: i64,
    pub complexity_p95: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FileMetrics {
    pub ccn: i64,
    pub loc: i64,
    pub language: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LanguageRow {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Count")]
    pub count: i64,
    #[serde(rename = "Code")]
    pub code: i64,
    #[serde(rename = "Complexity")]
    pub complexity: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Payload {
    pub(crate) available: bool,
    pub summary: Summary,
    pub per_file: BTreeMap<String, FileMetrics>,
    pub languages: Vec<LanguageRow>,
}

#[derive(Deserialize)]
struct SccLanguage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Count")]
    count: i64,
    #[serde(rename = "Code")]
    code: i64,
    #[serde(rename = "Complexity")]
    complexity: i64,
    #[serde(rename = "Files")]
    files: Vec<SccFile>,
}

#[derive(Deserialize)]
struct SccFile {
    #[serde(rename = "Complexity")]
    complexity: i64,
    #[serde(rename = "Code")]
    code: i64,
    #[serde(rename = "Location")]
    location: String,
}

fn load_or_compute_uncached(repo_root: &Path) -> Payload {
    if !repo_root.join(".git").exists() {
        return empty_payload();
    }
    let executable = match executable_path("scc") {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Error: repo context unavailable: {error:#}");
            return empty_payload();
        }
    };
    let before = match fingerprint(repo_root, &executable) {
        Ok(Some(value)) => value,
        Ok(None) => {
            eprintln!("Error: repo context input scan failed: git file discovery unavailable");
            return empty_payload();
        }
        Err(error) => {
            eprintln!("Error: repo context unavailable: {error:#}");
            return empty_payload();
        }
    };
    let key = format!("{CACHE_KEY_PREFIX}{before}");
    if let Some(cached) = cache::load_bytes(cache::NAMESPACE_FILE, &key, repo_root)
        .and_then(|bytes| serde_json::from_slice::<Payload>(&bytes).ok())
    {
        if cached.available {
            return cached;
        }
    }
    let payload = match compute(repo_root, &executable) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Error: repo context unavailable: {error:#}");
            return empty_payload();
        }
    };
    match fingerprint_uncached(repo_root, &executable) {
        Ok(Some(after)) if after == before => {
            if serde_json::to_value(&payload)
                .ok()
                .and_then(|value| cache::save(cache::NAMESPACE_FILE, &key, &value, repo_root).ok())
                .is_some()
            {
                cache::evict_prefixed(cache::NAMESPACE_FILE, "repo_context_v", &key, repo_root);
            }
        }
        Ok(Some(_)) => eprintln!(
            "Error: repo context inputs changed while scc was running; snapshot not cached"
        ),
        Ok(None) => {
            eprintln!("Error: repo context input rescan failed: git file discovery unavailable")
        }
        Err(error) => eprintln!("Error: repo context unavailable: {error:#}"),
    }
    payload
}

pub fn repo_context(path: &Path) -> Value {
    let root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    serde_json::to_value(&load_or_compute(&root).summary).unwrap_or_else(|_| json!({}))
}

pub fn metrics(repo_root: &Path) -> Arc<Payload> {
    load_or_compute(repo_root)
}

pub fn language_summary(repo_root: &Path) -> Vec<LanguageRow> {
    load_or_compute(repo_root).languages.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(path: &Path) -> repo_files::Stamp {
        let metadata = fs::symlink_metadata(path).unwrap();
        repo_files::Stamp {
            mode: metadata.mode(),
            size: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
            inode: metadata.ino(),
        }
    }

    #[test]
    fn missing_paths_do_not_change_the_fingerprint() {
        let directory = tempfile::tempdir().unwrap();
        let present = directory.path().join("present.py");
        fs::write(&present, "value = 1\n").unwrap();

        let without_missing = repo_files::TrackedFiles {
            paths: vec!["present.py".to_string()],
            stamps: vec![stamp(&present)],
        };
        let with_missing = repo_files::TrackedFiles {
            paths: vec!["present.py".to_string(), "missing.py".to_string()],
            stamps: vec![stamp(&present)],
        };

        assert_eq!(
            fingerprint_files(Path::new("/bin/sh"), &with_missing).unwrap(),
            fingerprint_files(Path::new("/bin/sh"), &without_missing).unwrap(),
        );
    }

    #[test]
    fn summary_cache_shape_round_trips() {
        let summary = Summary {
            total_files: 2,
            median_file_ccn: 3,
            complexity_p95: 5,
        };

        let bytes = serde_json::to_vec(&summary).unwrap();

        assert_eq!(serde_json::from_slice::<Summary>(&bytes).unwrap(), summary);
    }

    #[test]
    fn file_metrics_cache_shape_round_trips() {
        let metrics = FileMetrics {
            ccn: 3,
            loc: 12,
            language: "Rust".to_string(),
        };

        let bytes = serde_json::to_vec(&metrics).unwrap();

        assert_eq!(
            serde_json::from_slice::<FileMetrics>(&bytes).unwrap(),
            metrics
        );
    }

    #[test]
    fn language_row_cache_shape_round_trips() {
        let language = LanguageRow {
            name: "Rust".to_string(),
            count: 2,
            code: 12,
            complexity: 3,
        };

        let bytes = serde_json::to_vec(&language).unwrap();

        assert_eq!(
            serde_json::from_slice::<LanguageRow>(&bytes).unwrap(),
            language
        );
    }

    #[test]
    fn payload_cache_shape_round_trips() {
        let payload = Payload {
            available: true,
            summary: Summary {
                total_files: 2,
                median_file_ccn: 3,
                complexity_p95: 5,
            },
            per_file: BTreeMap::from([(
                "src/main.rs".to_string(),
                FileMetrics {
                    ccn: 3,
                    loc: 12,
                    language: "Rust".to_string(),
                },
            )]),
            languages: vec![LanguageRow {
                name: "Rust".to_string(),
                count: 2,
                code: 12,
                complexity: 3,
            }],
        };

        let bytes = serde_json::to_vec(&payload).unwrap();

        assert_eq!(serde_json::from_slice::<Payload>(&bytes).unwrap(), payload);
    }

    #[test]
    fn installed_v5_entry_deserializes() {
        let bytes = include_bytes!("../tests/fixtures/repo_context_v5_installed.json");
        let payload = serde_json::from_slice::<Payload>(bytes).unwrap();

        assert!(payload.available);
        assert_eq!(payload.per_file["sample.rs"].language, "Rust");
    }

    #[test]
    fn installed_v5_entry_is_served() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        assert!(Command::new("git")
            .arg("init")
            .current_dir(root)
            .status()
            .unwrap()
            .success());
        fs::write(root.join("sample.rs"), "").unwrap();
        assert!(Command::new("git")
            .args(["add", "sample.rs"])
            .current_dir(root)
            .status()
            .unwrap()
            .success());
        let executable = executable_path("scc").unwrap();
        let key = format!(
            "{CACHE_KEY_PREFIX}{}",
            fingerprint(root, &executable).unwrap().unwrap()
        );
        let cache_dir = root.join(".tracer-cache/file");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join(format!("{key}.json")),
            include_bytes!("../tests/fixtures/repo_context_v5_installed.json"),
        )
        .unwrap();

        let payload = load_or_compute_uncached(root);

        assert_eq!(payload.per_file["sample.rs"].language, "Rust");
    }

    #[test]
    fn unavailable_v5_entry_deserializes() {
        let payload = serde_json::from_slice::<Payload>(
            br#"{"available":false,"summary":{"total_files":0,"median_file_ccn":0,"complexity_p95":0},"per_file":{},"languages":[]}"#,
        )
        .unwrap();

        assert!(!payload.available);
    }

    #[test]
    fn unchanged_computes_write_identical_cache_bytes() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(".git")).unwrap();
        fs::write(directory.path().join("fixture.rs"), "fn fixture() {}\n").unwrap();
        let executable = executable_path("scc").unwrap();

        let first = compute(directory.path(), &executable).unwrap();
        let first_value = serde_json::to_value(first).unwrap();
        cache::save(
            "file",
            "repo_context_v5_fixture",
            &first_value,
            directory.path(),
        )
        .unwrap();
        let first_bytes = fs::read(
            directory
                .path()
                .join(".tracer-cache/file/repo_context_v5_fixture.json"),
        )
        .unwrap();

        let second = compute(directory.path(), &executable).unwrap();
        let second_value = serde_json::to_value(second).unwrap();
        cache::save(
            "file",
            "repo_context_v5_fixture",
            &second_value,
            directory.path(),
        )
        .unwrap();
        let second_bytes = fs::read(
            directory
                .path()
                .join(".tracer-cache/file/repo_context_v5_fixture.json"),
        )
        .unwrap();

        assert_eq!(second_bytes, first_bytes);
    }
}
