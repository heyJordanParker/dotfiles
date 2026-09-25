# WHY

Black-box behavior contract for the `trace` binary, pinning the observable surface that Agents and Skills consume instead of linking against tracer internals.

# Facts

- The Rust test package is named `tracer-cli-tests`.
- The suite drives the `trace` binary as a subprocess.
- `TRACE_BIN` selects the binary under test.
- `TRACE_BIN` defaults to `trace` on `PATH`.
- The suite asserts exit codes, stdout, stderr, JavaScript Object Notation shape, and wall-clock latency.
- `src/lib.rs` is the shared test harness.
- Each test fixture owns a hermetic on-disk git repository.
- `tests/worktree_anchoring.rs` pins the worktree-anchored `.tracer-cache/` contract.
- `tests/cache_and_backend.rs` pins the cache lifecycle and the single AST complexity backend.
- `tests/freshness.rs` pins that every call reflects the bytes on disk when it runs.
- `tests/concurrency.rs` pins that concurrent calls serialize index maintenance onto one update and all answer cleanly.
- `tests/architecture_commands.rs` and `tests/declarations_and_references.rs` pin the relations commands and the resolution model.
- `tests/docs_load.rs` and `tests/docs_graph.rs` pin doc-graph recognition.
- `tests/docs_prime.rs` pins the context-primer auto-load set.
- `tests/session_log.rs` and `tests/docs_load.rs` pin content-hash dedupe in the session log.
- `tests/docs_load.rs` pins user-global Rule scope splitting.
- `tests/document.rs` pins the one `{query, context, results, counts}` shape and the `--filter` contract.
- `tests/enrichment.rs` pins the per-file enrichment every command carries.
- `tests/search.rs`, `tests/git.rs`, and `tests/inspect.rs` pin the search, git, and inspection commands.
- `tests/declarations_and_references.rs` pins PSR-4 and `paths` resolution and the external rule.
- `tests/declarations_and_references.rs` and `tests/enrichment.rs` pin declaration classification and annotations.
- `tests/architecture_commands.rs` and `tests/primer.rs` pin the `stats` directory table and the primer's sections.
- `tests/enrichment.rs` pins the YAML front matter, its `directory` block on every call, and `--budget` fitting on `read`, `context`, and `docs`.
- `tests/declarations_and_references.rs` holds the per-language inventory fixture: one source per language whose declarations cover that language's row set, asserting `header`, `header_line`, `line`, `end_line`, `container`, and `parent` for each.
- `tests/declarations_and_references.rs` pins the ctags fallback on a shell fixture, the data-format deny list, retained Markdown headings, and that a warm `context` or `read` spawns no ctags.
- `tests/enrichment.rs` pins the surface on the first and second `context` of one session, on `read`, `info`, a window, `--offset`/`--limit`, the batch `--json` `results[].content`, `read --at`, and a budget-trimmed read.
- `tests/enrichment.rs` pins that a warm file call times one `git status`, one `rev-parse HEAD`, and one `for-each-ref` and no `ls-files` or `show-toplevel`, and that a 300-entry directory line costs no per-file facts.
- `tests/search.rs` pins matches grouped under their declarations and the `declaration` and `type` JavaScript Object Notation fields on `grep`, `grep --at`, and `pattern`, ripgrep's flags on `grep`, several path arguments, and the one-line-per-directory listing when bare paths overrun the budget.
- `tests/git.rs` pins the `changed:`, `removed:`, `added:`, and `touches:` rows on a `diff` row and their four JavaScript Object Notation arrays.
- `tests/architecture_commands.rs` pins the surface row on a `defines`, `usages`, and `dependencies` result and the unfabricated module row.
- `tests/session_log.rs` and `tests/concurrency.rs` pin the session directory baseline and the `status` and `diff` directory block.
- `tests/session_log.rs` pins the session-log event set and read coverage.
- `tests/concurrency.rs` holds the deadlock tripwire: a 600-source fixture whose primer runs twenty times under a ten-second deadline.
- `tests/speed.rs` contains loose regression tripwires for command latency; the speed gate is the `cargo xtask bench` run `tools/tracer/Claude.md` describes.
- Binary-file `read` exits 0 and names the file's size instead of its bytes.
- `survey` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- `status` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- Explicit not-found errors use stderr and exit 2.
