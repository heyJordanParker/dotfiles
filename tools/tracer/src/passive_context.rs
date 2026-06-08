//! Passive architectural-context shoulder. Pure functions, no I/O. The
//! one canonical lifecycle/complexity shoulder rendered for any FileFacts
//! (U+2026 ellipsis, fixed age-bucket thresholds).
//!
//! ONE field set, ONE source function (`parts`). `render` joins the full
//! field set into the bracketed one-liner everywhere a file appears;
//! `render_compact` is a density variant that selects the headline fields
//! from the SAME builder — never a parallel hand-written format. The shoulder
//! carries the complete repo-state picture: lifecycle, both ages
//! (created → modified), churn (total + 30-day velocity), changed-together,
//! deploy-branch presence, complexity, owner, last subject, and (when the
//! caller supplies the graph counts) callers + dependents.

use crate::file_facts::FileFacts;
use serde_json::Value;
use std::path::Path;

/// today - last_modified, as days. Civil-day arithmetic on UTC dates.
fn days_since(last_modified: &str) -> Option<i64> {
    let parts: Vec<&str> = last_modified.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    let last = days_from_civil(y, m, d);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    let today = now.div_euclid(86400);
    Some(today - last)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// A single date string bucketed to an age token (today/Nd/Nw/Nmo/NyNmo).
fn age_token(date: &str) -> Option<String> {
    let days = days_since(date)?;
    if days < 0 {
        return None;
    }
    Some(if days == 0 {
        "today".to_string()
    } else if days == 1 {
        "1d".to_string()
    } else if days < 14 {
        format!("{days}d")
    } else if days < 60 {
        format!("{}w", days / 7)
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        let years = days / 365;
        let months = (days % 365) / 30;
        if months != 0 {
            format!("{years}y{months}mo")
        } else {
            format!("{years}y")
        }
    })
}

/// Age shoulder spanning the file's lifetime: `<first-seen>→<last-modified>`.
/// Collapses to a single token when the file was first seen and last touched
/// in the same age bucket (or when `first_seen` is unknown), so a brand-new
/// file reads `today` rather than `today→today`.
fn age(facts: &FileFacts) -> Option<String> {
    let modified = facts.last_modified.as_ref().and_then(|m| age_token(m))?;
    match facts.first_seen.as_ref().and_then(|c| age_token(c)) {
        Some(created) if created != modified => Some(format!("{created}\u{2192}{modified}")),
        _ => Some(modified),
    }
}

/// Lifecycle label: working-tree state if dirty, else commit-count /
/// rename-derived history state.
fn state_label(facts: &FileFacts) -> String {
    match facts.working_state.as_deref() {
        Some("untracked") => return "untracked".into(),
        Some("added") => return "added (uncommitted)".into(),
        Some("renamed") => return "renamed (uncommitted)".into(),
        Some("modified") => {
            return if facts.commit_count <= 1 {
                "modified (new file)".into()
            } else {
                format!("modified ({} commits)", facts.commit_count)
            };
        }
        _ => {}
    }
    if let Some(rf) = &facts.rename_from {
        return format!("renamed-from {rf}");
    }
    if facts.commit_count == 0 {
        "no-history".into()
    } else if facts.commit_count == 1 {
        "new (1 commit)".into()
    } else {
        format!("{} commits", facts.commit_count)
    }
}

/// Clip a commit subject to `max_chars`, appending an ellipsis when cut.
fn clip_subject(subject: &str, max_chars: usize) -> String {
    let count = subject.chars().count();
    if count <= max_chars {
        return subject.to_string();
    }
    let truncated: String = subject.chars().take(max_chars - 1).collect();
    format!("{}\u{2026}", truncated.trim_end())
}

/// Churn shoulder: total commits plus recent velocity (commits in the last
/// 30 days). `state_label` carries the commit count only on some lifecycle
/// states and never the velocity, so this is the one field where both the
/// lifetime total and the recent rate always appear together.
fn churn(facts: &FileFacts) -> String {
    let total = facts.commit_count;
    let unit = if total == 1 { "commit" } else { "commits" };
    format!("churn: {total} {unit}, {}/30d", facts.commits_30d)
}

/// Changed-together shoulder: the basenames of the top files that co-change
/// with this one. `co_changed` is already ranked by co-change count, so the
/// top `max` entries are the strongest couplings. Empty when the file has no
/// recorded co-change history.
fn changed_together(facts: &FileFacts, max: usize) -> Option<String> {
    if facts.co_changed.is_empty() {
        return None;
    }
    let names: Vec<String> = facts
        .co_changed
        .iter()
        .take(max)
        .map(|(path, _)| {
            Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone())
        })
        .collect();
    Some(format!("together: {}", names.join(", ")))
}

/// The single source of truth for the shoulder's field set. `graph` is the
/// optional {"callers": int, "depended_on_by_modules": int} map. When
/// `dense` is false the full field set is built; when true only the headline
/// density variant (state · age · churn · ccn) is built — same builder, same
/// vocabulary, never a parallel format.
fn parts(facts: &FileFacts, graph: Option<&Value>, dense: bool) -> Vec<String> {
    let mut parts: Vec<String> = vec![format!("git: {}", state_label(facts))];
    if let Some(a) = age(facts) {
        parts.push(format!("age: {a}"));
    }
    if dense {
        parts.push(churn(facts));
        parts.push(format!(
            "ccn: {} {}",
            facts.cyclomatic_complexity_total, facts.rank
        ));
        return parts;
    }
    if !facts.present_in.is_empty() {
        parts.push(format!("presence: {}", facts.present_in.join(", ")));
    } else {
        parts.push("presence: local-only".into());
    }
    parts.push(churn(facts));
    if let Some(g) = graph {
        let callers = g.get("callers").and_then(|x| x.as_i64()).unwrap_or(0);
        let dep = g
            .get("depended_on_by_modules")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        parts.push(format!("callers: {callers} · dependents: {dep}"));
    }
    parts.push(format!(
        "ccn: {} {}",
        facts.cyclomatic_complexity_total, facts.rank
    ));
    if let Some(t) = changed_together(facts, 3) {
        parts.push(t);
    }
    if let Some(ta) = &facts.top_author {
        parts.push(format!("owner: {ta}"));
    }
    if let Some(ls) = &facts.last_subject {
        parts.push(format!("last: {}", clip_subject(ls, 60)));
    }
    parts
}

/// The canonical one-line passive-context shoulder carrying the complete
/// repo-state picture. `graph` is the optional {"callers": int,
/// "depended_on_by_modules": int} map.
pub fn render(facts: &FileFacts, graph: Option<&Value>) -> String {
    format!("[{}]", parts(facts, graph, false).join(" · "))
}

/// The dense density variant — the headline fields (state · age · churn ·
/// ccn) from the SAME field builder as `render`, for listings where one file
/// is one line among many. Bracketed identically so the two read alike.
pub fn render_compact(facts: &FileFacts) -> String {
    format!("[{}]", parts(facts, None, true).join(" · "))
}
