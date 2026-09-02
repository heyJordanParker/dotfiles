//! Bulk git activity: single-pass log parser + per-file lifecycle facts.
//!
//! One `git log` subprocess for the whole repo's lifecycle facts instead of
//! N*2 per-file:
//!   1. `git log -n <cap> -M --diff-merges=first-parent --name-status
//!       --pretty=format:COMMIT|%H|%ad|%an|%s --date=short`
//! The 30-day commit counts are derived from the dated commits this same
//! walk parses — no second `git log`. The walk is bounded to `HISTORY_CAP`
//! recent commits; on a history deeper than the cap, `commit_count` becomes
//! a floor (commits within the cap, not the full-history total) and
//! `commit_count_is_floor` is set so consumers can see the count is partial.
//! Plus `git status --porcelain=v1 -z`, `git rev-parse HEAD`, and one
//! `git ls-tree -r --name-only <ref>` per deploy branch (the latter
//! disk-cached, keyed by the branch tip commit ids).

use crate::{cache, memo};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// The stored map is `path -> GitActivity`, so the entry deserializes
/// straight into this type. Reading it into a `serde_json::Value` first cost
/// more memory than the map itself on a repository of any size, and
/// `working_state` is deliberately absent from the entry — it is recomputed
/// live on every read, so it defaults like every other missing field.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
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
    /// Never written to the entry: it is a fact about the working tree, which
    /// moves independently of HEAD, so it is recomputed and overlaid on every
    /// read.
    #[serde(skip_serializing)]
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

/// Upper bound on the files one commit contributes to co-change. The pairing
/// is quadratic in the commit's file count, so an unbounded commit is an
/// unbounded allocation: a shallow clone's graft commit reports the whole tree
/// (29,122 files in `references/next.js`), which is 848 million retained pairs
/// at 270 bytes each. Above the cap a commit adds nothing, which loses no
/// signal — a commit touching hundreds of files raises every pair by one and
/// so ranks nothing.
const CO_CHANGE_COMMIT_CAP: usize = 100;

/// Upper bound on the pairs the whole walk retains. `CO_CHANGE_COMMIT_CAP`
/// bounds one commit; this bounds their sum, so a deep history of medium
/// commits cannot accumulate without limit either. The walk runs newest-first,
/// so the pairs kept are the recent couplings, and git log order is stable for
/// a fixed HEAD, so a truncated walk truncates identically on every run.
const CO_CHANGE_PAIR_CAP: usize = 1_000_000;

/// Deploy branches checked for file presence, in display order.
/// (label, ref). Refs absent from the repo are silently skipped.
const DEPLOY_BRANCHES: &[(&str, &str)] = &[
    ("prod", "origin/production"),
    ("staging", "origin/staging"),
    ("main", "origin/main"),
    ("main", "origin/master"),
];

/// Run git in `repo_root` and return stdout with the trailing newline
/// stripped, or `None` when git fails. Git terminates every command's output
/// with a newline that is never part of the value, so `rev-parse HEAD` yields
/// `"<sha>\n"`; stripping it here is what stops each caller having to
/// remember. Only the tail is cut — a leading space is data in some git
/// formats (the `XY` field of `status --porcelain`), and `porcelain_uncached`
/// reads those raw bytes itself rather than through this.
pub fn git_str(repo_root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn head_sha(repo_root: &Path) -> Option<String> {
    git_str(repo_root, &["rev-parse", "HEAD"])
}

/// The commits a shallow clone grafted its history onto, read from
/// `<git-dir>/shallow`. Git has no parent to diff a graft against, so
/// `--name-status` reports the entire working tree as added: in
/// `references/elementor` every one of 7,828 files claims the clone date as
/// its first commit and HEAD's author as its owner. The set is read rather
/// than derived from the oldest commit, because a clone can hold several —
/// `references/codex` lists nine, and its oldest reachable commit is the
/// third of them. Empty for a full clone.
fn grafts(repo_root: &Path) -> HashSet<String> {
    let shallow = git_str(repo_root, &["rev-parse", "--is-shallow-repository"]);
    if shallow.as_deref() != Some("true") {
        return HashSet::new();
    }
    let git_dir = match git_str(repo_root, &["rev-parse", "--git-dir"]) {
        Some(d) => PathBuf::from(d),
        None => return HashSet::new(),
    };
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        repo_root.join(git_dir)
    };
    match std::fs::read_to_string(git_dir.join("shallow")) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
        Err(_) => HashSet::new(),
    }
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
    // Deploy-branch presence is a fact about the current refs, not about
    // history, so it survives a repo whose history the walk dropped. In a
    // shallow clone every file's only commit is a graft, which leaves no
    // history entry and previously no entry at all — silently turning a file
    // that is on origin/main into `presence: local-only`.
    for (path, refs) in &presence {
        if out.contains_key(path) {
            continue;
        }
        out.insert(
            path.clone(),
            GitActivity {
                present_in: refs.clone(),
                ..GitActivity::empty()
            },
        );
    }
    out
}

/// Process-wide memo of the disk-cached bulk map, keyed by repo root.
///
/// `file_facts::get` joins git facts onto every single-file resolve, and
/// `glob`, `diff`, `blame`, and `structure` call it once per file, so without
/// the memo a run would re-read and re-parse the whole map N times. The lock
/// is held across the compute so concurrent callers serialize onto one build
/// rather than racing the same git subprocesses.
pub fn bulk_cached(repo_root: &Path) -> Arc<HashMap<String, GitActivity>> {
    static MEMO: memo::Memo<HashMap<String, GitActivity>> = OnceLock::new();
    memo::get_or_build(&MEMO, repo_root, || bulk_cached_uncached(repo_root))
}

/// Disk-cached bulk map. Historical fields are cached under
/// `git_activity__{head}__{30d cutoff}` in the file namespace; working-tree
/// state is always recomputed fresh and overlaid.
///
/// The cutoff date is in the key because `commits_30d` is computed against
/// today's date: keyed by HEAD alone, a branch that sits idle keeps serving
/// the velocity it had on the day the entry was written.
fn bulk_cached_uncached(repo_root: &Path) -> HashMap<String, GitActivity> {
    let head = match head_sha(repo_root) {
        Some(h) => h,
        None => return bulk(repo_root),
    };
    let key = format!("git_activity__{head}__{}", cutoff_30d());
    if let Some(history) = cache::load_bytes(cache::NAMESPACE_FILE, &key, repo_root)
        .and_then(|b| serde_json::from_slice::<HashMap<String, GitActivity>>(&b).ok())
    {
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
    // `working_state` is `skip_serializing`, so the entry holds only the
    // HEAD-keyed facts the key actually determines.
    if let Ok(payload) = serde_json::to_value(&history) {
        let _ = cache::save(cache::NAMESPACE_FILE, &key, &payload, repo_root);
    }
    cache::evict_prefixed(cache::NAMESPACE_FILE, "git_activity__", &key, repo_root);
    history
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
    // `%H` leads the record so a graft commit can be recognized and dropped.
    let stdout = match git_str(
        repo_root,
        &[
            "log",
            &cap_arg,
            "-M",
            "--diff-merges=first-parent",
            "--name-status",
            "--pretty=format:COMMIT|%H|%ad|%an|%s",
            "--date=short",
        ],
    ) {
        Some(s) => s,
        None => return History::default(),
    };
    let graft_shas = grafts(repo_root);
    let cutoff_30d = cutoff_30d();

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
    // The commit's true file count. `current_paths` stops growing at the cap,
    // because its `contains` scan is linear and an unbounded list makes the
    // parse itself quadratic in the commit's size; this counter keeps going so
    // `flush_co` can tell a 100-file commit from a 29,122-file one.
    let mut current_files = 0usize;
    // Distinct pairs retained so far, weighed against `CO_CHANGE_PAIR_CAP`.
    let mut pairs_held = 0usize;
    // Set for a graft commit, whose diff is the whole working tree rather than
    // a change: it contributes no count, no author, and no co-change.
    let mut skip_commit = false;

    let flush_co = |co_by_path: &mut HashMap<String, OrderedCounter>,
                    pairs_held: &mut usize,
                    paths: &[String],
                    files: usize| {
        // A commit is all-or-nothing at both caps, so what a file carries is
        // whole commits and the ceiling truncates the same way on every run.
        if paths.len() <= 1 || files > CO_CHANGE_COMMIT_CAP {
            return;
        }
        if *pairs_held >= CO_CHANGE_PAIR_CAP {
            return;
        }
        for a in paths {
            let counter = co_by_path.entry(a.clone()).or_default();
            for b in paths {
                if a != b && counter.add(b) {
                    *pairs_held += 1;
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
            flush_co(&mut co_by_path, &mut pairs_held, &current_paths, current_files);
            current_paths.clear();
            current_files = 0;
            commit_seen += 1;
            let parts: Vec<&str> = rest.splitn(4, '|').collect();
            skip_commit = graft_shas.contains(parts.first().copied().unwrap_or(""));
            if parts.len() == 4 {
                current_date = Some(parts[1].to_string());
                current_author = Some(parts[2].to_string());
                current_subject = Some(parts[3].to_string());
            } else if parts.len() == 3 {
                current_date = Some(parts[1].to_string());
                current_author = Some(parts[2].to_string());
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
        if line.trim().is_empty() || current_date.is_none() || skip_commit {
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
            current_files += 1;
            if current_files <= CO_CHANGE_COMMIT_CAP && !current_paths.contains(&path) {
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
            current_files += 1;
            if current_files <= CO_CHANGE_COMMIT_CAP && !current_paths.contains(&target) {
                current_paths.push(target);
            }
        }
    }
    flush_co(&mut co_by_path, &mut pairs_held, &current_paths, current_files);

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
    /// True when `key` was not already counted. Each new key costs two owned
    /// `String`s here, so the co-change walk counts them to hold itself under
    /// `CO_CHANGE_PAIR_CAP`.
    fn add(&mut self, key: &str) -> bool {
        let fresh = !self.counts.contains_key(key);
        if fresh {
            self.order.push(key.to_string());
        }
        *self.counts.entry(key.to_string()).or_insert(0) += 1;
        fresh
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

/// The 30-day cutoff as a YYYY-MM-DD string, compared lexically against the
/// `--date=short` author dates the walk parses. Same author-date basis git's
/// own `--since` uses, so the derived counts match a `--since=<30d>` pass
/// exactly. Also part of the bulk cache key, because every `commits_30d` in
/// an entry is relative to it.
fn cutoff_30d() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    unix_to_ymd(now - 30 * 86400)
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
    porcelain(repo_root)
        .iter()
        .map(|(path, (state, _))| (path.clone(), state.clone()))
        .collect()
}

/// Where each changed file's change currently lives: `staged`, `unstaged`, or
/// `partly staged`. Untracked files have nothing staged and are absent.
///
/// `git status` reports two letters per file — the index side and the
/// worktree side — and which of them is set is the whole answer. An agent
/// that reads "modified" and commits finds it committed nothing, or commits
/// half of what it meant; the word is what prevents that.
pub fn staging_state(repo_root: &Path) -> HashMap<String, String> {
    porcelain(repo_root)
        .iter()
        .filter_map(|(path, (_, staging))| {
            staging.as_ref().map(|s| (path.clone(), s.clone()))
        })
        .collect()
}

/// Per-file `(working-tree state, staging state)` via
/// `git status --porcelain=v1 -z`.
///
/// Process-wide memo, keyed by repo root: the working tree does not change
/// within a single CLI invocation, so the `git status` subprocess runs at
/// most once per root. The lock is held across the compute so concurrent
/// callers (parallel primer sections, file-facts working-state overlay)
/// serialize onto one status run rather than racing `index.lock`.
type Porcelain = HashMap<String, (String, Option<String>)>;

fn porcelain(repo_root: &Path) -> Arc<Porcelain> {
    static MEMO: memo::Memo<Porcelain> = OnceLock::new();
    memo::get_or_build(&MEMO, repo_root, || porcelain_uncached(repo_root))
}

/// The staging word for one porcelain `XY` pair, or None when nothing is
/// staged to describe (an untracked file).
fn staging_word(x: char, y: char) -> Option<String> {
    if x == '?' || y == '?' {
        return None;
    }
    let indexed = x != ' ';
    let in_worktree = y != ' ';
    Some(
        match (indexed, in_worktree) {
            (true, true) => "partly staged",
            (true, false) => "staged",
            _ => "unstaged",
        }
        .to_string(),
    )
}

fn porcelain_uncached(repo_root: &Path) -> Porcelain {
    let out = match Command::new("git")
        .args(["status", "--porcelain=v1", "-z"])
        .current_dir(repo_root)
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return HashMap::new(),
    };
    let mut result: Porcelain = HashMap::new();
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
            result.insert(path, ("renamed".to_string(), staging_word(x, y)));
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
        result.insert(path, (state.to_string(), staging_word(x, y)));
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
            Some((*label, *r#ref, git_str(repo_root, &["rev-parse", r#ref])?))
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

    if let Some(cached) = cache::load_bytes(cache::NAMESPACE_FILE, &key, repo_root)
        .and_then(|b| serde_json::from_slice::<HashMap<String, Vec<String>>>(&b).ok())
    {
        return cached;
    }

    let computed = compute_presence(repo_root, &present);
    let payload = json!(computed);
    let _ = cache::save(
        cache::NAMESPACE_FILE,
        &key,
        &payload,
        repo_root,
    );
    cache::evict_prefixed(cache::NAMESPACE_FILE, "git_presence__", &key, repo_root);
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
            match git_str(repo_root, &["ls-tree", "-r", "--name-only", r#ref]) {
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
