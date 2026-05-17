//! `trace glob` — Claude-Glob-shaped full-path pattern search.
//! `**` recurses, gitignore-respecting (universe = git ls-files),
//! deterministically sorted. Bare paths by default; `--details` adds
//! ccn + rank + lifecycle shoulder.

use crate::{cache, file_facts, passive_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".next",
    ".tracer-cache",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "vendor",
    "worktrees",
];

/// Absolute resolved paths git considers tracked-or-not-ignored under
/// `base`. None when not in a git repo.
fn tracked_universe(base: &Path) -> Option<BTreeSet<PathBuf>> {
    let out = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(base)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let p = base.join(l);
                p.canonicalize().unwrap_or(p)
            })
            .collect(),
    )
}

/// SKIP_DIRS-bounded filesystem walk under `base` — the non-git fallback
/// universe.
fn walk_universe(base: &Path) -> BTreeSet<PathBuf> {
    let skip: BTreeSet<&str> = SKIP_DIRS.iter().copied().collect();
    let mut out = BTreeSet::new();
    let walker = walkdir::WalkDir::new(base).into_iter().filter_entry(|e| {
        if e.file_type().is_dir() {
            let name = e.file_name().to_string_lossy();
            e.path() == base
                || (!skip.contains(name.as_ref()) && !name.starts_with('.'))
        } else {
            true
        }
    });
    for entry in walker.flatten() {
        if entry.file_type().is_file() {
            let p = entry.path();
            out.insert(p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));
        }
    }
    out
}

/// Resolve a glob against the universe. Returns (sorted absolute file
/// matches, ignore_policy).
///
/// Performance: the result is the glob match ∩ `universe`. `universe` is
/// already the gitignore/skip-dirs-pruned file set (no
/// `node_modules`/`vendor` walk), so rather than walking the filesystem
/// unboundedly, the pattern is compiled once with `globset` and tested
/// against each universe path's base-relative form. `globset` is the
/// pure-Rust matcher ripgrep uses — preserves the single-static-binary
/// property. Glob semantics: `/`-segmented, `**` = zero-or-more
/// directories, `*`/`?`/`[...]` never cross `/` (`literal_separator(true)`).
fn resolve_glob(pattern: &str, base: &Path) -> (Vec<PathBuf>, &'static str) {
    let (universe, policy) = match tracked_universe(base) {
        Some(u) => (u, "gitignore"),
        None => (walk_universe(base), "skip_dirs"),
    };
    let matcher = match build_glob(pattern) {
        Some(m) => m,
        // Unparseable pattern → no matches.
        None => return (Vec::new(), policy),
    };
    let base_resolved = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let mut set: BTreeSet<PathBuf> = BTreeSet::new();
    for p in &universe {
        if !p.is_file() {
            continue;
        }
        let rel = match p.strip_prefix(&base_resolved) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if matcher.is_match(rel) {
            set.insert(p.clone());
        }
    }
    // BTreeSet already yields sorted, deduped order over the intersected set.
    (set.into_iter().collect(), policy)
}

/// Translate a glob pattern to a `globset::GlobMatcher`. `**` matches
/// zero-or-more path segments (globset `**` with `literal_separator(true)`
/// has that semantic), and a trailing `/**` matches every descendant. A
/// bare `**` matches everything. `*`/`?`/`[...]` are per-segment (never
/// cross `/`); `literal_separator(true)` enforces that.
fn build_glob(pattern: &str) -> Option<globset::GlobMatcher> {
    use globset::GlobBuilder;
    // Normalize: empty segments (leading/trailing/`//`) are ignored.
    let segs: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return None;
    }
    let normalized = segs.join("/");
    GlobBuilder::new(&normalized)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .ok()
        .map(|g| g.compile_matcher())
}

fn rel_to_base(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

pub fn run(pattern: &str, base: &str, details: bool, as_json: bool) -> Result<Value> {
    let abs = cache::absolutize(Path::new(base));
    if !abs.exists() {
        eprintln!("Error: {base} does not exist");
        std::process::exit(2);
    }
    if !abs.is_dir() {
        eprintln!("Error: {base} is not a directory");
        std::process::exit(2);
    }
    // Canonicalized base path, printed verbatim in output.
    let base_abs = abs.canonicalize().unwrap_or(abs);
    let base_path = base_abs.clone();

    let (matches, ignore_policy) = resolve_glob(pattern, &base_abs);
    let repo_root = cache::repo_root_for(&base_abs);

    // Build the value once; the human view reads from it (no second
    // file_facts pass for `--details`).
    let value = if details {
        let entries: Vec<Value> = matches
            .iter()
            .map(|p| {
                let rel = rel_to_base(p, &base_abs);
                match file_facts::get(p, &repo_root, None) {
                    None => json!({
                        "path": rel,
                        "ccn_total": 0,
                        "rank": "unknown",
                        "shoulder": Value::Null,
                    }),
                    Some(f) => json!({
                        "path": rel,
                        "ccn_total": f.cyclomatic_complexity_total,
                        "rank": f.rank,
                        "shoulder": passive_context::render_compact(&f),
                    }),
                }
            })
            .collect();
        json!({
            "pattern": pattern,
            "base": base_path.to_string_lossy(),
            "ignore_policy": ignore_policy,
            "match_count": entries.len(),
            "matches": entries,
        })
    } else {
        let rels: Vec<String> =
            matches.iter().map(|p| rel_to_base(p, &base_abs)).collect();
        json!({
            "pattern": pattern,
            "base": base_path.to_string_lossy(),
            "ignore_policy": ignore_policy,
            "match_count": rels.len(),
            "matches": rels,
        })
    };

    if as_json {
        return Ok(value);
    }

    if matches.is_empty() {
        println!("(no matches)");
        return Ok(value);
    }

    if details {
        for m in value["matches"].as_array().unwrap_or(&vec![]) {
            println!(
                "{}  [ccn={} {}]  {}",
                m["path"].as_str().unwrap_or(""),
                m["ccn_total"].as_i64().unwrap_or(0),
                m["rank"].as_str().unwrap_or(""),
                m["shoulder"].as_str().unwrap_or(""),
            );
        }
    } else {
        for m in value["matches"].as_array().unwrap_or(&vec![]) {
            println!("{}", m.as_str().unwrap_or(""));
        }
    }
    Ok(value)
}
