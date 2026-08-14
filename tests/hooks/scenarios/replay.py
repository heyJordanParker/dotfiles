#!/usr/bin/env python3
"""Replay one saved conversation through a stop-gate, by hand.

This is NOT a test suite. It is a manual tool: a scenario is serialized
conversation data, and this runs it through the real gate (real model call, no
mocks) and prints what the gate did. You read the result and judge it. Use it to
rerun a problem the architect hit while you iterate on a gate's prompt, until the
gate behaves on that real moment.

    python3 replay.py <scenario.json>

A scenario file:

    {
      "gate": "babysitter",
      "state": "proposing" | "executing",
      "transcript": [ ...real Claude transcript records... ],
      "note": "what went wrong here / what this is checking"   # for you; ignored at run
    }

`transcript` is the real conversation slice, in Claude's transcript shape (each
record {"type","message":{"role","content"}}). The last assistant message is the
reply the gate judges; everything before it is the context that led there.
"""

import json
import os
import subprocess
import sys
import tempfile

HOOKS = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))),
    "packages", "agents", "hooks")


def last_assistant_text(records):
    for r in reversed(records):
        msg = r.get("message", {})
        if msg.get("role") != "assistant":
            continue
        content = msg.get("content")
        if isinstance(content, str):
            return content
        if isinstance(content, list):
            for block in reversed(content):
                if isinstance(block, dict) and block.get("type") == "text":
                    return block.get("text", "")
    return ""


def _gate_fired(r):
    """Did the gate fire on this reply? A gate signals a problem two ways: a hard
    block on stderr with exit 2 (feedback.block), or a non-halting concern as a
    {"systemMessage": ...} JSON line on stdout with exit 0 (feedback.raise_concern,
    the channel the babysitter gate and its deterministic checks use).
    Either one is the gate firing. Returns (fired, message)."""
    if r.returncode == 2 and r.stderr.strip():
        return True, r.stderr.strip()
    for line in reversed(r.stdout.strip().splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            envelope = json.loads(line)
        except Exception:
            continue
        msg = envelope.get("systemMessage")
        if msg:
            return True, msg
    return False, ""


def replay(scenario):
    """Run the scenario's gate against its reconstructed world. Returns
    (fired, message) — whether the gate fired on the reply and what it said."""
    gate = scenario["gate"]
    records = scenario["transcript"]
    with tempfile.TemporaryDirectory() as tmp:
        data_root = os.path.join(tmp, "claude")
        sess = os.path.join(data_root, "sessions", "scenario")
        os.makedirs(sess)
        with open(os.path.join(sess, "state.json"), "w") as fh:
            json.dump({"state": scenario.get("state", "proposing")}, fh)
        tpath = os.path.join(tmp, "transcript.jsonl")
        with open(tpath, "w") as fh:
            for r in records:
                fh.write(json.dumps(r, separators=(",", ":")) + "\n")
        event = json.dumps({
            "session_id": "scenario", "transcript_path": tpath,
            "last_assistant_message": last_assistant_text(records), "cwd": HOOKS})
        # The gate resolves the governing session through lib.event.owner_session,
        # which prefers CLAUDE_CODE_SESSION_ID over the payload. Left alone, the
        # session running this replay leaks in and the gate reads that session's
        # state instead of the scenario's — silently judging under the wrong axis.
        env = dict(os.environ, CLAUDE_DATA_ROOT=data_root,
                   CLAUDE_CODE_SESSION_ID="scenario")
        r = subprocess.run(["python3", os.path.join(HOOKS, gate + ".py")],
                           input=event, text=True, capture_output=True, env=env)
    return _gate_fired(r)


def main(argv):
    if len(argv) != 1:
        print("usage: replay.py <scenario.json>")
        return 1
    with open(argv[0]) as fh:
        scenario = json.load(fh)
    fired, feedback = replay(scenario)
    print("gate: %s | state: %s" % (scenario["gate"], scenario.get("state", "proposing")))
    if scenario.get("note"):
        print("note: %s" % scenario["note"])
    print("FIRED" if fired else "clean")
    if feedback:
        print("---")
        print(feedback)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
