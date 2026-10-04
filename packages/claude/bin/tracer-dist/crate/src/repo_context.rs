//! Repo-wide scc metrics: one stored entry, refreshed for the files whose
//! stamps moved since it was written.
//!
//! The entry keeps every counted file's metrics beside the stamp it was
//! counted at. A call compares the stamps it holds against the tracked
//! files' current ones and hands scc only the paths that differ, so an edit
//! costs one scc run over one file instead of a walk over the repository.
//! scc reads its ignore files only when it walks, and counts a path it is
//! handed whether or not the walk would have. So a new path is tested
//! against the tracked ignore files here, with gitignore semantics, and
//! counted alone when the walk would count it. A change to an ignore file,
//! or to the scc binary itself, recounts the whole tree.

use crate::{cache, memo, repo_files};
use anyhow::{Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
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

const CACHE_KEY: &str = "repo_context_v6";
const SCC_ARGS: [&str; 5] = [
    "--format",
    "json",
    "--by-file",
    "--exclude-dir",
    ".tracer-cache",
];
/// The files scc reads while walking and never when handed paths, so a
/// change to one of them is a change to what the walk would count.
const IGNORE_FILES: [&str; 3] = [".sccignore", ".gitignore", ".ignore"];

fn is_ignore_file(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORE_FILES.contains(&name))
}

/// What the walk leaves out, read from the tracked ignore files. Each file
/// governs its own directory, and the deepest file that names a path
/// decides, which is how the walk itself reads them. Only files git lists
/// count: an untracked ignore file is one the next call sees as a new path.
struct WalkSkips {
    /// Deepest directory first, each with the directory it governs.
    matchers: Vec<(PathBuf, Gitignore)>,
}

impl WalkSkips {
    fn new(repo_root: &Path, tracked: &[String]) -> Self {
        let mut ignore_files: Vec<&String> =
            tracked.iter().filter(|path| is_ignore_file(path)).collect();
        ignore_files.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
        let matchers = ignore_files
            .into_iter()
            .filter_map(|path| {
                let directory = repo_root.join(Path::new(path).parent()?);
                let mut builder = GitignoreBuilder::new(&directory);
                if builder.add(repo_root.join(path)).is_some() {
                    return None;
                }
                Some((directory, builder.build().ok()?))
            })
            .collect();
        Self { matchers }
    }

    fn skips(&self, repo_root: &Path, path: &str) -> bool {
        let absolute = repo_root.join(path);
        self.matchers
            .iter()
            .filter(|(directory, _)| absolute.starts_with(directory))
            .find_map(|(_, matcher)| {
                let matched = matcher.matched_path_or_any_parents(&absolute, false);
                (!matched.is_none()).then(|| matched.is_ignore())
            })
            .unwrap_or(false)
    }
}
/// Paths handed to one scc invocation, well inside the argument limit.
const SCC_BATCH: usize = 256;

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

/// The scc binary's identity: its path, stat, and the arguments it is run
/// with. A replaced binary counts differently, so its rows are recounted.
fn executable_identity(executable: &Path) -> Result<String> {
    let metadata =
        fs::metadata(executable).with_context(|| format!("stat {}", executable.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(b"repo-context-executable-v6\0");
    hasher.update(executable.as_os_str().as_bytes());
    for value in [
        metadata.mode() as u64,
        metadata.len(),
        metadata.mtime() as u64,
        metadata.mtime_nsec() as u64,
        metadata.ctime() as u64,
        metadata.ctime_nsec() as u64,
        metadata.ino(),
    ] {
        hasher.update(value.to_le_bytes());
    }
    for argument in SCC_ARGS {
        hasher.update(argument.as_bytes());
        hasher.update(b"\0");
    }
    Ok(hex::encode(hasher.finalize()))
}

fn current_stamps(files: &repo_files::TrackedFiles) -> BTreeMap<String, repo_files::Stamp> {
    files
        .iter()
        .cloned()
        .zip(files.stamps.iter().cloned())
        .collect()
}

/// scc's per-file rows, keyed by repo-relative path: the whole repository
/// when `targets` is None, the named files otherwise.
fn scc_rows(
    repo_root: &Path,
    executable: &Path,
    targets: Option<&[&str]>,
) -> Result<BTreeMap<String, FileMetrics>> {
    let root_resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut rows = BTreeMap::new();
    let batches: Vec<Vec<PathBuf>> = match targets {
        None => vec![vec![repo_root.to_path_buf()]],
        Some(paths) => paths
            .chunks(SCC_BATCH)
            .map(|chunk| chunk.iter().map(|path| repo_root.join(path)).collect())
            .collect(),
    };
    for batch in batches {
        let out = Command::new(executable)
            .args(SCC_ARGS)
            .args(&batch)
            .output()
            .context("start scc")?;
        if !out.status.success() {
            anyhow::bail!(
                "scc failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let languages: Vec<SccLanguage> =
            serde_json::from_slice(&out.stdout).context("parse scc output")?;
        for language in languages {
            for file in language.files {
                let relative = Path::new(&file.location)
                    .strip_prefix(&root_resolved)
                    .unwrap_or_else(|_| Path::new(&file.location))
                    .to_string_lossy()
                    .to_string();
                rows.insert(
                    relative,
                    FileMetrics {
                        ccn: file.complexity,
                        loc: file.code,
                        language: language.name.clone(),
                    },
                );
            }
        }
    }
    Ok(rows)
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

/// The summary and per-language rows, both derived from the per-file rows.
fn payload_from(per_file: BTreeMap<String, FileMetrics>) -> Payload {
    let mut complexities: Vec<i64> = per_file.values().map(|file| file.ccn).collect();
    complexities.sort_unstable();
    let p95 = if complexities.is_empty() {
        0
    } else {
        complexities[(((complexities.len() as f64) * 0.95) as i64 - 1).max(0) as usize]
    };
    let mut by_language: BTreeMap<&str, LanguageRow> = BTreeMap::new();
    for file in per_file.values() {
        let row = by_language
            .entry(file.language.as_str())
            .or_insert_with(|| LanguageRow {
                name: file.language.clone(),
                count: 0,
                code: 0,
                complexity: 0,
            });
        row.count += 1;
        row.code += file.loc;
        row.complexity += file.ccn;
    }
    Payload {
        available: true,
        summary: Summary {
            total_files: complexities.len() as i64,
            median_file_ccn: median_int(&complexities),
            complexity_p95: p95,
        },
        languages: by_language.into_values().collect(),
        per_file,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LanguageRow {
    pub name: String,
    pub count: i64,
    pub code: i64,
    pub complexity: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Payload {
    pub(crate) available: bool,
    pub summary: Summary,
    pub per_file: BTreeMap<String, FileMetrics>,
    pub languages: Vec<LanguageRow>,
}

/// The entry on disk: every counted file's metrics and the stamp it was
/// counted at, plus the identity of the scc that counted them.
#[derive(Default, Deserialize, Serialize)]
struct Stored {
    executable: String,
    per_file: BTreeMap<String, FileMetrics>,
    stamps: BTreeMap<String, repo_files::Stamp>,
}

#[derive(Deserialize)]
struct SccLanguage {
    #[serde(rename = "Name")]
    name: String,
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

/// What a call must recount to bring `stored` up to `current`.
enum Refresh {
    Nothing,
    Whole,
    Paths {
        changed: Vec<String>,
        removed: Vec<String>,
    },
}

fn refresh_for(
    stored: &Stored,
    identity: &str,
    current: &BTreeMap<String, repo_files::Stamp>,
    skipped: impl Fn(&str) -> bool,
) -> Refresh {
    if stored.executable != identity {
        return Refresh::Whole;
    }
    // Handed a path, scc counts it whether or not the walk would have. A
    // file the last walk counted is recounted by path; a path the walk saw
    // and left out stays left out; a path the walk has never seen is counted
    // by path unless the ignore files say the walk would skip it. An ignore
    // file's own change sends the whole tree back through the walk.
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let mut walk = false;
    for (path, stamp) in current {
        match stored.stamps.get(path) {
            Some(known) if known == stamp => {}
            Some(_) if is_ignore_file(path) => walk = true,
            Some(_) if stored.per_file.contains_key(path) => changed.push(path.clone()),
            Some(_) => {}
            None if is_ignore_file(path) => walk = true,
            None if skipped(path) => {}
            None => changed.push(path.clone()),
        }
    }
    for path in stored.stamps.keys() {
        if !current.contains_key(path) {
            if is_ignore_file(path) {
                walk = true;
            }
            removed.push(path.clone());
        }
    }
    if walk {
        return Refresh::Whole;
    }
    if changed.is_empty() && removed.is_empty() && stored.stamps.len() == current.len() {
        return Refresh::Nothing;
    }
    Refresh::Paths { changed, removed }
}

fn load_stored(repo_root: &Path) -> Option<Stored> {
    cache::load_bytes(cache::NAMESPACE_FILE, CACHE_KEY, repo_root)
        .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
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
    let identity = match executable_identity(&executable) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("Error: repo context unavailable: {error:#}");
            return empty_payload();
        }
    };
    let Some(files) = repo_files::tracked_files(repo_root, None) else {
        eprintln!("Error: repo context input scan failed: git file discovery unavailable");
        return empty_payload();
    };
    let current = current_stamps(&files);
    let skips = WalkSkips::new(repo_root, &files);
    let plan = |stored: &Option<Stored>| match stored {
        Some(stored) => refresh_for(stored, &identity, &current, |path| {
            skips.skips(repo_root, path)
        }),
        None => Refresh::Whole,
    };
    let stored = load_stored(repo_root);
    if let (Refresh::Nothing, Some(stored)) = (plan(&stored), stored) {
        return payload_from(stored.per_file);
    }
    // One maintainer at a time; the holder may have counted this change
    // while we waited, so the entry is read again under the lock.
    let _lock = cache::maintain(repo_root);
    let stored = load_stored(repo_root);
    let per_file = match (plan(&stored), stored) {
        (Refresh::Nothing, Some(stored)) => return payload_from(stored.per_file),
        (Refresh::Paths { changed, removed }, Some(stored)) => {
            let mut per_file = stored.per_file;
            for path in changed.iter().chain(removed.iter()) {
                per_file.remove(path);
            }
            let targets: Vec<&str> = changed.iter().map(String::as_str).collect();
            match scc_rows(repo_root, &executable, Some(&targets)) {
                Ok(rows) => per_file.extend(rows),
                Err(error) => {
                    eprintln!("Error: repo context unavailable: {error:#}");
                    return empty_payload();
                }
            }
            per_file
        }
        _ => match scc_rows(repo_root, &executable, None) {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("Error: repo context unavailable: {error:#}");
                return empty_payload();
            }
        },
    };
    let entry = Stored {
        executable: identity,
        per_file,
        stamps: current,
    };
    if let Some(true) = serde_json::to_value(&entry)
        .ok()
        .and_then(|value| cache::save(cache::NAMESPACE_FILE, CACHE_KEY, &value, repo_root).ok())
    {
        cache::evict_prefixed(cache::NAMESPACE_FILE, "repo_context_v", CACHE_KEY, repo_root);
    }
    payload_from(entry.per_file)
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
    fn languages_and_summary_derive_from_the_rows() {
        let per_file = BTreeMap::from([
            (
                "a.rs".to_string(),
                FileMetrics {
                    ccn: 3,
                    loc: 12,
                    language: "Rust".to_string(),
                },
            ),
            (
                "b.rs".to_string(),
                FileMetrics {
                    ccn: 5,
                    loc: 20,
                    language: "Rust".to_string(),
                },
            ),
            (
                "c.py".to_string(),
                FileMetrics {
                    ccn: 1,
                    loc: 4,
                    language: "Python".to_string(),
                },
            ),
        ]);

        let payload = payload_from(per_file);

        assert_eq!(payload.summary.total_files, 3);
        assert_eq!(payload.summary.median_file_ccn, 3);
        assert_eq!(
            payload.languages,
            vec![
                LanguageRow {
                    name: "Python".to_string(),
                    count: 1,
                    code: 4,
                    complexity: 1,
                },
                LanguageRow {
                    name: "Rust".to_string(),
                    count: 2,
                    code: 32,
                    complexity: 8,
                },
            ]
        );
    }

    #[test]
    fn an_ignore_file_change_recounts_the_whole_tree() {
        let stamp = repo_files::Stamp {
            size: 1,
            mtime: 1,
            ctime: 1,
            inode: 1,
        };
        let stored = Stored {
            executable: "scc".to_string(),
            per_file: BTreeMap::from([(
                "u.py".to_string(),
                FileMetrics {
                    ccn: 0,
                    loc: 1,
                    language: "Python".to_string(),
                },
            )]),
            stamps: BTreeMap::from([("u.py".to_string(), stamp.clone())]),
        };
        let mut current = stored.stamps.clone();
        current.insert(".sccignore".to_string(), stamp.clone());

        assert!(matches!(
            refresh_for(&stored, "scc", &current, |_| false),
            Refresh::Whole
        ));
        assert!(matches!(
            refresh_for(&stored, "scc", &stored.stamps, |_| false),
            Refresh::Nothing
        ));
        let mut edited = stored.stamps.clone();
        edited.insert("u.py".to_string(), repo_files::Stamp { size: 2, ..stamp });
        assert!(matches!(
            refresh_for(&stored, "scc", &edited, |_| false),
            Refresh::Paths { changed, removed } if changed == vec!["u.py".to_string()] && removed.is_empty()
        ));
    }

    #[test]
    fn a_new_path_is_counted_alone_unless_the_walk_would_skip_it() {
        let stamp = repo_files::Stamp {
            size: 1,
            mtime: 1,
            ctime: 1,
            inode: 1,
        };
        let stored = Stored {
            executable: "scc".to_string(),
            per_file: BTreeMap::new(),
            stamps: BTreeMap::from([("u.py".to_string(), stamp.clone())]),
        };
        let mut current = stored.stamps.clone();
        current.insert("vendor/v.py".to_string(), stamp.clone());
        current.insert("app/n.py".to_string(), stamp);

        let refresh = refresh_for(&stored, "scc", &current, |path| path.starts_with("vendor/"));

        assert!(matches!(
            refresh,
            Refresh::Paths { changed, removed } if changed == vec!["app/n.py".to_string()] && removed.is_empty()
        ));
    }

    #[test]
    fn the_deepest_tracked_ignore_file_decides_what_the_walk_skips() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join(".ignore"), "vendor/\n*.min.js\n").unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("assets/.sccignore"), "!*.min.js\n").unwrap();
        let tracked = vec![".ignore".to_string(), "assets/.sccignore".to_string()];

        let skips = WalkSkips::new(root, &tracked);

        assert!(skips.skips(root, "vendor/lib/v.py"));
        assert!(skips.skips(root, "app/a.min.js"));
        assert!(!skips.skips(root, "assets/a.min.js"));
        assert!(!skips.skips(root, "app/a.py"));
    }

    #[test]
    fn unchanged_counts_write_identical_cache_bytes() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(".git")).unwrap();
        fs::write(directory.path().join("fixture.rs"), "fn fixture() {}\n").unwrap();
        let executable = executable_path("scc").unwrap();

        let first = scc_rows(directory.path(), &executable, None).unwrap();
        let first_value = serde_json::to_value(first).unwrap();
        cache::save(
            "file",
            "repo_context_fixture",
            &first_value,
            directory.path(),
        )
        .unwrap();
        let first_bytes = cache::load_bytes("file", "repo_context_fixture", directory.path()).unwrap();

        let second = scc_rows(directory.path(), &executable, None).unwrap();
        let second_value = serde_json::to_value(second).unwrap();
        cache::save(
            "file",
            "repo_context_fixture",
            &second_value,
            directory.path(),
        )
        .unwrap();
        let second_bytes = cache::load_bytes("file", "repo_context_fixture", directory.path()).unwrap();

        assert_eq!(second_bytes, first_bytes);
    }
}
