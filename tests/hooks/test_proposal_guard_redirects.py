"""Behavioral coverage for the proposing-state edit guard.

The guard (block_edits_during_proposal.py) blocks commands that would mutate a file
inside the repo while the session is "proposing" — writes, deletes, moves, creates,
and perms. It must NOT mistake shell file-descriptor duplication (`2>&1`, `>&2`,
`&>`-with-fd, `>&-`) for a file target: those never write a repo file, and
false-blocking them obstructs ordinary commands. A real mutation of a repo file
must still block — that protection is the point. Mutations into the whitelisted
dirs (`/tmp`, `docs/plans`, `docs/shaping`, `.claude/plans`, `.claude/shaping`,
`~`-expansions of them) stay allowed.

Each case runs the hook as a subprocess with a proposing-state /tmp state file, the
same way Claude Code invokes it, and asserts the exit code: 0 = allowed, 2 = blocked.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "block_edits_during_proposal.py")
SID = "test_proposal_guard_redirects"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


@pytest.fixture
def proposing_state(tmp_path, monkeypatch):
    """Pin the session into the proposing state on the spine for the run. The
    setenv reaches the hook subprocess, which inherits the parent environment."""
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    session_dir = tmp_path / "sessions" / SID
    session_dir.mkdir(parents=True)
    (session_dir / "state.json").write_text(json.dumps({"state": "proposing"}))


def _run(command):
    payload = json.dumps({
        "session_id": SID,
        "cwd": REPO,
        "tool_input": {"command": command},
    })
    return subprocess.run(
        ["python3", HOOK], input=payload, text=True, capture_output=True, cwd=REPO
    ).returncode


def _run_file(file_path):
    payload = json.dumps({
        "session_id": SID,
        "cwd": REPO,
        "tool_input": {"file_path": file_path},
    })
    return subprocess.run(
        ["python3", HOOK], input=payload, text=True, capture_output=True, cwd=REPO
    ).returncode


# fd-duplication — never a file target, must be allowed (exit 0)
FD_DUPLICATION_ALLOWED = [
    "make test 2>&1",
    "cmd >/dev/null 2>&1",
    "pytest 2>&1 | tail",
    "cmd >&2",
    "cmd 1>&2",
    "cmd >&-",
]

# real redirect writing a repo file — must still block (exit 2)
REPO_FILE_REDIRECT_BLOCKED = [
    "echo x > note.txt",
    "cmd &> out.log",
    "cmd >& file.log",
    "cmd 2>>err.log",
]

# mutations of a repo file with no redirect — delete, move, create, perms, git —
# the class the guard missed when an `rm` slipped through proposing. Must block.
REPO_MUTATION_BLOCKED = [
    "rm note.txt",
    "rm -rf tests/hooks",
    "rmdir tests/hooks",
    "unlink note.txt",
    "mv a.txt b.txt",
    "mkdir newdir",
    "ln -s /etc/hosts link",
    "chmod 755 replay.py",
    "git rm note.txt",
    "git clean -fd",
    "git mv a.txt b.txt",
]

# the same mutations aimed at whitelisted dirs — must stay allowed (exit 0)
WHITELISTED_MUTATION_ALLOWED = [
    "rm /tmp/x",
    "rm -rf /tmp/scratch",
    "rmdir /tmp/d",
    "mkdir /tmp/d",
    "chmod 755 /tmp/x",
    "touch ~/.claude/plans/foo.md",
    "touch docs/plans/foo-V1.md",
    "mkdir docs/shaping/feature",
    "touch .claude/plans/foo.md",
    "mkdir .claude/shaping/feature",
]

WHITELISTED_FILE_MUTATION_ALLOWED = [
    "docs/plans/foo-V1.md",
    "docs/shaping/feature/shaping.md",
    ".claude/plans/foo.md",
    ".claude/shaping/feature/shaping.md",
]


@pytest.mark.parametrize("command", FD_DUPLICATION_ALLOWED)
def test_fd_duplication_is_allowed(proposing_state, command):
    assert _run(command) == 0


@pytest.mark.parametrize("command", REPO_FILE_REDIRECT_BLOCKED)
def test_repo_file_redirect_is_blocked(proposing_state, command):
    assert _run(command) == 2


def test_redirect_to_repo_file_still_blocks_alongside_fd_dup(proposing_state):
    """A real file write and an fd-dup in the same command: the file write blocks."""
    assert _run("echo x > note.txt 2>&1") == 2
    # /tmp target plus fd-dup is fine — neither is a repo file.
    assert _run("cmd > /tmp/x 2>&1") == 0


@pytest.mark.parametrize("command", REPO_MUTATION_BLOCKED)
def test_repo_mutation_is_blocked(proposing_state, command):
    assert _run(command) == 2


@pytest.mark.parametrize("command", WHITELISTED_MUTATION_ALLOWED)
def test_whitelisted_mutation_is_allowed(proposing_state, command):
    assert _run(command) == 0


@pytest.mark.parametrize("file_path", WHITELISTED_FILE_MUTATION_ALLOWED)
def test_whitelisted_file_mutation_is_allowed(proposing_state, file_path):
    assert _run_file(file_path) == 0


def test_repo_file_mutation_is_blocked(proposing_state):
    assert _run_file("README.md") == 2


def test_would_regress_on_unfixed_logic():
    """Pin the bug: the pre-fix logic (no fd-reference exclusion) false-blocked
    fd-duplication. This fails loud if someone reverts the _is_fd_reference guard."""
    import block_edits_during_proposal as guard
    from lib.command import segments

    def pre_fix_targets(words):
        i = 0
        while i < len(words) and guard._ASSIGN.match(words[i]):
            i += 1
        words = words[i:]
        if not words:
            return []
        return [
            words[j + 1]
            for j, t in enumerate(words)
            if guard._is_redirect(t) and j + 1 < len(words)
        ]

    def blocks(targets_fn, command):
        for seg in segments(command):
            for target in targets_fn(seg):
                if not guard._allowed_target(target, REPO):
                    return True
        return False

    # The old logic wrongly blocked every fd-duplication case.
    for command in FD_DUPLICATION_ALLOWED:
        assert blocks(pre_fix_targets, command), f"expected old logic to block {command!r}"
    # The fixed logic allows them.
    for command in FD_DUPLICATION_ALLOWED:
        assert not blocks(guard._segment_targets, command), f"fix should allow {command!r}"
    # Both old and new still block a genuine repo-file redirect.
    for command in REPO_FILE_REDIRECT_BLOCKED:
        assert blocks(pre_fix_targets, command) and blocks(guard._segment_targets, command)
