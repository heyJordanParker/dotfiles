"""Contract for the shared shell reader (lib/command).

Three guards now decide from `invocations` — solo mode, the access declarations,
and anything after them — so what it resolves is what they enforce. The cases
below are the shapes that walked around the string matches these guards used to
carry: a leading space, a second line, an environment prefix, an absolute path,
a shell inside a shell, and a flag whose value hides the command behind it.
"""

import pytest
from lib.command import command_head, invocations, redirects_output, segments


def heads(line):
    found = invocations(line)
    return None if found is None else [head for head, _ in found]


@pytest.mark.parametrize("line", [
    "codex-run @architect 'review'",
    " codex-run @architect 'review'",
    "echo hi\ncodex-run @architect 'review'",
    "env FOO=1 codex-run @architect 'review'",
    "FOO=1 codex-run @architect 'review'",
    "/Users/jordan/.local/bin/codex-run @architect 'review'",
    "./codex-run @architect 'review'",
    "cd /tmp && codex-run @architect 'review'",
    "$(codex-run @architect 'review')",
    "bash -c 'codex-run @architect review'",
    "sh -lc 'codex-run @architect review'",
    "timeout 600 codex-run @architect 'review'",
    "sudo -u jordan codex-run @architect 'review'",
    "nohup codex-run @architect 'review'",
])
def test_the_command_is_found_whatever_shape_it_arrives_in(line):
    assert "codex-run" in heads(line)


@pytest.mark.parametrize("line", [
    "echo codex-run",
    "cat docs/codex-run-notes.md",
    "grep codex README.md",
    "rg ssh packages/ssh/config",
])
def test_a_command_named_in_an_argument_is_not_a_command(line):
    assert heads(line)[0] in ("echo", "cat", "grep", "rg")
    assert "codex-run" not in heads(line)[1:]


def test_an_unparseable_line_resolves_to_nothing_readable():
    assert invocations("codex-run 'unbalanced") is None
    assert redirects_output("cat 'unbalanced") is None


def test_a_prefix_with_no_flags_yields_one_command():
    """`env FOO=1 ls file` must not offer `file` as a command — a read-only agent
    would lose every ordinary read to a spare candidate."""
    assert heads("env FOO=1 ls file") == ["env", "ls"]


def test_a_prefix_whose_flag_may_have_eaten_the_command_yields_candidates():
    """`sudo -u jordan ssh prod`: nothing here knows that `-u` takes a value, so
    every word after it is a candidate and the real command is never missed."""
    assert "ssh" in heads("sudo -u jordan ssh prod")


def test_a_shell_inside_a_shell_still_reaches_the_command():
    assert heads("""bash -c "bash -c 'codex-run @architect review'" """) == ["codex-run"]


def test_the_codex_shell_wrapper_is_transport_not_a_command():
    """codex hands every shell call over as a list that command_str joins with
    spaces. Judging `zsh` would take the whole shell off a read-only agent there
    while leaving the same command allowed on Claude."""
    assert heads("/bin/zsh -lc curl -s https://example.com") == ["curl"]
    # The flags of the script itself survive the wrapper: `-i` is what makes this
    # command a write, and dropping it let every read-only agent edit on codex.
    head, args = invocations("/bin/zsh -lc sed -i '' s/a/b/ f")[0]
    assert (head, "-i" in args) == ("sed", True)


def test_a_shell_with_no_script_stays_a_command():
    """`bash run.sh` runs something the string does not contain."""
    assert heads("bash run.sh") == ["bash"]


def test_a_write_inside_a_shell_script_is_read():
    assert heads("bash -c 'sed -i \"\" s/a/b/ f'") == ["sed"]
    assert redirects_output("bash -c 'cat a > b'") is True


@pytest.mark.parametrize("line,expected", [
    ("cat a.py > b.py", True),
    ("echo x >> notes.md", True),
    ("uv run pytest 2> errors.txt", True),
    ("trace grep foo", False),
    ("rg 'a > b' file", False),
    ("find . -newer x", False),
])
def test_output_redirection_is_read_from_the_token_not_the_text(line, expected):
    assert redirects_output(line) is expected


def test_command_head_agrees_with_the_first_invocation():
    for line in ("env X=1 /usr/bin/sed -i '' s/a/b/ f", "timeout 5 trace grep x"):
        assert command_head(segments(line)[0]) == heads(line)[1]
