//! File digest helpers.
//! leading_comment, top_callers, immediate_dependencies, nearest_doc.

use crate::architecture::{self, Graph};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const DOC_FILENAMES: &[&str] = &[
    "Claude.md",
    "CLAUDE.md",
    "Readme.md",
    "README.md",
    "ARCHITECTURE.md",
    "architecture.md",
];

/// Walk up from the file to find the nearest project doc. Iterates the
/// fixed `DOC_FILENAMES` list and returns the first existing: any match in
/// a directory wins, and list order is the deterministic tiebreak.
pub fn nearest_doc(path: &Path) -> Option<String> {
    let abs = crate::cache::absolutize(path);
    let mut current = if abs.is_file() {
        abs.parent().map(|p| p.to_path_buf())
    } else {
        Some(abs.clone())
    };
    while let Some(dir) = current {
        if dir == Path::new("/") {
            // The filesystem root itself is not probed.
            break;
        }
        for name in DOC_FILENAMES {
            let candidate = dir.join(name);
            if candidate.exists() {
                // `absolutize` (cwd-join) leaves `.` components when
                // `--path` defaulted to `.`; normalize them out lexically
                // so the returned path has no `/./` segments.
                return Some(normalize_dots(&candidate).to_string_lossy().to_string());
            }
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Lexically collapse `.` and `..` components — no filesystem access,
/// pure path arithmetic.
fn normalize_dots(p: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

const BLOCK_OPEN: &[&str] = &["/*", "/**"];
const BLOCK_CLOSE: &str = "*/";
const TRIPLE_QUOTES: &[&str] = &["\"\"\"", "'''"];

/// Extract the leading comment block as plain text. None when no
/// recognizable docblock is at the top.
pub fn leading_comment(path: &Path, max_lines: usize) -> Option<String> {
    let content = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&content);
    // Read up to max_lines lines, keeping line endings.
    let all_lines: Vec<&str> = text.split_inclusive('\n').collect();
    let lines: Vec<String> = all_lines
        .iter()
        .take(max_lines)
        .map(|s| s.to_string())
        .collect();
    if lines.is_empty() {
        return None;
    }

    let mut i = 0;
    while i < lines.len()
        && (lines[i].trim().is_empty() || lines[i].starts_with("#!"))
    {
        i += 1;
    }
    if i < lines.len() && lines[i].trim_start().starts_with("<?") {
        i += 1;
    }
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    if i >= lines.len() {
        return None;
    }

    let head = lines[i].trim().to_string();

    // Block comment.
    if BLOCK_OPEN.iter().any(|t| head.starts_with(t)) {
        let mut body: Vec<String> = Vec::new();
        for line in &lines[i..] {
            let stripped = line.trim();
            let mut cleaned = stripped.to_string();
            for token in ["/**", "/*"] {
                if cleaned.starts_with(token) {
                    cleaned = cleaned[token.len()..].trim_start().to_string();
                }
            }
            if cleaned.ends_with(BLOCK_CLOSE) {
                cleaned = cleaned[..cleaned.len() - BLOCK_CLOSE.len()]
                    .trim_end()
                    .to_string();
                if cleaned.starts_with('*') {
                    cleaned = cleaned[1..].trim_start().to_string();
                }
                if !cleaned.is_empty() {
                    body.push(cleaned);
                }
                break;
            }
            if cleaned.starts_with('*') {
                cleaned = cleaned[1..].trim_start().to_string();
            }
            body.push(cleaned);
        }
        let joined: Vec<String> = body.into_iter().filter(|l| !l.is_empty()).collect();
        return if joined.is_empty() {
            None
        } else {
            Some(joined.join("\n"))
        };
    }

    // Triple-quoted docstring.
    for quote in TRIPLE_QUOTES {
        if head.starts_with(quote) {
            let mut body: Vec<String> = Vec::new();
            let first_after = &head[quote.len()..];
            if let Some(idx) = first_after.find(quote) {
                let single = first_after[..idx].trim();
                return if single.is_empty() {
                    None
                } else {
                    Some(single.to_string())
                };
            }
            if !first_after.is_empty() {
                body.push(first_after.to_string());
            }
            for line in &lines[i + 1..] {
                let stripped = line.trim_end();
                if let Some(idx) = stripped.find(quote) {
                    body.push(stripped[..idx].trim_end().to_string());
                    break;
                }
                body.push(stripped.to_string());
            }
            let joined = body.join("\n");
            return if joined.is_empty() {
                None
            } else {
                Some(joined)
            };
        }
    }

    // Run of // or # line comments.
    let line_tokens = ["# ", "#", "// ", "//", "*", "* "];
    if line_tokens.iter().any(|t| head.starts_with(t)) {
        let mut body: Vec<String> = Vec::new();
        for line in &lines[i..] {
            let stripped = line.trim_end();
            if stripped.is_empty() {
                break;
            }
            let ls = stripped.trim_start();
            let mut matched = false;
            for token in &line_tokens {
                if ls.starts_with(token) {
                    body.push(ls[token.len()..].trim().to_string());
                    matched = true;
                    break;
                }
            }
            if !matched {
                break;
            }
        }
        let joined: Vec<String> = body.into_iter().filter(|l| !l.is_empty()).collect();
        return if joined.is_empty() {
            None
        } else {
            Some(joined.join("\n"))
        };
    }

    None
}

/// Up to `limit` top callers of the module owning `relative_file`.
pub fn top_callers(
    graph: &Graph,
    relative_file: &str,
    repo_root: Option<&Path>,
    limit: usize,
) -> Vec<Value> {
    let module_id = match graph.file_to_module_id.get(relative_file) {
        Some(m) => m.clone(),
        None => return vec![],
    };
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for edge in architecture::dependents_of(graph, &module_id) {
        if seen.contains(&edge.source) {
            continue;
        }
        seen.push(edge.source.clone());
        let node = match graph.nodes.get(&edge.source) {
            Some(n) => n,
            None => continue,
        };
        let meaningful_line = if node.kind != "module" {
            node.source_line
        } else {
            None
        };
        let mut summary: Option<String> = None;
        if let (Some(root), Some(sf)) = (repo_root, &node.source_file) {
            let caller_path = root.join(sf);
            if caller_path.is_file() {
                if let Some(c) = leading_comment(&caller_path, 15) {
                    if let Some(first) = c.lines().next() {
                        let t = first.trim();
                        if !t.is_empty() {
                            summary = Some(t.to_string());
                        }
                    }
                }
            }
        }
        out.push(json!({
            "source_file": node.source_file,
            "source_line": meaningful_line,
            "label": node.label,
            "kind": node.kind,
            "summary": summary,
        }));
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// Immediate (one-hop) dependencies of the module owning a file.
pub fn immediate_dependencies(
    graph: &Graph,
    relative_file: &str,
    limit: usize,
) -> Vec<Value> {
    let module_id = match graph.file_to_module_id.get(relative_file) {
        Some(m) => m.clone(),
        None => return vec![],
    };
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for edge in architecture::dependencies_of(graph, &module_id) {
        if seen.contains(&edge.target) {
            continue;
        }
        seen.push(edge.target.clone());
        let label = graph
            .nodes
            .get(&edge.target)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| edge.target.clone());
        out.push(json!({"module": label, "confidence": edge.confidence}));
        if out.len() >= limit {
            break;
        }
    }
    out
}
