//! One language table for every search.
//!
//! `grep` filters through ripgrep's `--type`, `pattern` parses through
//! ast-grep's `-l`, and `grep --at <ref>` filters through a git pathspec,
//! which takes neither. The three spell languages differently: ripgrep has no
//! `tsx` type (`.tsx` lives under `ts`) while ast-grep needs `tsx` to parse
//! one. Before this table `trace grep -l tsx` returned zero matches with no
//! error — ripgrep rejected the type, its stderr was discarded, and an empty
//! result reads to an agent as "this does not exist". A language name now
//! means the same thing on every search, and a name none of them knows is
//! refused by name instead of answered with silence.

/// One row: what the caller writes, what ripgrep filters on, what ast-grep
/// parses with, and the extensions a git pathspec needs. A row whose `sg` is
/// empty is a language ast-grep has no grammar for, so `pattern` refuses it
/// while `grep` still filters on it.
pub struct Language {
    pub name: &'static str,
    pub rg: &'static str,
    pub sg: &'static str,
    pub exts: &'static [&'static str],
}

/// The table, in the words agents write. Aliases sit beside their canonical
/// name rather than in a second map, so one row carries the whole answer.
const TABLE: &[Language] = &[
    Language { name: "python", rg: "py", sg: "python", exts: &["py", "pyi"] },
    Language { name: "py", rg: "py", sg: "python", exts: &["py", "pyi"] },
    Language { name: "javascript", rg: "js", sg: "javascript", exts: &["js", "cjs", "mjs", "jsx"] },
    Language { name: "js", rg: "js", sg: "javascript", exts: &["js", "cjs", "mjs", "jsx"] },
    Language { name: "jsx", rg: "js", sg: "jsx", exts: &["jsx"] },
    Language { name: "typescript", rg: "ts", sg: "typescript", exts: &["ts", "cts", "mts"] },
    Language { name: "ts", rg: "ts", sg: "typescript", exts: &["ts", "cts", "mts"] },
    Language { name: "tsx", rg: "ts", sg: "tsx", exts: &["tsx"] },
    Language { name: "php", rg: "php", sg: "php", exts: &["php", "phtml"] },
    Language { name: "rust", rg: "rust", sg: "rust", exts: &["rs"] },
    Language { name: "rs", rg: "rust", sg: "rust", exts: &["rs"] },
    Language { name: "go", rg: "go", sg: "go", exts: &["go"] },
    Language { name: "java", rg: "java", sg: "java", exts: &["java"] },
    Language { name: "kotlin", rg: "kotlin", sg: "kotlin", exts: &["kt", "kts"] },
    Language { name: "swift", rg: "swift", sg: "swift", exts: &["swift"] },
    Language { name: "scala", rg: "scala", sg: "scala", exts: &["scala", "sc"] },
    Language { name: "ruby", rg: "ruby", sg: "ruby", exts: &["rb"] },
    Language { name: "rb", rg: "ruby", sg: "ruby", exts: &["rb"] },
    Language { name: "c", rg: "c", sg: "c", exts: &["c", "h"] },
    Language { name: "cpp", rg: "cpp", sg: "cpp", exts: &["cpp", "cc", "cxx", "hpp"] },
    Language { name: "csharp", rg: "csharp", sg: "csharp", exts: &["cs"] },
    Language { name: "cs", rg: "csharp", sg: "csharp", exts: &["cs"] },
    Language { name: "elixir", rg: "elixir", sg: "elixir", exts: &["ex", "exs"] },
    Language { name: "lua", rg: "lua", sg: "lua", exts: &["lua"] },
    Language { name: "bash", rg: "sh", sg: "bash", exts: &["sh", "bash", "zsh"] },
    Language { name: "sh", rg: "sh", sg: "bash", exts: &["sh", "bash", "zsh"] },
    Language { name: "shell", rg: "sh", sg: "bash", exts: &["sh", "bash", "zsh"] },
    Language { name: "html", rg: "html", sg: "html", exts: &["html", "htm"] },
    Language { name: "css", rg: "css", sg: "css", exts: &["css"] },
    Language { name: "json", rg: "json", sg: "json", exts: &["json"] },
    Language { name: "yaml", rg: "yaml", sg: "yaml", exts: &["yaml", "yml"] },
    Language { name: "yml", rg: "yaml", sg: "yaml", exts: &["yaml", "yml"] },
    Language { name: "sql", rg: "sql", sg: "sql", exts: &["sql"] },
    Language { name: "toml", rg: "toml", sg: "", exts: &["toml"] },
    Language { name: "markdown", rg: "md", sg: "", exts: &["md", "markdown"] },
    Language { name: "md", rg: "md", sg: "", exts: &["md", "markdown"] },
];

/// The row for `name`, matched case-insensitively.
pub fn resolve(name: &str) -> Option<&'static Language> {
    let wanted = name.to_ascii_lowercase();
    TABLE.iter().find(|l| l.name == wanted)
}

/// Every accepted name, for the error a rejected one prints.
pub fn accepted() -> String {
    TABLE
        .iter()
        .map(|l| l.name)
        .collect::<Vec<_>>()
        .join(", ")
}
