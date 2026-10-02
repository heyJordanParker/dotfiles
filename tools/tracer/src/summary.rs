//! A file's facts — the one structure every command shows about a file.
//!
//! `Facts` is built from the cached `FileFacts` and the graph around the file,
//! printed as YAML front matter through `yamlfmt`, and carried under the same
//! keys in `--json`. Keys use the words git, GitHub, and editors use; values
//! are counts, names, and ages, never ratios. `headline` is the one-line form
//! a list of many files shows.

use crate::commands::session_log;
use crate::file_facts::FileFacts;
use crate::git_activity::GitActivity;
use crate::relations::{self, ModuleCounts};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Serialize)]
pub struct Call {
    pub line: i64,
    pub method: String,
    pub file: String,
    pub declared_at: i64,
    pub callers: usize,
    pub source: String,
    pub also_called_from: Vec<CallSite>,
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
    let mut sources: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut out = Vec::new();
    for (name, lines) in called {
        let sites = relations::use_sites(name, repo_root);
        for line in lines {
            let here: Vec<&relations::UseSite> =
                sites.iter().filter(|site| site.file == relative && site.line == line).collect();
            let [site] = here.as_slice() else {
                continue;
            };
            if site.confidence == "AMBIGUOUS" || site.target.kind != "function" {
                continue;
            }
            let target = &site.target;
            let mut callers: Vec<&relations::UseSite> = sites
                .iter()
                .filter(|other| other.target_file == site.target_file && other.target.line == target.line)
                .collect();
            callers.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            let target_source = sources
                .entry(site.target_file.clone())
                .or_insert_with(|| source_lines(&repo_root.join(&site.target_file)));
            let span = target.end_line - target.header_line + 1;
            let read = session_log::has_read(
                &repo_root.join(&site.target_file),
                &session_log::content_hash(&target_source.concat()),
                target.header_line as usize,
                target.end_line as usize,
            );
            let source = if read || span > CALL_BODY_LINES {
                target.header.clone()
            } else {
                excerpt(target_source, target.header_line, target.end_line)
            };
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
                method: match &target.container {
                    Some(container) => format!("{container}::{}", target.name),
                    None => target.name.clone(),
                },
                file: site.target_file.clone(),
                declared_at: target.line,
                callers: callers.len(),
                source,
                also_called_from,
            });
        }
    }
    out.sort_by_key(|call| call.line);
    out
}

/// The calls block in at most `budget` characters with every call still
/// named: each call starts as its head line, then calls get their source back
/// in line order, then their other call sites, while the budget holds — the
/// way `surface::render_within` grows rows. `None` renders every call whole.
/// A function the window calls on several lines shows its source and other
/// call sites once, under its first line.
pub fn render_calls(calls: &[Call], budget: Option<usize>) -> String {
    if calls.is_empty() {
        return String::new();
    }
    let mut seen = BTreeSet::new();
    let first: Vec<bool> = calls.iter().map(|call| seen.insert((&call.file, call.declared_at))).collect();
    let heads: Vec<String> = calls
        .iter()
        .map(|call| {
            format!(
                "  L{} {}  {}:{}  {{callers: {}}}\n",
                call.line, call.method, call.file, call.declared_at, call.callers
            )
        })
        .collect();
    let sourced: Vec<String> = calls
        .iter()
        .zip(&heads)
        .zip(&first)
        .map(|((call, head), &first)| match first {
            true => call.source.lines().fold(head.clone(), |text, line| text + &format!("    {line}\n")),
            false => head.clone(),
        })
        .collect();
    let whole: Vec<String> = calls
        .iter()
        .zip(&sourced)
        .zip(&first)
        .map(|((call, sourced), &first)| {
            if !first || call.also_called_from.is_empty() {
                return sourced.clone();
            }
            call.also_called_from
                .iter()
                .fold(format!("{sourced}    also called from:\n"), |text, site| {
                    let caller = site.caller.as_ref().map(|caller| format!("  {caller}")).unwrap_or_default();
                    text + &format!("      {}:{}{caller}  {}\n", site.file, site.line, clip(&site.text, 100))
                })
        })
        .collect();
    let Some(budget) = budget.map(|budget| budget.saturating_sub("calls:\n".len())) else {
        return format!("calls:\n{}", whole.concat());
    };
    if whole.iter().map(String::len).sum::<usize>() <= budget {
        return format!("calls:\n{}", whole.concat());
    }
    let shortened = crate::output::shortened_line(calls.len(), calls.len(), "calls");
    let budget = budget.saturating_sub(shortened.len() + 3);
    let mut texts = heads;
    let mut size: usize = texts.iter().map(String::len).sum();
    for level in [&sourced, &whole] {
        for (text, grown) in texts.iter_mut().zip(level) {
            let resized = size - text.len() + grown.len();
            if resized <= budget {
                size = resized;
                *text = grown.clone();
            }
        }
    }
    let cut = texts.iter().zip(&whole).filter(|(text, whole)| text != whole).count();
    format!(
        "calls:\n{}  {}\n",
        texts.concat(),
        crate::output::shortened_line(cut, calls.len(), "calls")
    )
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
