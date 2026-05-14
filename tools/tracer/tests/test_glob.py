"""Tests for `trace glob`.

Builds a real git repo in a tmp_path so gitignore semantics are exercised
end-to-end, then drives the click command via CliRunner.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest
from click.testing import CliRunner

from tracer.commands.glob import command as glob_command


def _git(cwd: Path, *args: str) -> None:
    subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=True,
        capture_output=True,
        env={
            "PATH": "/usr/bin:/bin:/usr/local/bin",
            "HOME": str(cwd),
            "GIT_AUTHOR_NAME": "Test",
            "GIT_AUTHOR_EMAIL": "test@example.com",
            "GIT_COMMITTER_NAME": "Test",
            "GIT_COMMITTER_EMAIL": "test@example.com",
        },
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """Initialize a tmp git repo with a representative file layout."""
    _git(tmp_path, "init", "--quiet")
    _git(tmp_path, "config", "user.email", "test@example.com")
    _git(tmp_path, "config", "user.name", "Test")

    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "alpha.py").write_text("def a():\n    return 1\n")
    (tmp_path / "src" / "beta.py").write_text("def b():\n    if True:\n        return 2\n")
    (tmp_path / "src" / "nested").mkdir()
    (tmp_path / "src" / "nested" / "gamma.py").write_text("def c():\n    pass\n")
    (tmp_path / "src" / "front.tsx").write_text("export const X = 1;\n")
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs" / "readme.md").write_text("# readme\n")
    (tmp_path / "node_modules").mkdir()
    (tmp_path / "node_modules" / "ignored.py").write_text("# should be hidden\n")
    (tmp_path / ".gitignore").write_text("node_modules/\n")

    _git(tmp_path, "add", "-A")
    _git(tmp_path, "commit", "-m", "init", "--quiet")
    return tmp_path


def _run(*args: str) -> tuple[int, str]:
    runner = CliRunner()
    result = runner.invoke(glob_command, list(args))
    return result.exit_code, result.output


# ---------------------------------------------------------------------------
# Bare pattern modes
# ---------------------------------------------------------------------------


def test_bare_pattern_matches_top_level_only(repo: Path) -> None:
    """`*.py` resolves only at the base level, not recursively."""
    exit_code, output = _run("*.py", str(repo))
    assert exit_code == 0
    # No top-level .py files in this layout; src/*.py and src/nested/*.py
    # should not show up under a non-recursive pattern.
    assert output.strip() == "(no matches)"


def test_recursive_double_star_matches_all_py(repo: Path) -> None:
    """`**/*.py` walks recursively and returns every .py under the base."""
    exit_code, output = _run("**/*.py", str(repo))
    assert exit_code == 0
    lines = [line for line in output.strip().splitlines() if line]
    assert lines == [
        "src/alpha.py",
        "src/beta.py",
        "src/nested/gamma.py",
    ]


def test_path_bearing_pattern(repo: Path) -> None:
    """`src/**/*.py` scopes the recursion to the src subtree."""
    exit_code, output = _run("src/**/*.py", str(repo))
    assert exit_code == 0
    lines = [line for line in output.strip().splitlines() if line]
    assert lines == [
        "src/alpha.py",
        "src/beta.py",
        "src/nested/gamma.py",
    ]


def test_base_argument_scopes_resolution(repo: Path) -> None:
    """When `base` is `src`, the pattern resolves under src and paths are
    rendered relative to src."""
    exit_code, output = _run("**/*.py", str(repo / "src"))
    assert exit_code == 0
    lines = [line for line in output.strip().splitlines() if line]
    assert lines == [
        "alpha.py",
        "beta.py",
        "nested/gamma.py",
    ]


# ---------------------------------------------------------------------------
# Gitignore behavior
# ---------------------------------------------------------------------------


def test_gitignored_paths_are_excluded(repo: Path) -> None:
    """Files under node_modules are present on disk but gitignored — must not
    appear in glob output."""
    exit_code, output = _run("**/*.py", str(repo))
    assert exit_code == 0
    assert "node_modules" not in output
    assert "ignored.py" not in output


def test_ignore_policy_reported_in_json(repo: Path) -> None:
    exit_code, output = _run("**/*.py", str(repo), "--json")
    assert exit_code == 0
    payload = json.loads(output)
    assert payload["ignore_policy"] == "gitignore"


# ---------------------------------------------------------------------------
# Determinism
# ---------------------------------------------------------------------------


def test_results_are_lexicographically_sorted(repo: Path) -> None:
    """Two invocations produce the same order; that order is lexicographic
    by full path."""
    _, first = _run("**/*.py", str(repo))
    _, second = _run("**/*.py", str(repo))
    assert first == second
    lines = [line for line in first.strip().splitlines() if line]
    assert lines == sorted(lines)


# ---------------------------------------------------------------------------
# JSON envelope
# ---------------------------------------------------------------------------


def test_json_default_returns_path_list(repo: Path) -> None:
    exit_code, output = _run("**/*.py", str(repo), "--json")
    assert exit_code == 0
    payload = json.loads(output)
    assert payload["pattern"] == "**/*.py"
    assert payload["match_count"] == 3
    assert payload["matches"] == [
        "src/alpha.py",
        "src/beta.py",
        "src/nested/gamma.py",
    ]


def test_json_details_returns_enriched_entries(repo: Path) -> None:
    exit_code, output = _run("**/*.py", str(repo), "--details", "--json")
    assert exit_code == 0
    payload = json.loads(output)
    assert payload["match_count"] == 3
    entry = payload["matches"][0]
    assert entry["path"] == "src/alpha.py"
    # Each detail entry carries ccn / rank / shoulder fields.
    assert "ccn_total" in entry
    assert "rank" in entry
    assert "shoulder" in entry


# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


def test_missing_base_exits_non_zero(repo: Path) -> None:
    exit_code, output = _run("**/*.py", str(repo / "does-not-exist"))
    assert exit_code == 2
    assert "does not exist" in output


def test_base_that_is_a_file_exits_non_zero(repo: Path) -> None:
    exit_code, output = _run("**/*.py", str(repo / "src" / "alpha.py"))
    assert exit_code == 2
    assert "not a directory" in output


def test_no_matches_returns_zero_with_message(repo: Path) -> None:
    exit_code, output = _run("**/*.nonexistent", str(repo))
    assert exit_code == 0
    assert output.strip() == "(no matches)"


# ---------------------------------------------------------------------------
# Non-git base — fallback walker
# ---------------------------------------------------------------------------


def test_non_git_base_falls_back_to_skip_dirs_walker(tmp_path: Path) -> None:
    """When base is not in a git repo, gitignore can't be consulted — the
    fallback walker uses SKIP_DIRS and still returns matches."""
    (tmp_path / "x.py").write_text("pass\n")
    (tmp_path / "node_modules").mkdir()
    (tmp_path / "node_modules" / "skip.py").write_text("pass\n")
    exit_code, output = _run("**/*.py", str(tmp_path), "--json")
    assert exit_code == 0
    payload = json.loads(output)
    # node_modules excluded by SKIP_DIRS; x.py present.
    assert payload["ignore_policy"] == "skip_dirs"
    assert "x.py" in payload["matches"]
    assert all("node_modules" not in m for m in payload["matches"])
