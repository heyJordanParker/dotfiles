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
- `tests/docs_load.rs`, `tests/docs_graph.rs`, and `tests/context_prime.rs` pin doc-graph recognition.
- `tests/session_log.rs` and `tests/docs_load.rs` pin content-hash dedupe in the session log.
- `tests/context_prime.rs` pins the context-primer auto-load set.
- `tests/drift.rs` pins context-primer drift reconciliation.
- `tests/docs_load.rs` pins user-global Rule scope splitting.
- `tests/speed.rs` contains loose regression tripwires for command latency.
- Binary-file `read` exits 0 and prints replacement characters.
- `survey` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- `status` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- Explicit not-found errors use stderr and exit 2.
