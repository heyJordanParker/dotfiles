//! `trace find` — name- and path-pattern search with code-intelligence
//! enrichment. Replaces `find <dir> -type f -name "*.ext"` and the
//! `**/dir/*.ext` path-shape search with one enriched call: matching paths
//! annotated with complexity rank + the lifecycle summary. Respects
//! .gitignore inside a git repo (git ls-files), SKIP_DIRS walk otherwise.
//!
//! One pattern argument answers both questions, split by `path_matcher`: a
//! pattern carrying `/` or `**` is a path pattern, anything else is a
//! basename pattern. Every result carries its context — there is no
//! bare-path mode to fall into.

use super::glob_match;
use crate::summary::Facts;
use globset::GlobMatcher;
use crate::{cache, file_facts};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Candidate files: the shared repository listing scoped to `base` — git's
/// file list, or the shared walk outside git, so ignored files, nested
/// repositories and linked worktrees stay out the same way for every
/// command. `include_dirs` adds the parent directories of those files,
/// stopping at `base`.
fn list_files(repo_root: &Path, base: &Path, include_dirs: bool) -> Vec<PathBuf> {
    let files = match crate::repo_files::tracked_paths(repo_root, Some(base)) {
        Some(files) if !files.is_empty() => files,
        _ => crate::repo_files::walk_files(base),
    };
    if !include_dirs {
        return files;
    }
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for f in &files {
        let mut cur = f.parent();
        while let Some(p) = cur {
            if p == base {
                break;
            }
            dirs.insert(p.to_path_buf());
            cur = p.parent();
        }
    }
    let mut result = files;
    result.extend(dirs);
    result
}

/// A pattern that names a path — it carries a `/` or a `**` — is matched
/// against the base-relative path; anything else is matched against the
/// basename. That is the one rule that lets `find` answer both the `find
/// -name "*.php"` question and the `**/Jobs/*.php` path-shape question that
/// was its own command.
///
/// Path patterns compile through `globset`, the pure-Rust matcher ripgrep
/// uses (single-static-binary preserved). `literal_separator(true)` keeps
/// `*`, `?`, and `[...]` inside one segment and gives `**` its
/// zero-or-more-directories meaning.
fn path_matcher(pattern: &str) -> Option<GlobMatcher> {
    if !pattern.contains('/') && !pattern.contains("**") {
        return None;
    }
    let segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }
    globset::GlobBuilder::new(&segments.join("/"))
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .ok()
        .map(|g| g.compile_matcher())
}

struct Matchers {
    by_path: Option<GlobMatcher>,
    by_name: GlobMatcher,
    path_filter: Option<GlobMatcher>,
    excludes: Vec<GlobMatcher>,
}

impl Matchers {
    fn new(pattern: &str, path_filter: Option<&str>, excludes: &[String]) -> Self {
        Self {
            by_path: path_matcher(pattern),
            by_name: glob_match::matcher(pattern),
            path_filter: path_filter.map(glob_match::matcher),
            excludes: excludes.iter().map(|exclude| glob_match::matcher(exclude)).collect(),
        }
    }

    fn matches(&self, path: &Path, base: &Path) -> bool {
        let named = match &self.by_path {
            Some(matcher) => matcher.is_match(path.strip_prefix(base).unwrap_or(path)),
            None => path.file_name().is_some_and(|name| self.by_name.is_match(name)),
        };
        named
            && self.path_filter.as_ref().is_none_or(|filter| filter.is_match(path))
            && !self.excludes.iter().any(|exclude| exclude.is_match(path))
    }
}

/// A match, its facts, and the commit date `--sort recent` orders by.
struct E {
    path: String,
    kind: &'static str,
    facts: Option<Facts>,
    last_modified: Option<String>,
}

/// Every match under one base, with its facts.
fn matches_under(base_abs: &Path, matchers: &Matchers, include_dirs: bool, entries: &mut Vec<E>) {
    let repo_root =
        cache::worktree_root_for(base_abs).unwrap_or_else(|| cache::display_root(base_abs));
    let matched: Vec<PathBuf> = list_files(&repo_root, base_abs, include_dirs)
        .into_iter()
        .filter(|p| if include_dirs { p.is_dir() } else { p.is_file() })
        .filter(|p| matchers.matches(p, base_abs))
        .collect();

    let root_resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.clone());
    let index = (!include_dirs).then(|| crate::relations::get(&repo_root));
    for chunk in matched.chunks(file_facts::RESOLVE_CHUNK) {
        let facts = if include_dirs {
            std::collections::HashMap::new()
        } else {
            file_facts::get_batch(chunk, &repo_root)
        };
        for path in chunk {
            // Relative path against the UNRESOLVED repo root — symlinked dirs
            // keep their tracked name and are not collapsed onto their target.
            let relative = path
                .strip_prefix(&root_resolved)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            if include_dirs {
                entries.push(E {
                    path: relative,
                    kind: "directory",
                    facts: None,
                    last_modified: None,
                });
                continue;
            }
            let fkey = cache::relative_to_root(path, &repo_root);
            let fact = facts.get(&fkey);
            entries.push(E {
                path: relative,
                kind: "file",
                facts: fact.map(|f| {
                    let graph = index.as_ref().and_then(|index| index.module_counts(&fkey));
                    Facts::of(f, graph.as_ref())
                }),
                last_modified: fact.and_then(|f| f.last_modified.clone()),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: &str,
    bases: &[String],
    path_filter: Option<String>,
    excludes: Vec<String>,
    type_filter: String,
    limit: usize,
    sort: String,
    as_json: bool,
) -> Result<Value> {
    let include_dirs = type_filter.to_lowercase() == "d";
    let matchers = Matchers::new(pattern, path_filter.as_deref(), &excludes);
    let mut entries: Vec<E> = Vec::new();
    // Canonicalized bases, printed verbatim in output. A base that is not a
    // directory is reported and the others are still searched.
    let mut searched: Vec<PathBuf> = Vec::new();
    for base in bases {
        let abs = cache::absolutize(Path::new(base));
        if !abs.is_dir() {
            let why = if abs.exists() { "is not a directory" } else { "does not exist" };
            crate::pathval::report(Path::new(base), "BASE", why);
            continue;
        }
        let base_abs = abs.canonicalize().unwrap_or(abs);
        matches_under(&base_abs, &matchers, include_dirs, &mut entries);
        searched.push(base_abs);
    }
    // Overlapping bases name a file once.
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries.dedup_by(|a, b| a.path == b.path);
    let base_list = searched
        .iter()
        .map(|base| base.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let complexity = |e: &E| e.facts.as_ref().map_or(0, |f| f.cyclomatic_complexity);
    match sort.as_str() {
        "complexity" => {
            entries.sort_by(|a, b| (-complexity(a), &a.path).cmp(&(-complexity(b), &b.path)));
        }
        "recent" => {
            entries.sort_by(|a, b| {
                let ka = (a.last_modified.clone().unwrap_or_default(), a.path.clone());
                let kb = (b.last_modified.clone().unwrap_or_default(), b.path.clone());
                kb.cmp(&ka)
            });
        }
        _ => entries.sort_by(|a, b| a.path.cmp(&b.path)),
    }

    // The pre-cap total is the number the agent needs: "3 matches, truncated"
    // does not say whether the answer was 4 or 4,000, so it cannot choose a
    // `--limit` and cannot tell a narrow result from a hidden one. `list`
    // already reports its pre-cap total; `find` reports it the same way.
    let total = entries.len();
    let truncated = total > limit;
    entries.truncate(limit);

    // Each file's facts are per-file enrichment: they sit in `context` under
    // the same path the row carries, out of reach of a row projection.
    let mut file_facts_by_path = serde_json::Map::new();
    let json_entries: Vec<_> = entries
        .iter()
        .map(|e| {
            if let Some(facts) = &e.facts {
                file_facts_by_path.insert(e.path.clone(), Value::Object(facts.to_map()));
            }
            json!({"path": e.path, "kind": e.kind})
        })
        .collect();
    // An empty result over a base that contains nested checkouts is a scope
    // fact, not an absence fact — name them so the next call is scoped inside.
    let nested: Vec<String> = if entries.is_empty() {
        searched
            .iter()
            .flat_map(|base| crate::repo_files::nested_repo_rels(base))
            .collect()
    } else {
        Vec::new()
    };
    let value = crate::output::document(
        json!({
            "pattern": pattern,
            "bases": base_list,
            "path_filter": path_filter,
            "excludes": excludes,
            "type": type_filter,
            "limit": limit,
            "sort": sort,
        }),
        json!({"nested_repos": nested, "files": Value::Object(file_facts_by_path)}),
        json!(json_entries),
        json!({"matches": json_entries.len(), "total": total, "truncated": truncated}),
    );

    if as_json {
        return Ok(value);
    }

    if entries.is_empty() {
        println!("(no matches)");
        for r in &nested {
            println!("nested repository (its own search scope): {r}");
        }
        return Ok(value);
    }

    let under = base_list.join(", ");
    let header = if truncated {
        format!("{} matches under {under}, showing {}:", total, entries.len())
    } else {
        format!("{} matches under {under}:", total)
    };
    println!("{header}");
    let more = truncated.then(|| format!("... {} more (see all: --limit {total})", total - entries.len()));
    // Every row stays; the budget cuts the files the fewest others import
    // back to their path first.
    let rows: Vec<crate::output::Entry> = entries
        .iter()
        .map(|e| match (&e.facts, e.kind == "directory") {
            (_, true) => crate::output::Entry { rank: i64::MAX, levels: vec![format!("  📁 {}/", e.path)] },
            (Some(facts), false) => crate::output::Entry {
                rank: facts.imported_by.unwrap_or(0) as i64,
                levels: vec![format!("  {}  {}", e.path, facts.headline()), format!("  {}", e.path)],
            },
            (None, false) => crate::output::Entry { rank: 0, levels: vec![format!("  {}", e.path)] },
        })
        .collect();
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let fixed = header.len() + 1 + more.as_ref().map_or(0, |line| line.len() + 1) + crate::output::closing_room(rows.len(), "files");
    let (texts, shortened) = crate::output::fit_listing(&rows, &paths, fixed);
    for text in texts {
        println!("{text}");
    }
    if let Some(line) = more {
        println!("{line}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, rows.len(), "files"));
    }
    Ok(value)
}
