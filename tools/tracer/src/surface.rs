use crate::ccn::FunctionFact;
use crate::extraction::ExtractionResult;
use crate::file_facts::FileFacts;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Row {
    pub header_line: i64,
    pub line: i64,
    pub end_line: i64,
    pub kind: String,
    pub name: String,
    pub container: Option<String>,
    pub parent: Option<u32>,
    pub header: String,
    pub annotations: Vec<String>,
    pub cyclomatic_complexity: Option<i64>,
}

pub fn rows(facts: &FileFacts, window: Option<(i64, i64)>) -> Vec<Row> {
    facts
        .extraction
        .as_ref()
        .map_or_else(Vec::new, |extraction| rows_from(extraction, &facts.functions, window))
}

pub fn rows_at(repo_root: &Path, revision: &str, path: &str) -> Vec<Row> {
    crate::git_activity::blob(repo_root, revision, path)
        .and_then(|bytes| crate::file_facts::extraction_of(&bytes, path, repo_root))
        .map(|extraction| rows_from(&extraction, &[], None))
        .unwrap_or_default()
}

fn rows_from(extraction: &ExtractionResult, functions: &[FunctionFact], window: Option<(i64, i64)>) -> Vec<Row> {
    let mut wanted: HashSet<usize> = match window {
        None => (0..extraction.declarations.len()).collect(),
        Some((start, end)) => extraction
            .declarations
            .iter()
            .enumerate()
            .filter(|(_, declaration)| {
                declaration.header_line <= end && declaration.end_line >= start
            })
            .map(|(index, _)| index)
            .collect(),
    };
    let mut parents: Vec<usize> = wanted.iter().copied().collect();
    while let Some(index) = parents.pop() {
        if let Some(parent) = extraction.declarations[index]
            .parent
            .map(|parent| parent as usize)
        {
            if wanted.insert(parent) {
                parents.push(parent);
            }
        }
    }
    let mut old_to_new = HashMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for (index, declaration) in extraction.declarations.iter().enumerate() {
        if !wanted.contains(&index) {
            continue;
        }
        old_to_new.insert(index as u32, rows.len() as u32);
        rows.push(Row {
            header_line: declaration.header_line,
            line: declaration.line,
            end_line: declaration.end_line,
            kind: declaration.kind.clone(),
            name: declaration.name.clone(),
            container: declaration.container.clone(),
            parent: declaration.parent,
            header: declaration.header.clone(),
            annotations: declaration.annotations.clone(),
            cyclomatic_complexity: functions
                .iter()
                .find(|function| {
                    function.start_line <= declaration.line
                        && declaration.line < function.start_line + function.nloc
                        && function.name.ends_with(&declaration.name)
                })
                .map(|function| function.cyclomatic_complexity),
        });
    }
    for row in &mut rows {
        row.parent = row
            .parent
            .and_then(|parent| old_to_new.get(&parent).copied());
    }
    rows
}

/// The rows of `file`, one declaration header per row, nested by parent, in
/// at most `budget` characters with every declaration
/// still named: each row starts as `L<n> name()`, then rows get their whole
/// header back in order — rows inside `window` first, then public rows, then
/// the rest — while the budget holds. `None` renders every header whole.
/// When even one `L<n> name()` per row overruns the budget, the names go one
/// line per parent, and past that each parent with its count per kind, the
/// same floor `output::fit_listing` gives a list of files.
pub fn render_within(
    rows: &[Row],
    file: &str,
    window: Option<(i64, i64)>,
    budget: Option<usize>,
) -> String {
    let whole = render_rows(rows, file, window, None);
    match budget {
        Some(budget) if whole.text.len() > budget => {
            let closing = crate::output::shortened_line(whole.of, whole.of, "declarations").len() + 1;
            let fitted = render_rows(rows, file, window, Some(budget.saturating_sub(closing)));
            format!("{}{}\n", fitted.text, crate::output::shortened_line(fitted.cut, fitted.of, "declarations"))
        }
        _ => whole.text,
    }
}

/// Rows fitted to a budget: the text, and how many of its `of` declarations
/// the budget cut.
pub struct Fitted {
    pub text: String,
    pub cut: usize,
    pub of: usize,
}

/// One file's rows fitted to `budget` the way `render_within` fits them, with
/// no closing line, so a listing of many files closes once for all of them.
pub fn render_rows(rows: &[Row], file: &str, window: Option<(i64, i64)>, budget: Option<usize>) -> Fitted {
    let shown: Vec<usize> = (0..rows.len())
        .filter(|&index| {
            index == 0
                || !(rows[index - 1].line == rows[index].line
                    && rows[index - 1].header == rows[index].header)
        })
        .collect();
    let of = shown.len();
    let whole: Vec<String> = shown.iter().map(|&index| whole_row(rows, index, file)).collect();
    let Some(budget) = budget.filter(|&budget| whole.iter().map(String::len).sum::<usize>() > budget) else {
        return Fitted {
            text: whole.concat(),
            cut: 0,
            of,
        };
    };
    let mut texts: Vec<String> = shown.iter().map(|&index| short_row(rows, index)).collect();
    let mut size: usize = texts.iter().map(String::len).sum();
    if size > budget {
        return Fitted {
            text: by_parent(rows, &shown, budget),
            cut: of,
            of,
        };
    }
    let mut order: Vec<usize> = (0..of).collect();
    order.sort_by_key(|&at| {
        let row = &rows[shown[at]];
        let in_window = window.is_some_and(|(start, end)| row.header_line <= end && row.end_line >= start);
        (!in_window, !is_public(row, file))
    });
    for at in order {
        let grown = size - texts[at].len() + whole[at].len();
        if grown <= budget {
            size = grown;
            texts[at] = whole[at].clone();
        }
    }
    let cut = texts.iter().zip(&whole).filter(|(text, whole)| text != whole).count();
    Fitted {
        text: texts.concat(),
        cut,
        of,
    }
}

/// Every shown row's name, one line per parent (`L39 Contact: a(), b()`),
/// or, when that overruns `budget` too, each parent with its count per kind
/// (`L39 Contact: 12 functions, 4 properties`).
fn by_parent(rows: &[Row], shown: &[usize], budget: usize) -> String {
    let mut parents: Vec<(Option<u32>, Vec<usize>)> = Vec::new();
    let mut at: HashMap<Option<u32>, usize> = HashMap::new();
    for &index in shown {
        let parent = rows[index].parent;
        let group = *at.entry(parent).or_insert_with(|| {
            parents.push((parent, Vec::new()));
            parents.len() - 1
        });
        parents[group].1.push(index);
    }
    let label = |parent: Option<u32>| match parent.map(|parent| &rows[parent as usize]) {
        Some(row) => format!("L{} {}", row.line, row.name),
        None => "top level".to_string(),
    };
    let names: String = parents
        .iter()
        .map(|(parent, members)| {
            let listed: Vec<String> = members
                .iter()
                .map(|&index| called(&rows[index]))
                .collect();
            format!("{}: {}\n", label(*parent), listed.join(", "))
        })
        .collect();
    if names.len() <= budget {
        names
    } else {
        parents
            .iter()
            .map(|(parent, members)| {
                let mut kinds: Vec<(&str, usize)> = Vec::new();
                for &index in members {
                    let kind = rows[index].kind.as_str();
                    match kinds.iter_mut().find(|(known, _)| *known == kind) {
                        Some((_, count)) => *count += 1,
                        None => kinds.push((kind, 1)),
                    }
                }
                let counted: Vec<String> = kinds
                    .iter()
                    .map(|(kind, count)| {
                        let many = match kind.strip_suffix('y') {
                            Some(stem) => format!("{stem}ies"),
                            None if kind.ends_with('s') => format!("{kind}es"),
                            None => format!("{kind}s"),
                        };
                        crate::output::counted(*count, kind, &many)
                    })
                    .collect();
                format!("{}: {}\n", label(*parent), counted.join(", "))
            })
            .collect()
    }
}

/// A row's header as rows print it: a data declaration's multi-line
/// initializer is elided like a body, so a 200-line table prints as
/// `const TABLE: &[Row] = …`.
fn header(row: &Row) -> std::borrow::Cow<'_, str> {
    let data = matches!(
        row.kind.as_str(),
        "constant" | "const" | "variable" | "var" | "field" | "property"
    );
    let declared_at: usize = row
        .header
        .split_inclusive('\n')
        .take((row.line - row.header_line).max(0) as usize)
        .map(str::len)
        .sum();
    match row.header[declared_at..].find(" = ").map(|at| declared_at + at) {
        Some(at) if data && row.header[at..].contains('\n') => format!("{} = …", &row.header[..at]).into(),
        _ => row.header.as_str().into(),
    }
}

fn whole_row(rows: &[Row], index: usize, file: &str) -> String {
    let row = &rows[index];
    let prefix = format!("L{:<5}{}", row.line, "  ".repeat(depth(index, rows)));
    let mut output = String::new();
    for (at, line) in header(row).lines().enumerate() {
        if at > 0 {
            output.push('\n');
            output.push_str(&" ".repeat(prefix.len()));
        } else {
            output.push_str(&prefix);
        }
        output.push_str(line);
    }
    output.push_str(&complexity_comment(row, file));
    output.push('\n');
    output
}

/// A row cut to its line and name.
fn short_row(rows: &[Row], index: usize) -> String {
    format!("L{:<5}{}{}\n", rows[index].line, "  ".repeat(depth(index, rows)), called(&rows[index]))
}

/// A row's name, with `()` when it is called.
fn called(row: &Row) -> String {
    let call = if matches!(row.kind.as_str(), "function" | "method") { "()" } else { "" };
    format!("{}{call}", row.name)
}

/// Whether a row is part of its file's public surface, read from the header
/// the way the language spells it.
fn is_public(row: &Row, file: &str) -> bool {
    let words = modifiers(&row.header, row.line - row.header_line, &row.name);
    if file.ends_with(".rs") {
        return words.contains(&"pub");
    }
    !(words.contains(&"private") || words.contains(&"protected") || row.name.starts_with('_') || row.name.starts_with('#'))
}

pub fn modifiers<'a>(header: &'a str, offset: i64, name: &str) -> Vec<&'a str> {
    let name = name.trim_start_matches(['$', '#']);
    header
        .lines()
        .nth(offset.max(0) as usize)
        .unwrap_or("")
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|word| !word.is_empty())
        .take_while(|word| *word != name)
        .collect()
}

/// One row of `file` on one line.
pub fn inline(row: &Row, file: &str) -> String {
    let mut header = header(row)
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    header.push_str(&complexity_comment(row, file));
    header
}

/// A row's complexity as a trailing comment in its file's own syntax, so the
/// row still reads as that language: `  // complexity 4`, `  # complexity 4`.
fn complexity_comment(row: &Row, file: &str) -> String {
    let Some(complexity) = row.cyclomatic_complexity else {
        return String::new();
    };
    let comment = match std::path::Path::new(file).extension().and_then(|e| e.to_str()) {
        Some("py" | "pyi" | "rb") => "#",
        _ => "//",
    };
    format!("  {comment} complexity {complexity}")
}

pub fn innermost(spans: impl IntoIterator<Item = (i64, i64)>, line: i64) -> Option<usize> {
    spans
        .into_iter()
        .enumerate()
        .filter(|(_, (start, end))| (*start..=*end).contains(&line))
        .min_by_key(|(index, (start, end))| (end - start, std::cmp::Reverse(*index)))
        .map(|(index, _)| index)
}

pub fn enclosing(rows: &[Row], line: i64) -> (Option<&Row>, Option<&Row>) {
    let Some(index) = innermost(rows.iter().map(|row| (row.header_line, row.end_line)), line) else {
        return (None, None);
    };

    let mut type_index = Some(index);
    while let Some(current) = type_index {
        let row = &rows[current];
        if matches!(
            row.kind.as_str(),
            "class" | "interface" | "enum" | "type" | "impl"
        ) {
            let declaration = (!matches!(
                rows[index].kind.as_str(),
                "class" | "interface" | "enum" | "type" | "impl"
            ))
            .then_some(&rows[index]);
            return (declaration, Some(row));
        }
        type_index = row.parent.map(|parent| parent as usize);
    }

    (Some(&rows[index]), None)
}

/// How many parents a row sits under.
pub fn depth(index: usize, rows: &[Row]) -> usize {
    let mut depth = 0;
    let mut parent = rows[index].parent.map(|parent| parent as usize);
    while let Some(next) = parent {
        if next >= rows.len() || depth == rows.len() {
            break;
        }
        depth += 1;
        parent = rows[next].parent.map(|parent| parent as usize);
    }
    depth
}
