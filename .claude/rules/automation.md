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
Edit the Hook's `BINDING` and run `sync.py`.
Never: hand-edit the generated regions.
