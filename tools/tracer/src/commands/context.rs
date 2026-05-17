//! `trace context` — two modes off one command.
//!
//! No-args: the eight-section session-start primer (environment, identity,
//! tech stack, layout, common directories, git, rules, spine) plus the
//! repo_context footer. First invocation warms the file + architecture
//! caches. File-arg: single-file enrichment — one passive_context line.
//!
//! CCN is AST-derived; the Layout per-path aggregation uses the real
//! `file_facts::get` (no lite-facts shortcut).

use crate::{architecture, cache, file_facts, git_activity, passive_context, repo_context, repo_files};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PRIMER_LANGUAGE_LIMIT: usize = 10;
const PRIMER_DIRTY_LIMIT: usize = 10;
const PRIMER_COMMIT_LIMIT: usize = 10;
const PRIMER_SPINE_LIMIT: usize = 10;
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

fn file_mode(p: &Path) -> Result<()> {
    let facts = match file_facts::get(p, &cache::repo_root_for(p), None) {
        Some(f) => f,
        None => return Ok(()),
    };
    let repo_root = cache::repo_root_for(p);
    let gc = graph_counts(p, &repo_root);
    let line = passive_context::render(&facts, gc.as_ref());
    if !line.is_empty() {
        println!("{line}");
    }
    Ok(())
}

// --- Primer mode --------------------------------------------------------

pub fn run(path: Option<&Path>, force_directory: bool) -> Result<()> {
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
                return Ok(());
            }
            file_mode(&p)
        }
    }
}

fn primer_mode() -> Result<()> {
    let repo_root = cache::repo_root_for(Path::new("."));
    let _ = architecture::get(&repo_root);
    let tracked = repo_files::tracked_files(&repo_root, None).unwrap_or_default();

    emit_environment(&repo_root);
    println!();
    emit_identity(&repo_root);
    println!();
    emit_tech_stack(&repo_root);
    println!();
    emit_layout(&repo_root, &tracked);
    println!();
    emit_common_directories(&repo_root);
    println!();
    emit_git(&repo_root);
    println!();
    emit_rules(&repo_root, &tracked);
    println!();
    emit_spine(&repo_root);

    let ctx = repo_context::repo_context(&repo_root);
    println!();
    println!(
        "repo_context: complexity_p95={} median={} files={}",
        ctx["complexity_p95"].as_i64().unwrap_or(0),
        ctx["median_file_ccn"].as_i64().unwrap_or(0),
        ctx["total_files"].as_i64().unwrap_or(0),
    );
    Ok(())
}

// --- Section: Environment ----------------------------------------------

fn emit_environment(repo_root: &Path) {
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
    let git_user = git_user(repo_root);
    let date = unix_to_ymd(now_secs());

    println!("## Environment");
    println!("  cwd: {cwd}");
    println!("  repo root: {}", repo_root.to_string_lossy());
    println!("  git repository: {}", if is_git { "yes" } else { "no" });
    println!("  worktree: {}", if is_worktree { "yes" } else { "no" });
    println!("  platform: {}", os_system().to_lowercase());
    println!("  shell: {shell}");
    println!("  os version: {} {}", os_system(), os_release());
    println!("  git user: {git_user}");
    println!("  date: {date}");
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
    let name = git_str(repo_root, &["config", "--get", "user.name"]).unwrap_or_default();
    let email = git_str(repo_root, &["config", "--get", "user.email"]).unwrap_or_default();
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

fn emit_identity(repo_root: &Path) {
    let languages = repo_context::language_summary(repo_root);
    println!("## Identity");
    if languages.is_empty() {
        println!("  (scc unavailable or empty result)");
        return;
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

    println!("  Files: {total_files}  Lines of code: {total_loc}");
    println!("  Languages:");
    for lang in sorted.iter().take(PRIMER_LANGUAGE_LIMIT) {
        println!(
            "    {:<20} files={:<5} loc={:<8} complexity={}",
            lang.get("Name").and_then(|x| x.as_str()).unwrap_or("?"),
            lang.get("Count").and_then(|x| x.as_i64()).unwrap_or(0),
            lang.get("Code").and_then(|x| x.as_i64()).unwrap_or(0),
            lang.get("Complexity").and_then(|x| x.as_i64()).unwrap_or(0),
        );
    }
    let extra = sorted.len() as i64 - PRIMER_LANGUAGE_LIMIT as i64;
    if extra > 0 {
        println!("    … {extra} more languages");
    }
}

// --- Section: Tech Stack ------------------------------------------------

fn emit_tech_stack(repo_root: &Path) {
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

    println!("## Tech Stack");
    if !managers.is_empty() {
        println!("  Package managers:");
        for (manager, files) in &managers {
            println!("    {manager}: {}", files.join(", "));
        }
    }
    if !configs.is_empty() {
        println!("  Build / test / lint configs:");
        for config in &configs {
            println!("    {config}");
        }
    }
    if managers.is_empty() && configs.is_empty() {
        println!("  (no package or tool configs detected at repo root)");
    }
}

// --- Section: Layout ----------------------------------------------------

fn emit_layout(repo_root: &Path, tracked: &[String]) {
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

    println!("## Layout");
    if by_top.is_empty() {
        println!("  (no source directories at top level)");
        return;
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
        let summary =
            aggregate_paths(paths, &source_exts, &git_map, &facts_map);
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
        println!("  📁 {name}/  ({})", bits.join(" · "));
    }
}

/// Per-subdir aggregation. Source files pull real per-file facts (no
/// lite-facts shortcut). Non-source files take last_modified /
/// working_state from the bulk-cached git map.
fn aggregate_paths(
    relative_paths: &[String],
    source_exts: &std::collections::HashSet<&str>,
    git_map: &HashMap<String, git_activity::GitActivity>,
    facts_map: &HashMap<String, file_facts::FileFacts>,
) -> (usize, i64, Option<String>, bool) {
    let mut ccn_total = 0i64;
    let mut last_modified: Option<String> = None;
    let mut has_uncommitted = false;

    for rel in relative_paths {
        let ext = Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());
        let (ccn, modified, state) = if ext
            .as_deref()
            .map(|e| source_exts.contains(e))
            .unwrap_or(false)
        {
            match facts_map.get(rel) {
                Some(f) => (
                    f.cyclomatic_complexity_total,
                    f.last_modified.clone(),
                    f.working_state.clone(),
                ),
                None => {
                    let a = git_map.get(rel).cloned().unwrap_or_else(git_activity::GitActivity::empty);
                    (0, a.last_modified, a.working_state)
                }
            }
        } else {
            let a = git_map.get(rel).cloned().unwrap_or_else(git_activity::GitActivity::empty);
            (0, a.last_modified, a.working_state)
        };
        ccn_total += ccn;
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

fn emit_common_directories(repo_root: &Path) {
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

    println!("## Common Directories");
    let mut any_found = false;
    for kind in COMMON_KINDS {
        let entries = &classifications[kind];
        if entries.is_empty() {
            continue;
        }
        any_found = true;
        println!("  {kind}:");
        for (path, marker) in entries {
            println!("    {path}  ({marker})");
        }
    }
    if !any_found {
        println!("  (no common directories detected)");
    }
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

fn emit_git(repo_root: &Path) {
    println!("## Git");
    if !repo_root.join(".git").exists() {
        println!("  (not a git repository)");
        return;
    }

    let origin_head = origin_head_branch(repo_root);
    let current = current_branch(repo_root);
    let dirty = git_activity::working_tree_state(repo_root);
    let commits = recent_commit_subjects(repo_root, PRIMER_COMMIT_LIMIT);

    let candidates = primary_branch_candidates(repo_root, origin_head.as_deref());
    let ahead_behind = ahead_behind(repo_root, &current, origin_head.as_deref());

    if !candidates.is_empty() {
        println!("  Primary branch candidates:");
        for (name, info) in &candidates {
            println!("    {name}  ({info})");
        }
    }

    let suffix = if ahead_behind.is_empty() {
        String::new()
    } else {
        format!("  ({ahead_behind})")
    };
    println!("  Current branch: {current}{suffix}");

    if !dirty.is_empty() {
        println!("  Dirty files ({}):", dirty.len());
        for line in render_dirty(repo_root, &dirty) {
            println!("    {line}");
        }
    } else {
        println!("  Working tree clean");
    }

    if !commits.is_empty() {
        println!("  Recent commits ({}):", commits.len());
        for line in &commits {
            println!("    {line}");
        }
    }
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
    let mut scored: Vec<(i64, i64, String, String)> = Vec::new();
    for (path, state) in dirty {
        let abs = repo_root.join(path);
        let facts = if abs.exists() {
            file_facts::get(&abs, repo_root, None)
        } else {
            None
        };
        let mut callers = 0i64;
        if let Some(g) = &graph {
            if let Some(module_id) = g.file_to_module_id.get(path) {
                callers = architecture::dependents_of(g, module_id).len() as i64;
            }
        }
        let ccn = facts.as_ref().map(|f| f.cyclomatic_complexity_total).unwrap_or(0);
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

fn emit_rules(_repo_root: &Path, tracked: &[String]) {
    let claude_md = collect_claude_md(tracked);
    let rules_files = collect_rules_dir(tracked);

    println!("## Rules");
    if !claude_md.is_empty() {
        println!("  Claude.md files ({}):", claude_md.len());
        for rel in &claude_md {
            println!("    {rel}");
        }
    }
    if !rules_files.is_empty() {
        println!("  Project rules ({}):", rules_files.len());
        for rel in &rules_files {
            println!("    {rel}");
        }
    }
    if claude_md.is_empty() && rules_files.is_empty() {
        println!("  (no Claude.md or .claude/rules/ found)");
    }
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

fn emit_spine(repo_root: &Path) {
    println!("## Spine");
    let graph = match architecture::load_cached(repo_root) {
        Some(g) if !g.edges.is_empty() => g,
        _ => {
            println!(
                "  (architecture graph empty — run `trace cache build` if you expect data)"
            );
            return;
        }
    };

    // Counter(edge.target).most_common(): count desc, ties keep first-seen.
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for edge in &graph.edges {
        if !counts.contains_key(&edge.target) {
            order.push(edge.target.clone());
        }
        *counts.entry(edge.target.clone()).or_insert(0) += 1;
    }
    let mut ranked: Vec<(usize, String, i64)> = order
        .iter()
        .enumerate()
        .map(|(i, t)| (i, t.clone(), counts[t]))
        .collect();
    ranked.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    let ranked: Vec<(String, i64)> = ranked
        .into_iter()
        .filter(|(_, id, _)| !id.starts_with("module::external::"))
        .map(|(_, id, c)| (id, c))
        .take(PRIMER_SPINE_LIMIT)
        .collect();

    if ranked.is_empty() {
        println!("  (no internal nodes in the architecture graph)");
        return;
    }

    println!("  Top {} most-depended-on nodes:", ranked.len());
    println!("    {:<3} {:>6}  {:<10} symbol @ source", "#", "direct", "kind");
    for (rank, (node_id, direct)) in ranked.iter().enumerate() {
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
        println!(
            "    {:<3} {:>6}  {:<10} {} @ {}",
            rank + 1,
            direct,
            node.kind,
            node.label,
            location,
        );
    }
}
