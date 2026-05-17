//! Bulk git activity: single-pass log parser + per-file lifecycle facts.
//!
//! Two main subprocesses for the whole repo instead of N*2 per-file:
//!   1. `git log -M --diff-merges=first-parent --name-status
//!       --pretty=format:COMMIT|%ad|%an|%s --date=short`
//!   2. `git log --since=<30d> --diff-merges=first-parent --name-only
//!       --pretty=format:`
//! Plus `git status --porcelain=v1 -z`, `git rev-parse HEAD`, and one
//! `git ls-tree -r --name-only <ref>` per deploy branch.

use crate::cache;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct GitActivity {
    pub last_modified: Option<String>,
    pub last_author: Option<String>,
    pub commits_30d: i64,
    pub first_seen: Option<String>,
    pub commit_count: i64,
    pub rename_from: Option<String>,
    pub working_state: Option<String>,
    pub present_in: Vec<String>,
    pub last_subject: Option<String>,
    pub top_author: Option<String>,
    pub co_changed: Vec<(String, i64)>,
}

impl GitActivity {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Deploy branches checked for file presence, in display order.
/// (label, ref). Refs absent from the repo are silently skipped.
const DEPLOY_BRANCHES: &[(&str, &str)] = &[
    ("prod", "origin/production"),
    ("staging", "origin/staging"),
    ("main", "origin/main"),
    ("main", "origin/master"),
];

fn git_str(repo_root: &Path, args: &[&str], timeout_ok: bool) -> Option<String> {
    let _ = timeout_ok;
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn head_sha(repo_root: &Path) -> Option<String> {
    git_str(repo_root, &["rev-parse", "HEAD"], true).map(|s| s.trim().to_string())
}

/// Full relpath -> GitActivity map for the repo (single git log pass).
pub fn bulk(repo_root: &Path) -> HashMap<String, GitActivity> {
    let history = walk_history(repo_root);
    let counts_30d = commits_30d_per_file(repo_root);
    let working = working_tree_state(repo_root);
    let presence = presence_by_path(repo_root);

    let mut out: HashMap<String, GitActivity> = HashMap::new();
    for (path, info) in &history {
        out.insert(
            path.clone(),
            GitActivity {
                last_modified: info.last_modified.clone(),
                last_author: info.last_author.clone(),
                commits_30d: *counts_30d.get(path).unwrap_or(&0),
                first_seen: info.first_seen.clone(),
                commit_count: info.commit_count,
                rename_from: info.rename_from.clone(),
                working_state: working.get(path).cloned(),
                present_in: presence.get(path).cloned().unwrap_or_default(),
                last_subject: info.last_subject.clone(),
                top_author: info.top_author.clone(),
                co_changed: info.co_changed.clone(),
            },
        );
    }
    for (path, state) in &working {
        if out.contains_key(path) {
            continue;
        }
        out.insert(
            path.clone(),
            GitActivity {
                working_state: Some(state.clone()),
                present_in: presence.get(path).cloned().unwrap_or_default(),
                ..GitActivity::empty()
            },
        );
    }
    out
}

/// Disk-cached bulk map. Historical fields are cached under
/// `git_activity__{head}` in the file namespace; working-tree state is
/// always recomputed fresh and overlaid.
pub fn bulk_cached(repo_root: &Path) -> HashMap<String, GitActivity> {
    let head = match head_sha(repo_root) {
        Some(h) => h,
        None => return bulk(repo_root),
    };
    let key = format!("git_activity__{head}");
    if let Some(Value::Object(map)) = cache::load(cache::NAMESPACE_FILE, &key, repo_root) {
        let mut history: HashMap<String, GitActivity> = HashMap::new();
        for (path, fields) in &map {
            history.insert(path.clone(), git_activity_from_json(fields));
        }
        let working = working_tree_state(repo_root);
        if working.is_empty() {
            return history;
        }
        let mut out = history;
        for (path, state) in &working {
            let entry = out.entry(path.clone()).or_insert_with(GitActivity::empty);
            entry.working_state = Some(state.clone());
        }
        return out;
    }
    let history = bulk(repo_root);
    // Persist without working_state — it is recomputed live, never cached.
    let mut payload = serde_json::Map::new();
    for (path, act) in &history {
        payload.insert(path.clone(), git_activity_to_json(act, false));
    }
    let _ = cache::save(
        cache::NAMESPACE_FILE,
        &key,
        &Value::Object(payload),
        repo_root,
    );
    history
}

fn git_activity_to_json(a: &GitActivity, include_working: bool) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("last_modified".into(), opt_str(&a.last_modified));
    m.insert("last_author".into(), opt_str(&a.last_author));
    m.insert("commits_30d".into(), json!(a.commits_30d));
    m.insert("first_seen".into(), opt_str(&a.first_seen));
    m.insert("commit_count".into(), json!(a.commit_count));
    m.insert("rename_from".into(), opt_str(&a.rename_from));
    if include_working {
        m.insert("working_state".into(), opt_str(&a.working_state));
    }
    m.insert(
        "present_in".into(),
        json!(a.present_in.clone()),
    );
    m.insert("last_subject".into(), opt_str(&a.last_subject));
    m.insert("top_author".into(), opt_str(&a.top_author));
    m.insert(
        "co_changed".into(),
        Value::Array(
            a.co_changed
                .iter()
                .map(|(p, c)| json!([p, c]))
                .collect(),
        ),
    );
    Value::Object(m)
}

fn git_activity_from_json(v: &Value) -> GitActivity {
    let g = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
    let n = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    GitActivity {
        last_modified: g("last_modified"),
        last_author: g("last_author"),
        commits_30d: n("commits_30d"),
        first_seen: g("first_seen"),
        commit_count: n("commit_count"),
        rename_from: g("rename_from"),
        working_state: None,
        present_in: v
            .get("present_in")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        last_subject: g("last_subject"),
        top_author: g("top_author"),
        co_changed: v
            .get("co_changed")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|pair| {
                        let arr = pair.as_array()?;
                        Some((
                            arr.first()?.as_str()?.to_string(),
                            arr.get(1)?.as_i64()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn opt_str(o: &Option<String>) -> Value {
    match o {
        Some(s) => json!(s),
        None => Value::Null,
    }
}

#[derive(Default, Clone)]
struct HistoryEntry {
    last_modified: Option<String>,
    last_author: Option<String>,
    last_subject: Option<String>,
    first_seen: Option<String>,
    commit_count: i64,
    rename_from: Option<String>,
    top_author: Option<String>,
    co_changed: Vec<(String, i64)>,
}

/// Single git log pass with rename detection, producing
/// last/first/count/rename/top_author/co_changed per file.
fn walk_history(repo_root: &Path) -> BTreeMap<String, HistoryEntry> {
    let stdout = match git_str(
        repo_root,
        &[
            "log",
            "-M",
            "--diff-merges=first-parent",
            "--name-status",
            "--pretty=format:COMMIT|%ad|%an|%s",
            "--date=short",
        ],
        true,
    ) {
        Some(s) => s,
        None => return BTreeMap::new(),
    };

    let mut out: BTreeMap<String, HistoryEntry> = BTreeMap::new();
    let mut aliases: HashMap<String, String> = HashMap::new();
    let mut authors_by_path: HashMap<String, OrderedCounter> = HashMap::new();
    let mut co_by_path: HashMap<String, OrderedCounter> = HashMap::new();
    let mut current_date: Option<String> = None;
    let mut current_author: Option<String> = None;
    let mut current_subject: Option<String> = None;
    let mut current_paths: Vec<String> = Vec::new();

    let flush_co = |co_by_path: &mut HashMap<String, OrderedCounter>,
                    paths: &[String]| {
        if paths.len() <= 1 {
            return;
        }
        for a in paths {
            let counter = co_by_path.entry(a.clone()).or_default();
            for b in paths {
                if a != b {
                    counter.add(b);
                }
            }
        }
    };

    let ensure = |out: &mut BTreeMap<String, HistoryEntry>,
                  path: &str,
                  date: &str,
                  author: &Option<String>,
                  subject: &Option<String>| {
        out.entry(path.to_string()).or_insert_with(|| HistoryEntry {
            last_modified: Some(date.to_string()),
            last_author: Some(author.clone().unwrap_or_default()),
            last_subject: subject.clone(),
            first_seen: Some(date.to_string()),
            commit_count: 0,
            rename_from: None,
            top_author: None,
            co_changed: Vec::new(),
        });
    };

    for line in stdout.split('\n') {
        if let Some(rest) = line.strip_prefix("COMMIT|") {
            flush_co(&mut co_by_path, &current_paths);
            current_paths.clear();
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                current_date = Some(parts[0].to_string());
                current_author = Some(parts[1].to_string());
                current_subject = Some(parts[2].to_string());
            } else if parts.len() == 2 {
                current_date = Some(parts[0].to_string());
                current_author = Some(parts[1].to_string());
                current_subject = None;
            }
            continue;
        }
        if line.trim().is_empty() || current_date.is_none() {
            continue;
        }
        let date = current_date.clone().unwrap();
        let tokens: Vec<&str> = line.split('\t').collect();
        if tokens.len() < 2 {
            continue;
        }
        let status = tokens[0];
        if status.starts_with('R') || status.starts_with('C') {
            if tokens.len() < 3 {
                continue;
            }
            let old_path = tokens[1].to_string();
            let path = tokens[2].to_string();
            ensure(&mut out, &path, &date, &current_author, &current_subject);
            let e = out.get_mut(&path).unwrap();
            e.commit_count += 1;
            if e.rename_from.is_none() {
                e.rename_from = Some(old_path.clone());
            }
            e.first_seen = Some(date.clone());
            if let Some(a) = &current_author {
                authors_by_path.entry(path.clone()).or_default().add(a);
            }
            if !current_paths.contains(&path) {
                current_paths.push(path.clone());
            }
            aliases.entry(old_path).or_insert(path);
        } else {
            let path = tokens[1].to_string();
            let target = aliases.get(&path).cloned().unwrap_or(path);
            ensure(&mut out, &target, &date, &current_author, &current_subject);
            let e = out.get_mut(&target).unwrap();
            e.commit_count += 1;
            e.first_seen = Some(date.clone());
            if let Some(a) = &current_author {
                authors_by_path.entry(target.clone()).or_default().add(a);
            }
            if !current_paths.contains(&target) {
                current_paths.push(target);
            }
        }
    }
    flush_co(&mut co_by_path, &current_paths);

    for (path, entry) in out.iter_mut() {
        if let Some(ac) = authors_by_path.get(path) {
            entry.top_author = ac.most_common_n(1).into_iter().next().map(|(k, _)| k);
        }
        if let Some(cc) = co_by_path.get(path) {
            entry.co_changed = cc.most_common_n(5);
        }
    }
    out
}

/// Insertion-ordered counter: `most_common` breaks count ties by
/// first-insertion order.
#[derive(Default, Clone)]
struct OrderedCounter {
    counts: HashMap<String, i64>,
    order: Vec<String>,
}

impl OrderedCounter {
    fn add(&mut self, key: &str) {
        if !self.counts.contains_key(key) {
            self.order.push(key.to_string());
        }
        *self.counts.entry(key.to_string()).or_insert(0) += 1;
    }

    /// Top `n` by count descending; ties keep first-insertion order via a
    /// stable sort over the insertion-ordered keys.
    fn most_common_n(&self, n: usize) -> Vec<(String, i64)> {
        let mut v: Vec<(usize, String, i64)> = self
            .order
            .iter()
            .enumerate()
            .map(|(idx, k)| (idx, k.clone(), self.counts[k]))
            .collect();
        v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        v.into_iter().take(n).map(|(_, k, c)| (k, c)).collect()
    }
}

/// Commit count per file over the last 30 days.
fn commits_30d_per_file(repo_root: &Path) -> HashMap<String, i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let since_secs = now - 30 * 86400;
    let since = unix_to_ymd(since_secs);
    let stdout = match git_str(
        repo_root,
        &[
            "log",
            &format!("--since={since}"),
            "--diff-merges=first-parent",
            "--name-only",
            "--pretty=format:",
        ],
        true,
    ) {
        Some(s) => s,
        None => return HashMap::new(),
    };
    let mut counts: HashMap<String, i64> = HashMap::new();
    for line in stdout.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        *counts.entry(line.to_string()).or_insert(0) += 1;
    }
    counts
}

/// UTC date YYYY-MM-DD from a unix timestamp (days-since-epoch civil calc).
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
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Per-file working-tree state via `git status --porcelain=v1 -z`.
pub fn working_tree_state(repo_root: &Path) -> HashMap<String, String> {
    let out = match Command::new("git")
        .args(["status", "--porcelain=v1", "-z"])
        .current_dir(repo_root)
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return HashMap::new(),
    };
    let mut result: HashMap<String, String> = HashMap::new();
    let parts: Vec<&[u8]> = out.split(|&b| b == 0).collect();
    let mut i = 0;
    while i < parts.len() {
        let chunk = String::from_utf8_lossy(parts[i]);
        if chunk.len() < 3 {
            i += 1;
            continue;
        }
        let xy: Vec<char> = chunk.chars().take(2).collect();
        let path: String = chunk.chars().skip(3).collect();
        let x = xy[0];
        let y = xy[1];
        if x == 'R' || x == 'C' || y == 'R' || y == 'C' {
            if i + 1 < parts.len() {
                i += 2;
            } else {
                i += 1;
            }
            result.insert(path, "renamed".to_string());
            continue;
        }
        let state = if x == '?' || y == '?' {
            "untracked"
        } else if x == 'A' || y == 'A' {
            "added"
        } else if x == 'D' || y == 'D' {
            "deleted"
        } else {
            "modified"
        };
        result.insert(path, state.to_string());
        i += 1;
    }
    result
}

/// For each path, which deploy branches contain it.
fn presence_by_path(repo_root: &Path) -> HashMap<String, Vec<String>> {
    let mut labels_for: HashMap<String, Vec<String>> = HashMap::new();
    for (label, r#ref) in DEPLOY_BRANCHES {
        let stdout =
            match git_str(repo_root, &["ls-tree", "-r", "--name-only", r#ref], true) {
                Some(s) => s,
                None => continue,
            };
        for path in stdout.split('\n') {
            if path.is_empty() {
                continue;
            }
            let v = labels_for.entry(path.to_string()).or_default();
            if !v.contains(&label.to_string()) {
                v.push(label.to_string());
            }
        }
    }
    labels_for
        .into_iter()
        .map(|(p, mut labels)| {
            labels.sort();
            labels.dedup();
            (p, labels)
        })
        .collect()
}
