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
//! Plus live working state, `git rev-parse HEAD`, one
//! `git for-each-ref` observation of every deploy tip, and one `git ls-tree
//! -r --name-only <ref>` per present deploy branch (the latter disk-cached,
//! keyed by the branch tip commit ids).

use crate::{cache, memo};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
#[derive(Debug, Clone, Default, PartialEq)]
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
    pub working_state: Option<String>,
    pub present_in: Vec<&'static str>,
    pub last_subject: Option<String>,
    pub top_author: Option<String>,
    pub co_changed: Vec<(Arc<str>, i64)>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct StoredGitActivity {
    last_modified: Option<String>,
    last_author: Option<String>,
    commits_30d: i64,
    first_seen: Option<String>,
    commit_count: i64,
    commit_count_is_floor: bool,
    rename_from: Option<String>,
    last_subject: Option<String>,
    top_author: Option<String>,
    present_in: Vec<String>,
    co_changed: Vec<(String, i64)>,
}

impl From<StoredGitActivity> for GitActivity {
    fn from(stored: StoredGitActivity) -> Self {
        Self {
            last_modified: stored.last_modified,
            last_author: stored.last_author,
            commits_30d: stored.commits_30d,
            first_seen: stored.first_seen,
            commit_count: stored.commit_count,
            commit_count_is_floor: stored.commit_count_is_floor,
            rename_from: stored.rename_from,
            working_state: None,
            present_in: stored
                .present_in
                .into_iter()
                .map(|label| deploy_label(&label))
                .collect(),
            last_subject: stored.last_subject,
            top_author: stored.top_author,
            co_changed: stored
                .co_changed
                .into_iter()
                .map(|(path, count)| (Arc::from(path), count))
                .collect(),
        }
    }
}

impl<'de> Deserialize<'de> for GitActivity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        StoredGitActivity::deserialize(deserializer).map(Self::from)
    }
}

impl Serialize for GitActivity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("GitActivity", 11)?;
        state.serialize_field("last_modified", &self.last_modified)?;
        state.serialize_field("last_author", &self.last_author)?;
        state.serialize_field("commits_30d", &self.commits_30d)?;
        state.serialize_field("first_seen", &self.first_seen)?;
        state.serialize_field("commit_count", &self.commit_count)?;
        state.serialize_field("commit_count_is_floor", &self.commit_count_is_floor)?;
        state.serialize_field("rename_from", &self.rename_from)?;
        state.serialize_field("present_in", &self.present_in)?;
        state.serialize_field("last_subject", &self.last_subject)?;
        state.serialize_field("top_author", &self.top_author)?;
        let co_changed: Vec<(&str, i64)> = self
            .co_changed
            .iter()
            .map(|(path, count)| (path.as_ref(), *count))
            .collect();
        state.serialize_field("co_changed", &co_changed)?;
        state.end()
    }
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

fn deploy_label(label: &str) -> &'static str {
    DEPLOY_BRANCHES
        .iter()
        .find_map(|(configured, _)| (*configured == label).then_some(*configured))
        .unwrap_or_else(|| Box::leak(label.to_string().into_boxed_str()))
}

fn intern_co_changed(mut activities: HashMap<String, GitActivity>) -> HashMap<String, GitActivity> {
    let mut paths: HashMap<String, Arc<str>> = HashMap::new();
    for activity in activities.values_mut() {
        for (path, _) in &mut activity.co_changed {
            let interned = paths
                .entry(path.to_string())
                .or_insert_with(|| Arc::clone(path));
            *path = Arc::clone(interned);
        }
    }
    activities
}

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

fn historical(repo_root: &Path) -> HashMap<String, GitActivity> {
    let history = walk_history(repo_root);
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
                working_state: None,
                present_in: Vec::new(),
                last_subject: info.last_subject.clone(),
                top_author: info.top_author.clone(),
                co_changed: info
                    .co_changed
                    .iter()
                    .map(|(path, count)| (Arc::from(path.as_str()), *count))
                    .collect(),
            },
        );
    }
    out
}

fn compose_live(
    mut out: HashMap<String, GitActivity>,
    working: HashMap<String, String>,
    presence: HashMap<String, Vec<&'static str>>,
) -> HashMap<String, GitActivity> {
    for activity in out.values_mut() {
        activity.working_state = None;
        activity.present_in.clear();
    }
    for (path, state) in working {
        let entry = out.entry(path).or_insert_with(GitActivity::empty);
        entry.working_state = Some(state);
    }
    // Deploy-branch presence is a fact about the current refs, not about
    // history, so it survives a repo whose history the walk dropped. In a
    // shallow clone every file's only commit is a graft, which leaves no
    // history entry and previously no entry at all — silently turning a file
    // that is on origin/main into `presence: local-only`.
    for (path, refs) in presence {
        let entry = out.entry(path).or_insert_with(GitActivity::empty);
        entry.present_in = refs;
    }
    intern_co_changed(out)
}

/// Complete disk-cached activity, memoized once per repository root.
pub fn bulk_cached(repo_root: &Path) -> Arc<HashMap<String, GitActivity>> {
    complete_cached(repo_root)
}

static COMPLETE_MEMO: memo::Memo<HashMap<String, GitActivity>> = OnceLock::new();

fn complete_cached(repo_root: &Path) -> Arc<HashMap<String, GitActivity>> {
    memo::get_or_build(&COMPLETE_MEMO, repo_root, || {
        bulk_cached_uncached(repo_root)
    })
}

/// Disk-cached bulk map. Historical fields are cached under
/// `git_activity_v2__{head}__{30d cutoff}` in the file namespace; working-tree
/// state is always recomputed fresh and overlaid.
///
/// The cutoff date is in the key because `commits_30d` is computed against
/// today's date: keyed by HEAD alone, a branch that sits idle keeps serving
/// the velocity it had on the day the entry was written.
fn bulk_cached_uncached(repo_root: &Path) -> HashMap<String, GitActivity> {
    compose_live(
        cached_history(repo_root),
        working_tree_state(repo_root),
        presence_by_path(repo_root),
    )
}

fn cached_history(repo_root: &Path) -> HashMap<String, GitActivity> {
    let head = match head_sha(repo_root) {
        Some(h) => h,
        None => return historical(repo_root),
    };
    let key = format!("git_activity_v2__{head}__{}", cutoff_30d());
    if let Some(history) = cache::load_bytes(cache::NAMESPACE_FILE, &key, repo_root)
        .and_then(|b| serde_json::from_slice::<HashMap<String, GitActivity>>(&b).ok())
    {
        return history;
    }
    let history = historical(repo_root);
    // `working_state` is `skip_serializing`, so the entry holds only the
    // HEAD-keyed facts the key actually determines.
    if let Ok(payload) = serde_json::to_value(&history) {
        let _ = cache::save(cache::NAMESPACE_FILE, &key, &payload, repo_root);
    }
    cache::evict_prefixed(cache::NAMESPACE_FILE, "git_activity", &key, repo_root);
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
    let stdout = match Command::new("git")
        .args([
            "log",
            &cap_arg,
            "-M",
            "--diff-merges=first-parent",
            "--name-status",
            "-z",
            "--pretty=format:COMMIT|%H|%ad|%an|%s%x00",
            "--date=short",
        ])
        .current_dir(repo_root)
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        _ => return History::default(),
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
    let fields: Vec<&[u8]> = stdout.split(|byte| *byte == 0).collect();
    let mut index = 0;
    while index < fields.len() {
        let field = String::from_utf8_lossy(fields[index]);
        let field = field.trim_start_matches('\n');
        if let Some(rest) = field.strip_prefix("COMMIT|") {
            flush_co(
                &mut co_by_path,
                &mut pairs_held,
                &current_paths,
                current_files,
            );
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
            index += 1;
            continue;
        }
        if field.is_empty() || current_date.is_none() {
            index += 1;
            continue;
        }
        let date = current_date.clone().unwrap();
        let status = field;
        let path_fields = if status.starts_with('R') || status.starts_with('C') {
            2
        } else {
            1
        };
        if index + path_fields >= fields.len() {
            break;
        }
        if skip_commit {
            index += path_fields + 1;
            continue;
        }
        let first_path = String::from_utf8_lossy(fields[index + 1]).into_owned();
        if first_path.is_empty() {
            index += 1;
            continue;
        }
        if status.starts_with('R') || status.starts_with('C') {
            let old_path = first_path;
            let path = String::from_utf8_lossy(fields[index + 2]).into_owned();
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
            index += 3;
        } else {
            let path = first_path;
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
            index += 2;
        }
    }
    flush_co(
        &mut co_by_path,
        &mut pairs_held,
        &current_paths,
        current_files,
    );

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
        .as_deref()
        .into_iter()
        .flat_map(|entries| entries.iter())
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
        .as_deref()
        .into_iter()
        .flat_map(|entries| entries.iter())
        .filter_map(|(path, (_, staging))| staging.as_ref().map(|s| (path.clone(), s.clone())))
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
pub(crate) type Porcelain = HashMap<String, (String, Option<String>)>;
static PORCELAIN_MEMO: memo::Memo<Option<Arc<Porcelain>>> = OnceLock::new();

pub(crate) fn porcelain(repo_root: &Path) -> Option<Arc<Porcelain>> {
    memo::get_or_build(&PORCELAIN_MEMO, repo_root, || {
        porcelain_uncached(repo_root, "all").map(Arc::new)
    })
    .as_ref()
    .as_ref()
    .map(Arc::clone)
}

/// Paths reported by the invocation's one porcelain scan. Repository file
/// discovery reuses these for untracked files instead of starting a second
/// directory walk through `git ls-files --others`.
pub(crate) fn working_paths(repo_root: &Path) -> Option<Vec<String>> {
    Some(porcelain(repo_root)?.keys().cloned().collect())
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

pub(crate) fn porcelain_uncached(repo_root: &Path, untracked: &str) -> Option<Porcelain> {
    let untracked = format!("--untracked-files={untracked}");
    let out = match Command::new("git")
        .args(["status", "--porcelain=v1", "-z", &untracked])
        .current_dir(repo_root)
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return None,
    };
    Some(parse_porcelain(&out))
}

fn parse_porcelain(out: &[u8]) -> Porcelain {
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
        let mut path: String = chunk.chars().skip(3).collect();
        path = normalize_working_path(path);
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

fn normalize_working_path(path: String) -> String {
    if path
        .split('/')
        .next()
        .is_some_and(|segment| segment == crate::cache::CACHE_DIR_NAME)
    {
        return format!("{}/", crate::cache::CACHE_DIR_NAME);
    }
    path
}

/// For each path, which deploy branches contain it.
///
/// The `ls-tree -r` walks change only when a deploy branch tip moves, which
/// is far rarer than HEAD moving. So the result is disk-cached keyed by the
/// tip commit ids of the present deploy branches (absent branches drop out
/// of both the key and the result, exactly as the live computation does). A
/// local commit that doesn't touch a deploy branch reuses this cache; a
/// deploy-branch fast-forward rotates the key and recomputes.
fn presence_by_path(repo_root: &Path) -> HashMap<String, Vec<&'static str>> {
    let configured: Vec<(&str, &str, String)> = DEPLOY_BRANCHES
        .iter()
        .map(|(label, r#ref)| (*label, *r#ref, format!("refs/remotes/{ref}")))
        .collect();
    let output = match Command::new("git")
        .arg("for-each-ref")
        .arg("--format=%(refname)%00%(objectname)")
        .args(configured.iter().map(|(_, _, full_ref)| full_ref))
        .current_dir(repo_root)
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        _ => return HashMap::new(),
    };
    let mut observed: HashMap<String, String> = HashMap::new();
    for record in output.split(|byte| *byte == b'\n') {
        let Some(separator) = record.iter().position(|byte| *byte == 0) else {
            continue;
        };
        let r#ref = String::from_utf8_lossy(&record[..separator]).into_owned();
        let tip = String::from_utf8_lossy(&record[separator + 1..]).into_owned();
        if !tip.is_empty() {
            observed.insert(r#ref, tip);
        }
    }
    // `for-each-ref` may treat an argument as a namespace prefix. Join back
    // through the full configured names so similarly prefixed and unrelated
    // refs never enter the cache identity or displayed presence.
    let present: Vec<(&str, &str, String)> = configured
        .into_iter()
        .filter_map(|(label, r#ref, full_ref)| Some((label, r#ref, observed.remove(&full_ref)?)))
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
        format!("git_presence_v2__{}", hex::encode(hasher.finalize()))
    };

    if let Some(cached) = cache::load_bytes(cache::NAMESPACE_FILE, &key, repo_root)
        .and_then(|b| serde_json::from_slice::<HashMap<String, Vec<String>>>(&b).ok())
    {
        return cached
            .into_iter()
            .map(|(path, labels)| {
                (
                    path,
                    labels
                        .into_iter()
                        .map(|label| deploy_label(&label))
                        .collect(),
                )
            })
            .collect();
    }

    let computed = compute_presence(repo_root, &present);
    let payload = json!(computed);
    let _ = cache::save(cache::NAMESPACE_FILE, &key, &payload, repo_root);
    cache::evict_prefixed(cache::NAMESPACE_FILE, "git_presence", &key, repo_root);
    computed
}

/// Run the `ls-tree` walks over the resolved present deploy branches.
fn compute_presence(
    repo_root: &Path,
    present: &[(&str, &str, String)],
) -> HashMap<String, Vec<&'static str>> {
    let mut labels_for: HashMap<String, Vec<&'static str>> = HashMap::new();
    for (label, _, tip) in present {
        let output = match Command::new("git")
            .args(["ls-tree", "-rz", "--name-only", tip])
            .current_dir(repo_root)
            .output()
        {
            Ok(output) if output.status.success() => output.stdout,
            _ => continue,
        };
        for path in output.split(|byte| *byte == 0) {
            if path.is_empty() {
                continue;
            }
            let path = String::from_utf8_lossy(path).into_owned();
            let v = labels_for.entry(path).or_default();
            let label = deploy_label(label);
            if !v.contains(&label) {
                v.push(label);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_cache_shape_round_trips() {
        let activity = GitActivity {
            last_modified: Some("2026-09-07".to_string()),
            last_author: Some("Jordan".to_string()),
            commits_30d: 3,
            first_seen: Some("2026-08-01".to_string()),
            commit_count: 7,
            commit_count_is_floor: false,
            rename_from: Some("old.rs".to_string()),
            working_state: None,
            present_in: vec!["main"],
            last_subject: Some("change".to_string()),
            top_author: Some("Jordan".to_string()),
            co_changed: vec![(Arc::from("other.rs"), 2)],
        };

        let bytes = serde_json::to_vec(&activity).unwrap();

        assert_eq!(
            serde_json::from_slice::<GitActivity>(&bytes).unwrap(),
            activity
        );
    }

    #[test]
    fn installed_v2_entry_round_trips() {
        let bytes = include_bytes!("../tests/fixtures/git_activity_v2_installed.json");
        let activity = serde_json::from_slice::<HashMap<String, GitActivity>>(bytes).unwrap();

        assert_eq!(serde_json::to_vec(&activity).unwrap(), bytes);
    }
}
