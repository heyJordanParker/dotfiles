//! `trace status` — the working tree's changed files, each with its facts,
//! ordered by blast radius: the files most others import first.

use super::diff;
use crate::summary::Facts;
use crate::{cache, file_facts, git_activity, relations};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::Path;

/// Human-output grouping order; also the tertiary sort key.
const STATE_ORDER: &[&str] = &["added", "renamed", "modified", "deleted", "untracked"];

fn state_rank(state: &str) -> usize {
    STATE_ORDER
        .iter()
        .position(|s| *s == state)
        .unwrap_or(STATE_ORDER.len())
}

struct Entry {
    path: String,
    state: String,
    staging: Option<String>,
    facts: Option<Facts>,
}

impl Entry {
    /// Blast radius first: most importers, then most complex, then state, then path.
    fn sort_key(&self) -> (i64, i64, usize, &str) {
        let facts = self.facts.as_ref();
        (
            -(facts.and_then(|f| f.imported_by).unwrap_or(0) as i64),
            -facts.map(|f| f.cyclomatic_complexity).unwrap_or(0),
            state_rank(&self.state),
            &self.path,
        )
    }
}

fn entries(repo_root: &Path, states: &[(String, String)], index: &relations::Relations) -> Vec<Entry> {
    // Staging is one more fact per file, from the same `git status` read the
    // states came from.
    let staging = git_activity::staging_state(repo_root);
    let mut out = Vec::with_capacity(states.len());
    for chunk in states.chunks(file_facts::RESOLVE_CHUNK) {
        // Project each bounded resolve into owned rows before resolving the
        // next chunk, so whole FileFacts never accumulate repo-wide.
        let existing: Vec<std::path::PathBuf> = chunk
            .iter()
            .map(|(relative, _)| repo_root.join(relative))
            .filter(|abs| abs.exists())
            .collect();
        let facts_map = file_facts::get_batch(&existing, repo_root);
        for (relative, state) in chunk {
            let facts = repo_root
                .join(relative)
                .canonicalize()
                .ok()
                .map(|abs| cache::relative_to_root(&abs, repo_root))
                .and_then(|rel| facts_map.get(&rel))
                .map(|facts| Facts::of(facts, index.module_counts(relative).as_ref()));
            out.push(Entry {
                path: relative.clone(),
                state: state.clone(),
                staging: staging.get(relative).cloned(),
                facts,
            });
        }
    }
    out
}

pub fn run(as_json: bool, state_filter: Option<&str>) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let mut states: Vec<(String, String)> = git_activity::working_tree_state(&repo_root)
        .into_iter()
        .filter(|(_, s)| state_filter.is_none_or(|f| s == f))
        .collect();
    states.sort();

    let index = relations::get(&repo_root);
    let mut entries = entries(&repo_root, &states, &index);
    entries.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
    let directories = diff::directory_context(&index, &repo_root, &paths);

    let mut files = Map::new();
    let rows: Vec<Value> = entries
        .iter()
        .map(|entry| {
            if let Some(facts) = &entry.facts {
                files.insert(entry.path.clone(), Value::Object(facts.to_map()));
            }
            json!({"path": entry.path, "state": entry.state, "staging": entry.staging})
        })
        .collect();
    let value = crate::output::document(
        json!({"repo_root": repo_root.to_string_lossy(), "state": state_filter}),
        json!({"repo_root": repo_root.to_string_lossy(), "files": files, "directories": directories}),
        Value::Array(rows),
        json!({"files": entries.len(), "directories": directories.as_object().map_or(0, Map::len)}),
    );
    if as_json {
        return Ok(value);
    }
    if entries.is_empty() {
        println!("(working tree clean)");
        return Ok(value);
    }

    let head = format!(
        "{} files with uncommitted state:\n\n{}",
        entries.len(),
        diff::render_directory_context(&directories)
    );
    print!("{head}");
    // One line per file under its state's heading, blast radius first within
    // each state; the budget cuts the files the fewest others import back to
    // their path first.
    let mut by_state: Vec<&Entry> = entries.iter().collect();
    by_state.sort_by_key(|entry| state_rank(&entry.state));
    let mut rows: Vec<crate::output::Entry> = Vec::with_capacity(entries.len());
    let mut paths: Vec<&str> = Vec::with_capacity(entries.len());
    let mut current_state: Option<&str> = None;
    for entry in by_state {
        paths.push(&entry.path);
        let heading = if current_state != Some(entry.state.as_str()) {
            current_state = Some(&entry.state);
            format!("## {}\n", entry.state)
        } else {
            String::new()
        };
        let staging = entry.staging.as_deref().map(|s| format!(" · {s}")).unwrap_or_default();
        let path = format!("{heading}  {}{staging}", entry.path);
        rows.push(match &entry.facts {
            Some(facts) => crate::output::Entry {
                rank: facts.imported_by.unwrap_or(0) as i64,
                levels: vec![format!("{path}  {}", facts.headline()), path],
            },
            None => crate::output::Entry { rank: 0, levels: vec![path] },
        });
    }
    let fixed = head.len() + crate::output::closing_room(rows.len(), "files");
    let (texts, shortened) = crate::output::fit_listing(&rows, &paths, fixed);
    for text in texts {
        println!("{text}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, rows.len(), "files"));
    }
    Ok(value)
}
