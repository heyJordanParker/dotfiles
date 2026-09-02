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
- `tests/architecture_commands.rs` and `tests/declarations_and_references.rs` pin the relations commands and the resolution model.
- `tests/docs_load.rs` and `tests/docs_graph.rs` pin doc-graph recognition.
- `tests/docs_prime.rs` pins the context-primer auto-load set.
- `tests/session_log.rs` and `tests/docs_load.rs` pin content-hash dedupe in the session log.
- `tests/drift.rs` pins context-primer drift reconciliation.
- `tests/docs_load.rs` pins user-global Rule scope splitting.
- `tests/document.rs` pins the one `{query, context, results, counts}` shape and the `--filter` contract.
- `tests/enrichment.rs` pins the per-file enrichment every command carries.
- `tests/search.rs`, `tests/git.rs`, and `tests/inspect.rs` pin the search, git, and inspection commands.
- `tests/speed.rs` contains loose regression tripwires for command latency.
- Binary-file `read` exits 0 and prints replacement characters.
- `survey` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- `status` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- Explicit not-found errors use stderr and exit 2.
