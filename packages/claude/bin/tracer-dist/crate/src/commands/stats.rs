//! `trace stats` — repo-wide complexity distribution via scc.
//! One `scc --format json --by-file <path>` sweep → per-language
//! LOC/complexity aggregates, a per-file complexity distribution
//! (median/p75/p90/p95/max), and the top-10 most-complex files.

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

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
    #[serde(rename = "Location")]
    location: String,
    #[serde(rename = "Language")]
    language: String,
    /// Every line, as a file's `lines` fact counts them.
    #[serde(rename = "Lines")]
    lines: i64,
    #[serde(rename = "Complexity")]
    complexity: i64,
}

struct Summary {
    total_files: usize,
    languages: Vec<Language>,
    distribution: Option<Distribution>,
    top_complex: Vec<(usize, usize)>,
}

struct Language {
    name: String,
    files: i64,
    loc: i64,
    complexity: i64,
}

struct Distribution {
    median: i64,
    p75: i64,
    p90: i64,
    p95: i64,
    max: i64,
}

/// scc failure → stderr + exit 1 (hard-fail contract).
fn scc_by_file(path: &Path) -> Vec<SccLanguage> {
    let out = match Command::new("scc")
        .args([
            "--format",
            "json",
            "--by-file",
            "--exclude-dir",
            ".tracer-cache",
        ])
        .arg(path)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: scc failed: {e}");
            std::process::exit(1);
        }
    };
    if !out.status.success() {
        eprintln!(
            "Error: scc failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        std::process::exit(1);
    }
    serde_json::from_slice(&out.stdout).unwrap_or_default()
}

/// `int(statistics.median(values))` — average of the two middles for even
/// counts, truncated toward zero. `complexities` is sorted in place.
fn distribution(complexities: &mut Vec<i64>) -> Option<Distribution> {
    if complexities.is_empty() {
        return None;
    }
    complexities.sort_unstable();
    let n = complexities.len();
    let median = if n % 2 == 1 {
        complexities[n / 2]
    } else {
        ((complexities[n / 2 - 1] + complexities[n / 2]) as f64 / 2.0) as i64
    };
    let percentile = |p: f64| -> i64 {
        let index = (((n as f64) * p) as i64 - 1).max(0) as usize;
        complexities[index]
    };
    Some(Distribution {
        median,
        p75: percentile(0.75),
        p90: percentile(0.90),
        p95: percentile(0.95),
        max: complexities[n - 1],
    })
}

/// Builds per-language totals, the complexity distribution, and the top-10 indices.
fn summary(by_file: &[SccLanguage]) -> Summary {
    let mut languages = Vec::with_capacity(by_file.len());
    let mut complexities = Vec::new();
    let mut top_complex = Vec::new();
    for (language_index, language) in by_file.iter().enumerate() {
        languages.push(Language {
            name: language.name.clone(),
            files: language.count,
            loc: language.code,
            complexity: language.complexity,
        });
        for (file_index, file) in language.files.iter().enumerate() {
            complexities.push(file.complexity);
            top_complex.push((language_index, file_index));
        }
    }
    top_complex.sort_by(|(left_language, left_file), (right_language, right_file)| {
        by_file[*right_language].files[*right_file]
            .complexity
            .cmp(&by_file[*left_language].files[*left_file].complexity)
    });
    top_complex.truncate(10);
    let total_files = complexities.len();
    Summary {
        total_files,
        languages,
        distribution: distribution(&mut complexities),
        top_complex,
    }
}

fn directory_scope(path: &Path, repo_root: &Path) -> String {
    let path = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    match path.strip_prefix(repo_root).ok().and_then(Path::to_str) {
        Some("") | None => "./".to_string(),
        Some(relative) => format!("{}/", relative.trim_end_matches('/')),
    }
}

fn annotations(metrics: &crate::relations::DirectoryMetrics) -> Vec<(&str, usize)> {
    let mut names: Vec<(&str, usize)> = metrics
        .annotations
        .iter()
        .map(|(name, count)| (name.as_str(), *count))
        .collect();
    names.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    names
}

fn directory_value(metrics: &crate::relations::DirectoryMetrics) -> Value {
    json!({
        "files": metrics.files,
        "imported_by": metrics.imported_by,
        "imports": metrics.imports,
        "imported_directories": metrics.imported_directories,
        "annotations": metrics.annotations,
    })
}

/// Most depended-on first: a change there reaches the most files.
fn compare_directories(
    left: (&String, &crate::relations::DirectoryMetrics),
    right: (&String, &crate::relations::DirectoryMetrics),
) -> Ordering {
    right.1.imported_by.cmp(&left.1.imported_by).then_with(|| left.0.cmp(right.0))
}

pub fn run(path: &Path, as_json: bool) -> Result<Value> {
    crate::pathval::require_exists(path, "PATH");
    let abs = crate::cache::absolutize(path);
    let resolved = abs.canonicalize().unwrap_or(abs);
    let repo_root =
        crate::cache::worktree_root_for(&resolved).unwrap_or_else(|| PathBuf::from("."));
    let (by_file, metrics) = thread::scope(|scope| {
        let relations = scope.spawn(|| {
            crate::timing::phase("relations", || {
                crate::relations::get(&repo_root).directory_metrics()
            })
        });
        let scc = scope.spawn(|| crate::timing::phase("scc", || scc_by_file(&resolved)));
        (scc.join().unwrap(), relations.join().unwrap())
    });
    let summary = crate::timing::phase("summary", || summary(&by_file));
    let scope = directory_scope(&resolved, &repo_root);

    if as_json {
        return Ok(crate::timing::phase("document", || {
            let mut languages = Map::new();
            for language in &summary.languages {
                languages.insert(
                    language.name.clone(),
                    json!({
                        "files": language.files,
                        "lines_of_code": language.loc,
                        "cyclomatic_complexity": language.complexity,
                    }),
                );
            }
            let distribution = summary.distribution.as_ref().map_or_else(
                || json!({}),
                |distribution| {
                    json!({
                        "median": distribution.median,
                        "p75": distribution.p75,
                        "p90": distribution.p90,
                        "p95": distribution.p95,
                        "max": distribution.max,
                    })
                },
            );
            let top_complex = summary
                .top_complex
                .iter()
                .map(|(language_index, file_index)| {
                    let file = &by_file[*language_index].files[*file_index];
                    json!({
                        "path": crate::cache::relative_to_root(Path::new(&file.location), &repo_root),
                        "language": file.language,
                        "lines": file.lines,
                        "cyclomatic_complexity": file.complexity,
                    })
                })
                .collect::<Vec<_>>();
            let mut directories = Map::new();
            for (directory, metrics) in metrics
                .directories
                .iter()
                .filter(|(directory, _)| scope == "./" || directory.starts_with(scope.as_str()))
            {
                directories.insert(directory.clone(), directory_value(metrics));
            }
            crate::output::document(
                json!({"path": resolved.to_string_lossy()}),
                json!({"distribution": distribution, "directories": directories}),
                json!({"languages": languages, "top_complex": top_complex}),
                json!({
                    "files": summary.total_files,
                    "directories": metrics
                        .directories
                        .keys()
                        .filter(|directory| scope == "./" || directory.starts_with(scope.as_str()))
                        .count(),
                }),
            )
        }));
    }

    crate::timing::phase("render", || {
        let mut head = format!("Files: {}\n\nLanguages:\n", summary.total_files);
        let mut languages: Vec<&Language> = summary.languages.iter().collect();
        languages.sort_by(|left, right| right.loc.cmp(&left.loc));
        for language in languages.iter().take(15) {
            let facts = json!({
                "files": language.files,
                "lines_of_code": language.loc,
                "cyclomatic_complexity": language.complexity,
            });
            head.push_str(&format!("  {:<20} {}\n", language.name, crate::yamlfmt::flow(&facts, false)));
        }
        head.push('\n');
        if let Some(distribution) = &summary.distribution {
            head.push_str(&format!(
                "Complexity distribution (per file):\n  median={}  p75={}  p90={}  p95={}  max={}\n",
                distribution.median,
                distribution.p75,
                distribution.p90,
                distribution.p95,
                distribution.max,
            ));
        }
        head.push_str("\nTop 10 most-complex files:\n");
        for (language_index, file_index) in &summary.top_complex {
            let file = &by_file[*language_index].files[*file_index];
            let facts = json!({"cyclomatic_complexity": file.complexity, "lines": file.lines});
            head.push_str(&format!(
                "  {}  {}\n",
                crate::cache::relative_to_root(Path::new(&file.location), &repo_root),
                crate::yamlfmt::flow(&facts, false),
            ));
        }
        head.push_str(&format!("\nDirectories under {scope} (most imported first, direct files only):"));
        println!("{head}");
        let columns = "  imported_by  imports  files  directory                        annotations\n";
        let mut rows: Vec<(&String, &crate::relations::DirectoryMetrics)> = metrics
            .directories
            .iter()
            .filter(|(directory, _)| scope == "./" || directory.starts_with(scope.as_str()))
            .collect();
        rows.sort_by(|left, right| compare_directories(*left, *right));
        let mut entries: Vec<crate::output::Entry> = Vec::with_capacity(rows.len());
        let mut paths: Vec<&str> = Vec::with_capacity(rows.len());
        for (directory, metrics) in rows {
            let annotations = annotations(metrics)
                .into_iter()
                .take(3)
                .map(|(name, count)| format!("{name}×{count}"))
                .collect::<Vec<_>>()
                .join(" ");
            let line = format!(
                "  {:<12} {:<8} {:<6} {:<32} {}",
                metrics.imported_by,
                metrics.imports,
                metrics.files,
                directory,
                if annotations.is_empty() {
                    "-"
                } else {
                    &annotations
                },
            );
            let lead = if entries.is_empty() { columns } else { "" };
            entries.push(crate::output::Entry {
                rank: metrics.imported_by as i64,
                levels: vec![format!("{lead}{line}"), format!("{lead}  {directory}")],
            });
            paths.push(directory);
        }
        let fixed = head.len() + 1 + crate::output::closing_room(entries.len(), "directories");
        let (texts, shortened) = crate::output::fit_listing(&entries, &paths, fixed);
        for text in texts {
            println!("{text}");
        }
        if shortened > 0 {
            println!("{}", crate::output::shortened_line(shortened, entries.len(), "directories"));
        }
    });
    Ok(Value::Null)
}
