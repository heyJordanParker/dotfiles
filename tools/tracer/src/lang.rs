//! One language table for every search.
//!
//! `grep -t` filters through ripgrep's file types and `pattern -t` parses
//! through ast-grep's languages. The two spell languages differently: ripgrep
//! has no `tsx` type (`.tsx` lives under `ts`) while ast-grep needs `tsx` to
//! parse one. Before this table `trace grep -l tsx` returned zero matches with
//! no error — ripgrep rejected the type, its stderr was discarded, and an
//! empty result reads to an agent as "this does not exist". A language name
//! now means the same thing on every search, and a name nothing knows is
//! refused by name instead of answered with silence.

/// One row: what the caller writes, what ripgrep filters on, and what
/// ast-grep parses with. A row whose `sg` is empty is a language ast-grep has
/// no grammar for, so `pattern` refuses it while `grep` still filters on it.
pub struct Language {
    pub name: &'static str,
    pub rg: &'static str,
    pub sg: &'static str,
}

/// The table, in the words agents write. Aliases sit beside their canonical
/// name rather than in a second map, so one row carries the whole answer.
const TABLE: &[Language] = &[
    Language {
        name: "python",
        rg: "py",
        sg: "python",
    },
    Language {
        name: "py",
        rg: "py",
        sg: "python",
    },
    Language {
        name: "javascript",
        rg: "js",
        sg: "javascript",
    },
    Language {
        name: "js",
        rg: "js",
        sg: "javascript",
    },
    Language {
        name: "jsx",
        rg: "js",
        sg: "jsx",
    },
    Language {
        name: "typescript",
        rg: "ts",
        sg: "typescript",
    },
    Language {
        name: "ts",
        rg: "ts",
        sg: "typescript",
    },
    Language {
        name: "tsx",
        rg: "ts",
        sg: "tsx",
    },
    Language {
        name: "php",
        rg: "php",
        sg: "php",
    },
    Language {
        name: "rust",
        rg: "rust",
        sg: "rust",
    },
    Language {
        name: "rs",
        rg: "rust",
        sg: "rust",
    },
    Language {
        name: "go",
        rg: "go",
        sg: "go",
    },
    Language {
        name: "java",
        rg: "java",
        sg: "java",
    },
    Language {
        name: "kotlin",
        rg: "kotlin",
        sg: "kotlin",
    },
    Language {
        name: "swift",
        rg: "swift",
        sg: "swift",
    },
    Language {
        name: "scala",
        rg: "scala",
        sg: "scala",
    },
    Language {
        name: "ruby",
        rg: "ruby",
        sg: "ruby",
    },
    Language {
        name: "rb",
        rg: "ruby",
        sg: "ruby",
    },
    Language {
        name: "c",
        rg: "c",
        sg: "c",
    },
    Language {
        name: "cpp",
        rg: "cpp",
        sg: "cpp",
    },
    Language {
        name: "csharp",
        rg: "csharp",
        sg: "csharp",
    },
    Language {
        name: "cs",
        rg: "csharp",
        sg: "csharp",
    },
    Language {
        name: "elixir",
        rg: "elixir",
        sg: "elixir",
    },
    Language {
        name: "lua",
        rg: "lua",
        sg: "lua",
    },
    Language {
        name: "bash",
        rg: "sh",
        sg: "bash",
    },
    Language {
        name: "sh",
        rg: "sh",
        sg: "bash",
    },
    Language {
        name: "shell",
        rg: "sh",
        sg: "bash",
    },
    Language {
        name: "html",
        rg: "html",
        sg: "html",
    },
    Language {
        name: "css",
        rg: "css",
        sg: "css",
    },
    Language {
        name: "json",
        rg: "json",
        sg: "json",
    },
    Language {
        name: "yaml",
        rg: "yaml",
        sg: "yaml",
    },
    Language {
        name: "yml",
        rg: "yaml",
        sg: "yaml",
    },
    Language {
        name: "sql",
        rg: "sql",
        sg: "sql",
    },
    Language {
        name: "toml",
        rg: "toml",
        sg: "",
    },
    Language {
        name: "markdown",
        rg: "md",
        sg: "",
    },
    Language {
        name: "md",
        rg: "md",
        sg: "",
    },
];

/// The row for `name`, matched case-insensitively.
pub fn resolve(name: &str) -> Option<&'static Language> {
    let wanted = name.to_ascii_lowercase();
    TABLE.iter().find(|l| l.name == wanted)
}

/// Every accepted name, for the error a rejected one prints.
pub fn accepted() -> String {
    TABLE.iter().map(|l| l.name).collect::<Vec<_>>().join(", ")
}

/// `grep -t` names as ripgrep file types, and the matcher ripgrep builds from
/// them: a table name maps to its ripgrep type, any other name is taken as a
/// ripgrep type (`rg --type-list`), and one ripgrep does not know is refused
/// by name. `grep --at` filters a commit's files through the same matcher, so
/// a type selects the same files on a commit as on the working tree.
pub fn ripgrep_types(names: &[String]) -> Result<(Vec<String>, ignore::types::Types), String> {
    let mut builder = ignore::types::TypesBuilder::new();
    builder.add_defaults();
    let mut types = Vec::new();
    for name in names {
        let rg = resolve(name).map_or(name.as_str(), |row| row.rg);
        if !builder.definitions().iter().any(|definition| definition.name() == rg) {
            return Err(format!(
                "unknown type {name:?}. Accepted: {}, or any `rg --type-list` name",
                accepted()
            ));
        }
        builder.select(rg);
        types.push(rg.to_string());
    }
    let matcher = builder.build().map_err(|error| error.to_string())?;
    Ok((types, matcher))
}
