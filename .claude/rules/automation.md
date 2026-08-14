---
paths:
  - "scripts/**"
  - "tests/**"
  - "packages/agents/hooks/**"
---

### Keep automation runtime stdlib-only
The automation runtime is Python stdlib-only under the system `python3`. Dev tooling such as pytest and ruff lives in the uv-managed `.venv` and is never imported at runtime.

### Use `scripts/sync.py` as the maintenance entry point
`scripts/sync.py` is the one maintenance entry point: restow every package, regenerate the Codex Agent artifacts, and regenerate the Hook wiring. It is idempotent. Never duplicate its steps elsewhere.

### Run Hook tests from the repo root
Run the Hook tests with `uv run pytest tests/hooks` from the repo root.

IF starting a multi-commit session:
### Run `scripts/sync.py` up front
Run `python3 scripts/sync.py` once up front. The pre-commit Hook reruns it on every commit and rewrites `settings.json`/`config.toml` to canonical form; if they were not canonical when you committed, the rewrite leaves them dirty after the commit lands.

IF changing where a Hook fires:
### Edit `BINDING` and sync
Edit the Hook's `BINDING` and run `sync.py`. Add `roots: "all"` when the Hook must hold in every profile config root, not the default one alone.
Never: hand-edit the generated regions, in `settings.json`, in a profile's `settings.json`, or in `config.toml`.

IF renaming or deleting a wired Hook:
### Migrate wiring-first
Add the new file, run `sync.py`, let the live sessions drain, then delete the old file and run `sync.py` again. A session holds the wiring snapshot taken at its start, and a wired Hook whose file is missing blocks every tool call in that session.

IF adding or changing a test under tests/:
### Earn the test with a demonstrated failure
A new test is legal in exactly three cases: it fails against a demonstrated bug before the fix, it covers a new branch in a shared library under packages/agents/hooks/lib/, or it is the crash contract picking up a new hook. Never write a test from the implementation it tests — a test generated from the code encodes the code, bugs included. Gate behavior beyond one blocking and one allowing case per surface does not get tests.
