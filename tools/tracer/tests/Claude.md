# Tracer Test Suite
v1.7 | Updated: 2026-05-17

## Why

The consuming skill and explorer agent depend only on the CLI's observable surface, not on how it is produced. This suite is the behavior contract: one set of black-box assertions that pins `trace`'s observable behavior — exit codes, stdout, stderr, `--json` shape, latency — so any change that alters that surface is caught, without the suite ever trusting the binary's internals.

## What

A Rust integration-test crate that drives the `trace` binary as a subprocess and asserts only on observable behavior: exit code, stdout, stderr, `--json` document shape, and wall-clock latency. The binary under test is selectable via `TRACE_BIN`, so the same suite validates any built `trace` binary.

### Requirements

- The binary under test must be resolved from the `TRACE_BIN` environment variable, defaulting to `trace` on `PATH` — this single seam lets the suite run against any build (PATH binary, a specific `target/release/trace`, the plugin launcher)
- Every test must own a hermetic on-disk git fixture (isolated `HOME`, fixed author/committer, no system git config) deleted on drop — the suite must never depend on the developer's environment or leak state between parallel tests
- Assertions must cover only the CLI's observable surface; the green test count is the contract — coverage must not silently shrink

### Boundaries

- Never import or link tracer internals — the only contract is the CLI's observable surface; importing internals would let a bug pass by matching itself
- Never rename the `TRACE_BIN` or `TRACER_CCN_BACKEND` environment-variable names — they are an external protocol shared with the tracer, its docs, and the hooks. Changing a protocol name is a contract change: surface it, never do it as part of a naming pass

## Workflow

### Running

```bash
cd tools/tracer/tests
cargo test                 # every test against `trace` on PATH
cargo test -- --nocapture  # show panics with full stdout/stderr diagnostics
```

Tests spawn real subprocesses and create throwaway git repositories under the system temp dir. They run in parallel; each test owns an isolated fixture, so there is no shared cache state.

### Running against a specific binary

`TRACE_BIN` selects the binary under test:

```bash
cargo test                                              # `trace` on PATH
TRACE_BIN=/abs/path/to/target/release/trace cargo test  # a specific build
```

A green run means that binary reproduces every behavior the suite pins, within the speed budgets.

### Layout

```
tests/
├── src/lib.rs                        shared harness: TRACE_BIN resolution, subprocess runner,
│                                      timing, hermetic git-fixture builder, standard_repo()
└── tests/
    ├── per_file_commands.rs          doctor, read (+ docs toggle), docs, info, structure,
    │                                  tree, list, survey, context; cross-command session dedupe
    ├── architecture_commands.rs      callers, defines, symbols, upstream/downstream
    ├── search_commands.rs            grep, struct, find, glob
    ├── git_commands.rs               history, blame, diff, status
    ├── cache_and_backend.rs          build/warm/invalidate, namespaces, clear, stats,
    │                                  TRACER_CCN_BACKEND + cache segregation
    ├── primer.rs                     trace context (no args) — all sections + cache warming
    ├── edge_cases.rs                 missing/outside-repo/empty/non-source/binary/large paths
    ├── filter.rs                     global --filter: requires --json, fail-fast, stream, identity
    └── speed.rs                      per-command wall-clock budgets; cold-vs-warm pairs
```

## How

### Speed thresholds

The budgets in `tests/speed.rs` are deliberately loose regression tripwires, not micro-benchmarks. Raise them if a constrained CI runner flakes; the cold-vs-warm ratio check is the signal that survives threshold tuning and catches a cache that silently stopped working.

### Pinned behaviors

The suite pins these deliberate behaviors — they are the contract, not accidents of implementation:

- Binary-file `read` exits 0 and prints replacement characters rather than rejecting the file
- `survey` / `status` outside a git repo exit 0 with structurally valid empty JSON rather than erroring
- Explicit "not found" errors use stderr + exit 2; the suite asserts the exit-2 explicit-error paths

## Ledger

- v1.7: Shoulder age the only exempt non-deterministic axis
- v1.6: Exact-value assertions replace shape checks
- v1.5: Graph pinned by absence depth and confidence
- v1.4: Exact per-language complexity and extraction values
- v1.3: Coverage for the global --filter option
- v1.2: Coverage for `docs` and the `read` docs toggle
- v1.1: Suite reframed as the tracer's own behavior contract
- v1.0: Baseline for the black-box behavior suite
