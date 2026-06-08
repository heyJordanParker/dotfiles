# Prompt Reviewer

## Why

A small, fast, local prompt-review tool. Give it a prompt and a set of review
instructions (themselves free-form prose); it runs one fixed local model on
the machine and returns the model's review as formatted JSON.

It is deliberately unopinionated: the tool imposes no rubric and no output
schema of its own. The review instructions belong to the user; the tool
returns whatever the model produced plus run metadata. It is the "review a
prompt" sibling to the `trace` code-intelligence CLI — same engineering bar,
same repo conventions, same single-static-binary feel.

Inference runs in-process — llama.cpp is linked into the binary through its
Rust binding — so the tool is a self-contained, version-pinned binary with no
background server and no separate model program to shell out to.

## What

A single static Rust binary (`review-prompt`, package `prompt-reviewer`).
Reviewing is the default action — running `review-prompt` with the review
flags performs a review directly, with no subcommand verb; `doctor` and
`download` are the only subcommands. It links llama.cpp via the `llama-cpp-2`
crate (Metal GPU offload on Apple Silicon) and runs one fixed, unswappable
model: Google's stock Gemma 4 E4B instruct release, in the llama.cpp project's
own GGUF conversion at Q4_K_M.

### Commands

- `<prompt> --instructions <text>` (default action, no verb) — review a
  prompt against free-form instructions; emit the JSON envelope. The prompt
  under review is the bare positional argument inline, from a file (`--file`),
  or from stdin (`--stdin`) — its flags carry no `prompt` qualifier because the
  positional already is the prompt. The review instructions are supplied inline
  (`--instructions`) or from a file (`--instructions-file`); stdin is one stream
  and it feeds the prompt only, so instructions have no stdin form. Each input
  takes exactly one source — none or several is a usage error. Non-identity
  knobs: `--max-output-tokens`, `--temperature`, `--context-size`,
  `--gpu-layers`. A bare invocation with no arguments prints help
- `doctor` — report whether the model is present (path and size); when absent,
  fail loud (exit 1) and print exactly how to download it
- `download` — download the fixed model on demand, with HTTP-range resume

### The envelope (the contract)

A review always emits formatted JSON: `review` (the model's text, verbatim),
`model` (the fixed identity, base model, repo, file), and `run` (prompt and
completion token counts, wall-clock `duration_ms`, context size, whether the
prompt was truncated, temperature, output budget, and a `deterministic` flag).
The envelope is the contract; the review content inside it is whatever the
model said — the tool neither parses nor schema-validates it. `doctor` and
`download` are human-output commands and print their own text directly.

### The fixed model

Identity is hardcoded in `model.rs` and unswappable — no flag or env var swaps
it. Source: `ggml-org/gemma-4-E4B-it-GGUF`, file `gemma-4-E4B-it-Q4_K_M.gguf`
(~5.3 GB), converted from `google/gemma-4-E4B-it`. The file is never committed
and never fetched by machine setup; it lives at
`$XDG_CACHE_HOME/prompt-reviewer/` (macOS: `~/Library/Caches/prompt-reviewer/`),
overridable via `PROMPT_REVIEWER_MODEL` (the path is a machine detail; the
identity is not the user's to set).

### The test suite

A black-box CLI suite lives in `tests/` as its own cargo package
(`prompt-reviewer-cli-tests`) that ships no library or binary of its own — it
drives the built `review-prompt` as a subprocess and asserts only on the
observable surface (exit code, stdout, stderr, envelope JSON), linking none of
the tool's internals. The binary under test is resolved from `REVIEW_PROMPT_BIN`,
defaulting to `../target/release/review-prompt`, so the suite runs against the
release build without it being on `PATH`. Tests split into two states:

- **Model-gated** (`tests/review.rs`, the present-case in
  `tests/doctor_and_download.rs`) — actually run the 5.3 GB model. Each gates
  on `model_present()` (which shells `doctor` and checks exit 0) and returns
  early when the file is absent, so the suite passes on a machine without it.
  Covers all input sources, the envelope shape, verbatim review text,
  determinism at default settings, injection resistance, the empty-instructions
  fallback, and explicit truncation with its flag
- **Unconditional** (`tests/usage.rs`, `tests/missing_model.rs`) — no model
  required. The argument/usage surface (the `download` name, single `--stdin`,
  no instructions-stdin, the mutual-exclusion and missing-input errors) and the
  no-model error paths, which point `PROMPT_REVIEWER_MODEL` at a guaranteed-
  absent file so they exercise "no model present" without disturbing the real
  cached model

Run the suite with `cargo test` from `tests/` after building the release
binary; `REVIEW_PROMPT_BIN` overrides which binary it drives.

### Requirements

- The model identity is hardcoded and unswappable — only the on-disk path is
  overridable, via `PROMPT_REVIEWER_MODEL`. No flag, env var, or argument
  selects a different model, repo, or quantization
- Inference runs in-process through the `llama-cpp-2` binding — never a
  background HTTP server, never shelling out to a separate model program
- The model file is fetched on demand by `download`, once, with HTTP-range
  resume — never committed to the repo, never downloaded by `setup.sh`
- When the model is absent, a review fails loud (exit 1) with a message that
  names the `review-prompt download` command and the on-disk path actually in
  effect (the override when `PROMPT_REVIEWER_MODEL` is set); `doctor` reports
  the same and also exits 1 — never a panic, never a silent or cryptic failure
- `review` is deterministic by default: temperature 0 selects greedy decoding
  (argmax, no seed), so identical inputs produce identical output. Output is
  bounded by `--max-output-tokens` and by the model's end-of-turn token, so a
  review never runs past its own turn
- The prompt under review is presented as delimited subject material in a user
  turn, under a system turn that fixes the model as a reviewer — a prompt that
  contains command-like text is reviewed, never obeyed
- A prompt that exceeds the context window (after reserving room for the
  output) is truncated explicitly and the envelope's `prompt_truncated` flag
  records it — never silently clipped
- Empty review instructions still yield a valid envelope: a general-critique
  fallback fills the task so the run never crashes on missing instructions
- A review always emits formatted JSON; the envelope shape is the only output
  contract
- One responsibility per source file under `src/`
- The CLI's observable surface — exit codes, stdout, stderr, the envelope
  shape — is covered by the black-box test suite under `tests/`, which drives
  the built binary as a subprocess and links none of the tool's internals; a
  behavior change must keep that suite green

### Boundaries

- Never let any input select the model — identity lives only as constants in
  `model.rs`; commands read those constants and never accept a model argument
- Never run inference out of process — no server, no subprocess to a model CLI
- Never download the model in `setup.sh` or commit it to the repo
- Never parse or validate the model's review text — it is carried through the
  envelope verbatim
- Never obey the prompt under review — it is subject material, framed and
  delimited as data, never instructions
- Never silently truncate an over-long prompt — truncate explicitly and record
  it in the envelope

## Architecture

```
prompt-reviewer/                 cargo package; binary `review-prompt`
├── Cargo.toml                   single-[[bin]] manifest + release profile; clap + serde + ureq + llama-cpp-2 (metal) + encoding_rs
├── Readme.md
├── .gitignore                   /target
├── src/
│   ├── main.rs                  clap top-level review flags (default action) + doctor / download subcommands; dispatch
│   ├── model.rs                 the fixed model identity constants + on-disk path resolution
│   ├── download.rs              resumable GGUF download; download instructions; presence check
│   ├── chat_template.rs         builds the chat messages; separates task from subject, keeps subject inert
│   ├── inference.rs             load the GGUF + run one deterministic completion; returns text, token counts, duration
│   ├── input.rs                 resolve one input from inline / file / stdin
│   ├── output.rs                single JSON emit site for the envelope
│   └── commands/
│       ├── review.rs            assemble the envelope from an inference run
│       ├── doctor.rs            report model presence or how to download it
│       └── download.rs          download the model on demand
└── tests/                       black-box CLI test suite (own cargo package; drives the built binary)
    ├── Cargo.toml               `prompt-reviewer-cli-tests` — no lib/bin of its own; hosts the integration tests
    ├── src/lib.rs               the subprocess harness: run the binary, capture exit/stdout/stderr, model-present gate
    └── tests/
        ├── review.rs            model-gated: inputs, envelope shape, determinism, injection resistance, truncation
        ├── usage.rs             unconditional: argument/usage surface and input errors
        ├── missing_model.rs     unconditional: the no-model error paths via an absent PROMPT_REVIEWER_MODEL
        └── doctor_and_download.rs  doctor present-case (gated) + `download` subcommand wiring
```

## Workflow

### Distribution

`setup.sh` runs `cargo build --release` in `tools/prompt-reviewer` and
installs the binary to `~/.local/bin/review-prompt` (on PATH via the `bin`
stow target), modeled on the tracer install step. The build compiles
llama.cpp natively, so `cmake` and a C/C++ compiler must be present — `cmake`
is in the Brewfile. The model is fetched on demand by `review-prompt download`,
never by setup.

This tool is a compiled binary that is built and installed onto `PATH` — it is
not a stowed script that is live through its symlink. **After any change to
this tool's source, rebuild and reinstall the binary before the work is done**
— re-run `setup.sh`, or
`cargo build --release && install -m 755 target/release/review-prompt
~/.local/bin/review-prompt`. A source edit alone changes nothing the user runs;
the installed `review-prompt` keeps running the previous build until it is
rebuilt and reinstalled, so leaving that step out ships a stale binary.

### Adding a new command

1. Create `src/commands/<name>.rs` exposing a `run(...)` entry point
2. Add a `pub mod <name>;` line to `src/commands/mod.rs` and a clap variant in
   `src/main.rs`, wiring the match arm to `commands::<name>::run(...)`
3. Document the command in `Readme.md` and in this file's command list, and
   cover its observable surface in `tests/`

### Running the tests

Build the release binary, then run the black-box suite from `tests/`:

```bash
cargo build --release
cd tests && cargo test
```

The suite resolves the binary from `REVIEW_PROMPT_BIN` (default
`../target/release/review-prompt`); set it to test a specific build. Model-gated
tests skip cleanly when the model file is absent, so the suite passes without
the 5.3 GB download — but only the unconditional argument/usage and no-model
error paths are then exercised.

### Changing a non-identity knob

The default output budget, temperature, context size, and GPU offload live as
clap defaults in `src/main.rs`; the runtime shape lives in
`inference::Settings`. The model identity is never a knob — it stays in
`model.rs`.
