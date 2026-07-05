# trace cache

Use this Reference when a trace command is slow, stale, missing dependencies, or needs prebuild.

## 1. Know the cache namespaces

### `file/` stores per-file facts
It stores complexity, lines of code, language, imports, exports, and git activity. It invalidates per file when content changes.

### `architecture/` stores the cross-file graph
It stores symbol and module nodes for code, doc-file nodes for `Claude.md`, `CLAUDE.md`, `Agents.md`, `AGENTS.md`, their `.local.md` peers, and `.claude/rules/*.md`, plus `@include` edges and conditional `paths:` frontmatter. It invalidates when any per-file SHA changes, git HEAD moves, or a tracked doc-file mtime changes.

### `sessions/<session_id>/<agent_id>/` stores Context logs
It stores `events.jsonl` and `view.json` for the per-session, per-Agent docs Context log. It no-ops without a session id.

## 2. Prebuild before heavy use

### Build the cache explicitly
The first Architecture command in a fresh repository builds the graph, typically five to thirty seconds for about one thousand files while respecting `.gitignore`. Later commands return well under a second.

Template:
  ```bash
  trace cache build [<path>]
  ```

## 3. Inspect or clear cache state

### Use cache verbs instead of deleting files
`trace cache stats` reports entries and bytes per namespace. `trace cache clear` clears the chosen namespace.

Template:
  ```bash
  trace cache stats
  trace cache clear --namespace file
  trace cache clear --namespace architecture
  trace cache clear --all
  ```

## 4. Verify installation

IF a trace command errors with missing dependencies:
### Run `trace doctor`
`trace doctor` verifies ast-grep, scc, universal-ctags, ripgrep, and git, then prints per-platform install instructions.

### Plugin users get the binary on PATH
When the plugin is enabled, the `trace` binary lands on PATH automatically.

### Standalone users build from `tools/tracer`
Build with `cargo build --release`, put `target/release/trace` on PATH, then run `trace doctor`.
