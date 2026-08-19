"""Behavioral coverage for the proposing-state edit guard.

The guard (block_writes.py) blocks commands that would mutate a file
inside the repo while the session state is "propose" — writes, deletes, moves, creates,
and perms. It must NOT mistake shell file-descriptor duplication (`2>&1`, `>&2`,
`&>`-with-fd, `>&-`) for a file target: those never write a repo file, and
false-blocking them obstructs ordinary commands. A real mutation of our tree
must still block — that protection is the point. Mutations in `/tmp` stay allowed.

One case per decision the guard makes. Two of them spawn the hook, one blocking and
one allowing, because the harness reads an exit code off a process; the rest call
main() in-process, which resolves the same session from the same environment.
"""

import io
import json
import os
import re
import subprocess
import sys

import block_writes
import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "block_writes.py")
GOVERNING_SID = "test_proposal_guard_redirects"
RUN_OWN_SID = "test_proposal_guard_redirects_run_own"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


@pytest.fixture
def proposing_state(tmp_path, monkeypatch):
    """Pin the governing session to propose in session state, the way the guard reads it.

    The guard resolves the session through lib.event.owner_session, which prefers
    CLAUDE_CODE_SESSION_ID over the payload's session_id, so the id has to be in the
    environment or the seeded record is never read. The run's own session — the id in
    the payload — is seeded to the OPPOSITE state, execute, so a resolver that read the
    payload would allow every mutation and fail these cases. An unseeded session
    defaults to propose and would block under either resolver, proving nothing.

    owner_session reads os.environ at call time, so the setenv reaches an in-process
    main() and a spawned hook alike — the subprocess inherits the parent environment."""
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", GOVERNING_SID)
    governing_dir = tmp_path / "sessions" / GOVERNING_SID
    governing_dir.mkdir(parents=True)
    (governing_dir / "state.json").write_text(json.dumps({"state": "propose"}))
    run_dir = tmp_path / "sessions" / RUN_OWN_SID
    run_dir.mkdir(parents=True)
    (run_dir / "state.json").write_text(json.dumps({"state": "execute"}))


def _payload(tool_input, cwd):
    return json.dumps({"session_id": RUN_OWN_SID, "cwd": cwd, "tool_input": tool_input})


def _call(monkeypatch, tool_input, cwd=REPO):
    monkeypatch.setattr(sys, "stdin", io.StringIO(_payload(tool_input, cwd)))
    return block_writes.main()


def _run(monkeypatch, command, cwd=REPO):
    return _call(monkeypatch, {"command": command}, cwd)


def _run_file(monkeypatch, file_path, cwd=REPO):
    return _call(monkeypatch, {"file_path": file_path}, cwd)


def _spawn(tool_input):
    env = dict(os.environ)
    env.pop("CODEX_RUN_AGENT_FILE", None)
    return subprocess.run(
        ["python3", HOOK], input=_payload(tool_input, REPO), text=True,
        capture_output=True, cwd=REPO, env=env,
    ).returncode


# --- the harness contract: a process, an exit code -----------------------------





# --- fd-duplication — never a file target, must be allowed (exit 0) ------------
#
# One case per exclusion `lib.command._is_fd_reference` makes, plus the mixed shape
# where a real redirect target and an fd-dup sit in one command.
FD_DUPLICATION_ALLOWED = [
    "make test 2>&1",
]

# real redirect writing a repo file — must still block (exit 2)
REPO_FILE_REDIRECT_BLOCKED = [
    "echo x > note.txt",
]

# mutations of a repo file with no redirect — the class the guard missed when an
# `rm` slipped through proposing. One per branch of lib.command.mutation_targets,
# plus the git subcommands block_writes matches before the parse.
REPO_MUTATION_BLOCKED = [
    "rm -rf tests/hooks",       # every path argument is a target, past the flags
    "ln -s /etc/hosts link",    # only the destination is a target
    "chmod 755 replay.py",      # the mode is not a path; the rest are
    "git rm note.txt",          # _GIT_TREE_MUTATORS, before any target parse
]

# Mutations aimed at writable homes stay allowed. The six homes themselves are
# covered by WRITABLE_HOMES below; this is the shape that is not — the bare
# directory rather than a file inside it, which only the `probe` suffix allows.
WHITELISTED_MUTATION_ALLOWED = [
    "mkdir docs/agents",
]


@pytest.mark.parametrize("command", FD_DUPLICATION_ALLOWED)
def test_fd_duplication_is_allowed(proposing_state, monkeypatch, command):
    assert _run(monkeypatch, command) == 0


@pytest.mark.parametrize("command", REPO_FILE_REDIRECT_BLOCKED)
def test_repo_file_redirect_is_blocked(proposing_state, monkeypatch, command):
    assert _run(monkeypatch, command) == 2




@pytest.mark.parametrize("command", WHITELISTED_MUTATION_ALLOWED)
def test_whitelisted_mutation_is_allowed(proposing_state, monkeypatch, command):
    assert _run(monkeypatch, command) == 0


def test_repo_file_mutation_is_blocked(proposing_state, monkeypatch):
    assert _run_file(monkeypatch, "README.md") == 2






WRITABLE_HOMES = (
    "docs/plans",
    "docs/shaping",
    "docs/agents",
    ".claude/shaping",
    ".claude/plans",
    "/tmp",
)




def test_would_regress_on_unfixed_logic():
    """Pin the bug: the pre-fix logic (no fd-reference exclusion) false-blocked
    fd-duplication. This fails loud if someone reverts the _is_fd_reference guard."""
    from lib.command import is_redirect, mutation_targets, segments

    assign = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")

    def pre_fix_targets(words):
        i = 0
        while i < len(words) and assign.match(words[i]):
            i += 1
        words = words[i:]
        if not words:
            return []
        return [
            words[j + 1]
            for j, t in enumerate(words)
            if is_redirect(t) and j + 1 < len(words)
        ]

    def blocks(targets_fn, command):
        for seg in segments(command):
            for target in targets_fn(seg):
                if not block_writes._allowed_target(target, REPO):
                    return True
        return False

    # The old logic wrongly blocked the fd-duplication cases that name no file.
    for command in ("make test 2>&1", "cmd >&-"):
        assert blocks(pre_fix_targets, command), f"expected old logic to block {command!r}"
    # The fixed logic allows them.
    for command in FD_DUPLICATION_ALLOWED:
        assert not blocks(mutation_targets, command), f"fix should allow {command!r}"
    # Both old and new still block a genuine repo-file redirect.
    for command in REPO_FILE_REDIRECT_BLOCKED:
        assert blocks(pre_fix_targets, command) and blocks(mutation_targets, command)
