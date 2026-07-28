"""Contract for the hook-wiring generator (scripts/hooks.py).

The generator turns each hook's module-level BINDING into both harness wirings:
Claude's settings.json hooks section and codex's config.toml [hooks] tables +
[hooks.state] trust hashes. These tests exercise it against fixture hooks (with
BINDINGs we write here) and fixture wiring files, asserting the contract the
real regeneration depends on:

- a "claude" hook is emitted to Claude only, absent from codex;
- an "all" hook is emitted to both;
- codex emits a trust hash for every command it writes;
- non-hook sections and unmanaged hook entries are byte-identical on a no-op.
"""

import hashlib
import json
import re

import hooks as generator
import pytest

# --- fixtures ----------------------------------------------------------------

def _write_hook(hooks_dir, name, binding):
    """Write a fixture hook file carrying a module-level BINDING. The body is
    inert — the generator reads BINDING statically and never runs it."""
    body = "BINDING = " + repr(binding) + "\n\n\ndef main():\n    pass\n"
    (hooks_dir / f"{name}.py").write_text(body)


@pytest.fixture
def hooks_dir(tmp_path):
    d = tmp_path / "hooks"
    d.mkdir()
    # An "all" hook on a tool event (both harnesses) and a "claude"-only hook on
    # a non-tool event (Claude wiring only).
    _write_hook(d, "guard_shell", {
        "events": {"PreToolUse": ["Bash"]},
        "harness": "all",
    })
    _write_hook(d, "classify_prompt", {
        "events": {"UserPromptSubmit": []},
        "harness": "claude",
    })
    # An "all" hook carrying an explicit timeout, on its own event so the rendered
    # timeout is unambiguous to assert against in both wirings.
    _write_hook(d, "validate_completion", {
        "events": {"Stop": []},
        "harness": "all",
        "timeout": 90,
    })
    return d


@pytest.fixture
def settings_path(tmp_path):
    """A Claude settings.json with: a non-hook section (permissions), an
    unmanaged shell-glue hook, an unmanaged inline prompt hook, and a stale
    managed entry the generator must rewrite."""
    settings = {
        "permissions": {"allow": ["Bash(git status:*)"], "defaultMode": "bypassPermissions"},
        "hooks": {
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [
                    {"type": "command", "command": "python3 ~/.agents/hooks/guard_shell.py"},
                ]},
                {"matcher": "*", "hooks": [
                    {"type": "command", "command": "bash '/Users/jordan/.claude/hooks/herdr.sh' working"},
                ]},
            ],
            "UserPromptSubmit": [
                {"hooks": [
                    {"type": "command", "command": "python3 ~/.agents/hooks/classify_prompt.py"},
                ]},
            ],
            "AskUserQuestion": [
                {"matcher": "AskUserQuestion", "hooks": [
                    {"type": "prompt", "prompt": "Question quality gate."},
                ]},
            ],
        },
    }
    path = tmp_path / "settings.json"
    path.write_text(json.dumps(settings, indent=2) + "\n")
    return path


@pytest.fixture
def config_path(tmp_path):
    """A codex config.toml with non-hook sections on both sides of the hook
    region, the [[hooks.*]] blocks, and a [hooks.state] table."""
    text = (
        'model = "gpt-5.5"\n'
        "\n"
        "[features]\n"
        "hooks = true\n"
        "\n"
        "[[hooks.pre_tool_use]]\n"
        "[[hooks.pre_tool_use.hooks]]\n"
        'type = "command"\n'
        'command = "python3 /Users/jordan/.agents/hooks/guard_shell.py"\n'
        "\n"
        "[hooks.state]\n"
        "\n"
        '[hooks.state."/Users/jordan/.codex/config.toml:pre_tool_use:0:0"]\n'
        'trusted_hash = "sha256:deadbeef"\n'
        "\n"
        "[projects.\"/Users/jordan/dotfiles\"]\n"
        'trust_level = "trusted"\n'
    )
    path = tmp_path / "config.toml"
    path.write_text(text)
    return path


# --- BINDING discovery -------------------------------------------------------

def test_read_bindings_is_static(hooks_dir):
    """BINDING is read without importing the hook — a hook that raises on import
    still yields its BINDING."""
    (hooks_dir / "explodes.py").write_text(
        'BINDING = {"events": {"Stop": []}, "harness": "all"}\n'
        'raise RuntimeError("import side effect")\n'
    )
    bindings = generator.read_bindings(str(hooks_dir))
    assert bindings["explodes"] == {"events": {"Stop": []}, "harness": "all"}


def test_hook_without_binding_is_skipped(hooks_dir):
    (hooks_dir / "no_binding.py").write_text("def main():\n    pass\n")
    bindings = generator.read_bindings(str(hooks_dir))
    assert "no_binding" not in bindings


# --- Claude wiring -----------------------------------------------------------

def test_claude_all_and_claude_hooks_present(hooks_dir, settings_path, config_path, profiles_dir):
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    settings = json.loads(settings_path.read_text())
    commands = _claude_commands(settings)
    assert "python3 ~/.agents/hooks/guard_shell.py" in commands      # harness: all
    assert "python3 ~/.agents/hooks/classify_prompt.py" in commands  # harness: claude


def test_claude_unmanaged_entries_survive_noop_byte_identical(hooks_dir, settings_path, config_path, profiles_dir):
    """A no-op regeneration leaves the inline-prompt gate and the shell-glue
    entry byte-identical — same hook objects, in the same groups, unchanged.
    These have no BINDING source; only managed `.py` command entries are rewritten."""
    inline_prompt = {"type": "prompt", "prompt": "Question quality gate."}
    shell_glue = {"type": "command", "command": "bash '/Users/jordan/.claude/hooks/herdr.sh' working"}

    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    hooks_section = json.loads(settings_path.read_text())["hooks"]

    all_entries = [h for groups in hooks_section.values() for group in groups for h in group["hooks"]]
    assert inline_prompt in all_entries
    assert shell_glue in all_entries


def test_claude_non_hook_sections_untouched(hooks_dir, settings_path, config_path, profiles_dir):
    before = json.loads(settings_path.read_text())["permissions"]
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    after = json.loads(settings_path.read_text())["permissions"]
    assert after == before


def test_claude_noop_is_idempotent(hooks_dir, settings_path, config_path, profiles_dir):
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    once = settings_path.read_text()
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    assert settings_path.read_text() == once


# --- codex wiring ------------------------------------------------------------

def test_codex_excludes_claude_only_hook(hooks_dir, settings_path, config_path, profiles_dir):
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    text = config_path.read_text()
    assert "guard_shell.py" in text          # harness: all -> present
    assert "classify_prompt.py" not in text  # harness: claude -> absent


def test_codex_event_tables_are_pascal_case(hooks_dir, settings_path, config_path, profiles_dir):
    """The table name is the Event verbatim.

    codex renames the fields of its hooks struct to the PascalCase event names and
    sets no deny_unknown_fields, so a snake_case table is an unknown key: it parses
    to an empty struct, discovers no handler, and warns about nothing. Every codex
    hook was inert this way, which is invisible from the config file alone.
    """
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    text = config_path.read_text()
    assert "[[hooks.PreToolUse]]" in text
    assert "[[hooks.pre_tool_use]]" not in text


def _expected_trust_hash(module, snake_event, timeout):
    """The hash codex computes for one emitted handler.

    Spelled out here rather than called from the generator so the test pins the
    formula instead of agreeing with whatever the generator currently does.
    """
    identity = {
        "event_name": snake_event,
        "hooks": [{"type": "command",
                   "command": f"python3 {generator.CODEX_HOOK_DIR}/{module}.py",
                   "async": False,
                   "timeout": timeout}],
    }
    canonical = json.dumps(identity, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def test_codex_trust_hash_covers_the_normalized_identity(hooks_dir, settings_path, config_path, profiles_dir):
    """The hash covers codex's normalized identity, not the command string.

    codex hashes the event's snake_case label plus the matcher group reduced to the
    one handler with its timeout resolved, key-sorted and serialized compactly. A
    hash it disagrees with reads as Modified and suppresses the handler as quietly
    as a misnamed table does.
    """
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    text = config_path.read_text()
    digest = _expected_trust_hash("validate_completion", "stop", 90)
    assert f'trusted_hash = "sha256:{digest}"' in text
    assert hashlib.sha256(
        f"python3 {generator.CODEX_HOOK_DIR}/validate_completion.py".encode()
    ).hexdigest() not in text          # the command-string hash codex rejects


def test_codex_trust_hash_resolves_an_omitted_timeout(hooks_dir, settings_path, config_path, profiles_dir):
    """An omitted timeout is absent from the file but present in the hash.

    codex resolves it to its own default before hashing, so a hash taken over what
    the file shows drifts for exactly the hooks that declare no timeout.
    """
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    text = config_path.read_text()
    digest = _expected_trust_hash("guard_shell", "pre_tool_use", generator._CODEX_DEFAULT_TIMEOUT)
    assert f'trusted_hash = "sha256:{digest}"' in text


def test_codex_emits_a_trust_entry_per_command(hooks_dir, settings_path, config_path, profiles_dir):
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    text = config_path.read_text()
    commands = re.findall(r'^command = "(.+)"$', text, re.M)
    assert commands, "expected at least one emitted codex command"
    assert len(re.findall(r"^trusted_hash = ", text, re.M)) == len(commands)


def test_codex_non_hook_sections_untouched(hooks_dir, settings_path, config_path, profiles_dir):
    before = config_path.read_text()
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    after = config_path.read_text()
    for section in ('model = "gpt-5.5"', "[features]", "hooks = true",
                    '[projects."/Users/jordan/dotfiles"]', 'trust_level = "trusted"'):
        assert section in before and section in after


def test_codex_noop_is_idempotent(hooks_dir, settings_path, config_path, profiles_dir):
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    once = config_path.read_text()
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    assert config_path.read_text() == once


# --- timeout -----------------------------------------------------------------

def test_timeout_rendered_into_both_wirings_only_when_declared(hooks_dir, settings_path, config_path, profiles_dir):
    """A BINDING with `timeout` renders it into both wirings (Claude `timeout`
    field, codex `timeout = N`); a BINDING without one renders no timeout in
    either. validate_completion declares timeout 90; guard_shell declares none."""
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    settings = json.loads(settings_path.read_text())
    config = config_path.read_text()

    # declared -> present in both
    with_timeout = _claude_entry(settings, "python3 ~/.agents/hooks/validate_completion.py")
    assert with_timeout["timeout"] == 90
    assert "command = \"python3 /Users/jordan/.agents/hooks/validate_completion.py\"\ntimeout = 90" in config

    # omitted -> absent in both
    without_timeout = _claude_entry(settings, "python3 ~/.agents/hooks/guard_shell.py")
    assert "timeout" not in without_timeout
    assert "command = \"python3 /Users/jordan/.agents/hooks/guard_shell.py\"\ntimeout" not in config


# --- roots -------------------------------------------------------------------

@pytest.fixture
def profiles_dir(tmp_path):
    """Two profile config roots, each with its own settings.json — one declaring
    an empty hooks object, one with an unmanaged entry to preserve."""
    d = tmp_path / "profiles"
    (d / "copywriter").mkdir(parents=True)
    (d / "experimental").mkdir(parents=True)
    (d / "copywriter" / "settings.json").write_text(json.dumps({
        "hooks": {"Stop": [{"hooks": [{"type": "prompt", "prompt": "profile gate"}]}]},
        "theme": "dark",
    }, indent=2) + "\n")
    (d / "experimental" / "settings.json").write_text(json.dumps({"hooks": {}}, indent=2) + "\n")
    return d


def test_roots_all_hook_lands_in_every_profile(hooks_dir, settings_path, config_path, profiles_dir):
    _write_hook(hooks_dir, "block_memory_access", {
        "events": {"PreToolUse": ["mcp__plugin_honcho_honcho__.*"]},
        "harness": "claude",
        "roots": "all",
    })
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))

    for profile in ("copywriter", "experimental"):
        settings = json.loads((profiles_dir / profile / "settings.json").read_text())
        assert "python3 ~/.agents/hooks/block_memory_access.py" in _claude_commands(settings)
    # and in the default root too
    assert "python3 ~/.agents/hooks/block_memory_access.py" in \
        _claude_commands(json.loads(settings_path.read_text()))


def test_profiles_get_only_the_opted_in_hooks(hooks_dir, settings_path, config_path, profiles_dir):
    """A profile that declares no hooks stays that way apart from the opt-ins —
    the workflow hooks belong to the default root alone."""
    _write_hook(hooks_dir, "block_memory_access", {
        "events": {"PreToolUse": ["mcp__plugin_honcho_honcho__.*"]},
        "harness": "claude",
        "roots": "all",
    })
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    commands = _claude_commands(json.loads((profiles_dir / "experimental" / "settings.json").read_text()))
    assert commands == ["python3 ~/.agents/hooks/block_memory_access.py"]


def test_profile_unmanaged_entries_and_settings_survive(hooks_dir, settings_path, config_path, profiles_dir):
    _write_hook(hooks_dir, "block_memory_access", {
        "events": {"PreToolUse": ["mcp__plugin_honcho_honcho__.*"]},
        "harness": "claude",
        "roots": "all",
    })
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    settings = json.loads((profiles_dir / "copywriter" / "settings.json").read_text())
    assert settings["theme"] == "dark"
    assert settings["hooks"]["Stop"] == [{"hooks": [{"type": "prompt", "prompt": "profile gate"}]}]


def test_profile_generation_is_idempotent(hooks_dir, settings_path, config_path, profiles_dir):
    _write_hook(hooks_dir, "block_memory_access", {
        "events": {"PreToolUse": ["mcp__plugin_honcho_honcho__.*"]},
        "harness": "claude",
        "roots": "all",
    })
    path = profiles_dir / "copywriter" / "settings.json"
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    once = path.read_text()
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    assert path.read_text() == once


def test_profiles_untouched_without_an_opted_in_hook(hooks_dir, settings_path, config_path, profiles_dir):
    """No hook declares roots: all, so no profile gains anything."""
    path = profiles_dir / "experimental" / "settings.json"
    before = path.read_text()
    generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
    assert path.read_text() == before


# --- helpers -----------------------------------------------------------------

def _claude_entry(settings, command):
    for groups in settings["hooks"].values():
        for group in groups:
            for h in group["hooks"]:
                if h.get("command") == command:
                    return h
    raise AssertionError(f"no Claude hook entry for command {command!r}")

def _claude_commands(settings):
    return [
        h.get("command")
        for groups in settings["hooks"].values()
        for group in groups
        for h in group["hooks"]
        if h.get("type") == "command"
    ]


def test_an_event_codex_cannot_fire_stops_the_generator(hooks_dir, settings_path, config_path, profiles_dir):
    """A codex-bound Event codex has no field for must fail loudly.

    codex sets no deny_unknown_fields on its hooks struct, so a table it does not
    know is dropped in silence — the failure that left the whole codex wiring inert.
    A generator that emits one ships that silence again.
    """
    _write_hook(hooks_dir, "on_session_end", {"events": {"SessionEnd": []}, "harness": "all"})
    with pytest.raises(ValueError, match="codex has no SessionEnd event"):
        generator.generate(str(hooks_dir), str(settings_path), str(config_path), str(profiles_dir))
