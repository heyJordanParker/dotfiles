"""Contract for the shared shell reader (lib/command).

Three guards now decide from `invocations` — solo mode, the access declarations,
and anything after them — so what it resolves is what they enforce. The cases
below are the shapes that walked around the string matches these guards used to
carry: a leading space, a second line, an environment prefix, an absolute path,
a shell inside a shell, and a flag whose value hides the command behind it.
"""

import pytest
from lib.command import (
    command_head,
    composition_refusal,
    invocations,
    redirects_output,
    segments,
)


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


def test_a_program_the_line_does_not_carry_is_unreadable():
    """`bash run.sh` and `python3 -c` run something the string does not contain, so
    the line resolves to nothing readable and every guard refuses it. Our own
    tooling is the exception: `sync.py` is the repository's own entry point."""
    assert invocations("sh /tmp/x.sh") is None
    assert invocations("python3 -c 'import os'") is None
    assert invocations("uv run python -c 'import os'") is None
    # The program glued to its flag, and a program arriving on stdin.
    assert invocations("python3 -c'import os'") is None
    assert invocations("sh -c'codex-run @x y'") is None
    assert invocations("echo 'import os' | python3") is None
    # Our own tooling is the readable case, a runtime running its own flags names no
    # program to hide, and a shell's own -c string still reads.
    assert heads("python3 scripts/sync.py") == ["python3"]
    assert heads("python3 --version") == ["python3"]
    assert heads("node -v") == ["node"]
    assert heads("sh -c 'codex-run @x y'") == ["codex-run"]


@pytest.mark.parametrize("line", [
    "source /tmp/x.sh",
    ". /tmp/x.sh",
    "eval 'codex-run @x y'",
    "$SHELL -c 'codex-run @x y'",
    "`codex-run @x y`",
    "alias c=codex-run && c @x y",
    "awk 'BEGIN{system(\"codex-run @x y\")}'",
])
def test_a_command_the_shell_itself_hides_is_unreadable(line):
    """Each shape ran a command no guard could see, and none of them can be resolved
    without interpreting the shell, so the line reads as nothing."""
    assert invocations(line) is None


@pytest.mark.parametrize("line", [
    "for f in *; do codex-run @x y; done",
    "if true; then codex-run @x y; fi",
    "find . -name x -exec codex-run @x y ;",
])
def test_a_command_behind_a_keyword_or_a_find_is_read_by_name(line):
    """These shapes hid the command too, but the command is right there in the line,
    so it is resolved rather than refused."""
    assert "codex-run" in heads(line)


@pytest.mark.parametrize("line", [
    "awk '{print $1}' README.md",
    "for f in *.md; do wc -l $f; done",
    "if true; then git status; fi",
    "grep then README.md",
    "echo $HOME",
    "make test",
    "npm run lint",
])
def test_the_same_shapes_carrying_ordinary_commands_still_read(line):
    assert invocations(line) is not None


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


@pytest.mark.parametrize("line", [
    "cd /Users/jordan/dotfiles && uv run pytest -q",
    "echo one; echo two",
    "uv run pytest -q || echo failed",
    "codex-run @explorer 'x' &",
    "ls -1 packages/agents/hooks\nls -1 packages/claude/hooks",
    # The two shapes the transcripts show most: an investigation packed into one
    # call, and a loop wrapped around a single command.
    "echo '=== hooks ==='; ls -1 packages/agents/hooks; echo; ls -1 packages/bin",
    "for f in packages/agents/hooks/*.py; do python3 -m py_compile \"$f\"; done",
    "for f in 1; do codex-run --help; done",
    "while read -r f; do wc -l $f; done",
    "find . -name '*.py' -exec rm {} ;",
    # A pipe whose consumer changes something is a program, not a read.
    "ls | xargs -n1 basename",
    "cat notes.md | tee /tmp/copy.md",
    "grep -rl foo . | xargs sed -i '' s/foo/bar/",
    # The longest recorded commands: a file authored through a heredoc.
    "cat > /tmp/orchestrate.py << 'PYEOF'\nimport json\nPYEOF",
    "gh issue create --title x --body \"$(cat <<'EOF'\n## Why\nEOF\n)\"",
    # codex hands every shell call over wrapped, so the composition sits one
    # level in and the outer line looks like a single command.
    "/bin/zsh -lc 'cd /tmp && ls'",
    # The `&&` sits outside the quotes, so it chains two local steps and the
    # ssh exemption does not reach it.
    "ssh prod uptime && echo done",
])
def test_a_line_carrying_more_than_one_step_is_refused(line):
    assert composition_refusal(line) != ""


@pytest.mark.parametrize("line", [
    "uv run pytest -q",
    "trace grep composition_refusal",
    "git status",
    "python3 scripts/sync.py",
    "cat a.py > b.py",
    # The read-side pipe: the consumer only shapes what it is given.
    "uv run pytest -q 2>&1 | tail -40",
    "git status --porcelain | wc -l",
    "rg import packages/agents/hooks | sort | uniq -c",
    # A separator inside a quoted argument is part of that argument.
    "rg 'a && b' README.md",
    "git commit -m 'add x; drop y'",
    "echo 'one | two'",
    # Composition is judged per machine: a remote session's state dies with the
    # call, so a chain over ssh cannot be split into calls.
    "ssh prod 'export PGPASSWORD=$(grep DB .env | cut -d= -f2) && psql -c \"\\dt\"'",
    # The same remote chain in codex's shape: the harness sends the shell call as
    # a list that command_str joins with spaces, so the wrapper's own quoting is
    # gone and only the payload's survives.
    "/bin/zsh -lc ssh prod 'cd /srv && ls'",
])
def test_a_single_step_line_is_allowed(line):
    assert composition_refusal(line) == ""


def test_a_remote_line_is_exempt_from_composition_but_still_read():
    """Composition is judged per machine; the write guards are judged everywhere.
    The exemption must never take the remote `rm` away from `invocations`."""
    line = "ssh prod 'rm -rf /srv && systemctl restart app'"
    assert composition_refusal(line) == ""
    assert "rm" in heads(line)


def test_a_line_hiding_its_own_program_is_refused():
    """`hides_execution` already answers this for the declaration gates. The atomic
    gate reaches every session, so the same shapes answer here too."""
    assert composition_refusal("python3 -c 'import os'") != ""
    assert composition_refusal("bash /tmp/scratch.sh") != ""
    assert composition_refusal("cat 'unbalanced") != ""
