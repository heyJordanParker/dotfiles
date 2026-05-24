# Tracer Test Suite
v1.8 | Updated: 2026-05-24

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
    ├── architecture_commands.rs     callers, defines, symbols, upstream/downstream
    ├── search_commands.rs            grep, struct, find, glob
    ├── git_commands.rs               history, blame, diff, status
    ├── cache_and_backend.rs          build/warm/invalidate, namespaces, clear, stats,
    │                                  TRACER_CCN_BACKEND + cache segregation
    ├── complexity_exact.rs           per-language exact CCN values via the AST walker
    ├── declarations_and_references.rs   per-language exact extraction values
    ├── worktree_anchoring.rs         `.tracer-cache/` lives only at a worktree root;
    │                                  linked-worktree isolation; no-repo execution
    │                                  reads but never writes a cache
    ├── session_log.rs                events.jsonl + view.json schema; content-hash
    │                                  dedupe; per-agent isolation; concurrent-writer
    │                                  safety; subagent-stop archive + read fallback
    ├── docs_load.rs                  `trace docs` path mode + `docs load` alias:
    │                                  unified response shape, `--source` and
    │                                  `--triggering-*` round-trip, `already_loaded`
    │                                  attribution, AGENTS.md / Agents.md (+ `.local.md`)
    │                                  recognition with distinct kinds
    ├── docs_status.rs                `trace docs status` session manifest + per-path
    │                                  loaded/not_loaded partitioning; the docs hint
    │                                  on `trace context <file>`
    ├── docs_graph.rs                 `trace docs --graph` projected from the unified
    │                                  `architecture/` entry: build, cache reuse,
    │                                  doc-mtime + HEAD invalidation, @include edges,
    │                                  conditional `paths:` promotion
    ├── context_prime.rs              `trace context prime --reason …` primer: empty
    │                                  case, project-root CLAUDE.md, recursive @include
    │                                  graph, depth cap, MEMORY.md exclusion, AGENTS.md
    │                                  mirrored with its own kind
    ├── drift.rs                      `trace context prime --observed-from` reconciler:
    │                                  no-observation no-op, equal sets emit no event,
    │                                  divergence appends one `context_prime_drift` event
    │                                  and reconciles `view.json` to observed paths + hashes
    ├── regressions_v4_9.rs           pinned regression cases from the v4.9 contract probe
    ├── primer.rs                     trace context (no args) — all sections + cache warming
    ├── edge_cases.rs                 missing/outside-repo/empty/non-source/binary/large paths
    ├── filter.rs                     global --filter: requires --json, fail-fast, stream, identity
    └── speed.rs                      per-command wall-clock budgets; cold-vs-warm pairs
```

The v5.11/v5.12 invariants pinned by these files:

- **Worktree-anchored cache** (`worktree_anchoring.rs`) — `.tracer-cache/` lands only at the worktree root, linked worktrees stay isolated from the main checkout, and execution outside any worktree still returns results but writes zero cache directories
- **Doc-graph recognition** (`docs_load.rs`, `docs_graph.rs`, `context_prime.rs`) — both Claude Code's `CLAUDE.md`/`Claude.md` family and OpenAI's cross-harness `AGENTS.md`/`Agents.md` family, including their `.local.md` peers, surface in the per-file doc walk and the docs graph with distinct kinds
- **Content-hash dedupe in the session log** (`session_log.rs`, `docs_load.rs`) — a doc whose hash matches the materialized view records no second event, so repeat invocations are no-ops
- **Context-primer auto-load set** (`context_prime.rs`) — the primer's set is exactly the user-global CLAUDE.md plus the project-root chain (and their `@include` graphs); the Claude memory file is never mirrored
- **Context-primer drift reconciliation** (`drift.rs`) — when the primer's predicted set diverges from the observed set Claude Code's harness actually injected, one `context_prime_drift` event appends and `view.json` is rewritten to the observed paths and their hook-supplied content hashes, while `events.jsonl` preserves the diff payload

## How

### Speed thresholds

The budgets in `tests/speed.rs` are deliberately loose regression tripwires, not micro-benchmarks. Raise them if a constrained CI runner flakes; the cold-vs-warm ratio check is the signal that survives threshold tuning and catches a cache that silently stopped working.

### Pinned behaviors

The suite pins these deliberate behaviors — they are the contract, not accidents of implementation:

- Binary-file `read` exits 0 and prints replacement characters rather than rejecting the file
- `survey` / `status` outside a git repo exit 0 with structurally valid empty JSON rather than erroring
- Explicit "not found" errors use stderr + exit 2; the suite asserts the exit-2 explicit-error paths

## Ledger

- v1.8: Layout names v5 invariants new tests pin
- v1.7: Shoulder age the only exempt non-deterministic axis
- v1.6: Exact-value assertions replace shape checks
- v1.5: Graph pinned by absence depth and confidence
- v1.4: Exact per-language complexity and extraction values
- v1.3: Coverage for the global --filter option
- v1.2: Coverage for `docs` and the `read` docs toggle
- v1.1: Suite reframed as the tracer's own behavior contract
- v1.0: Baseline for the black-box behavior suite
