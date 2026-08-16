# WHY

Local code-intelligence command-line interface for Agents working in a repository, returning dense structured facts about files, symbols, project docs, git history, and session context in one call.

# Facts

- The Rust package is named `tracer`.
- The binary is named `trace`.
- `tools/tracer` is a Cargo workspace with an `xtask` member.
- `trace` works without Claude Code.
- `doctor` verifies required external binaries.
- The external binaries are `ast-grep`, `scc`, `universal-ctags`, `ripgrep`, and `git`.
- Tree-sitter grammars are compiled into the binary.
- Per-function complexity is computed by the in-process tree-sitter decision-node walker.
- `.tracer-cache/` lives at the target repository root.
- The cache namespaces are `file/`, `architecture/`, and `sessions/<session_id>/<agent_id>/`.
- The `file/` namespace stores per-file facts.
- The `architecture/` namespace stores the unified symbol graph, module graph, and doc-file graph.
- The `sessions/` namespace stores session-context events and the materialized session view.
- `commands::session_log` is the single owner of session-context state.
- Recognized project-doc files include `CLAUDE.md`, `Claude.md`, `AGENTS.md`, `Agents.md`, their `.local.md` peers, and `.claude/rules/*.md`.
- `trace docs --graph` projects the doc-file graph from the `architecture/` namespace.
- `trace docs status` is the Agent-facing query for loaded docs and read coverage.
- `trace docs reset` clears the current session's surfaced-docs view.
- User-global `$HOME/.claude/rules/*.md` files are included in the `trace docs` walk.
- Directory-scoped `trace docs` calls surface unconditional user-global Rules.
- File-scoped `trace docs` calls surface conditional user-global Rules matched against that file.
- `commands::logs` reads log files directly, so a gitignored or untracked log is searchable.
- `commands::logs` frames one entry per line and attaches an untimestamped line to the entry above it.
- `--filter` runs an in-process jq program through the `jaq` crates.
- `--filter` requires `--json`.
- `jsonfmt` owns the stable JavaScript Object Notation byte format for command output and cache entries.
- `setup.sh` builds the release binary and installs it to `~/.local/bin/trace`.
- `packages/claude/bin/trace` is the plugin-distributed launcher.
- `packages/claude/bin/tracer-dist/crate/` is the plugin build-from-source fallback mirror.
- `cargo xtask sync-dist` regenerates the plugin fallback mirror.
- `cargo xtask build-bin` builds every prebuilt the plugin ships from that mirror.
- The shipped prebuilts are `mac-arm64`, `linux-x86_64`, and `linux-arm64`.
- The Linux prebuilts cross-compile on the host through `cargo-zigbuild`.
- The Linux prebuilts pin their glibc floor at 2.17 through the target triple.
- `build-bin` runs every compile through `rustup run stable cargo`.
- `packages/claude/bin/tracer-dist/bin/source.sha256` records the crate the prebuilts were built from.
- `scripts/sync.py` rebuilds a mirror or prebuilt that has fallen behind the tracer source.
- `tools/tracer/tests` contains the black-box command-line test suite.
