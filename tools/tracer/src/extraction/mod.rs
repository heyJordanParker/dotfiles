//! Per-language extraction.
//!
//! Each extractor returns an `ExtractionResult` of module-level imports and
//! exports via fixed tree-sitter query strings. The architecture graph and
//! `structure` command consume this. Per-function CCN is a separate
//! concern — see `crate::ccn`.

pub mod php;
pub mod python;
pub mod typescript;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub symbol: Option<String>,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub kind: String,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub language: String,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}

impl ExtractionResult {
    /// Matches `ExtractionResult.to_dict()`.
    pub fn to_json(&self) -> Value {
        json!({
            "language": self.language,
            "imports": self.imports.iter().map(|i| json!({
                "module": i.module,
                "symbol": i.symbol,
                "line": i.line,
            })).collect::<Vec<_>>(),
            "exports": self.exports.iter().map(|e| json!({
                "name": e.name,
                "kind": e.kind,
                "line": e.line,
            })).collect::<Vec<_>>(),
        })
    }

    pub fn from_json(v: &Value) -> Self {
        ExtractionResult {
            language: v
                .get("language")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .to_string(),
            imports: v
                .get("imports")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .map(|i| Import {
                            module: i
                                .get("module")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string(),
                            symbol: i
                                .get("symbol")
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string()),
                            line: i.get("line").and_then(|s| s.as_i64()).unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            exports: v
                .get("exports")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .map(|e| Export {
                            name: e
                                .get("name")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string(),
                            kind: e
                                .get("kind")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string(),
                            line: e.get("line").and_then(|s| s.as_i64()).unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// Extensions with a registered extractor (lowercase, no leading dot).
pub fn supported_extensions() -> &'static [&'static str] {
    &["py", "ts", "tsx", "js", "jsx", "php"]
}

pub fn is_supported(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => supported_extensions().contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

/// Dispatch to the per-language extractor. None for unsupported extensions.
pub fn extract(source: &[u8], path: &str) -> Option<ExtractionResult> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())?;
    match ext.as_str() {
        "py" => Some(python::extract(source)),
        "ts" | "js" => Some(typescript::extract(source, path, false)),
        "tsx" | "jsx" => Some(typescript::extract(source, path, true)),
        "php" => Some(php::extract(source)),
        _ => None,
    }
}
