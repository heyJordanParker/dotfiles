//! Passive architectural-context shoulder. Pure functions, no I/O. The
//! one-line lifecycle/complexity shoulder rendered for any FileFacts
//! (U+2026 ellipsis, fixed age-bucket thresholds).

use crate::file_facts::FileFacts;
use serde_json::Value;

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

/// Age shoulder from `last_modified`, bucketed (today/Nd/Nw/Nmo/NyNmo).
fn age(facts: &FileFacts) -> Option<String> {
    let lm = facts.last_modified.as_ref()?;
    let days = days_since(lm)?;
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

/// The full one-line passive-context shoulder. `graph` is the optional
/// {"callers": int, "depended_on_by_modules": int} map.
pub fn render(facts: &FileFacts, graph: Option<&Value>) -> String {
    let state = state_label(facts);
    let complexity = format!(
        "ccn: {} {}",
        facts.cyclomatic_complexity_total, facts.rank
    );
    let mut parts: Vec<String> = vec![format!("git: {state}")];
    if let Some(a) = age(facts) {
        parts.push(format!("age: {a}"));
    }
    if !facts.present_in.is_empty() {
        parts.push(format!("presence: {}", facts.present_in.join(", ")));
    } else {
        parts.push("presence: local-only".into());
    }
    if let Some(g) = graph {
        let callers = g.get("callers").and_then(|x| x.as_i64()).unwrap_or(0);
        let dep = g
            .get("depended_on_by_modules")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        parts.push(format!("callers: {callers} · dependents: {dep}"));
    }
    parts.push(complexity);
    if let Some(ta) = &facts.top_author {
        parts.push(format!("owner: {ta}"));
    }
    if let Some(ls) = &facts.last_subject {
        parts.push(format!("last: {}", clip_subject(ls, 60)));
    }
    format!("[{}]", parts.join(" · "))
}

/// The compact shoulder. Format: `<state> · <age>`.
pub fn render_compact(facts: &FileFacts) -> String {
    let state = state_label(facts);
    let a = age(facts).unwrap_or_else(|| "—".to_string());
    format!("{state} · {a}")
}
