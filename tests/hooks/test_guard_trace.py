"""The trace-routing gate. Raw file reads and listings on repo paths are
refused toward the matching trace subcommand; paths outside the repo pass."""

import importlib
import io
import json
import sys


def _run(monkeypatch, command, cwd):
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "cwd": cwd,
    }
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return importlib.import_module("guard_trace").main()


def test_bare_ls_is_refused_as_a_repo_listing(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "ls", str(tmp_path)) == 2
    assert "trace list" in capsys.readouterr().err


def test_ls_with_flags_only_is_refused(monkeypatch, tmp_path):
    assert _run(monkeypatch, "ls -la", str(tmp_path)) == 2


def test_ls_on_a_repo_directory_is_refused(monkeypatch, tmp_path):
    (tmp_path / "src").mkdir()
    assert _run(monkeypatch, "ls src", str(tmp_path)) == 2


def test_tree_is_refused_toward_trace_tree(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "tree", str(tmp_path)) == 2
    assert "trace tree" in capsys.readouterr().err


def test_ls_outside_the_repo_passes(monkeypatch, tmp_path, tmp_path_factory):
    outside = tmp_path_factory.mktemp("outside")
    assert _run(monkeypatch, f"ls {outside}", str(tmp_path)) == 0


def test_ls_on_an_unresolvable_outside_glob_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "ls -d /somewhere/else/prefix-*", str(tmp_path)) == 0


def test_cat_on_a_repo_file_is_still_refused(monkeypatch, tmp_path):
    (tmp_path / "app.py").write_text("x = 1\n")
    assert _run(monkeypatch, "cat app.py", str(tmp_path)) == 2


def test_plain_trace_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "trace grep pattern", str(tmp_path)) == 0


def test_cat_on_a_repo_glob_is_refused(monkeypatch, tmp_path):
    (tmp_path / "output.rs").write_text("fn main() {}\n")
    assert _run(monkeypatch, "cat outp*.rs", str(tmp_path)) == 2


def test_cat_inside_a_shell_wrapper_is_refused(monkeypatch, tmp_path):
    (tmp_path / "app.py").write_text("x = 1\n")
    assert _run(monkeypatch, f"zsh -lc 'cat {tmp_path}/app.py'", str(tmp_path)) == 2


def test_git_grep_on_the_worktree_is_refused(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "git grep -n pattern", str(tmp_path)) == 2
    assert "trace grep" in capsys.readouterr().err


def test_git_grep_at_a_ref_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "git grep pattern HEAD -- admin", str(tmp_path)) == 0


def test_git_show_of_a_blob_is_refused(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "git show HEAD:app/Entity.php", str(tmp_path)) == 2
    assert "trace read" in capsys.readouterr().err


def test_git_show_of_a_commit_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "git show -s --format=full bb99f6a1", str(tmp_path)) == 0


def test_git_log_pickaxe_is_refused(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "git log -S data-stuck", str(tmp_path)) == 2
    assert "trace history" in capsys.readouterr().err


def test_git_log_patch_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "git log -p -S data-stuck", str(tmp_path)) == 0


def test_git_log_on_a_repo_file_is_refused(monkeypatch, tmp_path):
    (tmp_path / "app.py").write_text("x = 1\n")
    assert _run(monkeypatch, "git log -- app.py", str(tmp_path)) == 2


def test_git_status_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "git status --porcelain", str(tmp_path)) == 0


def test_git_blame_is_refused(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "git blame -L 669,695 app/X.php", str(tmp_path)) == 2
    assert "trace blame" in capsys.readouterr().err


def test_plain_git_diff_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "git diff HEAD", str(tmp_path)) == 0


def test_unbounded_search_into_sort_is_refused(monkeypatch, capsys, tmp_path):
    outside = "/Users/nobody/corpus"
    assert _run(monkeypatch, f"rg -o pattern {outside} | sort", str(tmp_path)) == 2
    assert "rg -c" in capsys.readouterr().err


def test_counted_search_into_sort_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "rg -c pattern /Users/nobody/corpus | sort", str(tmp_path)) == 0


def test_search_bounded_by_head_before_sort_passes(monkeypatch, tmp_path):
    command = "rg -o pattern /Users/nobody/corpus | head -200 | sort"
    assert _run(monkeypatch, command, str(tmp_path)) == 0
