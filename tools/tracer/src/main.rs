//! tracer — code-intelligence CLI. Binary name: `trace`.
//! Per-function cyclomatic complexity is AST-derived (tree-sitter
//! decision-node walker), the single CCN backend.

mod architecture;
mod cache;
mod ccn;
mod commands;
mod digest;
mod extraction;
mod file_facts;
mod filter;
mod git_activity;
mod jsonfmt;
mod output;
mod passive_context;
mod pathval;
mod repo_context;
mod repo_files;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "trace",
    version,
    about = "Code intelligence CLI for mapping architectural relationships."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Run a jq program over this command's JSON output, in-process.
    /// Requires --json. Replaces piping `trace ... --json | jq`.
    #[arg(long, global = true, value_name = "JQ")]
    filter: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Complexity structure + architectural overview of a file or directory.
    Info {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        brief: bool,
    },
    /// Verify required external binaries are installed.
    Doctor,
    /// Repo-wide language + LOC + complexity distribution.
    Survey {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Manage the .tracer-cache/ disk cache.
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    /// Methods, properties, variables, imports, and exports for one file.
    Structure {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Text search via ripgrep with per-match architectural enrichment.
    Grep {
        pattern: String,
        #[arg(short = 'l', long)]
        lang: Option<String>,
        #[arg(long, default_value = ".")]
        path: String,
        #[arg(long)]
        json: bool,
    },
    /// Structural (AST) search via ast-grep with per-match enrichment.
    Struct {
        pattern: String,
        #[arg(short = 'l', long)]
        lang: String,
        #[arg(long, default_value = ".")]
        path: String,
        #[arg(long)]
        json: bool,
    },
    /// All places a symbol is defined, via the architecture graph.
    Defines {
        symbol: String,
        #[arg(long)]
        json: bool,
    },
    /// Direct callers / importers of a symbol via the architecture graph.
    Callers {
        symbol: String,
        #[arg(long)]
        json: bool,
    },
    /// Module-level symbols of a file from the architecture graph.
    Symbols {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// What a symbol depends on (transitive), or highest-coupling symbols in a path.
    Upstream {
        symbol: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        depth: i64,
        #[arg(long, default_value_t = 10)]
        limit: i64,
        #[arg(long)]
        json: bool,
    },
    /// What depends on a symbol (transitive), or most-depended-on symbols in a path.
    Downstream {
        symbol: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        depth: i64,
        #[arg(long, default_value_t = 10)]
        limit: i64,
        #[arg(long)]
        json: bool,
    },
    /// One-level annotated ls: files + sub-directories with passive context.
    List {
        path: PathBuf,
        #[arg(long = "all")]
        show_hidden: bool,
        #[arg(long)]
        json: bool,
    },
    /// Annotated file tree with per-file complexity ranks (recursive).
    Tree {
        path: PathBuf,
        #[arg(long, default_value_t = 4)]
        depth: usize,
        #[arg(long)]
        json: bool,
    },
    /// Find files (or directories) by name pattern with code intelligence.
    Find {
        pattern: String,
        #[arg(default_value = ".")]
        base: String,
        #[arg(long = "path")]
        path_filter: Option<String>,
        #[arg(long = "exclude")]
        excludes: Vec<String>,
        #[arg(long = "type", value_parser = ["f", "d"], default_value = "f")]
        type_filter: String,
        #[arg(long, default_value_t = 200)]
        limit: usize,
        #[arg(long, value_parser = ["complexity", "recent", "path"], default_value = "path")]
        sort: String,
        #[arg(long)]
        json: bool,
    },
    /// File-path pattern search, Claude Glob shape (`**` recursive).
    Glob {
        pattern: String,
        #[arg(default_value = ".")]
        base: String,
        #[arg(long)]
        details: bool,
        #[arg(long)]
        json: bool,
    },
    /// Files (or symbols) changed between HEAD and a base ref, load-bearing first.
    Diff {
        #[arg(long, default_value = commands::diff::DEFAULT_BASE)]
        base: String,
        #[arg(long = "symbols")]
        symbol_mode: bool,
        #[arg(long)]
        json: bool,
    },
    /// Working-tree dirty set with code intelligence, ordered by blast radius.
    Status {
        #[arg(long)]
        json: bool,
        #[arg(long = "state", value_parser = ["added", "renamed", "modified", "deleted", "untracked"])]
        state: Option<String>,
    },
    /// Deduped project-docs set (Claude.md / rules ancestors) for a path.
    Docs {
        path: PathBuf,
        #[arg(long = "directory")]
        directory: bool,
        #[arg(long)]
        json: bool,
    },
    /// Session-start primer (no args) or single-file enrichment (path arg).
    Context {
        path: Option<PathBuf>,
        #[arg(long = "directory")]
        force_directory: bool,
    },
    /// Cleaned read: whole file, method, line range, or anchor section; worktree or git ref.
    Read {
        #[arg(required = true)]
        paths: Vec<String>,
        #[arg(long = "method")]
        method: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        raw: bool,
        #[arg(long = "at", value_name = "REF")]
        at: Option<String>,
        #[arg(long = "lines", value_name = "L1:L2")]
        lines: Option<String>,
        #[arg(long, num_args = 2, value_names = ["START", "END"])]
        between: Option<Vec<String>>,
        #[arg(long = "diff")]
        as_diff: bool,
        /// Inject project-docs content (off by default).
        #[arg(long = "docs")]
        docs: bool,
    },
    /// Symbol-aware blame collapsed into per-region commit summaries.
    Blame {
        file: PathBuf,
        symbol: Option<String>,
        #[arg(long = "lines", value_name = "L1:L2")]
        lines: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Git archaeology: whole-file log, function-line history, or pickaxe.
    History {
        file: Option<PathBuf>,
        symbol: Option<String>,
        #[arg(long)]
        contains: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CacheCommand {
    /// Prebuild the cache for a repo so the first agent query is fast.
    Build {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Delete cache entries.
    Clear {
        /// Limit clear to one namespace; default clears both.
        #[arg(long, value_parser = ["file", "architecture"])]
        namespace: Option<String>,
        /// Remove .tracer-cache/ entirely.
        #[arg(long = "all")]
        clear_all: bool,
    },
    /// Show cache size and entry count per namespace.
    Stats {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = cli.filter.as_deref();
    match cli.command {
        Command::Info { path, json, brief } => {
            output::run_value(json, filter, || commands::info::run(&path, json, brief))
        }
        Command::Doctor => {
            output::guard(false, filter)?;
            commands::doctor::run()
        }
        Command::Survey { path, json } => {
            output::run_value(json, filter, || commands::survey::run(&path, json))
        }
        Command::Cache { command } => match command {
            CacheCommand::Build { path } => {
                output::guard(false, filter)?;
                commands::cache::build(&path)
            }
            CacheCommand::Clear {
                namespace,
                clear_all,
            } => {
                output::guard(false, filter)?;
                commands::cache::clear(Path::new("."), namespace.as_deref(), clear_all)
            }
            CacheCommand::Stats { json } => output::run_value(json, filter, || {
                commands::cache::stats(Path::new("."), json)
            }),
        },
        Command::Structure { path, json } => {
            output::run_value(json, filter, || commands::structure::run(&path, json))
        }
        Command::Grep {
            pattern,
            lang,
            path,
            json,
        } => output::run_value(json, filter, || {
            commands::grep::run(&pattern, lang.as_deref(), &path, json)
        }),
        Command::Struct {
            pattern,
            lang,
            path,
            json,
        } => output::run_value(json, filter, || {
            commands::struct_::run(&pattern, &lang, &path, json)
        }),
        Command::Defines { symbol, json } => {
            output::run_value(json, filter, || commands::defines::run(&symbol, json))
        }
        Command::Callers { symbol, json } => {
            output::run_value(json, filter, || commands::callers::run(&symbol, json))
        }
        Command::Symbols { file, json } => {
            output::run_value(json, filter, || commands::symbols::run(&file, json))
        }
        Command::Upstream {
            symbol,
            path,
            depth,
            limit,
            json,
        } => output::run_value(json, filter, || {
            commands::upstream::run(symbol.as_deref(), path.as_deref(), depth, limit, json)
        }),
        Command::Downstream {
            symbol,
            path,
            depth,
            limit,
            json,
        } => output::run_value(json, filter, || {
            commands::downstream::run(symbol.as_deref(), path.as_deref(), depth, limit, json)
        }),
        Command::List {
            path,
            show_hidden,
            json,
        } => output::run_value(json, filter, || {
            commands::list_::run(&path, show_hidden, json)
        }),
        Command::Tree { path, depth, json } => {
            output::run_value(json, filter, || commands::tree::run(&path, depth, json))
        }
        Command::Find {
            pattern,
            base,
            path_filter,
            excludes,
            type_filter,
            limit,
            sort,
            json,
        } => output::run_value(json, filter, || {
            commands::find::run(
                &pattern,
                &base,
                path_filter,
                excludes,
                type_filter,
                limit,
                sort,
                json,
            )
        }),
        Command::Glob {
            pattern,
            base,
            details,
            json,
        } => output::run_value(json, filter, || {
            commands::glob::run(&pattern, &base, details, json)
        }),
        Command::Diff {
            base,
            symbol_mode,
            json,
        } => output::run_value(json, filter, || {
            commands::diff::run(&base, symbol_mode, json)
        }),
        Command::Status { json, state } => output::run_value(json, filter, || {
            commands::status::run(json, state.as_deref())
        }),
        Command::Docs {
            path,
            directory,
            json,
        } => output::run_value(json, filter, || {
            commands::docs::run(&path, directory, json)
        }),
        Command::Context {
            path,
            force_directory,
        } => {
            output::guard(false, filter)?;
            commands::context::run(path.as_deref(), force_directory)
        }
        Command::Read {
            paths,
            method,
            json,
            raw,
            at,
            lines,
            between,
            as_diff,
            docs,
        } => output::run_value(json, filter, || {
            let docs_override = if docs { Some(true) } else { None };
            commands::read::run(
                &paths,
                method.as_deref(),
                json,
                raw,
                at.as_deref(),
                lines.as_deref(),
                between.map(|v| (v[0].clone(), v[1].clone())),
                as_diff,
                docs_override,
            )
        }),
        Command::Blame {
            file,
            symbol,
            lines,
            json,
        } => output::run_value(json, filter, || {
            commands::blame::run(&file, symbol.as_deref(), lines.as_deref(), json)
        }),
        Command::History {
            file,
            symbol,
            contains,
            json,
        } => output::run_value(json, filter, || {
            commands::history::run(
                file.as_deref(),
                symbol.as_deref(),
                contains.as_deref(),
                json,
            )
        }),
    }
}
