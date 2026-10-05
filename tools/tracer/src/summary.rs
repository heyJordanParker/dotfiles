//! A file's facts — the one structure every command shows about a file.
//!
//! `Facts` is built from the cached `FileFacts` and the graph around the file,
//! printed as YAML front matter through `yamlfmt`, and carried under the same
//! keys in `--json`. Keys use the words git, GitHub, and editors use; values
//! are counts, names, and ages, never ratios. `headline` is the one-line form
//! a list of many files shows.

use crate::commands::session_log::{self, Shown, ShownKind, ShownRecord};
use crate::file_facts::FileFacts;
use crate::git_activity::GitActivity;
use crate::relations::{self, ModuleCounts};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

#[derive(Serialize)]
pub struct Facts {
    pub file: String,
    pub lines: i64,
    pub cyclomatic_complexity: i64,
    pub complexity_rank: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported_by: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<u32>,
    pub git: Git,
}

#[derive(Serialize)]
pub struct Git {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits: Option<i64>,
    /// Set instead of `commits` when the history walk stopped at its cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits_at_least: Option<i64>,
    pub commits_last_30_days: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub on_deploy_branches: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub usually_changed_with: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_author: Option<String>,
}

impl Facts {
    pub fn of(facts: &FileFacts, graph: Option<&ModuleCounts>) -> Facts {
        Facts {
            file: facts.path.clone(),
            lines: facts.loc,
            cyclomatic_complexity: facts.cyclomatic_complexity_total,
            complexity_rank: facts.rank.clone(),
            imported_by: graph.map(|graph| graph.imported_by),
            imports: graph.map(|graph| graph.imports),
            git: Git {
                status: status(facts.working_state.as_deref()),
                renamed_from: facts.rename_from.clone(),
                commits: (!facts.commit_count_is_floor).then_some(facts.commit_count),
                commits_at_least: facts.commit_count_is_floor.then_some(facts.commit_count),
                commits_last_30_days: facts.commits_30d,
                // A capped walk never reached the first commit, only a floor.
                first_commit: facts.first_seen.as_deref().and_then(age).map(|age| {
                    if facts.commit_count_is_floor {
                        format!("at least {age}")
                    } else {
                        age
                    }
                }),
                last_commit: last_commit(
                    facts.last_modified.as_deref(),
                    facts.last_author.as_deref(),
                    facts.last_subject.as_deref(),
                ),
                on_deploy_branches: facts.present_in.iter().map(|branch| branch.to_string()).collect(),
                // Whole paths: three `Claude.md` in one list name nothing.
                usually_changed_with: facts
                    .co_changed
                    .iter()
                    .take(3)
                    .map(|(path, _)| path.to_string())
                    .collect(),
                main_author: facts.top_author.clone(),
            },
        }
    }

    /// The facts as a mapping, so a command can add its own keys (the
    /// directory, the docs not yet loaded) before printing.
    pub fn to_map(&self) -> Map<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        }
    }

    /// The few facts a list of many files shows per file, so a command can add
    /// its own before printing.
    pub fn headline_map(&self) -> Map<String, Value> {
        let mut map = Map::new();
        if let Some(imported_by) = self.imported_by {
            map.insert("imported_by".into(), imported_by.into());
        }
        map.insert("cyclomatic_complexity".into(), self.cyclomatic_complexity.into());
        map.insert("lines".into(), self.lines.into());
        if self.git.status != "unmodified" {
            map.insert("git".into(), self.git.status.clone().into());
        }
        map
    }

    /// `headline_map` as the one-line flow mapping a list shows per file.
    pub fn headline(&self) -> String {
        crate::yamlfmt::flow(&Value::Object(self.headline_map()), false)
    }
}

/// The git facts the bulk activity map holds alone, for a file with no
/// per-file extraction: a listing entry, or a file that could not be read.
/// Keys match `Git`'s.
pub fn activity_git(activity: &GitActivity) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert("status".into(), status(activity.working_state.as_deref()).into());
    map.insert("commits".into(), activity.commit_count.into());
    map.insert("commits_last_30_days".into(), activity.commits_30d.into());
    if let Some(first) = activity.first_seen.as_deref().and_then(age) {
        map.insert("first_commit".into(), first.into());
    }
    if let Some(last) = activity.last_modified.as_deref().and_then(age) {
        map.insert("last_commit".into(), last.into());
    }
    if let Some(author) = &activity.top_author {
        map.insert("main_author".into(), author.clone().into());
    }
    map
}

/// A mapping as a YAML front matter block.
pub fn front_matter(map: &Map<String, Value>) -> String {
    format!("---\n{}---\n", crate::yamlfmt::block(map))
}

/// What a command prints above a file's rows: `heading`, then `map` whole.
/// Text output with the file's `facts` prints the whole block the first time
/// an Agent context is shown `facts` alone, through `record`'s `Facts` gate,
/// so every command shares one record of them. Later the heading line carries
/// `facts.headline()` instead, and a block holds only the lines of `facts`
/// that changed since, beside every key `facts` do not own: the command's
/// own answer, `docs_not_loaded`, and the `directory` its own gate left. A
/// command with no heading line passes `None` and takes `# <file>` as one.
/// The caller saves `record` once the text is flushed.
pub fn front_matter_once(
    heading: Option<&str>,
    path: &Path,
    facts: Option<&Facts>,
    map: &Map<String, Value>,
    record: &mut ShownRecord,
    once: bool,
) -> String {
    let whole = || heading.map(|heading| format!("{heading}\n")).unwrap_or_default() + &front_matter(map);
    let Some(facts) = facts.filter(|_| once) else {
        return whole();
    };
    let own = Value::Object(facts.to_map());
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut block = match record.shown(ShownKind::Facts, &key.to_string_lossy(), &own) {
        Shown::New => return whole(),
        Shown::Same => Map::new(),
        Shown::Changed(lines) => lines,
    };
    block.extend(
        map.iter()
            .filter(|(key, _)| own.get(*key).is_none())
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    let heading = heading.map_or_else(|| format!("# {}", facts.file), str::to_string);
    let block = if block.is_empty() { String::new() } else { front_matter(&block) };
    format!("{heading}  {}\n{block}", facts.headline())
}

fn status(working_state: Option<&str>) -> String {
    working_state.unwrap_or("unmodified").to_string()
}

fn last_commit(date: Option<&str>, author: Option<&str>, subject: Option<&str>) -> Option<String> {
    let mut text = age(date?)?;
    if let Some(author) = author {
        text.push_str(&format!(" by {author}"));
    }
    if let Some(subject) = subject {
        text.push_str(&format!(": {}", clip(subject, 60)));
    }
    Some(text)
}

pub(crate) fn clip(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars - 1).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// A `YYYY-MM-DD` date as the age GitHub shows: "today", "3 weeks ago".
pub fn age(date: &str) -> Option<String> {
    // Git dates carry the committer's local day while this counts UTC days,
    // so a commit just after local midnight can sit a day in the future.
    let days = days_since(date)?.max(0);
    let plural = |count: i64, unit: &str| {
        format!("{count} {unit}{} ago", if count == 1 { "" } else { "s" })
    };
    Some(match days {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        2..=13 => plural(days, "day"),
        14..=59 => plural(days / 7, "week"),
        60..=364 => plural(days / 30, "month"),
        _ => plural(days / 365, "year"),
    })
}

fn days_since(date: &str) -> Option<i64> {
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.get(..2)?.parse().ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(now.div_euclid(86400) - days_from_civil(y, m, d))
}

pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

const CALL_BODY_LINES: i64 = 12;

// A next.js mentioning file costs 0.9 to 3.7 ms to resolve alone, so 300 keep `calls:` near a second there; dotfiles windows read at most 241.
const CALLS_FILE_BUDGET: usize = 300;

#[derive(Serialize)]
pub struct Call {
    pub line: i64,
    pub method: String,
    #[serde(flatten)]
    pub target: Target,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Target {
    Resolved {
        file: String,
        declared_at: i64,
        callers: usize,
        source: String,
        also_called_from: Vec<CallSite>,
    },
    CallSitesPastBudget {
        file: String,
        declared_at: i64,
        source: String,
    },
    PastBudget {
        defined_in: usize,
    },
}

#[derive(Serialize)]
pub struct CallSite {
    pub file: String,
    pub line: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    pub text: String,
}

pub fn calls(facts: &FileFacts, relative: &str, window: (i64, i64), repo_root: &Path) -> Vec<Call> {
    crate::timing::phase("calls", || {
        let Some(extraction) = &facts.extraction else {
            return Vec::new();
        };
        let within = |line: i64| window.0 <= line && line <= window.1;
        let called: BTreeMap<&str, BTreeSet<i64>> = extraction
            .references
            .iter()
            .filter(|reference| within(reference.line) && reference.receiver.as_deref() != Some(&reference.name))
            .fold(BTreeMap::new(), |mut called, reference| {
                called.entry(reference.name.as_str()).or_default().insert(reference.line);
                called
            });
        let index = relations::get(repo_root);
        let mut out = Vec::new();
        let mut charged: HashSet<&str> = HashSet::from([relative]);

        let mut by_cost: Vec<(&str, Vec<&str>)> = called
            .keys()
            .map(|&name| (name, index.defined_in(name).collect::<Vec<&str>>()))
            .filter(|(_, defining)| !defining.is_empty())
            .collect();
        by_cost.sort_by_key(|(name, defining)| (defining.len(), called[name].first().copied()));
        let mut in_window = Vec::new();
        for (name, defining) in by_cost {
            if charge(&mut charged, &defining) {
                in_window.push(name);
                continue;
            }
            out.extend(called[name].iter().map(|&line| Call {
                line,
                method: name.to_string(),
                target: Target::PastBudget { defined_in: defining.len() },
            }));
        }
        let candidates = relations::candidates(&in_window, repo_root);
        let window_sites = relations::use_sites(&candidates, Some(relative), repo_root);

        let mut resolved_in_window = Vec::new();
        for ((name, candidates), sites) in in_window.into_iter().zip(candidates).zip(&window_sites) {
            let resolved: Vec<(i64, &relations::UseSite)> = called[name]
                .iter()
                .filter_map(|&line| resolved_at(sites, relative, line).map(|site| (line, site)))
                .collect();
            if !resolved.is_empty() {
                resolved_in_window.push((name, candidates, resolved, index.used_in(name).collect::<Vec<&str>>()));
            }
        }
        resolved_in_window.sort_by_cached_key(|(name, _, _, mentioning)| {
            (mentioning.iter().filter(|file| !charged.contains(*file)).count(), called[name].first().copied())
        });
        let mut sources: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let (mut everywhere, mut everywhere_candidates) = (Vec::new(), Vec::new());
        for (name, candidates, resolved, mentioning) in resolved_in_window {
            if charge(&mut charged, &mentioning) {
                everywhere.push(name);
                everywhere_candidates.push(candidates);
                continue;
            }
            for (line, site) in resolved {
                out.push(Call {
                    line,
                    method: target_method(site),
                    target: Target::CallSitesPastBudget {
                        file: site.target_file.clone(),
                        declared_at: site.target.line,
                        source: target_source(site, &mut sources, repo_root),
                    },
                });
            }
        }

        for (name, sites) in everywhere.iter().zip(relations::use_sites(&everywhere_candidates, None, repo_root)) {
            for &line in &called[name] {
                let Some(site) = resolved_at(&sites, relative, line) else {
                    continue;
                };
                let target = &site.target;
                let mut callers: Vec<&relations::UseSite> = sites
                    .iter()
                    .filter(|other| other.target_file == site.target_file && other.target.line == target.line)
                    .collect();
                callers.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
                let source = target_source(site, &mut sources, repo_root);
                let also_called_from = callers
                    .iter()
                    .filter(|other| !(other.file == relative && within(other.line)))
                    .map(|other| CallSite {
                        file: other.file.clone(),
                        line: other.line,
                        caller: other.caller.as_ref().map(|caller| caller.name.clone()),
                        text: excerpt(
                            sources
                                .entry(other.file.clone())
                                .or_insert_with(|| source_lines(&repo_root.join(&other.file))),
                            other.line,
                            other.line,
                        )
                        .trim()
                        .to_string(),
                    })
                    .collect();
                out.push(Call {
                    line,
                    method: target_method(site),
                    target: Target::Resolved {
                        file: site.target_file.clone(),
                        declared_at: target.line,
                        callers: callers.len(),
                        source,
                        also_called_from,
                    },
                });
            }
        }
        out.sort_by_key(|call| call.line);
        out
    })
}

fn resolved_at<'a>(sites: &'a [relations::UseSite], file: &str, line: i64) -> Option<&'a relations::UseSite> {
    match sites
        .iter()
        .filter(|site| site.file == file && site.line == line && site.confidence != relations::CONFIDENCE_AMBIGUOUS)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [site] if site.target.kind == "function" => Some(site),
        _ => None,
    }
}

/// Adds `files` to `charged` and returns true when the ones it does not
/// hold yet fit `CALLS_FILE_BUDGET`; else leaves `charged` as it was.
fn charge<'a>(charged: &mut HashSet<&'a str>, files: &[&'a str]) -> bool {
    let new: HashSet<&str> = files.iter().copied().filter(|file| !charged.contains(file)).collect();
    if charged.len() + new.len() > CALLS_FILE_BUDGET {
        return false;
    }
    charged.extend(new);
    true
}

fn target_method(site: &relations::UseSite) -> String {
    match &site.target.container {
        Some(container) => format!("{container}::{}", site.target.name),
        None => site.target.name.clone(),
    }
}

/// The function `site` resolves to as a call shows it: its whole source
/// while the session has not read it and it spans at most
/// `CALL_BODY_LINES`, else its header.
fn target_source(site: &relations::UseSite, sources: &mut BTreeMap<String, Vec<String>>, repo_root: &Path) -> String {
    let target = &site.target;
    let lines = sources
        .entry(site.target_file.clone())
        .or_insert_with(|| source_lines(&repo_root.join(&site.target_file)));
    let read = session_log::has_read(
        &repo_root.join(&site.target_file),
        &session_log::content_hash(&lines.concat()),
        target.header_line as usize,
        target.end_line as usize,
    );
    if read || target.end_line - target.header_line + 1 > CALL_BODY_LINES {
        target.header.clone()
    } else {
        excerpt(lines, target.header_line, target.end_line)
    }
}

/// The calls block of a window of `file` in at most `budget` characters,
/// at the first of three levels whose heads fit: one head per call, one head
/// per function naming every line that calls it, then one line per file the
/// calls reach with their count, and one line naming the functions past
/// `CALLS_FILE_BUDGET` that name no file. At the first two, heads get their
/// source back in line order, then their other call sites, calls into other
/// files first, while the budget holds — the way `surface::render_within`
/// grows rows. The last
/// keeps its lines while the budget holds, other files first, then `file`,
/// then the functions past `CALLS_FILE_BUDGET`, so the block never overruns
/// it. `None` renders every call whole. A function the window calls on
/// several lines shows its source and other call sites once, under its first
/// line.
pub fn render_calls(calls: &[Call], file: &str, budget: Option<usize>) -> String {
    if calls.is_empty() {
        return String::new();
    }
    let mut seen = HashSet::new();
    let per_call: Vec<CallEntry> = calls
        .iter()
        .map(|call| CallEntry::of(vec![call], seen.insert(target_key(call)), file))
        .collect();
    let whole: String = per_call.iter().map(|entry| entry.levels[0].as_str()).collect();
    let Some(budget) = budget.map(|budget| budget.saturating_sub("calls:\n".len())) else {
        return format!("calls:\n{whole}");
    };
    if whole.len() <= budget {
        return format!("calls:\n{whole}");
    }
    let shortened = crate::output::shortened_line(calls.len(), calls.len(), "calls");
    let Some(budget) = budget.checked_sub(shortened.len() + 3) else {
        return String::new();
    };
    let per_function: Vec<CallEntry> = grouped(calls, target_key)
        .into_iter()
        .map(|calls| CallEntry::of(calls, true, file))
        .collect();
    let (text, cut) = fit_entries(&per_call, budget)
        .or_else(|| fit_entries(&per_function, budget))
        .unwrap_or_else(|| (by_file(calls, file, budget), calls.len()));
    format!("calls:\n{text}  {}\n", crate::output::shortened_line(cut, calls.len(), "calls"))
}

struct CallEntry<'a> {
    calls: Vec<&'a Call>,
    levels: [String; 3],
    into_reading_file: bool,
}

impl<'a> CallEntry<'a> {
    fn of(calls: Vec<&'a Call>, detailed: bool, file: &str) -> CallEntry<'a> {
        let first = calls[0];
        let lines: Vec<String> = calls.iter().map(|call| format!("L{}", call.line)).collect();
        let (lines, method) = (lines.join(", "), &first.method);
        let trace = crate::output::trace_command();
        let (head, source, sites) = match &first.target {
            Target::Resolved { file, declared_at, callers, source, also_called_from } => (
                format!("  {lines} {method}  {file}:{declared_at}  {{callers: {callers}}}\n"),
                source.as_str(),
                also_called_from.as_slice(),
            ),
            Target::CallSitesPastBudget { file, declared_at, source } => (
                format!("  {lines} {method}  {file}:{declared_at} \u{2192} {trace} callers {method}\n"),
                source.as_str(),
                &[][..],
            ),
            Target::PastBudget { defined_in } => (
                format!("  {lines} {method}  {{defined_in: {defined_in}}} \u{2192} {trace} callers {method}\n"),
                "",
                &[][..],
            ),
        };
        let (source, sites) = if detailed { (source, sites) } else { ("", &[][..]) };
        let sourced = source.lines().fold(head.clone(), |text, line| text + &format!("    {line}\n"));
        let whole = match sites {
            [] => sourced.clone(),
            sites => sites.iter().fold(format!("{sourced}    also called from:\n"), |text, site| {
                let caller = site.caller.as_ref().map(|caller| format!("  {caller}")).unwrap_or_default();
                text + &format!("      {}:{}{caller}  {}\n", site.file, site.line, clip(&site.text, 100))
            }),
        };
        CallEntry {
            into_reading_file: target_file(first) == Some(file),
            calls,
            levels: [whole, sourced, head],
        }
    }
}

fn fit_entries(entries: &[CallEntry], budget: usize) -> Option<(String, usize)> {
    let mut chosen = vec![2; entries.len()];
    let mut size: usize = entries.iter().map(|entry| entry.levels[2].len()).sum();
    if size > budget {
        return None;
    }
    let mut other_files_first: Vec<usize> = (0..entries.len()).collect();
    other_files_first.sort_by_key(|&at| entries[at].into_reading_file);
    for (level, order) in [(1, (0..entries.len()).collect()), (0, other_files_first)] {
        for at in order {
            let levels = &entries[at].levels;
            let resized = size - levels[chosen[at]].len() + levels[level].len();
            if resized <= budget {
                size = resized;
                chosen[at] = level;
            }
        }
    }
    let cut = entries
        .iter()
        .zip(&chosen)
        .filter(|(entry, &level)| entry.calls.len() > 1 || entry.levels[level] != entry.levels[0])
        .map(|(entry, _)| entry.calls.len())
        .sum();
    let text = entries.iter().zip(&chosen).map(|(entry, &level)| entry.levels[level].as_str()).collect();
    Some((text, cut))
}

fn by_file(calls: &[Call], file: &str, budget: usize) -> String {
    let mut files = grouped(calls, target_file);
    files.sort_by_key(|calls| target_file(calls[0]).map_or(2, |into| usize::from(into == file)));
    let place = |calls: &[&Call]| match target_file(calls[0]) {
        Some(into) => into.to_string(),
        None => {
            let mut seen = HashSet::new();
            let names: Vec<&str> =
                calls.iter().map(|call| call.method.as_str()).filter(|name| seen.insert(*name)).collect();
            names.join(", ")
        }
    };
    let mut size = 0;
    files
        .iter()
        .map(|calls| format!("  {}  {{calls: {}}}\n", place(calls), calls.len()))
        .take_while(|line| {
            size += line.len();
            size <= budget
        })
        .collect()
}

fn grouped<'a, K: Eq + std::hash::Hash>(calls: &'a [Call], key: impl Fn(&'a Call) -> K) -> Vec<Vec<&'a Call>> {
    let mut groups: Vec<Vec<&Call>> = Vec::new();
    let mut at: HashMap<K, usize> = HashMap::new();
    for call in calls {
        let index = *at.entry(key(call)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[index].push(call);
    }
    groups
}

fn target_key(call: &Call) -> (&str, i64) {
    match &call.target {
        Target::Resolved { file, declared_at, .. } | Target::CallSitesPastBudget { file, declared_at, .. } => {
            (file, *declared_at)
        }
        Target::PastBudget { .. } => (&call.method, 0),
    }
}

fn target_file(call: &Call) -> Option<&str> {
    match &call.target {
        Target::Resolved { file, .. } | Target::CallSitesPastBudget { file, .. } => Some(file),
        Target::PastBudget { .. } => None,
    }
}

fn source_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|source| source.split_inclusive('\n').map(str::to_string).collect())
        .unwrap_or_default()
}

fn excerpt(lines: &[String], start: i64, end: i64) -> String {
    lines
        .iter()
        .skip((start - 1).max(0) as usize)
        .take((end - start + 1).max(0) as usize)
        .map(|line| line.trim_end_matches(['\n', '\r']))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(count: i64, floor: bool) -> FileFacts {
        FileFacts {
            path: "bounded.py".into(),
            language: Some("python".into()),
            loc: 1,
            function_count: 0,
            cyclomatic_complexity_total: 0,
            cyclomatic_complexity_max: 0,
            rank: "low".into(),
            functions: Vec::new(),
            extraction: None,
            last_modified: Some("2023-11-14".into()),
            last_author: None,
            commits_30d: 0,
            first_seen: Some("2023-11-14".into()),
            commit_count: count,
            commit_count_is_floor: floor,
            rename_from: None,
            working_state: None,
            present_in: Vec::new(),
            last_subject: None,
            top_author: None,
            co_changed: Vec::new(),
            mtime_ns: 0,
            size_bytes: 0,
        }
    }

    #[test]
    fn a_capped_history_says_at_least_and_never_an_exact_count() {
        let map = Facts::of(&facts(4000, true), None).to_map();
        assert_eq!(map["git"]["commits_at_least"], 4000);
        assert!(map["git"].get("commits").is_none());
        assert!(map["git"]["first_commit"].as_str().unwrap().starts_with("at least "));
    }

    #[test]
    fn keys_print_in_the_order_the_struct_declares() {
        let text = front_matter(&Facts::of(&facts(2, false), None).to_map());
        let keys: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with(' ') && line.contains(':'))
            .map(|line| line.split(':').next().unwrap())
            .collect();
        assert_eq!(keys, ["file", "lines", "cyclomatic_complexity", "complexity_rank", "git"]);
    }
}
