"""Behavioral coverage for the prompt-doc review guard.

review_doc_edit.py raises concerns on low-quality prompt docs, but planning
artifacts are owned by the planning validators. A blocking model verdict must
still flag an ordinary markdown doc while leaving Shaping and plan artifact
edits alone.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "review_doc_edit.py")


@pytest.fixture
def blocking_model(tmp_path, monkeypatch):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    claude = bin_dir / "claude"
    claude.write_text(
        "#!/bin/bash\n"
        "printf '%s\\n' '{\"result\":\"{\\\"block\\\":[{\\\"category\\\":\\\"x\\\","
        "\\\"location\\\":\\\"body\\\",\\\"evidence\\\":\\\"bad\\\"}],\\\"polish\\\":[]}\"}'\n"
    )
    claude.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}:{os.environ['PATH']}")
    monkeypatch.setenv("MODEL_CALL_BACKEND", "claude")


@pytest.fixture
def clean_model(tmp_path, monkeypatch):
    bin_dir = tmp_path / "clean-bin"
    bin_dir.mkdir()
    claude = bin_dir / "claude"
    claude.write_text(
        "#!/bin/bash\n"
        "printf '%s\\n' '{\"result\":\"{\\\"block\\\":[],\\\"polish\\\":[]}\"}'\n"
    )
    claude.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}:{os.environ['PATH']}")
    monkeypatch.setenv("MODEL_CALL_BACKEND", "claude")


def _run(file_path, cwd):
    payload = json.dumps({
        "tool_name": "Edit",
        "cwd": str(cwd),
        "session_id": "review-doc-edit-test",
        "tool_input": {
            "file_path": str(file_path),
            "old_string": "before",
            "new_string": "after",
        },
    })
    return subprocess.run(
        ["python3", HOOK], input=payload, text=True, capture_output=True, cwd=cwd
    )


@pytest.mark.parametrize("relative_path", ["docs/shaping/feature/shaping.md"])
def test_planning_artifact_markdown_skips_doc_review(tmp_path, blocking_model, relative_path):
    file_path = tmp_path / relative_path
    file_path.parent.mkdir(parents=True)
    file_path.write_text("before\n")

    result = _run(file_path, tmp_path)

    assert result.returncode == 0
    assert result.stdout == ""
    assert result.stderr == ""


def test_prompt_markdown_structural_finding_blocks(tmp_path, blocking_model):
    file_path = tmp_path / "rules" / "scope.md"
    file_path.parent.mkdir()
    file_path.write_text("before\n")

    result = _run(file_path, tmp_path)

    assert result.returncode == 2
    assert "BLOCKED: this edit breaks the Prompt Architecture" in result.stderr
    assert result.stdout == ""


def test_skill_past_the_compaction_limit_warns_without_blocking(tmp_path, clean_model):
    """A Skill the harness would truncate after a compaction is reported, not refused."""
    file_path = tmp_path / "skills" / "big" / "SKILL.md"
    file_path.parent.mkdir(parents=True)
    file_path.write_text("x" * 20000 + "before")

    result = _run(file_path, tmp_path)

    assert result.returncode == 0
    assert result.stderr == ""
    assert "20005 characters, over 20000" in result.stdout


def test_size_note_rides_along_on_a_refusal(tmp_path, blocking_model):
    """One message reaches the agent, so the size joins the review's own refusal."""
    file_path = tmp_path / "skills" / "big" / "SKILL.md"
    file_path.parent.mkdir(parents=True)
    file_path.write_text("x" * 20000 + "before")

    result = _run(file_path, tmp_path)

    assert result.returncode == 2
    assert "BLOCKED: this edit breaks the Prompt Architecture" in result.stderr
    assert "20005 characters, over 20000" in result.stderr


def test_skill_within_the_compaction_limit_says_nothing(tmp_path, clean_model):
    file_path = tmp_path / "skills" / "small" / "SKILL.md"
    file_path.parent.mkdir(parents=True)
    file_path.write_text("before")

    result = _run(file_path, tmp_path)

    assert result.returncode == 0
    assert result.stdout == ""




