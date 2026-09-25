#!/usr/bin/env python3
"""Run a group's quick hooks in one process and answer the harness once.

Both harnesses start every hook bound to a tool call at the same moment, each in
its own interpreter: one codex shell command started 22 of them, about 1.2
CPU-seconds, half of it Python starting up, and codex runs several commands at
once. The generator names the quick members of one group on this command line;
each runs here against the same event, exactly as the harness would run it alone
— its own `main()`, the event on stdin, its answer on stdout and stderr — and the
answers merge the way the harness merges separate hooks: every refusal is kept
and wins, context joins, and a permission decision keeps its strictest value.

A member that raises is reported on stderr and the rest still decide, which is
what the harness does with a hook that crashes. Two members rewriting the same
tool input cannot both be honoured, so that call is refused and says which.
"""

import importlib
import io
import json
import sys

from lib import feedback

_STRICTNESS = {"allow": 0, "ask": 1, "deny": 2}


def answer_of(module_name, raw_event):
    """(exit code, stdout, stderr) of one member run against the event."""
    streams = sys.stdin, sys.stdout, sys.stderr
    sys.stdin, sys.stdout, sys.stderr = io.StringIO(raw_event), io.StringIO(), io.StringIO()
    try:
        try:
            code = importlib.import_module(module_name).main()
        except SystemExit as exit_:
            code = exit_.code
        except Exception as exc:
            sys.stderr.write("%s: %s: %s\n" % (module_name, type(exc).__name__, exc))
            code = 1
        return (code if isinstance(code, int) else 1 if code else 0,
                sys.stdout.getvalue(), sys.stderr.getvalue())
    finally:
        sys.stdin, sys.stdout, sys.stderr = streams


class Answer:
    """The merged reply of every member."""

    def __init__(self, event_name):
        self.event_name = event_name
        self.refusals = []
        self.contexts = []
        self.messages = []
        self.rewrites = []
        self.decision = None
        self.reasons = []
        self.other = {}

    def absorb(self, output):
        for line in output.splitlines():
            if not line.strip():
                continue
            try:
                reply = json.loads(line)
            except ValueError:
                reply = None
            if not isinstance(reply, dict):
                self.contexts.append(line)
                continue
            specific = reply.pop("hookSpecificOutput", {}) or {}
            if specific.get("additionalContext"):
                self.contexts.append(specific["additionalContext"])
            if "updatedInput" in specific:
                self.rewrites.append(specific["updatedInput"])
            decision = specific.get("permissionDecision")
            if decision in _STRICTNESS and (
                    self.decision is None or _STRICTNESS[decision] > _STRICTNESS[self.decision]):
                self.decision = decision
            if specific.get("permissionDecisionReason"):
                self.reasons.append(specific["permissionDecisionReason"])
            if reply.get("systemMessage"):
                self.messages.append(reply.pop("systemMessage"))
            if reply.get("decision") == "block":
                self.refusals.append(reply.get("reason", ""))
                reply.pop("decision")
                reply.pop("reason", None)
            self.other.update(reply)

    def reply(self):
        """(exit code, stdout, stderr) the harness receives."""
        if len(self.rewrites) > 1:
            self.refusals.append(feedback.wrap(
                "combine_hooks",
                "BLOCKED: two hooks rewrote this tool call's input, and only one "
                "rewrite can run. The hook bindings need fixing, not the command."))
        if self.refusals:
            return 2, "", "\n\n".join(self.refusals) + "\n"
        output = dict(self.other)
        if self.messages:
            output["systemMessage"] = "\n\n".join(self.messages)
        specific = {}
        if self.contexts:
            specific["additionalContext"] = "\n\n".join(self.contexts)
        if self.rewrites:
            specific["updatedInput"] = self.rewrites[0]
        if self.decision:
            specific["permissionDecision"] = self.decision
        if self.reasons:
            specific["permissionDecisionReason"] = "\n\n".join(self.reasons)
        if specific:
            output["hookSpecificOutput"] = {"hookEventName": self.event_name, **specific}
        if not output:
            return 0, "", ""
        return 0, json.dumps(output, separators=(",", ":"), ensure_ascii=False) + "\n", ""


def main(argv):
    raw_event = sys.stdin.read()
    try:
        event_name = json.loads(raw_event).get("hook_event_name", "")
    except (ValueError, AttributeError):
        event_name = ""
    answer = Answer(event_name)
    diagnostics = []
    for module_name in argv[1:]:
        code, output, errors = answer_of(module_name, raw_event)
        if code == 2:
            answer.refusals.append(errors.rstrip("\n"))
            continue
        if errors:
            diagnostics.append(errors)
        answer.absorb(output)
    code, output, errors = answer.reply()
    sys.stdout.write(output)
    sys.stderr.write("".join(diagnostics) + errors)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
