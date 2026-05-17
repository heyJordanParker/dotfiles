// FileFacts — the per-file record persisted in the `file/` cache namespace.

use crate::cache::{self, NAMESPACE_FILE};
use crate::extraction::{self, ExtractionResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFacts {
    pub path: String,
    pub language: Option<String>,
    pub loc: u32,
    pub function_count: u32,
    pub cyclomatic_complexity_total: u32,
    pub cyclomatic_complexity_max: u32,
    pub rank: String,
    pub extraction: Option<ExtractionResult>,
}

pub fn rank(ccn: u32) -> &'static str {
    if ccn < 10 {
        "low"
    } else if ccn < 30 {
        "medium"
    } else if ccn < 80 {
        "high"
    } else {
        "critical"
    }
}

pub fn get(path: &Path, repo_root: &Path) -> Result<Option<FileFacts>> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    let key = cache::file_key(&bytes, path, repo_root);
    if let Some(cached) = cache::load::<FileFacts>(NAMESPACE_FILE, &key, repo_root) {
        return Ok(Some(cached));
    }
    let facts = build(path, &bytes, repo_root)?;
    cache::save(NAMESPACE_FILE, &key, &facts, repo_root)?;
    Ok(Some(facts))
}

fn build(path: &Path, bytes: &[u8], repo_root: &Path) -> Result<FileFacts> {
    let relative = path
        .canonicalize()
        .ok()
        .and_then(|abs| {
            repo_root
                .canonicalize()
                .ok()
                .and_then(|root| abs.strip_prefix(&root).ok().map(|r| r.to_path_buf()))
        })
        .unwrap_or_else(|| path.to_path_buf());
    let extraction = extraction::extract(path, bytes);
    let (function_count, ccn_total, ccn_max) = if let Some(ref ex) = extraction {
        let total: u32 = ex.functions.iter().map(|f| f.cyclomatic_complexity).sum();
        let max = ex
            .functions
            .iter()
            .map(|f| f.cyclomatic_complexity)
            .max()
            .unwrap_or(0);
        (ex.functions.len() as u32, total, max)
    } else {
        (0, 0, 0)
    };
    let loc = bytes.iter().filter(|&&b| b == b'\n').count() as u32;
    let language = extraction.as_ref().map(|e| e.language.clone());
    Ok(FileFacts {
        path: relative.to_string_lossy().to_string(),
        language,
        loc,
        function_count,
        cyclomatic_complexity_total: ccn_total,
        cyclomatic_complexity_max: ccn_max,
        rank: rank(ccn_total).to_string(),
        extraction,
    })
}
