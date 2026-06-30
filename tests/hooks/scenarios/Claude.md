# Gate scenarios — manual replay

## Why

When a stop-gate (`babysitter`, `validate_completion`) behaves wrong on a real
moment — a bad block, or a miss it should have caught — we save that exact moment
and rerun it through the gate while fixing the gate's prompt, until it behaves.

This is a manual tool, not a test suite. There is no automated run, no scoring, no
LLM grading another LLM. You run a scenario by hand, read what the gate did, and
judge it yourself. Nothing here runs during normal work with the architect.

## The SOP — in order

1. **Dig out the full original conversation.** Pull the real transcript where the
   gate misfired and rebuild the *larger* context — the whole exchange that led to
   the moment, not a stripped summary. This step carries all the value. A thin
   scenario proves nothing, so do this first and do it properly; never skip it.
2. **Save it as a scenario.** Serialize that conversation slice into
   `scenarios/<name>.json` (format below).
3. **Then iterate the gate prompt.** Run the scenario, read what the gate did,
   change the gate's prompt, rerun — until the gate behaves on the real moment.

## Scenario format

```
{
  "gate": "validate_completion" | "babysitter",
  "state": "proposing" | "executing",
  "transcript": [ ...real Claude transcript records... ],
  "note": "what went wrong here / what this is checking"
}
```

`transcript` is the real conversation slice in Claude's transcript shape — each
record `{"type": "...", "message": {"role": "...", "content": ...}}`. The last
assistant message is the reply the gate judges; the records before it are the
context that led there. `note` is for you; the runner ignores it.

## Run

```
python3 tests/hooks/scenarios/replay.py scenarios/<name>.json
```

It runs the real gate (real model call, no mocks) and prints whether the gate
fired — a hard block (exit 2) or a non-halting concern (a `systemMessage`) — and
the feedback it gave. You read it and decide whether that's right.
