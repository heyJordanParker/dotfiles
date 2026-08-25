//! `trace context` — two modes off one command.
//!
//! No-args: the eight-section session-start primer (environment, identity,
//! tech stack, layout, common directories, git, rules, spine) plus the
//! repo_context footer. First invocation warms the file + architecture
//! caches. File-arg: single-file enrichment — one passive_context line.
//!
//! CCN is AST-derived; the Layout per-path aggregation uses the real
//! `file_facts::get` (no lite-facts shortcut).

use super::{nested_memory, session_log};
use crate::{
    architecture, cache, docs_graph, file_facts, git_activity, passive_context, repo_context,
    repo_files,
};
use anyhow::Result;
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PRIMER_LANGUAGE_LIMIT: usize = 10;
const PRIMER_DIRTY_LIMIT: usize = 10;
const PRIMER_COMMIT_LIMIT: usize = 10;
const PRIMER_SPINE_LIMIT: usize = 10;
const PRIMER_APPLICABLE_RULES_LIMIT: usize = 10;
const PRIMER_BRANCH_STALE_DAYS: i64 = 21;

const FEATURE_PREFIXES: &[&str] = &[
    "feat/", "feature/", "fix/", "bugfix/", "hotfix/", "chore/", "refactor/",
    "docs/", "test/", "tests/", "ci/", "build/", "wip/", "experiment/",
    "spike/", "release/",
];

/// (filename, manager label). List order is significant: every present
/// config is collected, and this order determines the per-manager filename
/// order in the output.
const PACKAGE_CONFIGS: &[(&str, &str)] = &[
    ("package.json", "npm/node"),
    ("package-lock.json", "npm"),
    ("yarn.lock", "yarn"),
    ("pnpm-lock.yaml", "pnpm"),
    ("bun.lock", "bun"),
    ("bun.lockb", "bun"),
    ("composer.json", "composer"),
    ("composer.lock", "composer"),
    ("Gemfile", "bundler"),
    ("Gemfile.lock", "bundler"),
    ("Cargo.toml", "cargo"),
    ("Cargo.lock", "cargo"),
    ("pyproject.toml", "pip/poetry/hatch"),
    ("requirements.txt", "pip"),
    ("Pipfile", "pipenv"),
    ("Pipfile.lock", "pipenv"),
    ("go.mod", "go modules"),
    ("go.sum", "go modules"),
    ("Brewfile", "homebrew"),
    ("build.gradle", "gradle"),
    ("build.gradle.kts", "gradle"),
    ("pom.xml", "maven"),
    ("Package.swift", "swift package manager"),
    ("mix.exs", "mix"),
    ("deno.json", "deno"),
];

const TOOL_CONFIGS: &[&str] = &[
    "vite.config.js", "vite.config.ts", "webpack.config.js", "webpack.config.ts",
    "rollup.config.js", "rollup.config.ts", "esbuild.config.js", "esbuild.config.ts",
    "tsconfig.json", "jsconfig.json", "playwright.config.ts", "playwright.config.js",
    "vitest.config.ts", "vitest.config.js", "jest.config.js", "jest.config.ts",
    "phpunit.xml", "phpunit.xml.dist", "pytest.ini", "tox.ini", "biome.json",
    ".eslintrc.json", ".eslintrc.js", "prettier.config.js", ".prettierrc",
    "pint.json", ".rubocop.yml", "Dockerfile", "docker-compose.yml",
    "compose.yaml", "compose.yml", ".lando.yml", "wp-cli.yml", "Makefile",
];

const CI_MARKERS: &[(&str, &str)] = &[
    (".github/workflows", "GitHub Actions"),
    (".gitlab-ci.yml", "GitLab CI"),
    (".circleci/config.yml", "CircleCI"),
    ("Jenkinsfile", "Jenkins"),
    ("azure-pipelines.yml", "Azure Pipelines"),
    ("bitbucket-pipelines.yml", "Bitbucket Pipelines"),
    (".drone.yml", "Drone"),
    (".travis.yml", "Travis"),
];

const TEST_CONFIG_NAMES: &[&str] = &[
    "phpunit.xml", "phpunit.xml.dist", "pytest.ini", "tox.ini",
    "vitest.config.ts", "vitest.config.js", "jest.config.js", "jest.config.ts",
    "playwright.config.ts", "playwright.config.js",
];

const COMMON_KINDS: &[&str] = &[
    "frontend",
    "backend",
    "database-migrations",
    "tests",
    "scripts",
    "continuous-integration",
];

const DIRTY_STATES: &[&str] = &["untracked", "added", "modified", "renamed"];

fn git_str(repo_root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// --- File-enrichment helper --------------------------------------------

/// Caller and dependency counts for a file from the cached graph, or None
/// when the file has no module node.
fn graph_counts(file_path: &Path, repo_root: &Path) -> Option<Value> {
    let graph = architecture::load_cached(repo_root)?;
    let relative = cache::relative_to_root(file_path, repo_root);
    let module_id = graph.file_to_module_id.get(&relative)?;
    Some(json!({
        "callers": architecture::dependents_of(&graph, module_id).len(),
        "depended_on_by_modules": architecture::dependencies_of(&graph, module_id).len(),
    }))
}

/// Translate the read tool's `offset`/`limit` into the 1-based inclusive
/// `(start, end)` span `record_read` accumulates, or `None` for a whole-file
/// read (no offset, no limit — the shell-read path). `offset` defaults to line
/// 1; an open-ended `limit` reaches end-of-file (`record_read` clamps the span
/// to the file's real line count).
fn read_range(offset: Option<usize>, limit: Option<usize>) -> Option<(usize, usize)> {
    match (offset, limit) {
        (None, None) => None,
        (Some(o), Some(l)) => Some((o, o.saturating_add(l).saturating_sub(1))),
        (Some(o), None) => Some((o, usize::MAX)),
        (None, Some(l)) => Some((1, l)),
    }
}

fn file_mode(p: &Path, lines: Option<(usize, usize)>, record: bool) -> Result<()> {
    // Record that the agent just Read this file, and which line range it read.
    // Enables cross-tool dedup (a later doc-injection or read against the same
    // content returns "already loaded" without re-emitting) and accumulates
    // per-file read coverage. No-op without a session id. The return is whether
    // this is the file's first surfacing this session — drives the
    // once-per-session methods + directory-listing lines below.
    //
    // `record` is false for an Edit/Write: the agent gets the full shoulder but
    // the touch is not a read, so nothing is recorded and read coverage stays a
    // function of genuine reads alone. With no record there is no view to dedup
    // against, so the file surfaces fully (first_touch = true) — the same shape
    // the no-session standalone path takes.
    let first_touch = if record {
        session_log::record_read(p, "agent_read", lines)
    } else {
        true
    };

    let repo_root = cache::worktree_root_for(p).unwrap_or_else(|| cache::display_root(p));
    let facts = match file_facts::get(p, &repo_root) {
        Some(f) => f,
        None => return Ok(()),
    };
    let gc = graph_counts(p, &repo_root);
    let line = passive_context::render(&facts, gc.as_ref());
    if !line.is_empty() {
        println!("{line}");
    }
    let docs_line = docs_awareness_line(p, &repo_root);
    if !docs_line.is_empty() {
        println!("{docs_line}");
    }
    // First touch of this file: surface its symbol surface so one read gives
    // the agent the file's shape without a second `trace structure` call.
    if first_touch {
        let line = symbols_line(p);
        if !line.is_empty() {
            println!("{line}");
        }
    }
    // First touch of the file's immediate parent directory: surface that one
    // directory's file listing so the agent sees the file's neighbours.
    // Parent only — never the ancestor chain.
    if let Some(parent) = p.parent() {
        emit_directory_line_on_first_touch(parent);
    }
    Ok(())
}

/// One-line symbol surface for a file: every declared symbol rendered with
/// its full signature — the same per-method surface `trace structure`
/// produces, drawn from `structure::run`'s JSON so the signature surface has
/// one source of truth and isn't recomputed here. Each symbol reads
/// `[visibility] name<signature> -> <return> [ccn=N]`, joined by `; `. Empty
/// string when the file has no extracted symbols or structure fails.
fn symbols_line(path: &Path) -> String {
    let value = match super::structure::run(path, true) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    let by_kind = match value.get("symbols_by_kind").and_then(|v| v.as_object()) {
        Some(m) if !m.is_empty() => m,
        _ => return String::new(),
    };
    let mut entries: Vec<(i64, String)> = Vec::new();
    for symbols in by_kind.values() {
        let arr = match symbols.as_array() {
            Some(a) => a,
            None => continue,
        };
        for s in arr {
            let line_no = s.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
            entries.push((line_no, render_symbol(s)));
        }
    }
    if entries.is_empty() {
        return String::new();
    }
    // Source order — same ordering the structure view presents.
    entries.sort_by_key(|(line_no, _)| *line_no);
    let rendered: Vec<String> = entries.into_iter().map(|(_, text)| text).collect();
    format!("[symbols: {}]", rendered.join("; "))
}

/// One symbol from `structure`'s per-symbol JSON rendered to its callable
/// surface: visibility prefix, name, the parameter/return signature (the
/// ctags `signature` string when present, else reconstructed from the
/// tree-sitter `parameters`/`return_type` fields for languages ctags leaves
/// bare), and cyclomatic complexity. Mirrors the structure text view's
/// per-symbol line.
fn render_symbol(s: &Value) -> String {
    let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let mut text = String::new();
    if let Some(vis) = s.get("visibility").and_then(|v| v.as_str()) {
        text.push_str(vis);
        text.push(' ');
    }
    text.push_str(name);
    text.push_str(&signature_surface(s));
    if let Some(ccn) = s.get("cyclomatic_complexity").and_then(|c| c.as_i64()) {
        let _ = write!(text, " ccn={ccn}");
    }
    text
}

/// The parameter + return portion of a symbol's signature. Prefers the ctags
/// `signature` string (already a complete `(params) -> ret` for languages
/// ctags covers). When ctags left it null, reconstructs `(type name, …)` from
/// the tree-sitter `parameters` array and ` -> <return_type>` from
/// `return_type` so PHP / TypeScript / Python symbols carry their surface
/// rather than degrading to a bare name. Empty string for symbols with
/// neither (e.g. a property or class node with no callable shape).
fn signature_surface(s: &Value) -> String {
    if let Some(sig) = s.get("signature").and_then(|v| v.as_str()) {
        if !sig.is_empty() {
            return if sig.starts_with('(') {
                sig.to_string()
            } else {
                format!(" {sig}")
            };
        }
    }
    let mut out = String::new();
    if let Some(params) = s.get("parameters").and_then(|p| p.as_array()) {
        let rendered: Vec<String> = params.iter().map(render_parameter).collect();
        out.push('(');
        out.push_str(&rendered.join(", "));
        out.push(')');
    }
    if let Some(ret) = s.get("return_type").and_then(|r| r.as_str()) {
        if !ret.is_empty() {
            let _ = write!(out, " -> {}", ret.trim_start_matches(':').trim());
        }
    }
    out
}

/// One parameter from a tree-sitter `parameters` entry as `type name` (either
/// part may be absent). A trailing `= default` is appended when present.
fn render_parameter(p: &Value) -> String {
    let type_part = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let name_part = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let mut out = match (type_part.is_empty(), name_part.is_empty()) {
        (false, false) => format!("{type_part} {name_part}"),
        (true, false) => name_part.to_string(),
        (false, true) => type_part.to_string(),
        (true, true) => String::new(),
    };
    if let Some(default) = p.get("default").and_then(|d| d.as_str()) {
        let _ = write!(out, " = {default}");
    }
    out
}

/// Emit the one-level file listing for `directory` the first time it is
/// surfaced this session. Composes `commands::list_` (JSON mode, no stdout
/// side effect) for the listing and `session_log::record_directory_touch` for
/// the per-session first-touch dedup. Silent when the directory was already
/// surfaced, has no files, or the listing fails.
fn emit_directory_line_on_first_touch(directory: &Path) {
    if !session_log::record_directory_touch(directory, "agent_read") {
        return;
    }
    let line = directory_files_line(directory);
    if !line.is_empty() {
        println!("{line}");
    }
}

/// One-line listing of a single directory's contents (one level,
/// non-recursive): its sub-directories (each suffixed `/`) followed by its
/// files, drawn from `commands::list_` in JSON mode so the listing logic is
/// reused rather than re-derived. Empty string when the directory holds
/// neither sub-directories nor files.
fn directory_files_line(directory: &Path) -> String {
    let value = match super::list_::run(directory, false, false, None, true) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    let names_of = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut entries: Vec<String> =
        names_of("directories").into_iter().map(|d| format!("{d}/")).collect();
    entries.extend(names_of("files"));
    if entries.is_empty() {
        return String::new();
    }
    let dir_name = directory
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| directory.to_string_lossy().to_string());
    format!("[dir {}/: {}]", dir_name, entries.join(", "))
}

/// One-line context-awareness hint: how many Claude.md / rules ancestors
/// for this path are already in the agent's context, and which are not.
/// Surfaces on every Read so the agent immediately sees whether the
/// project's rules for the file it just opened are already loaded.
///
/// Pure read against the session log and the doc walk-up — no
/// recording, no mutation. Empty string when the path has no ancestor
/// docs at all (no signal to surface).
fn docs_awareness_line(file_path: &Path, repo_root: &Path) -> String {
    let mut empty_dedupe: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let chain = nested_memory::load_for_file(file_path, repo_root, &mut empty_dedupe, false);
    if chain.is_empty() {
        return String::new();
    }
    let loaded = session_log::loaded_paths();
    let (in_context, not_loaded): (Vec<_>, Vec<_>) =
        chain.iter().partition(|m| loaded.contains(&m.path));
    let total = chain.len();
    let mut parts: Vec<String> = vec![format!("docs: {}/{} in context", in_context.len(), total)];
    if !not_loaded.is_empty() {
        let names: Vec<String> = not_loaded.iter().map(|m| m.relative_path.clone()).collect();
        parts.push(format!("not loaded: {}", names.join(", ")));
    }
    format!("[{}]", parts.join(" · "))
}

// --- Primer mode --------------------------------------------------------

pub fn run(
    path: Option<&Path>,
    force_directory: bool,
    offset: Option<usize>,
    limit: Option<usize>,
    record: bool,
) -> Result<()> {
    match path {
        None => {
            if force_directory {
                return Ok(());
            }
            primer_mode()
        }
        Some(path) => {
            let p = cache::absolutize(path);
            if !p.exists() {
                return Ok(());
            }
            if force_directory || p.is_dir() {
                emit_directory_line_on_first_touch(&p);
                return Ok(());
            }
            file_mode(&p, read_range(offset, limit), record)
        }
    }
}

fn primer_mode() -> Result<()> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let _ = architecture::get(&repo_root);
    let tracked = repo_files::tracked_files(&repo_root, None).unwrap_or_default();

    // Each section is an independent, self-contained read of repo state —
    // git subprocesses, the cached graph, scc facts. They share no mutable
    // state, so they run concurrently; the per-section git-spawn cost is the
    // primer's whole budget, and running the sections in parallel collapses
    // the serial sum to the slowest single section. Output is assembled in
    // fixed order below, so the bytes are identical to serial emission.
    let sections: Vec<String> = (0..8usize)
        .into_par_iter()
        .map(|i| match i {
            0 => environment_section(&repo_root),
            1 => identity_section(&repo_root),
            2 => tech_stack_section(&repo_root),
            3 => layout_section(&repo_root, &tracked),
            4 => common_directories_section(&repo_root),
            5 => git_section(&repo_root),
            6 => rules_section(&repo_root, &tracked),
            7 => spine_section(&repo_root),
            _ => unreachable!(),
        })
        .collect();

    for section in &sections {
        print!("{section}");
        println!();
    }

    let ctx = repo_context::repo_context(&repo_root);
    println!(
        "repo_context: complexity_p95={} median={} files={}",
        ctx["complexity_p95"].as_i64().unwrap_or(0),
        ctx["median_file_ccn"].as_i64().unwrap_or(0),
        ctx["total_files"].as_i64().unwrap_or(0),
    );
    Ok(())
}

// --- Section: Environment ----------------------------------------------

fn environment_section(repo_root: &Path) -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_git = repo_root.join(".git").exists();
    let is_worktree = worktree(repo_root);
    let shell = std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(|x| x.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(unknown)".into());
    // git_user spawns two `git config` reads; os_release spawns `uname`.
    // Independent — run them concurrently.
    let (git_user, os_release) = rayon::join(|| git_user(repo_root), || os_release());
    let date = unix_to_ymd(now_secs());

    let mut out = String::new();
    let _ = writeln!(out, "## Environment");
    let _ = writeln!(out, "  cwd: {cwd}");
    let _ = writeln!(out, "  repo root: {}", repo_root.to_string_lossy());
    let _ = writeln!(out, "  git repository: {}", if is_git { "yes" } else { "no" });
    let _ = writeln!(out, "  worktree: {}", if is_worktree { "yes" } else { "no" });
    let _ = writeln!(out, "  platform: {}", os_system().to_lowercase());
    let _ = writeln!(out, "  shell: {shell}");
    let _ = writeln!(out, "  os version: {} {}", os_system(), os_release);
    let _ = writeln!(out, "  git user: {git_user}");
    let _ = writeln!(out, "  date: {date}");
    out
}

/// Host OS name: "Darwin" / "Linux" / "Windows".
fn os_system() -> &'static str {
    match std::env::consts::OS {
        "macos" => "Darwin",
        "linux" => "Linux",
        "windows" => "Windows",
        other => other,
    }
}

fn os_release() -> String {
    Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn worktree(repo_root: &Path) -> bool {
    let git_path = repo_root.join(".git");
    if !git_path.is_file() {
        return false;
    }
    std::fs::read_to_string(&git_path)
        .map(|c| c.contains("worktrees/"))
        .unwrap_or(false)
}

fn git_user(repo_root: &Path) -> String {
    let (name, email) = rayon::join(
        || git_str(repo_root, &["config", "--get", "user.name"]).unwrap_or_default(),
        || git_str(repo_root, &["config", "--get", "user.email"]).unwrap_or_default(),
    );
    if !name.is_empty() && !email.is_empty() {
        format!("{name} <{email}>")
    } else if !name.is_empty() {
        name
    } else if !email.is_empty() {
        email
    } else {
        "(unset)".into()
    }
}

// --- Section: Identity --------------------------------------------------

fn identity_section(repo_root: &Path) -> String {
    let languages = repo_context::language_summary(repo_root);
    let mut out = String::new();
    let _ = writeln!(out, "## Identity");
    if languages.is_empty() {
        let _ = writeln!(out, "  (scc unavailable or empty result)");
        return out;
    }
    let total_files: i64 = languages
        .iter()
        .map(|l| l.get("Count").and_then(|x| x.as_i64()).unwrap_or(0))
        .sum();
    let total_loc: i64 = languages
        .iter()
        .map(|l| l.get("Code").and_then(|x| x.as_i64()).unwrap_or(0))
        .sum();
    let mut sorted = languages.clone();
    sorted.sort_by(|a, b| {
        b.get("Code")
            .and_then(|x| x.as_i64())
            .unwrap_or(0)
            .cmp(&a.get("Code").and_then(|x| x.as_i64()).unwrap_or(0))
    });

    let _ = writeln!(out, "  Files: {total_files}  Lines of code: {total_loc}");
    let _ = writeln!(out, "  Languages:");
    for lang in sorted.iter().take(PRIMER_LANGUAGE_LIMIT) {
        let _ = writeln!(
            out,
            "    {:<20} files={:<5} loc={:<8} complexity={}",
            lang.get("Name").and_then(|x| x.as_str()).unwrap_or("?"),
            lang.get("Count").and_then(|x| x.as_i64()).unwrap_or(0),
            lang.get("Code").and_then(|x| x.as_i64()).unwrap_or(0),
            lang.get("Complexity").and_then(|x| x.as_i64()).unwrap_or(0),
        );
    }
    let extra = sorted.len() as i64 - PRIMER_LANGUAGE_LIMIT as i64;
    if extra > 0 {
        let _ = writeln!(out, "    … {extra} more languages");
    }
    out
}

// --- Section: Tech Stack ------------------------------------------------

fn tech_stack_section(repo_root: &Path) -> String {
    let mut managers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (filename, manager) in PACKAGE_CONFIGS {
        if repo_root.join(filename).exists() {
            managers
                .entry(manager.to_string())
                .or_default()
                .push(filename.to_string());
        }
    }
    let configs: Vec<&str> = TOOL_CONFIGS
        .iter()
        .copied()
        .filter(|f| repo_root.join(f).exists())
        .collect();

    let mut out = String::new();
    let _ = writeln!(out, "## Tech Stack");
    if !managers.is_empty() {
        let _ = writeln!(out, "  Package managers:");
        for (manager, files) in &managers {
            let _ = writeln!(out, "    {manager}: {}", files.join(", "));
        }
    }
    if !configs.is_empty() {
        let _ = writeln!(out, "  Build / test / lint configs:");
        for config in &configs {
            let _ = writeln!(out, "    {config}");
        }
    }
    if managers.is_empty() && configs.is_empty() {
        let _ = writeln!(out, "  (no package or tool configs detected at repo root)");
    }
    out
}

// --- Section: Layout ----------------------------------------------------

fn layout_section(repo_root: &Path, tracked: &[String]) -> String {
    let skip = repo_files::skip_dirs();
    let mut by_top: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in tracked {
        let head = match rel.split_once('/') {
            Some((h, _)) => h,
            None => continue,
        };
        if head.starts_with('.') || skip.contains(head) {
            continue;
        }
        by_top.entry(head.to_string()).or_default().push(rel.clone());
    }

    let mut out = String::new();
    let _ = writeln!(out, "## Layout");
    if by_top.is_empty() {
        let _ = writeln!(out, "  (no source directories at top level)");
        return out;
    }

    let source_exts: std::collections::HashSet<&str> =
        crate::extraction::supported_extensions().iter().copied().collect();
    let git_map = git_activity::bulk_cached(repo_root);

    // SINGLE batch over every source file across all top-dirs (was per-file
    // get() in aggregate_paths — the same O(N²)+fsync-storm class as the
    // old `list` 62s defect; this is the `context` primer hot regression).
    // get_batch hoists bulk maps once with an in-memory mtime fast-path so
    // warm is fast; cold inherits the accepted no-lite-facts floor
    // (documented), but the hot path is now competitive.
    let all_source: Vec<std::path::PathBuf> = tracked
        .iter()
        .filter(|r| {
            Path::new(r)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| source_exts.contains(e.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|r| repo_root.join(r))
        .collect();
    let facts_map = file_facts::get_batch(&all_source, repo_root);

    // Case-insensitive sort by lowercased key.
    let mut names: Vec<&String> = by_top.keys().collect();
    names.sort_by_key(|n| n.to_lowercase());

    for name in names {
        let paths = &by_top[name];
        let summary = aggregate_paths(paths, &git_map, &facts_map);
        let mut bits = vec![
            format!("{} files", summary.0),
            format!("ccn={}", summary.1),
        ];
        if let Some(lm) = &summary.2 {
            bits.push(format!("last: {lm}"));
        }
        if summary.3 {
            bits.push("uncommitted".into());
        }
        let _ = writeln!(out, "  📁 {name}/  ({})", bits.join(" · "));
    }
    out
}

/// Per-subdir aggregation. Complexity comes from the per-file facts (only
/// source files are batched, so the rest contribute 0); last_modified and
/// working_state come from the bulk-cached git map that owns them.
fn aggregate_paths(
    relative_paths: &[String],
    git_map: &HashMap<String, git_activity::GitActivity>,
    facts_map: &HashMap<String, file_facts::FileFacts>,
) -> (usize, i64, Option<String>, bool) {
    let mut ccn_total = 0i64;
    let mut last_modified: Option<String> = None;
    let mut has_uncommitted = false;

    for rel in relative_paths {
        ccn_total += facts_map
            .get(rel)
            .map(|f| f.cyclomatic_complexity_total)
            .unwrap_or(0);
        let activity = git_map.get(rel);
        let modified = activity.and_then(|a| a.last_modified.clone());
        let state = activity.and_then(|a| a.working_state.clone());
        if let Some(m) = &modified {
            if last_modified.as_ref().map(|lm| m > lm).unwrap_or(true) {
                last_modified = Some(m.clone());
            }
        }
        if state.as_deref().map(|s| DIRTY_STATES.contains(&s)).unwrap_or(false) {
            has_uncommitted = true;
        }
    }
    (relative_paths.len(), ccn_total, last_modified, has_uncommitted)
}

// --- Section: Common Directories ---------------------------------------

fn common_directories_section(repo_root: &Path) -> String {
    let skip = repo_files::skip_dirs();
    let mut classifications: BTreeMap<&str, Vec<(String, String)>> =
        COMMON_KINDS.iter().map(|k| (*k, Vec::new())).collect();

    for (marker, label) in CI_MARKERS {
        if repo_root.join(marker).exists() {
            classifications
                .get_mut("continuous-integration")
                .unwrap()
                .push((marker.to_string(), label.to_string()));
        }
    }

    // Walk `repo_root` then each child dir in raw OS directory order — do
    // NOT sort. The section order is intentionally the filesystem's own
    // `read_dir` order; sorting here would change the emitted section order.
    let mut candidate_dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(repo_root) {
        for child in rd.flatten().map(|e| e.path()) {
            if !child.is_dir() {
                continue;
            }
            let n = child.file_name().unwrap_or_default().to_string_lossy().to_string();
            if n.starts_with('.') || skip.contains(n.as_str()) {
                continue;
            }
            // A nested repository is its own scope: a directory holding many
            // checkouts must not report their contents as this root's layout.
            if child.join(".git").exists() {
                continue;
            }
            candidate_dirs.push(child.clone());
            if let Ok(sub_rd) = std::fs::read_dir(&child) {
                for sub in sub_rd.flatten().map(|e| e.path()) {
                    if !sub.is_dir() {
                        continue;
                    }
                    let sn = sub.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if sn.starts_with('.') || skip.contains(sn.as_str()) {
                        continue;
                    }
                    candidate_dirs.push(sub);
                }
            }
        }
    }

    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for d in &candidate_dirs {
        let rel = match d.strip_prefix(repo_root) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => continue,
        };
        for (kind, marker) in classify_directory(d) {
            let key = (kind.to_string(), rel.clone());
            if !seen.insert(key) {
                continue;
            }
            classifications.get_mut(kind).unwrap().push((rel.clone(), marker));
        }
    }

    let mut out = String::new();
    let _ = writeln!(out, "## Common Directories");
    let mut any_found = false;
    for kind in COMMON_KINDS {
        let entries = &classifications[kind];
        if entries.is_empty() {
            continue;
        }
        any_found = true;
        let _ = writeln!(out, "  {kind}:");
        for (path, marker) in entries {
            let _ = writeln!(out, "    {path}  ({marker})");
        }
    }
    if !any_found {
        let _ = writeln!(out, "  (no common directories detected)");
    }
    out
}

fn classify_directory(directory: &Path) -> Vec<(&'static str, String)> {
    let entries: Vec<std::path::PathBuf> = match std::fs::read_dir(directory) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => return vec![],
    };
    let mut file_names: Vec<String> = Vec::new();
    let mut ext_counts: HashMap<String, i64> = HashMap::new();
    for entry in &entries {
        if entry.is_file() {
            file_names.push(
                entry.file_name().unwrap_or_default().to_string_lossy().to_string(),
            );
            let ext = entry
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| format!(".{}", s.to_lowercase()))
                .unwrap_or_default();
            *ext_counts.entry(ext).or_insert(0) += 1;
        }
    }
    let ec = |e: &str| *ext_counts.get(e).unwrap_or(&0);
    let mut labels: Vec<(&'static str, String)> = Vec::new();

    let frontend = ec(".tsx") + ec(".jsx") + ec(".vue") + ec(".svelte");
    if frontend >= 3 {
        labels.push(("frontend", format!("{frontend} tsx/jsx/vue/svelte files")));
    }

    let mut backend: Vec<String> = Vec::new();
    let name_set: std::collections::HashSet<&str> =
        file_names.iter().map(|s| s.as_str()).collect();
    if name_set.contains("artisan") {
        backend.push("Laravel".into());
    }
    if ["manage.py", "wsgi.py", "asgi.py"].iter().any(|n| name_set.contains(n)) {
        backend.push("Django/Flask".into());
    }
    if name_set.contains("config.ru")
        || (name_set.contains("Gemfile")
            && directory.join("config").join("application.rb").exists())
    {
        backend.push("Rails".into());
    }
    let php_count = ec(".php");
    if php_count >= 3
        && file_names.iter().any(|n| {
            n.ends_with("Controller.php")
                || n.ends_with("Model.php")
                || n.ends_with("Service.php")
        })
    {
        backend.push(format!("{php_count} PHP files (controller/model/service)"));
    }
    if !backend.is_empty() {
        labels.push(("backend", backend.join(", ")));
    }

    let ts_prefixed = file_names
        .iter()
        .filter(|n| is_timestamp_prefixed(n))
        .count();
    if ts_prefixed >= 2 {
        labels.push((
            "database-migrations",
            format!("{ts_prefixed} timestamp-prefixed files"),
        ));
    }

    let test_configs: Vec<&String> = file_names
        .iter()
        .filter(|n| TEST_CONFIG_NAMES.contains(&n.as_str()))
        .collect();
    if !test_configs.is_empty() {
        let joined = test_configs
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        labels.push(("tests", format!("config: {joined}")));
    } else {
        let test_count = file_names
            .iter()
            .filter(|n| {
                n.contains("_test.")
                    || n.contains(".test.")
                    || n.starts_with("test_")
                    || n.ends_with("Test.php")
                    || n.ends_with("Spec.php")
            })
            .count();
        if test_count >= 2 {
            labels.push(("tests", format!("{test_count} test files")));
        }
    }

    let shell_count = file_names
        .iter()
        .filter(|n| n.ends_with(".sh") || n.ends_with(".bash") || n.ends_with(".zsh"))
        .count();
    if shell_count >= 2 {
        labels.push(("scripts", format!("{shell_count} shell scripts")));
    } else {
        let shebang = count_shebangs(&entries);
        if shebang >= 2 {
            labels.push(("scripts", format!("{shebang} shebang scripts")));
        }
    }

    labels
}

/// `re.match(r"^\d{4}[_-]\d{2}[_-]\d{2}", name)`.
fn is_timestamp_prefixed(name: &str) -> bool {
    let b = name.as_bytes();
    if b.len() < 10 {
        return false;
    }
    let d = |i: usize| b[i].is_ascii_digit();
    let sep = |i: usize| b[i] == b'_' || b[i] == b'-';
    d(0) && d(1) && d(2) && d(3) && sep(4) && d(5) && d(6) && sep(7) && d(8) && d(9)
}

fn count_shebangs(entries: &[std::path::PathBuf]) -> usize {
    let mut count = 0;
    for entry in entries {
        if !entry.is_file() || entry.extension().is_some() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(entry) {
            if bytes.len() >= 2 && &bytes[..2] == b"#!" {
                count += 1;
            }
        }
    }
    count
}

// --- Section: Git -------------------------------------------------------

fn git_section(repo_root: &Path) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## Git");
    if !repo_root.join(".git").exists() {
        let _ = writeln!(out, "  (not a git repository)");
        return out;
    }

    // Wave 1: four independent git reads run concurrently. Wave 2:
    // candidates needs origin_head, ahead_behind needs origin_head + current,
    // so they resolve once wave 1 lands.
    let ((origin_head, current), (dirty, commits)) = rayon::join(
        || rayon::join(|| origin_head_branch(repo_root), || current_branch(repo_root)),
        || {
            rayon::join(
                || git_activity::working_tree_state(repo_root),
                || recent_commit_subjects(repo_root, PRIMER_COMMIT_LIMIT),
            )
        },
    );
    let (candidates, ahead_behind) = rayon::join(
        || primary_branch_candidates(repo_root, origin_head.as_deref()),
        || ahead_behind(repo_root, &current, origin_head.as_deref()),
    );

    if !candidates.is_empty() {
        let _ = writeln!(out, "  Primary branch candidates:");
        for (name, info) in &candidates {
            let _ = writeln!(out, "    {name}  ({info})");
        }
    }

    let suffix = if ahead_behind.is_empty() {
        String::new()
    } else {
        format!("  ({ahead_behind})")
    };
    let _ = writeln!(out, "  Current branch: {current}{suffix}");

    if !dirty.is_empty() {
        let _ = writeln!(out, "  Dirty files ({}):", dirty.len());
        for line in render_dirty(repo_root, &dirty) {
            let _ = writeln!(out, "    {line}");
        }
    } else {
        let _ = writeln!(out, "  Working tree clean");
    }

    if !commits.is_empty() {
        let _ = writeln!(out, "  Recent commits ({}):", commits.len());
        for line in &commits {
            let _ = writeln!(out, "    {line}");
        }
    }
    out
}

fn primary_branch_candidates(
    repo_root: &Path,
    origin_head: Option<&str>,
) -> Vec<(String, String)> {
    let stdout = match git_str(
        repo_root,
        &[
            "for-each-ref",
            "--format=%(refname:short)\t%(committerdate:iso8601)",
            "refs/heads/",
            "refs/remotes/origin/",
        ],
    ) {
        Some(s) => s,
        None => return vec![],
    };

    let cutoff = now_secs() - PRIMER_BRANCH_STALE_DAYS * 86400;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut candidates: Vec<(String, String, bool)> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() != 2 {
            continue;
        }
        let raw_name = parts[0];
        let date_text = parts[1].trim();
        let short = raw_name
            .strip_prefix("origin/")
            .unwrap_or(raw_name)
            .to_string();
        if short == "HEAD" || seen.contains(&short) {
            continue;
        }
        let is_feature = FEATURE_PREFIXES.iter().any(|p| short.starts_with(p));
        let is_origin_head = Some(short.as_str()) == origin_head;
        if is_feature && !is_origin_head {
            continue;
        }
        let branch_secs = parse_iso_secs(date_text);
        let is_stale = branch_secs.map(|s| s < cutoff).unwrap_or(false);
        if is_stale && !is_origin_head {
            continue;
        }
        seen.insert(short.clone());
        let date_short = date_text.split_whitespace().next().unwrap_or("(unknown)");
        let marker = if is_origin_head { " [origin/HEAD]" } else { "" };
        candidates.push((short, format!("last: {date_short}{marker}"), is_origin_head));
    }

    candidates.sort_by(|a, b| {
        let ka = (if a.2 { 0 } else { 1 }, a.0.clone());
        let kb = (if b.2 { 0 } else { 1 }, b.0.clone());
        ka.cmp(&kb)
    });
    candidates.into_iter().map(|(n, i, _)| (n, i)).collect()
}

fn origin_head_branch(repo_root: &Path) -> Option<String> {
    let ref_ = git_str(
        repo_root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )?;
    let stripped = ref_.strip_prefix("origin/").unwrap_or(&ref_);
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

fn current_branch(repo_root: &Path) -> String {
    match git_str(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Some(s) if !s.is_empty() => s,
        Some(_) => "(detached)".into(),
        None => "(unknown)".into(),
    }
}

fn ahead_behind(repo_root: &Path, current: &str, base: Option<&str>) -> String {
    let base = match base {
        Some(b) => b,
        None => return String::new(),
    };
    if current == base || current == "(unknown)" || current == "(detached)" {
        return String::new();
    }
    let stdout = match git_str(
        repo_root,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("origin/{base}...HEAD"),
        ],
    ) {
        Some(s) => s,
        None => return String::new(),
    };
    let parts: Vec<&str> = stdout.split_whitespace().collect();
    if parts.len() != 2 {
        return String::new();
    }
    format!(
        "ahead: {}, behind: {} vs origin/{base}",
        parts[1], parts[0]
    )
}

fn render_dirty(repo_root: &Path, dirty: &HashMap<String, String>) -> Vec<String> {
    let graph = architecture::load_cached(repo_root);
    // One batch resolve for every dirty file — a single mtime-index load plus
    // parallel extraction — instead of a per-file get() loop that reloaded
    // the whole mtime index for each dirty file (the dominant primer cost on
    // a repo with many uncommitted files).
    let existing: Vec<std::path::PathBuf> = dirty
        .keys()
        .map(|p| repo_root.join(p))
        .filter(|abs| abs.exists())
        .collect();
    let facts_map = file_facts::get_batch(&existing, repo_root);
    let mut scored: Vec<(i64, i64, String, String)> = Vec::new();
    for (path, state) in dirty {
        let mut callers = 0i64;
        if let Some(g) = &graph {
            if let Some(module_id) = g.file_to_module_id.get(path) {
                callers = architecture::dependents_of(g, module_id).len() as i64;
            }
        }
        let rel = cache::relative_to_root(&repo_root.join(path), repo_root);
        let ccn = facts_map
            .get(&rel)
            .map(|f| f.cyclomatic_complexity_total)
            .unwrap_or(0);
        scored.push((callers, ccn, state.clone(), path.clone()));
    }
    // Rank by callers then ccn, both descending. Pre-sort by path so the
    // stable primary/secondary sort is fully deterministic.
    scored.sort_by(|a, b| a.3.cmp(&b.3));
    scored.sort_by(|a, b| (-a.0, -a.1).cmp(&(-b.0, -b.1)));

    let mut lines: Vec<String> = scored
        .iter()
        .take(PRIMER_DIRTY_LIMIT)
        .map(|(c, ccn, state, path)| {
            format!("{state:<10} {path}  (callers={c}, ccn={ccn})")
        })
        .collect();
    if scored.len() > PRIMER_DIRTY_LIMIT {
        lines.push(format!("… {} more", scored.len() - PRIMER_DIRTY_LIMIT));
    }
    lines
}

fn recent_commit_subjects(repo_root: &Path, limit: usize) -> Vec<String> {
    let out = Command::new("git")
        .args(["log", &format!("-n{limit}"), "--pretty=format:%h %s"])
        .current_dir(repo_root)
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect(),
        _ => vec![],
    }
}

/// Parse `git for-each-ref` iso8601 ("YYYY-MM-DD HH:MM:SS ±ZZZZ") to unix
/// seconds. Only the date portion is needed for the staleness compare, but
/// the time and UTC offset are honored for an exact instant.
fn parse_iso_secs(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mut it = text.split_whitespace();
    let date = it.next()?;
    let time = it.next().unwrap_or("00:00:00");
    let offset = it.next().unwrap_or("+0000");

    let dp: Vec<&str> = date.split('-').collect();
    if dp.len() != 3 {
        return None;
    }
    let y: i64 = dp[0].parse().ok()?;
    let mo: i64 = dp[1].parse().ok()?;
    let d: i64 = dp[2].parse().ok()?;
    let tp: Vec<&str> = time.split(':').collect();
    let hh: i64 = tp.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mm: i64 = tp.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let ss: i64 = tp.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    let days = days_from_civil(y, mo, d);
    let mut secs = days * 86400 + hh * 3600 + mm * 60 + ss;

    // offset like +0200 / -0500 — subtract to get UTC.
    if offset.len() == 5 {
        let sign = if &offset[..1] == "-" { -1 } else { 1 };
        let oh: i64 = offset[1..3].parse().unwrap_or(0);
        let om: i64 = offset[3..5].parse().unwrap_or(0);
        secs -= sign * (oh * 3600 + om * 60);
    }
    Some(secs)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn unix_to_ymd(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

// --- Section: Rules -----------------------------------------------------

fn rules_section(repo_root: &Path, tracked: &[String]) -> String {
    let claude_md = collect_claude_md(tracked);
    let rules_files = collect_rules_dir(tracked);

    let mut out = String::new();
    let _ = writeln!(out, "## Rules");
    if !claude_md.is_empty() {
        let _ = writeln!(out, "  Claude.md files ({}):", claude_md.len());
        for rel in &claude_md {
            let _ = writeln!(out, "    {rel}");
        }
    }
    if !rules_files.is_empty() {
        let _ = writeln!(out, "  Project rules ({}):", rules_files.len());
        for rel in &rules_files {
            let _ = writeln!(out, "    {rel}");
        }
    }
    if claude_md.is_empty() && rules_files.is_empty() {
        let _ = writeln!(out, "  (no Claude.md or .claude/rules/ found)");
    }

    // Conditional rules whose `paths:` globs match a working-tree-dirty file
    // but that aren't already in the session's context. The plain rules list
    // above tells the agent these files exist; this tells it which ones govern
    // the work it's about to touch, so a constraint is surfaced before it's
    // violated rather than discovered by the violation. Capped so a repo of
    // broad-glob rules can't flood the section.
    let applicable = applicable_unloaded_rules(repo_root);
    if !applicable.is_empty() {
        let _ = writeln!(
            out,
            "  Applies to your changes, not loaded ({}):",
            applicable.len()
        );
        for (rule, glob) in &applicable {
            let _ = writeln!(out, "    {rule} (matches {glob})");
        }
    }
    out
}

/// Conditional rules (`.claude/rules/*.md` with a `paths:` frontmatter) whose
/// globs match at least one working-tree-dirty file and that the session log
/// has not already surfaced into context. Each entry pairs the rule's path
/// with the glob that matched, so the agent sees why the rule applies. Capped
/// at `PRIMER_APPLICABLE_RULES_LIMIT`.
///
/// The conditional rules and their globs come from the cached architecture
/// graph's doc nodes (the docs-graph walk already parsed every rule's
/// frontmatter); the dirty set from the working tree; the loaded set from the
/// session log. The three join here at render time only.
fn applicable_unloaded_rules(repo_root: &Path) -> Vec<(String, String)> {
    let graph = match architecture::load_cached(repo_root) {
        Some(g) => g,
        None => return vec![],
    };
    let conditional: Vec<&docs_graph::DocNode> = graph
        .doc_nodes
        .iter()
        .filter(|n| n.paths_globs.as_ref().map(|g| !g.is_empty()).unwrap_or(false))
        .collect();
    if conditional.is_empty() {
        return vec![];
    }

    let dirty = git_activity::working_tree_state(repo_root);
    let dirty_abs: Vec<std::path::PathBuf> = dirty
        .iter()
        .filter(|(_, state)| DIRTY_STATES.contains(&state.as_str()))
        .map(|(rel, _)| repo_root.join(rel))
        .collect();
    if dirty_abs.is_empty() {
        return vec![];
    }

    let loaded = session_log::loaded_paths();

    let mut out: Vec<(String, String)> = Vec::new();
    for node in conditional {
        // A rule already in context needs no surfacing — the agent has its
        // text. Match against the session log's canonical-path keys.
        let rule_abs = repo_root.join(&node.path);
        let canonical = rule_abs
            .canonicalize()
            .unwrap_or(rule_abs)
            .to_string_lossy()
            .to_string();
        if loaded.contains(&canonical) {
            continue;
        }
        let globs = match &node.paths_globs {
            Some(g) => g,
            None => continue,
        };
        // First dirty file × first glob that matches — report the rule once
        // with the glob that triggered it, not once per matching file.
        let mut matched_glob: Option<&String> = None;
        'outer: for file in &dirty_abs {
            for glob in globs {
                if super::paths_match::matches_paths(file, std::slice::from_ref(glob), repo_root) {
                    matched_glob = Some(glob);
                    break 'outer;
                }
            }
        }
        if let Some(glob) = matched_glob {
            out.push((node.path.clone(), glob.clone()));
        }
    }
    out.sort();
    out.truncate(PRIMER_APPLICABLE_RULES_LIMIT);
    out
}

fn collect_claude_md(tracked: &[String]) -> Vec<String> {
    let skip = repo_files::skip_dirs();
    // Dedupe by casefolded path (case-insensitive filesystems can surface
    // the same file under either casing), then sort the original-case
    // paths, NOT the casefolded keys.
    let mut by_key: BTreeMap<String, String> = BTreeMap::new();
    for rel in tracked {
        if rel
            .split('/')
            .any(|part| part.starts_with('.') || skip.contains(part))
        {
            continue;
        }
        let basename = rel.rsplit('/').next().unwrap_or(rel);
        if basename.to_lowercase() != "claude.md" {
            continue;
        }
        by_key.entry(rel.to_lowercase()).or_insert_with(|| rel.clone());
    }
    let mut values: Vec<String> = by_key.into_values().collect();
    values.sort();
    values
}

fn collect_rules_dir(tracked: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tracked
        .iter()
        .filter(|r| r.starts_with(".claude/rules/") && r.ends_with(".md"))
        .cloned()
        .collect();
    out.sort();
    out
}

// --- Section: Spine -----------------------------------------------------

fn spine_section(repo_root: &Path) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## Spine");
    let graph = match architecture::load_cached(repo_root) {
        Some(g) if !g.edges.is_empty() => g,
        _ => {
            let _ = writeln!(
                out,
                "  (architecture graph empty — run `trace cache build` if you expect data)"
            );
            return out;
        }
    };

    // Rank by transitive dependent count — how many nodes ultimately depend
    // on a node — not raw direct in-edges, so a node imported by one hub that
    // everything else imports ranks as load-bearing rather than buried. Same
    // two-stage shape as `downstream --path`: count direct import in-edges
    // first, compute the transitive dependent set only for the top direct
    // candidates (the BFS is the cost), then re-rank by `(transitive, direct)`
    // descending. Reference edges are a separate dimension and stay out of
    // module-level centrality, matching `downstream --path`.
    let mut direct: HashMap<String, i64> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for edge in &graph.edges {
        if edge.relation != architecture::RELATION_IMPORTS {
            continue;
        }
        if !direct.contains_key(&edge.target) {
            order.push(edge.target.clone());
        }
        *direct.entry(edge.target.clone()).or_insert(0) += 1;
    }
    let mut by_direct: Vec<(usize, String, i64)> = order
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.starts_with("module::external::"))
        .map(|(i, t)| (i, t.clone(), direct[t]))
        .collect();
    by_direct.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));

    // Compute transitive dependents for a candidate pool wider than the final
    // cut (a node with few direct in-edges can still sit under a hub and reach
    // a large transitive set), then take the most-depended-on after re-ranking.
    let candidate_pool: Vec<String> = by_direct
        .iter()
        .take(PRIMER_SPINE_LIMIT * 3)
        .map(|(_, id, _)| id.clone())
        .collect();
    let mut transitive: HashMap<String, i64> = HashMap::new();
    for node_id in &candidate_pool {
        transitive.insert(
            node_id.clone(),
            architecture::transitive_dependents(&graph, node_id, i64::MAX).len() as i64,
        );
    }
    let first_seen: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut ranked: Vec<String> = candidate_pool.clone();
    ranked.sort_by(|a, b| {
        let ta = *transitive.get(a).unwrap_or(&0);
        let tb = *transitive.get(b).unwrap_or(&0);
        let da = *direct.get(a).unwrap_or(&0);
        let db = *direct.get(b).unwrap_or(&0);
        // (transitive, direct) descending, ties broken by first-seen order so
        // the output is fully deterministic.
        (tb, db)
            .cmp(&(ta, da))
            .then(first_seen[a.as_str()].cmp(&first_seen[b.as_str()]))
    });
    ranked.truncate(PRIMER_SPINE_LIMIT);

    if ranked.is_empty() {
        let _ = writeln!(out, "  (no internal nodes in the architecture graph)");
        return out;
    }

    let _ = writeln!(out, "  Top {} most-depended-on nodes:", ranked.len());
    let _ = writeln!(
        out,
        "    {:<3} {:>6} {:>10}  {:<10} symbol @ source",
        "#", "direct", "transitive", "kind"
    );
    for (rank, node_id) in ranked.iter().enumerate() {
        let node = match graph.nodes.get(node_id) {
            Some(n) => n,
            None => continue,
        };
        let location = match &node.source_file {
            Some(sf) => match node.source_line {
                Some(l) => format!("{sf}:{l}"),
                None => sf.clone(),
            },
            None => "(no source)".into(),
        };
        let _ = writeln!(
            out,
            "    {:<3} {:>6} {:>10}  {:<10} {} @ {}",
            rank + 1,
            direct.get(node_id).copied().unwrap_or(0),
            transitive.get(node_id).copied().unwrap_or(0),
            node.kind,
            node.label,
            location,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A throwaway git worktree — `git init` makes `cache::worktree_root_for`
    /// resolve to the fixture root, the same precondition the production read
    /// path always runs under. Dropped — and removed from disk — at scope end.
    struct Fixture {
        root: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            // Per-fixture unique suffix: tests run in parallel threads of one
            // process, so the pid is shared and `now_secs()` is second-granular
            // — a monotonic counter is what keeps two fixtures from colliding.
            use std::sync::atomic::{AtomicU64, Ordering};
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "tracer_ctx_test_{}_{}_{}",
                std::process::id(),
                now_secs(),
                SEQ.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir_all(&root).unwrap();
            let status = Command::new("git")
                .args(["init", "-q"])
                .current_dir(&root)
                .status()
                .expect("git init");
            assert!(status.success(), "git init failed in {}", root.display());
            Fixture { root }
        }

        fn write(&self, rel: &str, contents: &str) {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        /// Stage every file so `git ls-files` (the listing's tracked-file
        /// source) sees the tree. Git cannot track an empty directory, so a
        /// sub-directory only surfaces in the listing once it holds a tracked
        /// file — the same as in a real repo.
        fn stage(&self) {
            let status = Command::new("git")
                .args(["add", "-A"])
                .current_dir(&self.root)
                .status()
                .expect("git add");
            assert!(status.success(), "git add failed");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    // The richer symbol surface: the methods line carries each symbol's full
    // signature (parameters + return type) and per-method cyclomatic
    // complexity — the fidelity of `trace structure`'s per-method view — not
    // bare names.
    #[test]
    fn symbols_line_carries_signatures_not_bare_names() {
        let fx = Fixture::new();
        fx.write(
            "sample.rs",
            "pub fn greet(name: &str, times: u32) -> String {\n\
             \x20   let mut out = String::new();\n\
             \x20   for _ in 0..times { out.push_str(name); }\n\
             \x20   out\n\
             }\n\
             \n\
             fn helper(x: i64) -> i64 {\n\
             \x20   if x > 0 { x } else { -x }\n\
             }\n",
        );

        let line = symbols_line(&fx.root.join("sample.rs"));

        assert!(line.starts_with("[symbols: "), "got: {line}");
        // Full parameter + return signature, not a bare `greet`.
        assert!(
            line.contains("greet(name: &str, times: u32) -> String"),
            "expected greet's full signature, got: {line}"
        );
        assert!(
            line.contains("helper(x: i64) -> i64"),
            "expected helper's full signature, got: {line}"
        );
        // Per-method cyclomatic complexity is joined in, as in the structure view.
        assert!(line.contains("ccn="), "expected per-method ccn, got: {line}");
    }

    // The directory listing surfaces sub-directories alongside files: each
    // sub-directory is suffixed `/` and listed ahead of the files.
    #[test]
    fn directory_line_lists_subdirectories_and_files() {
        let fx = Fixture::new();
        // Git can't track an empty directory; a tracked file inside each
        // sub-directory is what makes it surface — same as a real repo.
        fx.write("sub_one/a.rs", "fn a() {}\n");
        fx.write("sub_two/b.rs", "fn b() {}\n");
        fx.write("readme.md", "x");
        fx.write("other.rs", "fn t() {}\n");
        fx.stage();

        let line = directory_files_line(&fx.root);

        assert!(line.starts_with("[dir "), "got: {line}");
        // Sub-directories present, suffixed `/`.
        assert!(line.contains("sub_one/"), "expected sub_one/, got: {line}");
        assert!(line.contains("sub_two/"), "expected sub_two/, got: {line}");
        // Files still present.
        assert!(line.contains("readme.md"), "expected readme.md, got: {line}");
        assert!(line.contains("other.rs"), "expected other.rs, got: {line}");
        // Sub-directories ordered ahead of files.
        let sub_one = line.find("sub_one/").unwrap();
        let readme = line.find("readme.md").unwrap();
        assert!(sub_one < readme, "sub-dirs should precede files, got: {line}");
    }
}
