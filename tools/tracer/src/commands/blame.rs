//! `trace blame` — scoped, symbol-aware blame.
//! Symbol-range resolution is AST-derived via `crate::ccn` (maps a symbol
//! name to its line span). Porcelain parsing, region collapse, and
//! untracked-file synthesis emit per-region commit + subject so no
//! follow-up `git show` is needed.

use crate::{cache, ccn, file_facts};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse_lines(spec: &str) -> Result<(i64, i64)> {
    let (a, b) = spec
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("--lines must be in form L1:L2"))?;
    let start: i64 = a
        .parse()
        .map_err(|_| anyhow::anyhow!("--lines must be integers, e.g. 10:42"))?;
    let end: i64 = b
        .parse()
        .map_err(|_| anyhow::anyhow!("--lines must be integers, e.g. 10:42"))?;
    if start < 1 || end < start {
        bail!("--lines start must be >= 1 and <= end");
    }
    Ok((start, end))
}

/// Resolve a symbol name to its (start_line, end_line) range via the AST CCN
/// backend. Matches an exact function name or a qualified suffix
/// (`Class.method`) — the same matcher shape `trace read` uses. end_line
/// is start_line + nloc - 1 (AST line span).
fn resolve_symbol_range(file: &Path, symbol: &str) -> Option<(i64, i64)> {
    let source = std::fs::read(file).ok()?;
    let functions = ccn::analyze(&source, &file.to_string_lossy())?;
    let suffix = format!(".{symbol}");
    let target = functions
        .iter()
        .find(|f| f.name == symbol || f.name.ends_with(&suffix))?;
    Some((target.start_line, target.start_line + target.nloc - 1))
}

const UNTRACKED_MARKERS: &[&str] = &[
    "no such path",
    "is outside repository",
    "not in a git repository",
];

fn run_blame(file: &Path, line_range: Option<(i64, i64)>) -> Result<String> {
    let mut args: Vec<String> = vec!["blame".into(), "--line-porcelain".into()];
    if let Some((s, e)) = line_range {
        args.push("-L".into());
        args.push(format!("{s},{e}"));
    }
    args.push("--".into());
    args.push(file.to_string_lossy().to_string());
    let parent = file.parent().unwrap_or_else(|| Path::new("."));
    let out = Command::new("git")
        .args(&args)
        .current_dir(parent)
        .output()?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).to_string());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    if UNTRACKED_MARKERS.iter().any(|m| stderr.contains(m)) {
        return Ok(synthesize_untracked_porcelain(file, line_range));
    }
    let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
    bail!(if msg.is_empty() {
        "git blame failed".to_string()
    } else {
        msg
    });
}

fn synthesize_untracked_porcelain(file: &Path, line_range: Option<(i64, i64)>) -> String {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let source_lines: Vec<&str> = text.split('\n').collect();
    // splitlines() drops a trailing newline's empty segment; emulate that.
    let line_count = if text.ends_with('\n') && !source_lines.is_empty() {
        source_lines.len() - 1
    } else {
        source_lines.len()
    };
    let (start, end) = match line_range {
        None => (1i64, line_count as i64),
        Some((s, e)) => (s, e.min(line_count as i64)),
    };
    let zero_sha = "0".repeat(40);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut chunks = String::new();
    for ln in start..=end {
        let content = if ln >= 1 && (ln as usize) <= line_count {
            source_lines[(ln - 1) as usize]
        } else {
            ""
        };
        chunks.push_str(&format!(
            "{zero_sha} {ln} {ln} 1\n\
             author Not Committed Yet\n\
             author-time {now}\n\
             author-tz +0000\n\
             summary (uncommitted)\n\
             \t{content}\n"
        ));
    }
    chunks
}

#[derive(Clone)]
struct BlameLine {
    line: i64,
    sha: String,
    author: String,
    author_time: i64,
    author_tz: String,
    summary: String,
}

/// Parse `git blame --porcelain`. The commit attribute block appears once
/// per sha; later lines carry only header + content, so attributes are
/// memoized.
fn parse_porcelain(output: &str) -> Vec<BlameLine> {
    use std::collections::HashMap;
    let mut commits: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut lines: Vec<BlameLine> = Vec::new();
    let mut current_sha: Option<String> = None;
    let mut current_line: Option<i64> = None;
    let mut pending: HashMap<String, String> = HashMap::new();

    for raw in output.split('\n') {
        if let Some(content) = raw.strip_prefix('\t') {
            let (sha, line) = match (&current_sha, current_line) {
                (Some(s), Some(l)) => (s.clone(), l),
                _ => continue,
            };
            let attrs = commits.entry(sha.clone()).or_default();
            for (k, v) in &pending {
                attrs.entry(k.clone()).or_insert_with(|| v.clone());
            }
            let _ = content;
            lines.push(BlameLine {
                line,
                sha: sha.clone(),
                author: attrs.get("author").cloned().unwrap_or_else(|| "unknown".into()),
                author_time: attrs
                    .get("author-time")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                author_tz: attrs
                    .get("author-tz")
                    .cloned()
                    .unwrap_or_else(|| "+0000".into()),
                summary: attrs.get("summary").cloned().unwrap_or_default(),
            });
            current_sha = None;
            current_line = None;
            pending = HashMap::new();
            continue;
        }
        let parts: Vec<&str> = raw.splitn(4, ' ').collect();
        if current_sha.is_none() && parts.len() >= 3 && parts[0].len() >= 7 {
            current_sha = Some(parts[0].to_string());
            match parts[2].parse::<i64>() {
                Ok(n) => current_line = Some(n),
                Err(_) => {
                    current_sha = None;
                    current_line = None;
                }
            }
            continue;
        }
        if let Some((k, v)) = raw.split_once(' ') {
            pending.insert(k.to_string(), v.to_string());
        }
    }
    lines
}

struct Region {
    line_start: i64,
    line_end: i64,
    sha: String,
    author: String,
    author_time: i64,
    author_tz: String,
    subject: String,
}

fn collapse_regions(lines: &[BlameLine]) -> Vec<Region> {
    let mut regions: Vec<Region> = Vec::new();
    for entry in lines {
        if let Some(last) = regions.last_mut() {
            if last.sha == entry.sha && last.line_end + 1 == entry.line {
                last.line_end = entry.line;
                continue;
            }
        }
        regions.push(Region {
            line_start: entry.line,
            line_end: entry.line,
            sha: entry.sha.clone(),
            author: entry.author.clone(),
            author_time: entry.author_time,
            author_tz: entry.author_tz.clone(),
            subject: entry.summary.clone(),
        });
    }
    regions
}

fn is_uncommitted(sha: &str) -> bool {
    sha.starts_with("00000000")
}

fn short_sha(sha: &str) -> String {
    if is_uncommitted(sha) {
        "uncommitted".into()
    } else {
        sha.chars().take(8).collect()
    }
}

fn subject(region: &Region) -> String {
    if is_uncommitted(&region.sha) {
        return "(uncommitted change)".into();
    }
    if region.subject.is_empty() {
        "(no subject)".into()
    } else {
        region.subject.clone()
    }
}

/// YYYY-MM-DD in UTC from author_time; the tz is informational only.
fn isoformat_date(author_time: i64, _author_tz: &str) -> String {
    if author_time == 0 {
        return "—".into();
    }
    ymd_from_unix(author_time)
}

fn humanize_age(author_time: i64) -> String {
    if author_time == 0 {
        return "uncommitted".into();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let seconds = now - author_time;
    if seconds < 60 {
        return format!("{seconds}s ago");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days}d ago");
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months}mo ago");
    }
    let years = days / 365;
    format!("{years}y ago")
}

/// UTC date YYYY-MM-DD from a unix timestamp (civil calc, same as
/// git_activity::unix_to_ymd which is module-private).
fn ymd_from_unix(secs: i64) -> String {
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

fn format_region(region: &Region) -> String {
    let span = if region.line_start == region.line_end {
        format!("L{}", region.line_start)
    } else {
        format!("L{}-L{}", region.line_start, region.line_end)
    };
    format!(
        "  {:<14} {:<11} {}  {:<10}  {:<22} {}",
        span,
        short_sha(&region.sha),
        isoformat_date(region.author_time, &region.author_tz),
        humanize_age(region.author_time),
        region.author,
        subject(region),
    )
}

pub fn run(
    file: &Path,
    symbol: Option<&str>,
    lines_spec: Option<&str>,
    as_json: bool,
) -> Result<Value> {
    if symbol.is_some() && lines_spec.is_some() {
        bail!("SYMBOL and --lines are mutually exclusive");
    }
    if !file.is_file() {
        bail!("file not found: {}", file.display());
    }
    let path = file.canonicalize().unwrap_or_else(|_| cache::absolutize(file));

    let mut line_range: Option<(i64, i64)> = None;
    let mut scope = "file";
    if let Some(sym) = symbol {
        match resolve_symbol_range(&path, sym) {
            Some(r) => {
                line_range = Some(r);
                scope = "symbol";
            }
            None => bail!("symbol '{}' not found in {}", sym, file.display()),
        }
    } else if let Some(spec) = lines_spec {
        line_range = Some(parse_lines(spec)?);
        scope = "lines";
    }

    let porcelain = run_blame(&path, line_range)?;
    let blame_lines = parse_porcelain(&porcelain);
    let regions = collapse_regions(&blame_lines);

    let repo_root = cache::worktree_root_for(&path).unwrap_or_else(|| cache::display_root(&path));
    let facts = file_facts::get(&path, &repo_root, None);
    let display_file = cache::relative_to_root(&path, &repo_root);

    let payload = json!({
        "file": display_file,
        "language": facts.as_ref().and_then(|f| f.language.clone()),
        "scope": scope,
        "symbol": symbol,
        "line_range": line_range.map(|(s, e)| json!({"start": s, "end": e})),
        "regions": regions.iter().map(|r| json!({
            "line_start": r.line_start,
            "line_end": r.line_end,
            "sha": short_sha(&r.sha),
            "author": r.author,
            "date": isoformat_date(r.author_time, &r.author_tz),
            "age": humanize_age(r.author_time),
            "subject": subject(r),
        })).collect::<Vec<_>>(),
        "region_count": regions.len(),
        "line_count": blame_lines.len(),
    });

    if as_json {
        return Ok(payload);
    }

    let mut header = format!("# {display_file}");
    if scope == "symbol" {
        let (s, e) = line_range.unwrap();
        header += &format!(" :: {}  (L{}-L{})", symbol.unwrap(), s, e);
    } else if scope == "lines" {
        let (s, e) = line_range.unwrap();
        header += &format!("  L{s}-L{e}");
    }
    println!("{header}");
    println!(
        "Regions: {}  Lines blamed: {}",
        regions.len(),
        blame_lines.len()
    );
    println!();
    println!(
        "  {:<14} {:<11} {:<10}  {:<10}  {:<22} subject",
        "lines", "commit", "date", "age", "author"
    );
    for region in &regions {
        println!("{}", format_region(region));
    }
    Ok(payload)
}
