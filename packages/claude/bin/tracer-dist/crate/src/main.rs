//! tracer — code-intelligence CLI. Binary name: `trace`.
//! Per-function cyclomatic complexity is AST-derived (tree-sitter
//! decision-node walker), the single CCN backend.

mod architecture;
mod cache;
mod ccn;
mod commands;
mod digest;
mod docs_graph;
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
    /// Timestamped entries from log files, including the ones the ignore
    /// walk skips. One line is one entry; a line with no timestamp of its
    /// own attaches to the entry above it, so a stack trace stays whole.
    Logs {
        pattern: Option<String>,
        #[arg(long, default_value = ".")]
        path: String,
        /// Filename glob under <path>. The default reaches access.log,
        /// access.log.1, access.log.2.gz, and laravel-2026-08-15.log.
        #[arg(long = "file", default_value = "*.log*")]
        file_glob: String,
        /// Window start: YYYY-MM-DD, 'YYYY-MM-DD HH:MM[:SS]', or HH:MM[:SS]
        /// on the day of the newest log selected.
        #[arg(long)]
        since: Option<String>,
        /// Window end, same three forms.
        #[arg(long)]
        until: Option<String>,
        /// Whole entries either side of each match.
        #[arg(long, default_value_t = 0)]
        around: usize,
        /// Cap on matching entries, keeping the newest.
        #[arg(long, default_value_t = 20)]
        limit: usize,
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
    /// One-level directory listing from the filesystem: every file on disk
    /// (gitignored included) with stat, code, and git column groups.
    List {
        path: PathBuf,
        #[arg(long = "all")]
        show_hidden: bool,
        /// Order files newest-first by filesystem mtime.
        #[arg(long)]
        recent: bool,
        /// Cap the file rows after ordering (no default cap); `entries=N`
        /// always reports the pre-cap total.
        #[arg(long)]
        limit: Option<usize>,
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
    /// Project-docs surface: path-scoped deduped set (default), `--graph` for
    /// the whole-repo docs graph, `load` for the hook-driven entrypoint
    /// (thin alias for path-mode), or `status` for the session manifest.
    #[command(args_conflicts_with_subcommands = true)]
    Docs {
        /// Path for the default path-mode (`trace docs <path>`) or for
        /// `--graph` (optional; defaults to the cwd's repo root). Replaced by
        /// any present sub-verb (`load`, `status`).
        path: Option<PathBuf>,
        /// Treat <path> as a directory even when it points at a file (path-mode only).
        #[arg(long = "directory")]
        directory: bool,
        /// Whole-repo docs graph projected from the unified `architecture/`
        /// cache entry, plus the available-but-not-loaded set. With this
        /// flag, <path> is optional.
        #[arg(long = "graph")]
        graph: bool,
        /// Names the calling surface (e.g. `trace_inject_hook`, `agent_read`).
        /// Lands verbatim in the log event's `source` field.
        #[arg(long, default_value = "trace_docs")]
        source: String,
        /// Tool that triggered this load (Bash, Read, Glob, …). Recorded
        /// on the log event for downstream auditing.
        #[arg(long = "triggering-tool")]
        triggering_tool: Option<String>,
        /// Command string that triggered this load (the agent's Bash
        /// invocation, the Read file_path, etc.).
        #[arg(long = "triggering-command")]
        triggering_command: Option<String>,
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        command: Option<DocsCommand>,
    },
    /// Session-start primer (no args / path arg) or `prime` sub-verb that
    /// records the docs Claude Code's harness auto-loaded into the session log.
    #[command(args_conflicts_with_subcommands = true)]
    Context {
        path: Option<PathBuf>,
        #[arg(long = "directory")]
        force_directory: bool,
        /// 1-based line the read started at (the read tool's `offset`); records
        /// which slice of the file the agent read for per-file read coverage.
        #[arg(long)]
        offset: Option<usize>,
        /// Number of lines the read covered (the read tool's `limit`).
        #[arg(long)]
        limit: Option<usize>,
        /// Render the file shoulder without recording a read. The enrich hook
        /// sets this for Edit/Write — an edit gets the file's architectural
        /// shoulder but is not a read, so it must not count toward per-file
        /// read coverage.
        #[arg(long = "no-record")]
        no_record: bool,
        #[command(subcommand)]
        command: Option<ContextCommand>,
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
        /// Return the whole selection with no size budget and no trim marker.
        #[arg(long = "all")]
        all: bool,
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
enum DocsCommand {
    /// Hook entrypoint: thin alias forwarding to path-mode with the
    /// `--source` default flipped to `trace_docs_load`. Returns the same
    /// `{ docs, doc_count, already_loaded? }` shape as `trace docs <path>`
    /// so the calling hook reads one contract.
    Load {
        path: PathBuf,
        /// Names the calling surface (e.g. `trace_inject_hook`, `agent_read`).
        /// Lands verbatim in the log event's `source` field.
        #[arg(long, default_value = "trace_docs_load")]
        source: String,
        /// Tool that triggered this load (Bash, Read, Glob, …). Recorded
        /// on the log event for downstream auditing.
        #[arg(long = "triggering-tool")]
        triggering_tool: Option<String>,
        /// Command string that triggered this load (the agent's Bash
        /// invocation, the Read file_path, etc.).
        #[arg(long = "triggering-command")]
        triggering_command: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Agent-facing "what do I have right now?" query against the session
    /// log. No path arg → the full session manifest with source
    /// attribution. With a path arg → that path's ancestor chain
    /// partitioned into loaded (with source) and not_loaded.
    Status {
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Clear the surfaced-docs state for the current session so a subsequent
    /// `trace docs <path>` re-surfaces docs as new. Driven by the Codex
    /// compaction/clear hook: a context reset drops injected rule text from
    /// the model, so the surfaced-docs state must reset to re-inject it.
    /// Append-only history is preserved — only the materialized view is cleared.
    Reset {
        /// Names the calling surface (e.g. `codex_compact_hook`). Lands
        /// verbatim in the log event's `source` field.
        #[arg(long, default_value = "trace_docs_reset")]
        source: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ContextCommand {
    /// Record the harness's auto-loaded docs into the session log so
    /// subsequent tracer emissions skip docs the agent already has in
    /// context. Invoked by the SessionStart / post-compact hook.
    Prime {
        #[arg(long, value_parser = ["session_start", "post_compact"])]
        reason: String,
        /// Path to the observed-set JSON (or `-` for stdin). When set,
        /// drift between the context primer's prediction and Claude Code's
        /// actual auto-load is detected and recorded into the session log.
        #[arg(long = "observed-from", value_name = "PATH")]
        observed_from: Option<String>,
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
        Command::Logs {
            pattern,
            path,
            file_glob,
            since,
            until,
            around,
            limit,
            json,
        } => output::run_value(json, filter, || {
            commands::logs::run(
                pattern.as_deref(),
                &path,
                &file_glob,
                since.as_deref(),
                until.as_deref(),
                around,
                limit,
                json,
            )
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
            recent,
            limit,
            json,
        } => output::run_value(json, filter, || {
            commands::list_::run(&path, show_hidden, recent, limit, json)
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
            graph,
            source,
            triggering_tool,
            triggering_command,
            json,
            command,
        } => match command {
            Some(DocsCommand::Load {
                path,
                source,
                triggering_tool,
                triggering_command,
                json,
            }) => output::run_value(json, filter, || {
                commands::docs::run(
                    &path,
                    false,
                    &source,
                    triggering_tool.as_deref(),
                    triggering_command.as_deref(),
                    json,
                )
            }),
            Some(DocsCommand::Status { path, json }) => output::run_value(json, filter, || {
                commands::docs::run_status(path.as_deref(), json)
            }),
            Some(DocsCommand::Reset { source, json }) => output::run_value(json, filter, || {
                commands::docs::run_reset(&source, json)
            }),
            None if graph => output::run_value(json, filter, || {
                commands::docs::run_graph(path.as_deref(), json)
            }),
            None => output::run_value(json, filter, || {
                let target = path.unwrap_or_else(|| {
                    eprintln!("Error: <PATH> is required (use `trace docs --graph` for the whole-repo graph)");
                    std::process::exit(2)
                });
                commands::docs::run(
                    &target,
                    directory,
                    &source,
                    triggering_tool.as_deref(),
                    triggering_command.as_deref(),
                    json,
                )
            }),
        },
        Command::Context {
            path,
            force_directory,
            offset,
            limit,
            no_record,
            command,
        } => match command {
            Some(ContextCommand::Prime {
                reason,
                observed_from,
                json,
            }) => output::run_value(json, filter, || {
                let parsed = commands::context_prime::parse_reason(&reason)?;
                commands::context_prime::run(parsed, observed_from.as_deref(), json)
            }),
            None => {
                output::guard(false, filter)?;
                commands::context::run(path.as_deref(), force_directory, offset, limit, !no_record)
            }
        },
        Command::Read {
            paths,
            method,
            json,
            raw,
            all,
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
                all,
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
