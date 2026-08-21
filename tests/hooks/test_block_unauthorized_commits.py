import json
import os
import subprocess

from conftest import PY_HOOKS


def _run(command, session_id, data_root):
    # owner_session prefers CLAUDE_CODE_SESSION_ID over the event; the tests'
    # own session must not leak in.
    env = dict(os.environ, CLAUDE_DATA_ROOT=str(data_root))
    env.pop("CLAUDE_CODE_SESSION_ID", None)
    return subprocess.run(
        ["python3", os.path.join(PY_HOOKS, "block_unauthorized_commits.py")],
        input=json.dumps({"tool_input": {"command": command},
                          "session_id": session_id}),
        text=True,
        capture_output=True,
        env=env,
    )


def test_blocks_commit_hidden_behind_git_global_flags(tmp_path):
    # The demonstrated 2026-08-21 escape: `git -C <path> commit` never matched
    # the gate's `git commit` pattern, so an unauthorized commit landed. The
    # whole option class (--no-pager and friends) is the lib contract's case.
    result = _run("git -C /Users/jordan/dotfiles commit -F /tmp/msg.txt",
                  "unauthorized-session", tmp_path)

    assert result.returncode == 2


def test_allows_commit_when_the_flag_is_set(tmp_path):
    session = "authorized-session"
    state_dir = tmp_path / "sessions" / session
    state_dir.mkdir(parents=True)
    (state_dir / "state.json").write_text(
        json.dumps({"session_id": session, "commit_requested": True}))

    result = _run("git -C /Users/jordan/dotfiles commit -F /tmp/msg.txt",
                  session, tmp_path)

    assert result.returncode == 0
