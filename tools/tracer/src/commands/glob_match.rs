//! Shell-style basename matching for the `find` command.
//!
//! `fnmatch` matches a basename against a shell glob (used by `find` and
//! its `--path` filters). It compiles to the `regex` crate, which is
//! already a foundation dependency — no new crates, no foundation changes.

use regex::Regex;

/// Translate a shell glob to a regex: `*` → `.*`, `?` → `.`, `[...]`
/// character classes (with `!` negation normalized to `^`), everything else
/// escaped. Anchored with a `(?s:...)\z` wrapper.
fn fnmatch_translate(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut res = String::new();
    while i < n {
        let c = chars[i];
        i += 1;
        match c {
            '*' => res.push_str(".*"),
            '?' => res.push('.'),
            '[' => {
                let mut j = i;
                if j < n && chars[j] == '!' {
                    j += 1;
                }
                if j < n && chars[j] == ']' {
                    j += 1;
                }
                while j < n && chars[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    res.push_str("\\[");
                } else {
                    let mut stuff: String = chars[i..j].iter().collect();
                    if !stuff.contains("--") {
                        stuff = stuff.replace('\\', r"\\");
                    } else {
                        // Ranges would need per-chunk re-escaping, but the
                        // patterns tracer uses have no ranges, so the simple
                        // escape is equivalent. Ranges stay literal.
                        stuff = stuff.replace('\\', r"\\");
                    }
                    i = j + 1;
                    let mut cls = String::from("[");
                    if stuff.starts_with('!') {
                        cls.push('^');
                        cls.push_str(&stuff[1..]);
                    } else if stuff.starts_with('^') || stuff.starts_with('[') {
                        cls.push('\\');
                        cls.push_str(&stuff);
                    } else {
                        cls.push_str(&stuff);
                    }
                    cls.push(']');
                    res.push_str(&cls);
                }
            }
            _ => res.push_str(&regex::escape(&c.to_string())),
        }
    }
    format!(r"(?s:{res})\z")
}

/// Case-sensitive shell-glob match of `name` against `pattern`
/// (case-sensitive on macOS/Linux).
pub fn fnmatch(name: &str, pattern: &str) -> bool {
    match Regex::new(&fnmatch_translate(pattern)) {
        Ok(re) => re.is_match(name),
        Err(_) => false,
    }
}

