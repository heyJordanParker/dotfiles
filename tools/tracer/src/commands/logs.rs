//! `trace logs` — timestamped entries from log files.
//!
//! Reads the named files directly, so a gitignored or untracked log is
//! searchable: `rg`'s ignore walk (the `grep` backend) skips them, and
//! `read` loads a whole file, which a rotated 80 MB directory cannot pay.
//! One line is one entry; a line carrying no timestamp of its own attaches
//! to the entry above it, which is what keeps a stack trace whole.

use crate::commands::glob_match::fnmatch;
use anyhow::Result;
use flate2::read::GzDecoder;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

type Stamp = (i32, u32, u32, u32, u32, u32);
type Date = (i32, u32, u32);

/// A timestamp only has to sit in the head of a line: an access log writes
/// the client address and the request method before its date.
const HEAD: usize = 64;

/// How far into a file to look for a framing timestamp before deciding the
/// file has none and every line is its own entry.
const FRAME_PROBE: usize = 200;

pub struct Entry {
    file: String,
    line: usize,
    stamp: Option<Stamp>,
    text: String,
}

fn iso_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(\d{4})[-/](\d{2})[-/](\d{2})[T ](\d{2}):(\d{2}):(\d{2})").unwrap()
    })
}

/// Common Log Format, as nginx and Apache access logs write it:
/// `10.0.0.1 - - [15/Aug/2026:10:52:01 +0000] "GET / HTTP/1.1"`.
fn clf_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d{2})/([A-Za-z]{3})/(\d{4}):(\d{2}):(\d{2}):(\d{2})").unwrap())
}

/// PHP's `error_log`, which is what a WordPress `debug.log` carries:
/// `[06-Jun-2026 12:52:27 UTC] WordPress database error ...`.
fn php_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(\d{1,2})-([A-Za-z]{3})-(\d{4})[ :](\d{2}):(\d{2}):(\d{2})").unwrap()
    })
}

/// Syslog's stamp carries no year, and is only trusted at line start —
/// unanchored it matches ordinary prose inside a message body.
fn syslog_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Za-z]{3})\s+(\d{1,2})\s+(\d{2}):(\d{2}):(\d{2})").unwrap())
}

fn name_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|[^0-9])(\d{4})-?(\d{2})-?(\d{2})(?:[^0-9]|$)").unwrap())
}

fn window_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(\d{4})-(\d{2})-(\d{2}))?\s*(?:(\d{1,2}):(\d{2})(?::(\d{2}))?)?$").unwrap()
    })
}

fn month_num(name: &str) -> Option<u32> {
    let months = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let lower = name.to_lowercase();
    months
        .iter()
        .position(|m| *m == lower)
        .map(|i| i as u32 + 1)
}

fn valid(m: u32, d: u32) -> bool {
    (1..=12).contains(&m) && (1..=31).contains(&d)
}

/// The head of a line, cut on a character boundary.
fn head(line: &str) -> &str {
    match line.char_indices().nth(HEAD) {
        Some((i, _)) => &line[..i],
        None => line,
    }
}

/// The timestamp a line carries. `year` fills in for syslog, whose stamp
/// has no year of its own.
fn stamp_in(text: &str, year: i32) -> Option<Stamp> {
    if let Some(c) = iso_re().captures(text) {
        let n = |i: usize| c[i].parse().unwrap_or(0);
        let (m, d) = (n(2), n(3));
        if valid(m, d) {
            return Some((c[1].parse().unwrap_or(0), m, d, n(4), n(5), n(6)));
        }
    }
    // Common Log Format and PHP's error_log write the same day-month-year
    // fields in the same capture order, so one reader serves both.
    for re in [clf_re(), php_re()] {
        if let Some(c) = re.captures(text) {
            let n = |i: usize| c[i].parse().unwrap_or(0);
            if let Some(m) = month_num(&c[2]) {
                let d = n(1);
                if valid(m, d) {
                    return Some((c[3].parse().unwrap_or(0), m, d, n(4), n(5), n(6)));
                }
            }
        }
    }
    if let Some(c) = syslog_re().captures(text) {
        let n = |i: usize| c[i].parse().unwrap_or(0);
        if let Some(m) = month_num(&c[1]) {
            let d = n(2);
            if valid(m, d) {
                return Some((year, m, d, n(3), n(4), n(5)));
            }
        }
    }
    None
}

/// Whether a line opens an entry, and the timestamp it opens with. A JSON
/// object opens one whatever field order it uses, so a JSON-lines log
/// frames one record per line; its timestamp is read from the whole line
/// rather than the head, because the field can sit anywhere in the object.
fn opens(line: &str, year: i32) -> Option<Option<Stamp>> {
    if let Some(s) = stamp_in(head(line), year) {
        return Some(Some(s));
    }
    if line.trim_start().starts_with('{') {
        return Some(stamp_in(line, year));
    }
    None
}

fn stamp_str(s: &Stamp) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        s.0, s.1, s.2, s.3, s.4, s.5
    )
}

/// Civil date from a Unix timestamp, days-from-civil inverted. This is what
/// supplies the year syslog omits and orders a log whose name carries no
/// date, so neither needs a date crate.
fn civil_from_epoch(secs: i64) -> Date {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

fn mtime_secs(path: &Path) -> i64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn mtime_date(path: &Path) -> Date {
    civil_from_epoch(mtime_secs(path))
}

/// The rotation date in a filename: `laravel-2026-08-15.log`,
/// `app.2026-08-15.log`, and `app-20260815.log` all resolve.
fn name_date(name: &str) -> Option<Date> {
    let c = name_date_re().captures(name)?;
    let y: i32 = c[1].parse().ok()?;
    let m: u32 = c[2].parse().ok()?;
    let d: u32 = c[3].parse().ok()?;
    if valid(m, d) {
        Some((y, m, d))
    } else {
        None
    }
}

struct LogFile {
    path: PathBuf,
    date: Option<Date>,
    /// Recency: the rotation date in the name, then the last write. Two
    /// logs of the same day are separated by which was written last.
    order: (Date, i64),
}

fn describe(path: PathBuf) -> LogFile {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let date = name_date(&name);
    let written = mtime_secs(&path);
    let order = (date.unwrap_or_else(|| civil_from_epoch(written)), written);
    LogFile { path, date, order }
}

/// Log files under `path`, newest first. Ignore rules are never consulted:
/// a log is gitignored in every project that has one.
fn select(path: &str, file_glob: &str) -> Vec<LogFile> {
    let root = Path::new(path);
    if root.is_file() {
        return vec![describe(root.to_path_buf())];
    }
    if !root.exists() {
        eprintln!("Error: path not found: {path}");
        std::process::exit(2);
    }
    let mut found: Vec<LogFile> = Vec::new();
    let walk = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        // A dependency or object directory holds no log anyone asks about,
        // and walking one costs more than reading every log in the project.
        .filter_entry(|e| {
            e.depth() == 0
                || !e.file_type().is_dir()
                || !matches!(
                    e.file_name().to_string_lossy().as_ref(),
                    ".git" | "node_modules" | "vendor" | ".tracer-cache"
                )
        });
    for entry in walk.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !fnmatch(&name, file_glob) {
            continue;
        }
        found.push(describe(entry.path().to_path_buf()));
    }
    // Newest first, and reverse-path within one day, so the render — which
    // reverses this order — reads oldest first and path-ascending.
    found.sort_by(|a, b| b.order.cmp(&a.order).then(b.path.cmp(&a.path)));
    found
}

fn reader(path: &Path) -> Option<Box<dyn BufRead>> {
    let file = File::open(path).ok()?;
    if path.extension().and_then(|e| e.to_str()) == Some("gz") {
        Some(Box::new(BufReader::new(GzDecoder::new(file))))
    } else {
        Some(Box::new(BufReader::new(file)))
    }
}

/// One line as text. Invalid UTF-8 comes back lossy, so a log with a
/// mangled byte stays readable instead of failing the file.
///
/// NUL bytes are dropped rather than counted as a binary marker: a real
/// `debug.log` truncated by a concurrent writer opens with a run of NULs
/// millions of bytes long, and treating that as binary loses the file. The
/// run is discarded as it streams, so it costs no memory either.
fn next_line(source: &mut dyn BufRead) -> Option<String> {
    let mut out: Vec<u8> = Vec::new();
    loop {
        let (found, used) = {
            let buf = match source.fill_buf() {
                Ok(b) => b,
                Err(_) => return None,
            };
            if buf.is_empty() {
                return if out.is_empty() {
                    None
                } else {
                    Some(finish(out))
                };
            }
            match buf.iter().position(|b| *b == b'\n') {
                Some(i) => {
                    out.extend(buf[..i].iter().filter(|b| **b != 0));
                    (true, i + 1)
                }
                None => {
                    out.extend(buf.iter().filter(|b| **b != 0));
                    (false, buf.len())
                }
            }
        };
        source.consume(used);
        if found {
            return Some(finish(out));
        }
    }
}

fn finish(mut raw: Vec<u8>) -> String {
    while raw.last() == Some(&b'\r') {
        raw.pop();
    }
    String::from_utf8_lossy(&raw).into_owned()
}

/// Whether any line in the head of the file opens an entry. A file with no
/// timestamp anywhere is read one entry per line, so a build log or a bare
/// text log still answers a query.
fn framed(path: &Path, year: i32) -> bool {
    let mut source = match reader(path) {
        Some(r) => r,
        None => return false,
    };
    for _ in 0..FRAME_PROBE {
        match next_line(&mut *source) {
            Some(line) => {
                if opens(&line, year).is_some() {
                    return true;
                }
            }
            None => break,
        }
    }
    false
}

/// Entries of one file, in file order.
struct Entries {
    source: Box<dyn BufRead>,
    file: String,
    frames: bool,
    year: i32,
    line_no: usize,
    pending: Option<Entry>,
    done: bool,
}

impl Iterator for Entries {
    type Item = Entry;

    fn next(&mut self) -> Option<Entry> {
        loop {
            if self.done {
                return self.pending.take();
            }
            let line = match next_line(&mut *self.source) {
                Some(l) => l,
                None => {
                    self.done = true;
                    return self.pending.take();
                }
            };
            self.line_no += 1;
            let opened = if self.frames {
                opens(&line, self.year)
            } else {
                Some(None)
            };
            match opened {
                Some(stamp) => {
                    if let Some(s) = stamp {
                        self.year = s.0;
                    }
                    let entry = Entry {
                        file: self.file.clone(),
                        line: self.line_no,
                        stamp,
                        text: line,
                    };
                    if let Some(previous) = self.pending.replace(entry) {
                        return Some(previous);
                    }
                }
                None => match self.pending.as_mut() {
                    Some(entry) => {
                        entry.text.push('\n');
                        entry.text.push_str(&line);
                    }
                    None => {
                        self.pending = Some(Entry {
                            file: self.file.clone(),
                            line: self.line_no,
                            stamp: None,
                            text: line,
                        });
                    }
                },
            }
        }
    }
}

fn entries_of(log: &LogFile) -> Option<Entries> {
    let year = log
        .date
        .map(|d| d.0)
        .unwrap_or_else(|| mtime_date(&log.path).0);
    let frames = framed(&log.path, year);
    Some(Entries {
        source: reader(&log.path)?,
        file: display_path(&log.path),
        frames,
        year,
        line_no: 0,
        pending: None,
        done: false,
    })
}

fn window_holds(entry: &Entry, since: Option<Stamp>, until: Option<Stamp>) -> bool {
    let stamp = match entry.stamp {
        Some(s) => s,
        // An entry with no timestamp of its own cannot be excluded without
        // dropping the head of a file whose first lines carry none.
        None => return true,
    };
    if since.is_some_and(|s| stamp < s) {
        return false;
    }
    if until.is_some_and(|u| stamp > u) {
        return false;
    }
    true
}

fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    match path.strip_prefix(&cwd) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// A lower-case pattern matches any case; an upper-case letter makes the
/// match exact. This is ripgrep's smart case, which serves both a level
/// like `ERROR` and a message fragment.
///
/// A pattern that is not valid regex is taken literally rather than
/// refused, the same fallback `read::compile_anchor` makes: an unbalanced
/// bracket in a log query is a fragment of a message far more often than a
/// mistyped expression.
fn compile(pattern: &str) -> Regex {
    let smart = if pattern.chars().any(|c| c.is_uppercase()) {
        pattern.to_string()
    } else {
        format!("(?i){pattern}")
    };
    Regex::new(&smart).unwrap_or_else(|_| Regex::new(&regex::escape(pattern)).unwrap())
}

struct Hit {
    is_match: bool,
    entry: Entry,
}

/// Drop the oldest matches until only `limit` remain, so the cap keeps the
/// newest entries of the file.
fn trim(out: &mut VecDeque<Hit>, matches: &mut usize, limit: usize) {
    while *matches > limit {
        while let Some(h) = out.pop_front() {
            if h.is_match {
                *matches -= 1;
                break;
            }
        }
    }
}

/// Matching entries of one file with `around` whole entries either side.
/// Only the context window and the capped result are held, so a file of any
/// size costs the window.
fn scan(
    log: &LogFile,
    pattern: Option<&Regex>,
    since: Option<Stamp>,
    until: Option<Stamp>,
    around: usize,
    limit: usize,
) -> VecDeque<Hit> {
    let mut out: VecDeque<Hit> = VecDeque::new();
    let mut before: VecDeque<Hit> = VecDeque::new();
    let mut after_left = 0usize;
    let mut matches = 0usize;
    let source = match entries_of(log) {
        Some(s) => s,
        None => return out,
    };
    for entry in source {
        if !window_holds(&entry, since, until) {
            continue;
        }
        let is_match = pattern.map(|re| re.is_match(&entry.text)).unwrap_or(true);
        let hit = Hit { is_match, entry };
        if is_match {
            matches += 1;
            while let Some(b) = before.pop_front() {
                out.push_back(b);
            }
            out.push_back(hit);
            after_left = around;
        } else if after_left > 0 {
            after_left -= 1;
            out.push_back(hit);
        } else if around > 0 {
            before.push_back(hit);
            if before.len() > around {
                before.pop_front();
            }
        }
        trim(&mut out, &mut matches, limit);
    }
    out
}

fn parse_window(value: &str, anchor: Date, end: bool) -> Option<Stamp> {
    let c = window_re().captures(value.trim())?;
    let has_date = c.get(1).is_some();
    let has_time = c.get(4).is_some();
    if !has_date && !has_time {
        return None;
    }
    let (y, m, d) = if has_date {
        (c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?)
    } else {
        anchor
    };
    if !valid(m, d) {
        return None;
    }
    let (hh, mm, ss) = if has_time {
        let seconds = match c.get(6) {
            Some(s) => s.as_str().parse().ok()?,
            None if end => 59,
            None => 0,
        };
        (c[4].parse().ok()?, c[5].parse().ok()?, seconds)
    } else if end {
        (23, 59, 59)
    } else {
        (0, 0, 0)
    };
    Some((y, m, d, hh, mm, ss))
}

fn bad_window(value: &str) -> ! {
    eprintln!("Error: expected YYYY-MM-DD, 'YYYY-MM-DD HH:MM[:SS]', or HH:MM[:SS], got '{value}'");
    std::process::exit(2);
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: Option<&str>,
    path: &str,
    file_glob: &str,
    since: Option<&str>,
    until: Option<&str>,
    around: usize,
    limit: usize,
    as_json: bool,
) -> Result<Value> {
    let mut files = select(path, file_glob);
    // A bare `HH:MM` means that time on the day of the newest log selected,
    // so a window needs no clock and no timezone: both sides of every
    // comparison are the wall clock the log itself wrote.
    let anchor = files
        .first()
        .map(|f| f.order.0)
        .unwrap_or_else(|| civil_from_epoch(0));

    let since_stamp =
        since.map(|v| parse_window(v, anchor, false).unwrap_or_else(|| bad_window(v)));
    let until_stamp = until.map(|v| parse_window(v, anchor, true).unwrap_or_else(|| bad_window(v)));

    // A dated filename is the cheapest window filter there is: a day
    // outside the window is never opened.
    files.retain(|f| match f.date {
        None => true,
        Some(d) => {
            since_stamp.map(|s| d >= (s.0, s.1, s.2)).unwrap_or(true)
                && until_stamp.map(|u| d <= (u.0, u.1, u.2)).unwrap_or(true)
        }
    });

    let re = pattern.map(compile);

    let mut scanned = 0usize;
    let mut per_file: Vec<VecDeque<Hit>> = Vec::new();
    let mut total = 0usize;
    for log in &files {
        if total >= limit {
            break;
        }
        scanned += 1;
        let hits = scan(
            log,
            re.as_ref(),
            since_stamp,
            until_stamp,
            around,
            limit - total,
        );
        total += hits.iter().filter(|h| h.is_match).count();
        if !hits.is_empty() {
            per_file.push(hits);
        }
    }

    // Files are scanned newest first so the cap keeps the newest entries.
    // They render oldest first, the order a log is read in.
    per_file.reverse();
    let entries: Vec<&Entry> = per_file.iter().flatten().map(|h| &h.entry).collect();
    let files_matched = per_file.len();

    let payload = json!({
        "query": pattern,
        "path": path,
        "file_glob": file_glob,
        "since": since_stamp.as_ref().map(stamp_str),
        "until": until_stamp.as_ref().map(stamp_str),
        "entries": entries.iter().map(|e| json!({
            "file": e.file,
            "line": e.line,
            "stamp": e.stamp.as_ref().map(stamp_str),
            "text": e.text,
        })).collect::<Vec<_>>(),
        "entry_count": entries.len(),
        "files_matched": files_matched,
        "files_scanned": scanned,
    });

    if !as_json {
        render_human(&entries, files_matched, scanned, since_stamp, until_stamp);
    }
    Ok(payload)
}

fn render_human(
    entries: &[&Entry],
    files_matched: usize,
    scanned: usize,
    since: Option<Stamp>,
    until: Option<Stamp>,
) {
    if entries.is_empty() {
        println!("(no entries)");
    }
    let mut current: Option<&str> = None;
    for e in entries {
        if current != Some(e.file.as_str()) {
            println!();
            println!("{}", e.file);
            current = Some(e.file.as_str());
        }
        let mut lines = e.text.split('\n');
        if let Some(first) = lines.next() {
            println!("  L{:<5} {}", e.line, first);
        }
        for rest in lines {
            println!("         {rest}");
        }
    }
    println!();
    let window = match (since, until) {
        (None, None) => String::new(),
        (s, u) => format!(
            " window={} → {}",
            s.as_ref().map(stamp_str).unwrap_or_else(|| "-".into()),
            u.as_ref().map(stamp_str).unwrap_or_else(|| "-".into())
        ),
    };
    println!(
        "entries={} files={} scanned={}{}",
        entries.len(),
        files_matched,
        scanned,
        window
    );
}
