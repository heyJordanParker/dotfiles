//! Per-language extraction.
//!
//! Each extractor returns an `ExtractionResult` of module-level imports and
//! exports via fixed tree-sitter query strings. The relations index and
//! `structure` command consume this. Per-function CCN is a separate
//! concern — see `crate::ccn`.

pub mod c;
pub mod go;
pub mod header;
pub mod java;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod typescript;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

#[allow(non_upper_case_globals)]
const ctags_denied_languages: &[&str] = &[
    "Json",
    "Yaml",
    "Iniconf",
    "Diff",
    "XML",
    "DTD",
    "SVG",
    "XSLT",
    "RelaxNG",
    "PlistXML",
    "Maven2",
    "Glade",
    "DBusIntrospect",
    "Ant",
    "Toml",
];

#[allow(non_upper_case_globals)]
const ctags_denied_kinds: &[&str] = &["heredoc"];

/// One imported name. A Rust `type Name = path;` imports `path` bound as
/// `Name`, the way `use path as Name;` does. `block` is the first and last
/// line of the block that holds the import, a Rust `mod` body or block, and
/// `None` for the whole file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub symbol: Option<String>,
    #[serde(default)]
    pub locals: Vec<String>,
    pub line: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<(i64, i64)>,
}

impl Import {
    /// The name the import binds: its alias, else its symbol.
    pub fn binding(&self) -> Option<&str> {
        self.locals.first().or(self.symbol.as_ref()).map(String::as_str)
    }

    /// Whether the import binds its name at `line`: its block holds the
    /// line, and no import of `imports` whose block holds it is smaller and
    /// binds the same name.
    pub fn in_force(&self, line: i64, imports: &[Import]) -> bool {
        let holds = |import: &Import| import.block.is_none_or(|(first, last)| (first..=last).contains(&line));
        let size = |import: &Import| import.block.map_or(i64::MAX, |(first, last)| last - first);
        holds(self)
            && !self.binding().is_some_and(|name| {
                imports
                    .iter()
                    .any(|inner| inner.binding() == Some(name) && holds(inner) && size(inner) < size(self))
            })
    }
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
/// `self_type` is a Rust method's `Self`, the type its `impl` names or its
/// trait, read the way a call's receiver is read. `module_file` is a Rust
/// `mod` item's `#[path]` value. `supertypes` are the types a PHP class,
/// interface, trait, or enum names after `extends` and `implements` and in
/// its body's trait `use`, each by its last segment, nearest first: its
/// traits, then its parent, then its interfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    pub name: String,
    pub kind: String,
    pub header_line: i64,
    pub line: i64,
    pub end_line: i64,
    pub container: Option<String>,
    pub parent: Option<u32>,
    pub header: String,
    #[serde(default)]
    pub annotations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supertypes: Vec<String>,
}

pub fn line_text(source: &[u8], line: i64) -> String {
    String::from_utf8_lossy(source)
        .lines()
        .nth(line.saturating_sub(1) as usize)
        .unwrap_or("")
        .trim()
        .to_string()
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
/// in `Foo::bar()` / `new Foo`), in Rust the whole path written before the
/// name (`crate::summary` in `crate::summary::front_matter()`), for a
/// `Member` call the receiver's type where PHP or Rust states it, else the
/// receiver's text in TypeScript, Python, Java, and Ruby, and `None`
/// otherwise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub name: String,
    pub line: i64,
    pub shape: RefShape,
    pub receiver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub language: String,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub declarations: Vec<Declaration>,
    pub references: Vec<Reference>,
}

#[derive(Deserialize, Serialize)]
struct CtagsMap {
    binary: CtagsBinary,
    extensions: HashMap<String, String>,
}

#[derive(Deserialize, Serialize, PartialEq, Eq)]
struct CtagsBinary {
    path: String,
    size: u64,
    modified_nanos: u128,
}

/// Extensions with a tree-sitter extractor (lowercase, no leading dot).
pub fn supported_extensions() -> &'static [&'static str] {
    &[
        "py", "ts", "tsx", "js", "jsx", "php", "rs", "go", "rb", "java", "c", "h",
    ]
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            let extension = extension.to_lowercase();
            supported_extensions().contains(&extension.as_str())
                || ctags_languages(path).contains_key(&extension)
        })
}

/// Dispatch to the per-language extractor. None for unsupported extensions.
pub fn extract(source: &[u8], path: &str) -> Option<ExtractionResult> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())?;
    let extraction = match ext.as_str() {
        "py" => python::extract(source),
        "ts" | "js" => typescript::extract(source, path, false),
        "tsx" | "jsx" => typescript::extract(source, path, true),
        "php" => php::extract(source),
        "rs" => rust::extract(source),
        "go" => go::extract(source),
        "rb" => ruby::extract(source),
        "java" => java::extract(source),
        "c" | "h" => c::extract(source),
        _ => return ctags_extract(source, path, &ext),
    };
    Some(extraction)
}

fn ctags_languages(path: &Path) -> &'static HashMap<String, String> {
    static LANGUAGES: OnceLock<HashMap<String, String>> = OnceLock::new();
    LANGUAGES.get_or_init(|| {
        let Some(root) = crate::cache::worktree_root_for(path) else {
            return HashMap::new();
        };
        let binary = ctags_binary();
        if let Some(map) = load_ctags_map(&root, &binary) {
            return map.extensions;
        }

        let _maintenance = crate::cache::maintain(&root);
        if let Some(map) = load_ctags_map(&root, &binary) {
            return map.extensions;
        }

        let map = CtagsMap {
            binary,
            extensions: list_ctags_maps(),
        };
        let _ = crate::cache::save(crate::cache::NAMESPACE_FILE, "ctags_maps_v1", &map, &root);
        map.extensions
    })
}

fn ctags_binary() -> CtagsBinary {
    let path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join("ctags"))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
        .unwrap_or_else(|| {
            eprintln!("ctags failed: could not resolve ctags on PATH");
            std::process::exit(2);
        });
    let metadata = fs::metadata(&path).unwrap_or_else(|error| {
        eprintln!("ctags failed: {error}");
        std::process::exit(2);
    });
    let modified_nanos = metadata
        .modified()
        .and_then(|modified| {
            modified
                .duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)
        })
        .map(|duration| duration.as_nanos())
        .unwrap_or_else(|error| {
            eprintln!("ctags failed: {error}");
            std::process::exit(2);
        });
    CtagsBinary {
        path: path.to_string_lossy().into_owned(),
        size: metadata.len(),
        modified_nanos,
    }
}

fn load_ctags_map(root: &Path, binary: &CtagsBinary) -> Option<CtagsMap> {
    let map: CtagsMap = serde_json::from_slice(&crate::cache::load_bytes(
        crate::cache::NAMESPACE_FILE,
        "ctags_maps_v1",
        root,
    )?)
    .ok()?;
    (map.binary == *binary).then_some(map)
}

fn list_ctags_maps() -> HashMap<String, String> {
    let maps = crate::timing::phase("ctags list maps", || {
        Command::new("ctags").arg("--list-maps").output()
    })
    .unwrap_or_else(|error| {
        eprintln!("ctags failed: {error}");
        std::process::exit(2);
    });
    if !maps.status.success() {
        eprintln!(
            "ctags failed: {}",
            String::from_utf8_lossy(&maps.stderr).trim()
        );
        std::process::exit(2);
    }
    let mut extensions = HashMap::new();
    for line in String::from_utf8_lossy(&maps.stdout).lines() {
        let mut fields = line.split_whitespace();
        let Some(language) = fields.next() else {
            continue;
        };
        if ctags_denied_languages
            .iter()
            .any(|denied_language| language.eq_ignore_ascii_case(denied_language))
        {
            continue;
        }
        for pattern in fields {
            let Some(extension) = pattern.strip_prefix("*.") else {
                continue;
            };
            if !extension.contains(['*', '[', ']']) {
                extensions
                    .entry(extension.to_lowercase())
                    .or_insert_with(|| language.to_string());
            }
        }
    }
    extensions
}

fn ctags_extract(source: &[u8], path: &str, extension: &str) -> Option<ExtractionResult> {
    let language = ctags_languages(Path::new(path)).get(extension)?.clone();
    let root = crate::cache::worktree_root_for(Path::new(path))?;
    let temporary_dir = root.join(crate::cache::CACHE_DIR_NAME).join("tmp");
    std::fs::create_dir_all(&temporary_dir).ok()?;
    let mut source_file = tempfile::Builder::new()
        .suffix(&format!(".{extension}"))
        .tempfile_in(temporary_dir)
        .ok()?;
    source_file.write_all(source).ok()?;
    let output = crate::timing::phase("ctags", || {
        Command::new("ctags")
            .args([
                "--output-format=json",
                &format!("--language-force={language}"),
                "--fields=+nezKSt",
                "--sort=no",
                "-f",
                "-",
            ])
            .arg(source_file.path())
            .output()
    })
    .unwrap_or_else(|error| {
        eprintln!("ctags failed: {error}");
        std::process::exit(2);
    });
    if !output.status.success() {
        eprintln!(
            "ctags failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        std::process::exit(2);
    }
    let mut declarations = Vec::new();
    let mut missing_ends = HashSet::new();
    for tag in String::from_utf8_lossy(&output.stdout).lines() {
        let tag: serde_json::Value = serde_json::from_str(tag).ok()?;
        let raw_kind = tag.get("kind")?.as_str()?;
        if ctags_denied_kinds.contains(&raw_kind) {
            continue;
        }
        let name = tag.get("name")?.as_str()?.to_string();
        let line = tag.get("line")?.as_i64()?;
        let scope = tag
            .get("scope")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let end = tag.get("end").and_then(serde_json::Value::as_i64);
        if end.is_none() {
            missing_ends.insert((name.clone(), line));
        }
        declarations.push(Declaration {
            name,
            kind: ctags_kind(raw_kind),
            header_line: line,
            line,
            end_line: end.unwrap_or(line),
            container: scope,
            parent: None,
            header: line_text(source, line)
                .split('{')
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            annotations: Vec::new(),
            self_type: None,
            module_file: None,
            supertypes: Vec::new(),
        });
    }
    declarations.sort_by_key(|declaration| declaration.line);
    let last_line = (source.iter().filter(|byte| **byte == b'\n').count() + 1) as i64;
    for index in 0..declarations.len() {
        if !missing_ends.contains(&(declarations[index].name.clone(), declarations[index].line)) {
            continue;
        }
        let depth = declarations[index]
            .container
            .as_deref()
            .map(|scope| (scope.matches("::").count() + scope.matches("\"\"").count()) as i64 + 1)
            .unwrap_or(1);
        declarations[index].end_line = declarations
            .iter()
            .skip(index + 1)
            .find(|next| {
                next.container
                    .as_deref()
                    .map(|scope| {
                        (scope.matches("::").count() + scope.matches("\"\"").count()) as i64 + 1
                    })
                    .unwrap_or(1)
                    <= depth
            })
            .map(|next| next.line - 1)
            .unwrap_or(last_line);
    }
    let mut parents = HashMap::new();
    for (index, declaration) in declarations.iter().enumerate() {
        parents.insert(declaration.name.clone(), index as u32);
        if let Some(scope) = declaration.container.as_deref() {
            parents.insert(format!("{scope}::{}", declaration.name), index as u32);
            parents.insert(format!("{scope}\"\"{}", declaration.name), index as u32);
        }
    }
    for declaration in &mut declarations {
        declaration.parent = declaration
            .container
            .as_ref()
            .and_then(|scope| parents.get(scope).copied());
    }
    Some(ExtractionResult {
        language: language.to_lowercase(),
        imports: Vec::new(),
        exports: Vec::new(),
        declarations,
        references: Vec::new(),
    })
}

fn ctags_kind(raw: &str) -> String {
    match raw {
        "c" => "class",
        "f" => "function",
        "m" => "method",
        "v" => "variable",
        "p" => "property",
        "F" => "field",
        "i" | "I" => "import",
        "n" => "namespace",
        "s" => "struct",
        "e" | "g" => "enum",
        "t" => "trait",
        "u" => "union",
        other => other,
    }
    .to_string()
}
