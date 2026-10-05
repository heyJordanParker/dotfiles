//! Bulk git activity: the commit index + per-file lifecycle facts.
//!
//! The commit index `commits_v1` holds the newest `HISTORY_CAP` commits, each
//! with its changed paths and their old and new blob ids. A HEAD move walks
//! only the commits the index lacks, through one
//!   `git log --no-walk=unsorted --stdin -M --diff-merges=first-parent --raw`
//! and the lifecycle facts, 30-day counts included, are aggregated from it.
//! On a history deeper than the cap, `commit_count` becomes a floor (commits
//! within the cap, not the full-history total) and `commit_count_is_floor` is
//! set so consumers can see the count is partial.
//! Plus live working state, `git rev-parse HEAD`, one
//! `git for-each-ref` observation of every deploy tip, and one `git ls-tree
//! -r --name-only <ref>` per present deploy branch (the latter disk-cached,
//! keyed by the branch tip commit ids).

use crate::{cache, memo};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Output, Stdio};
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

pub fn git_command<I, S, T>(repo_root: &Path, args: I, run: impl FnOnce(&mut Command) -> T) -> T
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let detail = args
        .iter()
        .take(2)
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    crate::timing::phase(&format!("git {detail}"), || {
        let mut command = Command::new("git");
        command.args(&args).current_dir(repo_root);
        run(&mut command)
    })
}

pub fn git_output<I, S>(repo_root: &Path, args: I) -> std::io::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    git_command(repo_root, args, |command| command.output())
}

/// Run git in `repo_root` and return stdout with the trailing newline stripped.
pub fn git_str(repo_root: &Path, args: &[&str]) -> Option<String> {
    let out = git_output(repo_root, args).ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn blob(repo_root: &Path, revision: &str, path: &str) -> Option<Vec<u8>> {
    let mut found = None;
    blobs(repo_root, &[format!("{revision}:{path}")], |_, bytes| {
        found = bytes.map(<[u8]>::to_vec);
    });
    found
}

pub fn blobs(repo_root: &Path, names: &[String], mut visit: impl FnMut(usize, Option<&[u8]>)) {
    let request: Vec<u8> = names
        .iter()
        .flat_map(|name| name.bytes().chain(std::iter::once(b'\n')))
        .collect();
    let mut visited = 0;
    let _ = piped(repo_root, &["cat-file", "--batch"], request, |_| {}, |stdout| {
        let mut stdout = BufReader::new(stdout);
        let mut header = Vec::new();
        let mut body = Vec::new();
        while visited < names.len() {
            header.clear();
            if stdout.read_until(b'\n', &mut header)? == 0 {
                break;
            }
            let header = String::from_utf8_lossy(&header);
            match header.trim_end().split(' ').collect::<Vec<_>>().as_slice() {
                [_, kind, size] => {
                    body.resize(size.parse().map_err(std::io::Error::other)?, 0);
                    stdout.read_exact(&mut body)?;
                    stdout.read_exact(&mut [0u8; 1])?;
                    visit(visited, (*kind == "blob").then_some(&body[..]));
                }
                _ => visit(visited, None),
            }
            visited += 1;
        }
        Ok(())
    });
    for index in visited..names.len() {
        visit(index, None);
    }
}

pub(crate) fn piped<T>(
    repo_root: &Path,
    args: &[&str],
    request: Vec<u8>,
    configure: impl FnOnce(&mut Command),
    read: impl FnOnce(ChildStdout) -> std::io::Result<T>,
) -> std::io::Result<(T, bool)> {
    git_command(repo_root, args.iter().copied(), |command| {
        configure(command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("git stdin is piped");
        let writer = std::thread::spawn(move || stdin.write_all(&request));
        let read = read(child.stdout.take().expect("git stdout is piped"));
        let _ = writer.join();
        let finished = child.wait()?.success();
        Ok((read?, finished))
    })
}

pub fn head_sha(repo_root: &Path) -> Option<String> {
    static HEAD_MEMO: memo::Memo<Option<String>> = OnceLock::new();
    memo::get_or_build(&HEAD_MEMO, repo_root, || git_str(repo_root, &["rev-parse", "HEAD"])).as_ref().clone()
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

fn historical(commits: &Commits, cutoff_30d: &str) -> (HashMap<String, GitActivity>, bool) {
    let history = walk_history(commits, cutoff_30d);
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
    (out, history.finished)
}

fn activity_for(
    path: &str,
    history: &HashMap<String, GitActivity>,
    working: Option<&(String, Option<String>, Option<String>)>,
    presence: Option<&Vec<&'static str>>,
) -> GitActivity {
    let renamed_from = working.and_then(|(_, _, from)| from.as_ref());
    let mut activity = history
        .get(path)
        .cloned()
        .or_else(|| {
            let from = renamed_from?;
            let mut earlier = history.get(from)?.clone();
            earlier.rename_from = Some(from.clone());
            Some(earlier)
        })
        .unwrap_or_else(GitActivity::empty);
    activity.working_state = working.map(|(state, _, _)| state.clone());
    activity.present_in = presence.cloned().unwrap_or_default();
    activity
}

fn compose_live(
    history: &HashMap<String, GitActivity>,
    working: &Porcelain,
    presence: &HashMap<String, Vec<&'static str>>,
) -> HashMap<String, GitActivity> {
    let mut paths: HashSet<&str> = history.keys().map(String::as_str).collect();
    paths.extend(working.keys().map(String::as_str));
    paths.extend(presence.keys().map(String::as_str));
    intern_co_changed(
        paths
            .into_iter()
            .map(|path| {
                (
                    path.to_string(),
                    activity_for(path, history, working.get(path), presence.get(path)),
                )
            })
            .collect(),
    )
}

/// Complete disk-cached activity, memoized once per repository root.
pub fn bulk_cached(repo_root: &Path) -> Arc<HashMap<String, GitActivity>> {
    complete_cached(repo_root)
}

pub fn for_paths(repo_root: &Path, paths: &[String]) -> HashMap<String, GitActivity> {
    let listing = crate::repo_files::tracked_set_state(repo_root);
    let tracked = crate::repo_files::tracked_set(repo_root).unwrap_or_default();
    let (history, (presence, working)) = rayon::join(
        || cached_history(repo_root),
        || {
            rayon::join(
                || cached_presence(repo_root),
                || {
                    if listing.as_ref().is_some_and(|listing| listing.discovered) {
                        porcelain(repo_root)
                    } else {
                        tracked_porcelain(repo_root)
                    }
                },
            )
        },
    );
    paths
        .iter()
        .filter_map(|path| {
            let working_entry = working.as_ref().and_then(|entries| entries.get(path));
            let mut activity = activity_for(path, &history, working_entry, presence.get(path));
            if !tracked.contains(path)
                && listing
                    .as_ref()
                    .is_some_and(|listing| listing.untracked.contains(path))
            {
                activity.working_state = Some("untracked".to_string());
            }
            (activity != GitActivity::empty()).then_some((path.clone(), activity))
        })
        .collect()
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
    let (history, (working, presence)) = rayon::join(
        || cached_history(repo_root),
        || rayon::join(|| porcelain(repo_root), || cached_presence(repo_root)),
    );
    compose_live(&history, working.as_deref().unwrap_or(&Porcelain::new()), &presence)
}

fn cached_history(repo_root: &Path) -> Arc<HashMap<String, GitActivity>> {
    static HISTORY_MEMO: memo::Memo<HashMap<String, GitActivity>> = OnceLock::new();
    memo::get_or_build(&HISTORY_MEMO, repo_root, || {
        cached_history_uncached(repo_root)
    })
}

fn cached_history_uncached(repo_root: &Path) -> HashMap<String, GitActivity> {
    let Some(head) = head_sha(repo_root) else {
        return HashMap::new();
    };
    let index = ActivityIndex {
        repo_root,
        head,
        cutoff: cutoff_30d(),
    };
    cache::index(&index, repo_root)
        .map(|activity| activity.map)
        .unwrap_or_default()
}

struct ActivityIndex<'a> {
    repo_root: &'a Path,
    head: String,
    cutoff: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
struct Activity {
    map: HashMap<String, GitActivity>,
    #[serde(skip)]
    incomplete: bool,
}

impl cache::Index for ActivityIndex<'_> {
    type Stored = Activity;
    type Change = ();

    fn key(&self) -> String {
        format!("git_activity_v2__{}__{}", self.head, self.cutoff)
    }

    fn read(&self, bytes: &[u8]) -> Option<Activity> {
        serde_json::from_slice(bytes).ok()
    }

    fn change(&self, stored: Option<&Activity>) -> Option<()> {
        stored.is_none().then_some(())
    }

    fn apply(&self, _stored: Option<Activity>, _change: ()) -> Activity {
        let (map, finished) = historical(&commits(self.repo_root), &self.cutoff);
        Activity {
            map,
            incomplete: !finished,
        }
    }

    fn complete(&self, stored: &Activity) -> bool {
        !stored.incomplete
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Commits {
    head: String,
    pub order: Vec<String>,
    pub commits: HashMap<String, Commit>,
    grafts: Vec<String>,
    pub binary: BTreeSet<String>,
    #[serde(skip)]
    incomplete: bool,
}

impl Commits {
    pub fn truncated(&self) -> bool {
        self.order.len() >= HISTORY_CAP || self.incomplete
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Commit {
    pub date: String,
    pub author: String,
    pub subject: String,
    pub merge: bool,
    pub changes: Vec<FileChange>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub status: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
}

impl FileChange {
    pub fn old_path(&self) -> &str {
        self.from.as_deref().unwrap_or(&self.path)
    }
}

pub enum CommitsChange {
    Walk,
    Binary(Vec<String>),
}

pub struct CommitIndex<'a> {
    pub repo_root: &'a Path,
}

pub fn commits(repo_root: &Path) -> Arc<Commits> {
    static COMMITS_MEMO: memo::Memo<Commits> = OnceLock::new();
    memo::get_or_build(&COMMITS_MEMO, repo_root, || {
        cache::index(&CommitIndex { repo_root }, repo_root).unwrap_or_default()
    })
}

impl cache::Index for CommitIndex<'_> {
    type Stored = Commits;
    type Change = CommitsChange;

    fn key(&self) -> String {
        "commits_v1".to_string()
    }

    fn read(&self, bytes: &[u8]) -> Option<Commits> {
        serde_json::from_slice(bytes).ok()
    }

    fn change(&self, stored: Option<&Commits>) -> Option<CommitsChange> {
        let head = head_sha(self.repo_root).unwrap_or_default();
        (stored.map(|commits| commits.head.as_str()) != Some(head.as_str())).then_some(CommitsChange::Walk)
    }

    fn apply(&self, stored: Option<Commits>, change: CommitsChange) -> Commits {
        let mut commits = stored.unwrap_or_default();
        match change {
            CommitsChange::Binary(blobs) => commits.binary.extend(blobs),
            CommitsChange::Walk => self.walk(&mut commits),
        }
        commits
    }

    fn complete(&self, stored: &Commits) -> bool {
        !stored.incomplete
    }
}

impl CommitIndex<'_> {
    fn walk(&self, commits: &mut Commits) {
        let order: Vec<String> = git_str(self.repo_root, &["rev-list", &format!("-n{HISTORY_CAP}"), "HEAD"])
            .map(|listed| listed.lines().map(str::to_string).collect())
            .unwrap_or_default();
        let mut grafts: Vec<String> = grafts(self.repo_root).into_iter().collect();
        grafts.sort();
        if grafts != commits.grafts {
            for regrafted in grafts.iter().chain(&commits.grafts) {
                commits.commits.remove(regrafted);
            }
            commits.grafts = grafts;
        }
        let reachable: HashSet<&str> = order.iter().map(String::as_str).collect();
        commits.commits.retain(|id, _| reachable.contains(id.as_str()));
        let missing: Vec<&str> = order
            .iter()
            .map(String::as_str)
            .filter(|id| !commits.commits.contains_key(*id))
            .collect();
        if !missing.is_empty() {
            let grafted: HashSet<&str> = commits.grafts.iter().map(String::as_str).collect();
            let (walked, finished) = walk_commits(self.repo_root, &missing, &grafted);
            commits.commits.extend(walked);
            commits.incomplete = !finished;
        }
        let held: HashSet<&str> = commits
            .commits
            .values()
            .flat_map(|commit| &commit.changes)
            .flat_map(|change| change.old.iter().chain(&change.new))
            .map(String::as_str)
            .collect();
        commits.binary.retain(|blob| held.contains(blob.as_str()));
        commits.head = head_sha(self.repo_root).unwrap_or_default();
        commits.order = order;
    }
}

fn walk_commits(repo_root: &Path, ids: &[&str], grafted: &HashSet<&str>) -> (HashMap<String, Commit>, bool) {
    let request = ids.join("\n").into_bytes();
    let walked = piped(
        repo_root,
        &[
            "log",
            "--no-walk=unsorted",
            "--stdin",
            "-M",
            "--diff-merges=first-parent",
            "--raw",
            "--no-abbrev",
            "-z",
            "--pretty=format:COMMIT|%H|%P|%ad|%an|%s%x00",
            "--date=short",
        ],
        request,
        |command| {
            command.env("GIT_NO_LAZY_FETCH", "1");
        },
        |mut stdout| {
            let mut read = Vec::new();
            stdout.read_to_end(&mut read)?;
            Ok(read)
        },
    );
    let Ok((stdout, finished)) = walked else {
        return (HashMap::new(), false);
    };
    let mut commits: HashMap<String, Commit> = HashMap::new();
    let mut current: Option<String> = None;
    let fields: Vec<&[u8]> = stdout.split(|byte| *byte == 0).collect();
    let mut index = 0;
    while index < fields.len() {
        let field = String::from_utf8_lossy(fields[index]);
        let field = field.trim_start_matches('\n');
        index += 1;
        if let Some(rest) = field.strip_prefix("COMMIT|") {
            let parts: Vec<&str> = rest.splitn(5, '|').collect();
            let [id, parents, date, author, subject] = parts.as_slice() else {
                current = None;
                continue;
            };
            commits.insert(
                id.to_string(),
                Commit {
                    date: date.to_string(),
                    author: author.to_string(),
                    subject: subject.to_string(),
                    merge: parents.split_whitespace().count() > 1,
                    changes: Vec::new(),
                },
            );
            current = (!grafted.contains(id)).then(|| id.to_string());
            continue;
        }
        let Some(raw) = field.strip_prefix(':') else {
            continue;
        };
        let columns: Vec<&str> = raw.split(' ').collect();
        let [_, _, old, new, status] = columns.as_slice() else {
            continue;
        };
        let renamed = status.starts_with('R') || status.starts_with('C');
        let paths = if renamed { 2 } else { 1 };
        if index + paths > fields.len() {
            break;
        }
        let first = String::from_utf8_lossy(fields[index]).into_owned();
        let (from, path) = if renamed {
            (Some(first), String::from_utf8_lossy(fields[index + 1]).into_owned())
        } else {
            (None, first)
        };
        index += paths;
        let blob = |id: &str| (!id.bytes().all(|byte| byte == b'0')).then(|| id.to_string());
        if let Some(commit) = current.as_ref().and_then(|id| commits.get_mut(id)) {
            commit.changes.push(FileChange {
                status: status.to_string(),
                path,
                from,
                old: blob(old),
                new: blob(new),
            });
        }
    }
    (commits, finished)
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

/// Result of the bounded history walk: the per-file entries, whether the walk
/// stopped before reaching the first commit (in which case every entry's
/// `commit_count` / `first_seen` is a floor), and whether git finished it.
#[derive(Default)]
struct History {
    entries: BTreeMap<String, HistoryEntry>,
    truncated: bool,
    finished: bool,
}

fn walk_history(commits: &Commits, cutoff_30d: &str) -> History {
    let mut out: BTreeMap<String, HistoryEntry> = BTreeMap::new();
    let mut aliases: HashMap<String, String> = HashMap::new();
    let mut authors_by_path: HashMap<String, OrderedCounter> = HashMap::new();
    let mut co_by_path: HashMap<String, OrderedCounter> = HashMap::new();
    let mut current_paths: Vec<String> = Vec::new();
    // The commit's true file count. `current_paths` stops growing at the cap,
    // because its `contains` scan is linear and an unbounded list makes the
    // parse itself quadratic in the commit's size; this counter keeps going so
    // `flush_co` can tell a 100-file commit from a 29,122-file one.
    let mut current_files = 0usize;
    // Distinct pairs retained so far, weighed against `CO_CHANGE_PAIR_CAP`.
    let mut pairs_held = 0usize;

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

    for commit in commits.order.iter().filter_map(|id| commits.commits.get(id)) {
        flush_co(
            &mut co_by_path,
            &mut pairs_held,
            &current_paths,
            current_files,
        );
        current_paths.clear();
        current_files = 0;
        let date = commit.date.clone();
        let current_author = Some(commit.author.clone());
        let current_subject = Some(commit.subject.clone());
        // `--date=short` dates are zero-padded YYYY-MM-DD, so a lexical
        // `>=` against the cutoff is a correct chronological compare.
        let current_recent = date.as_str() >= cutoff_30d;
        for change in &commit.changes {
            if change.from.is_none() {
                let target = aliases.get(&change.path).cloned().unwrap_or_else(|| change.path.clone());
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
                continue;
            }
            let old_path = change.old_path().to_string();
            let path = change.path.clone();
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
        truncated: commits.truncated(),
        finished: !commits.incomplete,
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
        .map(|(path, (state, _, _))| (path.clone(), state.clone()))
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
        .filter_map(|(path, (_, staging, _))| staging.as_ref().map(|s| (path.clone(), s.clone())))
        .collect()
}

/// Per-file `(working-tree state, staging state, renamed-from path)` via
/// `git status --porcelain=v1 -z`.
///
/// Process-wide memo, keyed by repo root: the working tree does not change
/// within a single CLI invocation, so the `git status` subprocess runs at
/// most once per root. The lock is held across the compute so concurrent
/// callers (parallel primer sections, file-facts working-state overlay)
/// serialize onto one status run rather than racing `index.lock`.
pub(crate) type Porcelain = HashMap<String, (String, Option<String>, Option<String>)>;
static PORCELAIN_MEMO: memo::Memo<Option<Arc<Porcelain>>> = OnceLock::new();

pub(crate) fn porcelain(repo_root: &Path) -> Option<Arc<Porcelain>> {
    memo::get_or_build(&PORCELAIN_MEMO, repo_root, || {
        porcelain_uncached(repo_root, "all").map(Arc::new)
    })
    .as_ref()
    .as_ref()
    .map(Arc::clone)
}

static TRACKED_PORCELAIN_MEMO: memo::Memo<Option<Arc<Porcelain>>> = OnceLock::new();

pub(crate) fn tracked_porcelain(repo_root: &Path) -> Option<Arc<Porcelain>> {
    if let Some(full) = PORCELAIN_MEMO
        .get()
        .and_then(|memo| memo.lock().unwrap().get(repo_root).cloned())
    {
        return full.as_ref().as_ref().map(Arc::clone);
    }
    memo::get_or_build(&TRACKED_PORCELAIN_MEMO, repo_root, || {
        porcelain_uncached(repo_root, "no").map(Arc::new)
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
    let out = match git_output(repo_root, ["status", "--porcelain=v1", "-z", &untracked]) {
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
            let from = parts
                .get(i + 1)
                .filter(|from| !from.is_empty())
                .map(|from| String::from_utf8_lossy(from).into_owned());
            i += if i + 1 < parts.len() { 2 } else { 1 };
            result.insert(path, ("renamed".to_string(), staging_word(x, y), from));
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
        result.insert(path, (state.to_string(), staging_word(x, y), None));
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
fn cached_presence(repo_root: &Path) -> Arc<HashMap<String, Vec<&'static str>>> {
    static PRESENCE_MEMO: memo::Memo<HashMap<String, Vec<&'static str>>> = OnceLock::new();
    memo::get_or_build(&PRESENCE_MEMO, repo_root, || presence_by_path(repo_root))
}

fn presence_by_path(repo_root: &Path) -> HashMap<String, Vec<&'static str>> {
    let configured: Vec<(&str, &str, String)> = DEPLOY_BRANCHES
        .iter()
        .map(|(label, r#ref)| (*label, *r#ref, format!("refs/remotes/{ref}")))
        .collect();
    let mut args = vec![
        OsString::from("for-each-ref"),
        OsString::from("--format=%(refname)%00%(objectname)"),
    ];
    args.extend(
        configured
            .iter()
            .map(|(_, _, full_ref)| OsString::from(full_ref)),
    );
    let output = match git_output(repo_root, args) {
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

    cache::index(&PresenceIndex { repo_root, present }, repo_root)
        .map(|presence| {
            presence
                .labels
                .into_iter()
                .map(|(path, labels)| (path, labels.iter().map(|label| deploy_label(label)).collect()))
                .collect()
        })
        .unwrap_or_default()
}

struct PresenceIndex<'a> {
    repo_root: &'a Path,
    present: Vec<(&'static str, &'static str, String)>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
struct Presence {
    labels: HashMap<String, Vec<String>>,
    #[serde(skip)]
    incomplete: bool,
}

impl cache::Index for PresenceIndex<'_> {
    type Stored = Presence;
    type Change = ();

    fn key(&self) -> String {
        let mut hasher = Sha256::new();
        for (label, r#ref, tip) in &self.present {
            hasher.update(label.as_bytes());
            hasher.update(b"\0");
            hasher.update(r#ref.as_bytes());
            hasher.update(b"\0");
            hasher.update(tip.as_bytes());
            hasher.update(b"\n");
        }
        format!("git_presence_v2__{}", hex::encode(hasher.finalize()))
    }

    fn read(&self, bytes: &[u8]) -> Option<Presence> {
        serde_json::from_slice(bytes).ok()
    }

    fn change(&self, stored: Option<&Presence>) -> Option<()> {
        stored.is_none().then_some(())
    }

    fn apply(&self, _stored: Option<Presence>, _change: ()) -> Presence {
        let (labels, complete) = compute_presence(self.repo_root, &self.present);
        Presence {
            labels: labels
                .into_iter()
                .map(|(path, labels)| (path, labels.into_iter().map(str::to_string).collect()))
                .collect(),
            incomplete: !complete,
        }
    }

    fn complete(&self, stored: &Presence) -> bool {
        !stored.incomplete
    }
}

/// Run the `ls-tree` walks over the resolved present deploy branches, and
/// whether every walk finished from the objects already on disk.
fn compute_presence(
    repo_root: &Path,
    present: &[(&str, &str, String)],
) -> (HashMap<String, Vec<&'static str>>, bool) {
    let mut labels_for: HashMap<String, Vec<&'static str>> = HashMap::new();
    let mut complete = true;
    for (label, _, tip) in present {
        let output = match git_command(repo_root, ["ls-tree", "-rz", "--name-only", tip], |command| {
            command.env("GIT_NO_LAZY_FETCH", "1").output()
        }) {
            Ok(output) if output.status.success() => output.stdout,
            _ => {
                complete = false;
                continue;
            }
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
    let labels = labels_for
        .into_iter()
        .map(|(p, mut labels)| {
            labels.sort();
            labels.dedup();
            (p, labels)
        })
        .collect();
    (labels, complete)
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
