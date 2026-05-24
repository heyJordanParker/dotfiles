//! Conditional-rule `paths:` glob matching.
//!
//! A `.claude/rules/*.md` file with a `paths:` frontmatter list applies only
//! when the file being read matches one of those globs. The semantics are
//! case-insensitive shell-glob fnmatch with a `**/` prefix special-case and a
//! basename fallback so `*.rs` matches any Rust file under any depth.
//!
//! Split out of `nested_memory` so the project-docs walk-up stays focused on
//! the walk; this module owns the glob-matching primitive it relies on.

use std::path::Path;

/// True when `file_path` matches any glob, using fnmatch semantics with
/// the `**/` special-case.
pub(super) fn matches_paths(file_path: &Path, globs: &[String], repo_root: &Path) -> bool {
    let relative = match file_path.strip_prefix(repo_root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => file_path.to_string_lossy().to_string(),
    };
    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    for glob in globs {
        if fnmatch(&relative, glob) {
            return true;
        }
        if let Some(rest) = glob.strip_prefix("**/") {
            if fnmatch(&relative, rest) {
                return true;
            }
        }
        if fnmatch(&name, glob) {
            return true;
        }
        if !glob.contains('/') && fnmatch(&name, glob) {
            return true;
        }
    }
    false
}

/// `fnmatch`: case-normalized shell-style match. Supports `*`, `?`,
/// `[seq]`, `[!seq]`. `*` treats `/` like any other char (no path-segment
/// special-casing).
fn fnmatch(name: &str, pattern: &str) -> bool {
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    fnmatch_inner(&n, &p)
}

fn fnmatch_inner(n: &[char], p: &[char]) -> bool {
    if p.is_empty() {
        return n.is_empty();
    }
    match p[0] {
        '*' => fnmatch_star(n, &p[1..]),
        '?' => !n.is_empty() && fnmatch_inner(&n[1..], &p[1..]),
        '[' => fnmatch_class(n, p),
        c => !n.is_empty() && n[0] == c && fnmatch_inner(&n[1..], &p[1..]),
    }
}

fn fnmatch_star(n: &[char], rest: &[char]) -> bool {
    if fnmatch_inner(n, rest) {
        return true;
    }
    !n.is_empty() && fnmatch_star(&n[1..], rest)
}

fn fnmatch_class(n: &[char], p: &[char]) -> bool {
    if n.is_empty() {
        return false;
    }
    let Some(close) = p.iter().position(|&c| c == ']').filter(|&i| i > 1) else {
        // No closing bracket — `[` is a literal.
        return n[0] == '[' && fnmatch_inner(&n[1..], &p[1..]);
    };
    let mut set = &p[1..close];
    let negate = set.first() == Some(&'!');
    if negate {
        set = &set[1..];
    }
    let hit = class_contains(set, n[0]);
    if hit != negate {
        fnmatch_inner(&n[1..], &p[close + 1..])
    } else {
        false
    }
}

fn class_contains(set: &[char], ch: char) -> bool {
    let mut i = 0;
    while i < set.len() {
        if i + 2 < set.len() && set[i + 1] == '-' {
            if ch >= set[i] && ch <= set[i + 2] {
                return true;
            }
            i += 3;
        } else {
            if ch == set[i] {
                return true;
            }
            i += 1;
        }
    }
    false
}
