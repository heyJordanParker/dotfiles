//! Bulk git activity: single-pass log parser + per-file lifecycle facts.
//!
//! One `git log` subprocess for the whole repo's lifecycle facts instead of
//! N*2 per-file:
//!   1. `git log -n <cap> -M --diff-merges=first-parent --name-status
//!       --pretty=format:COMMIT|%ad|%an|%s --date=short`
//! The 30-day commit counts are derived from the dated commits this same
//! walk parses — no second `git log`. The walk is bounded to `HISTORY_CAP`
//! recent commits; on a history deeper than the cap, `commit_count` becomes
//! a floor (commits within the cap, not the full-history total) and
//! `commit_count_is_floor` is set so consumers can see the count is partial.
//! Plus `git status --porcelain=v1 -z`, `git rev-parse HEAD`, and one
//! `git ls-tree -r --name-only <ref>` per deploy branch (the latter
//! disk-cached, keyed by the branch tip commit ids).

use crate::cache;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct GitActivity {
    pub last_modified: Option<String>,
    pub last_author: Option<String>,
    pub commits_30d: i64,
    pub first_seen: Option<String>,
    pub commit_count: i64,
    /// True when the history walk hit `HISTORY_CAP` and stopped before the
    /// repo's first commit: `commit_count` is then a floor (commits seen
    /// within the cap), not the exact full-history total. `first_seen` is
    /// likewise the oldest date within the cap, not the true first commit.
    pub commit_count_is_floor: bool,
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

/// Upper bound on commits the lifecycle walk parses. The walk produces
/// last/first/count/rename/top_author/co_changed; none of these need the
/// exact full-history total, so a deep history is bounded here to keep the
/// single git-log pass cheap. A repo with more commits than this gets a
/// `commit_count` floor (see `GitActivity::commit_count_is_floor`). The cap
/// is high enough that ordinary repos walk their whole history unchanged.
const HISTORY_CAP: usize = 4000;

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
    let working = working_tree_state(repo_root);
    let presence = presence_by_path(repo_root);

    let mut out: HashMap<String, GitActivity> = HashMap::new();
    for (path, info) in &history.entries {
        out.insert(
            path.clone(),
            GitActivity {
                last_modified: info.last_modified.clone(),
                last_author: info.last_author.clone(),
                commits_30d: info.commits_30d,
                first_seen: info.first_seen.clone(),
                commit_count: info.commit_count,
                commit_count_is_floor: history.truncated,
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
    m.insert("commit_count_is_floor".into(), json!(a.commit_count_is_floor));
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
        commit_count_is_floor: v
            .get("commit_count_is_floor")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
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
    commits_30d: i64,
    rename_from: Option<String>,
    top_author: Option<String>,
    co_changed: Vec<(String, i64)>,
}

/// Result of the bounded history walk: the per-file entries plus whether
/// the walk stopped at `HISTORY_CAP` before reaching the first commit (in
/// which case every entry's `commit_count` / `first_seen` is a floor).
#[derive(Default)]
struct History {
    entries: BTreeMap<String, HistoryEntry>,
    truncated: bool,
}

/// Single git log pass with rename detection, producing
/// last/first/count/commits_30d/rename/top_author/co_changed per file. The
/// 30-day counts are derived here from each commit's date — no second log
/// invocation. Bounded to `HISTORY_CAP` commits; `truncated` reports
/// whether the cap was hit.
fn walk_history(repo_root: &Path) -> History {
    let cap_arg = format!("-n{HISTORY_CAP}");
    let stdout = match git_str(
        repo_root,
        &[
            "log",
            &cap_arg,
            "-M",
            "--diff-merges=first-parent",
            "--name-status",
            "--pretty=format:COMMIT|%ad|%an|%s",
            "--date=short",
        ],
        true,
    ) {
        Some(s) => s,
        None => return History::default(),
    };

    // 30-day cutoff as a YYYY-MM-DD string, compared lexically against the
    // `--date=short` author dates the walk already parses. Same author-date
    // basis git's own `--since` uses, so the derived counts match the
    // previous second-pass `--since=<30d>` exactly.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff_30d = unix_to_ymd(now - 30 * 86400);

    // Count distinct COMMIT records to detect whether the cap truncated the
    // walk: a full history emits fewer than `HISTORY_CAP` commits.
    let mut commit_seen = 0usize;

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
            commits_30d: 0,
            rename_from: None,
            top_author: None,
            co_changed: Vec::new(),
        });
    };

    let mut current_recent = false;
    for line in stdout.split('\n') {
        if let Some(rest) = line.strip_prefix("COMMIT|") {
            flush_co(&mut co_by_path, &current_paths);
            current_paths.clear();
            commit_seen += 1;
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
            // `--date=short` dates are zero-padded YYYY-MM-DD, so a lexical
            // `>=` against the cutoff is a correct chronological compare.
            current_recent = current_date
                .as_deref()
                .map(|d| d >= cutoff_30d.as_str())
                .unwrap_or(false);
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
            if current_recent {
                e.commits_30d += 1;
            }
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
            if current_recent {
                e.commits_30d += 1;
            }
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
    History {
        entries: out,
        truncated: commit_seen >= HISTORY_CAP,
    }
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
///
/// Process-wide memo, keyed by repo root: the working tree does not change
/// within a single CLI invocation, so the `git status` subprocess runs at
/// most once per root. The lock is held across the compute so concurrent
/// callers (parallel primer sections, file-facts working-state overlay)
/// serialize onto one status run rather than racing `index.lock`.
pub fn working_tree_state(repo_root: &Path) -> HashMap<String, String> {
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, HashMap<String, String>>>> =
        OnceLock::new();
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = memo.lock().unwrap();
    if let Some(cached) = guard.get(repo_root) {
        return cached.clone();
    }
    let computed = working_tree_state_uncached(repo_root);
    guard.insert(repo_root.to_path_buf(), computed.clone());
    computed
}

fn working_tree_state_uncached(repo_root: &Path) -> HashMap<String, String> {
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
///
/// The `ls-tree -r` walks change only when a deploy branch tip moves, which
/// is far rarer than HEAD moving. So the result is disk-cached keyed by the
/// tip commit ids of the present deploy branches (absent branches drop out
/// of both the key and the result, exactly as the live computation does). A
/// local commit that doesn't touch a deploy branch reuses this cache; a
/// deploy-branch fast-forward rotates the key and recomputes.
fn presence_by_path(repo_root: &Path) -> HashMap<String, Vec<String>> {
    let present: Vec<(&str, &str, String)> = DEPLOY_BRANCHES
        .iter()
        .filter_map(|(label, r#ref)| {
            let tip = git_str(repo_root, &["rev-parse", r#ref], true)?;
            Some((*label, *r#ref, tip.trim().to_string()))
        })
        .collect();

    if present.is_empty() {
        return HashMap::new();
    }

    let key = {
        let mut hasher = Sha256::new();
        for (label, r#ref, tip) in &present {
            hasher.update(label.as_bytes());
            hasher.update(b"\0");
            hasher.update(r#ref.as_bytes());
            hasher.update(b"\0");
            hasher.update(tip.as_bytes());
            hasher.update(b"\n");
        }
        format!("git_presence__{}", hex::encode(hasher.finalize()))
    };

    if let Some(Value::Object(map)) = cache::load(cache::NAMESPACE_FILE, &key, repo_root) {
        return map
            .into_iter()
            .map(|(path, labels)| {
                let labels = labels
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                (path, labels)
            })
            .collect();
    }

    let computed = compute_presence(repo_root, &present);

    let mut payload = serde_json::Map::new();
    for (path, labels) in &computed {
        payload.insert(path.clone(), json!(labels));
    }
    let _ = cache::save(
        cache::NAMESPACE_FILE,
        &key,
        &Value::Object(payload),
        repo_root,
    );
    computed
}

/// Run the `ls-tree` walks over the resolved present deploy branches.
fn compute_presence(
    repo_root: &Path,
    present: &[(&str, &str, String)],
) -> HashMap<String, Vec<String>> {
    let mut labels_for: HashMap<String, Vec<String>> = HashMap::new();
    for (label, r#ref, _tip) in present {
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
