//! Per-language extraction.
//!
//! Each extractor returns an `ExtractionResult` of module-level imports and
//! exports via fixed tree-sitter query strings. The relations index and
//! `structure` command consume this. Per-function CCN is a separate
//! concern — see `crate::ccn`.

pub mod c;
pub mod go;
pub mod java;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod typescript;

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub symbol: Option<String>,
    #[serde(default)]
    pub locals: Vec<String>,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub kind: String,
    pub line: i64,
}

/// A declaration — every named definition in the file, including
/// non-exported top-levels, methods on classes, and nested definitions.
/// Distinct from `Export`, which is the narrower module-level/exported set.
/// `container` is the enclosing class/interface/trait/enum name for a method
/// declaration, `None` for a free function or a top-level type. Reference
/// resolution uses it to tell a method from a free function of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    pub name: String,
    pub kind: String,
    pub line: i64,
    pub container: Option<String>,
}

/// The syntactic shape of a call/use site. `Free` is a bare call
/// (`foo()`), `Member` is a call on a receiver (`$x->foo()`, `obj.foo()`),
/// `Static` names the class at the site (`Foo::bar()`, `new Foo`,
/// `Foo::class`, a type hint). Resolution narrows candidates by shape: a
/// `Free` call resolves only to free functions, a `Member` call only to
/// methods, a `Static` use to the named class (and its exact member).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefShape {
    Free,
    Member,
    Static,
}

/// A reference — an identifier use site (a call or qualified-name access).
/// Resolved into edges at graph-build time. `shape` is the call form;
/// `receiver` is the class named at the site for a `Static` use (e.g. `Foo`
/// in `Foo::bar()` / `new Foo`), or `None` when the receiver type is not
/// named (a `Member` call on a variable whose type the site doesn't state).
/// `enclosing` is the name of the nearest enclosing function/method
/// declaration the use site sits inside — captured during the same AST walk
/// that emits the reference (the walk already holds the enclosing scope, the
/// way `walk_declarations` holds `container`). It makes the resolved edge
/// function-granular: the edge's source becomes that calling symbol's node
/// (`file::enclosing`) rather than the importer module. `None` for a use
/// site at module top level, which keeps the importer-module source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub name: String,
    pub line: i64,
    pub shape: RefShape,
    pub receiver: Option<String>,
    pub enclosing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub language: String,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub declarations: Vec<Declaration>,
    pub references: Vec<Reference>,
}

/// Extensions with a registered extractor (lowercase, no leading dot).
pub fn supported_extensions() -> &'static [&'static str] {
    &[
        "py", "ts", "tsx", "js", "jsx", "php", "rs", "go", "rb", "java", "c", "h",
    ]
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
        "rs" => Some(rust::extract(source)),
        "go" => Some(go::extract(source)),
        "rb" => Some(ruby::extract(source)),
        "java" => Some(java::extract(source)),
        "c" | "h" => Some(c::extract(source)),
        _ => None,
    }
}
