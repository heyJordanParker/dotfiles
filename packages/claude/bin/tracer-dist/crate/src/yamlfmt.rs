//! Central YAML rendering — the text form of the facts tracer shows.
//!
//! YAML front matter is the metadata block agents and people already read
//! without instruction, on Markdown, Skills, and static sites, so a file's
//! facts print as a YAML block and `--json` carries the same keys. This module
//! is the single place that fixes that representation, as `jsonfmt` fixes
//! JSON's. It needs no YAML crate: the facts are mappings of scalars, and any
//! JSON string is a valid YAML double-quoted scalar, so a scalar that could
//! read back as anything but itself is written in JSON quotes.

use serde_json::{Map, Value};

/// A mapping as block YAML: one key per line, a nested mapping indented two
/// spaces under its key, and anything deeper on one line in flow style, so a
/// small table such as a set of annotation counts stays one line.
pub fn block(map: &Map<String, Value>) -> String {
    let mut out = String::new();
    write_block(&mut out, map, 0);
    out
}

fn write_block(out: &mut String, map: &Map<String, Value>, depth: usize) {
    for (key, value) in map {
        out.push_str(&"  ".repeat(depth));
        out.push_str(&scalar(key, false));
        out.push(':');
        match value {
            Value::Object(inner) if depth == 0 && !inner.is_empty() => {
                out.push('\n');
                write_block(out, inner, depth + 1);
            }
            _ => {
                out.push(' ');
                out.push_str(&flow(value, false));
                out.push('\n');
            }
        }
    }
}

/// A value on one line. `in_flow` is true inside `[…]` or `{…}`, where the
/// flow indicators also force quoting.
pub fn flow(value: &Value, in_flow: bool) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => scalar(value, in_flow),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(|item| flow(item, true)).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(key, value)| format!("{}: {}", scalar(key, true), flow(value, true)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A string as a plain scalar when YAML reads it back as that same string,
/// otherwise double-quoted.
pub fn scalar(text: &str, in_flow: bool) -> String {
    if is_plain(text, in_flow) {
        text.to_string()
    } else {
        serde_json::to_string(text).expect("a string always serializes")
    }
}

fn is_plain(text: &str, in_flow: bool) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    if text.trim() != text || "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        return false;
    }
    if text.chars().any(|c| c.is_control()) || text.contains(": ") || text.contains(" #") || text.ends_with(':') {
        return false;
    }
    if in_flow && text.contains([',', '[', ']', '{', '}']) {
        return false;
    }
    !reads_as_another_type(text)
}

/// Whether a plain scalar would resolve to a boolean, null, number, or date
/// under YAML 1.1 or 1.2, the two schemas parsers still ship.
fn reads_as_another_type(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "y" | "n" | "null" | "~"
    ) {
        return true;
    }
    let unsigned = lower.trim_start_matches(['+', '-']);
    if unsigned.parse::<f64>().is_ok()
        || matches!(unsigned, ".inf" | ".nan")
        || unsigned.starts_with("0x")
        || unsigned.starts_with("0o")
    {
        return true;
    }
    let bytes = text.as_bytes();
    bytes.len() >= 8 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_that_would_change_type_are_quoted() {
        for text in ["true", "No", "null", "42", "-3.5", "1e3", "0x1f", "2026-09-24", "~"] {
            assert!(scalar(text, false).starts_with('"'), "{text} left plain");
        }
        for text in ["modified", "3 weeks ago by Jordan Parker", "app/Contact.php", "Claude.md"] {
            assert_eq!(scalar(text, false), text);
        }
    }

    #[test]
    fn indicators_and_separators_force_quotes() {
        assert_eq!(scalar("merge: development", false), "\"merge: development\"");
        assert_eq!(scalar("#[Field]", true), "\"#[Field]\"");
        assert_eq!(scalar("a, b", true), "\"a, b\"");
        assert_eq!(scalar("a, b", false), "a, b");
        assert_eq!(scalar("", false), "\"\"");
    }

    #[test]
    fn nested_mappings_indent_once_then_go_flow() {
        let value = json!({"file": "a.php", "git": {"status": "modified", "on_deploy_branches": ["prod"]},
                           "directory": {"annotations": {"Field": 2}}});
        assert_eq!(
            block(value.as_object().unwrap()),
            "file: a.php\ngit:\n  status: modified\n  on_deploy_branches: [prod]\ndirectory:\n  annotations: {Field: 2}\n"
        );
    }
}
