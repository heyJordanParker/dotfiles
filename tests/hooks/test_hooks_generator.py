"""Failure contracts for generated Claude and codex hook wiring."""

import json
from pathlib import Path

import hooks
import pytest

REPO = Path(__file__).parents[2]
HOOKS_DIR = REPO / "packages" / "agents" / "hooks"
CLAUDE_SETTINGS = REPO / "packages" / "claude" / "settings.json"
CODEX_CONFIG = REPO / "packages" / "codex-system" / "config.toml"


def test_codex_event_tables_are_pascal_case():
    """Pins the inert-wiring failure caused by snake_case codex event tables."""
    rendered = hooks.render_codex(
        "[[hooks.old]]\n",
        {"guard": {"events": {"PreToolUse": ["*"]}, "harness": "codex"}},
    )

    assert "[[hooks.PreToolUse]]" in rendered
    assert "[[hooks.pre_tool_use]]" not in rendered


def test_canonical_claude_settings_are_byte_identical_after_generation(tmp_path):
    """Pins pre-commit churn when canonical settings.json is regenerated."""
    settings = tmp_path / "settings.json"
    config = tmp_path / "config.toml"
    profiles = tmp_path / "profiles"
    settings.write_bytes(CLAUDE_SETTINGS.read_bytes())
    config.write_bytes(CODEX_CONFIG.read_bytes())
    profiles.mkdir()
    before = settings.read_bytes()

    hooks.generate(HOOKS_DIR, settings, config, profiles)

    assert settings.read_bytes() == before


def test_canonical_codex_config_is_byte_identical_after_generation(tmp_path):
    """Pins pre-commit churn when canonical config.toml is regenerated."""
    settings = tmp_path / "settings.json"
    config = tmp_path / "config.toml"
    profiles = tmp_path / "profiles"
    settings.write_bytes(CLAUDE_SETTINGS.read_bytes())
    config.write_bytes(CODEX_CONFIG.read_bytes())
    profiles.mkdir()
    before = config.read_bytes()

    hooks.generate(HOOKS_DIR, settings, config, profiles)

    assert config.read_bytes() == before


def test_unmanaged_claude_hook_survives_generation(tmp_path):
    """Pins deletion of hand-written settings.json hooks during regeneration."""
    unmanaged = {"type": "command", "command": "notify-send done"}
    settings = tmp_path / "settings.json"
    config = tmp_path / "config.toml"
    profiles = tmp_path / "profiles"
    settings.write_text(json.dumps({"hooks": {"Stop": [{"hooks": [unmanaged]}]}}))
    config.write_text("[[hooks.old]]\n")
    profiles.mkdir()

    hooks.generate(tmp_path / "missing-hooks", settings, config, profiles)

    assert json.loads(settings.read_text())["hooks"]["Stop"] == [{"hooks": [unmanaged]}]


def test_unsupported_codex_event_fails_without_writing_dead_wiring(tmp_path):
    """Pins silent dead wiring when a BINDING names an event codex cannot fire."""
    hooks_dir = tmp_path / "hooks"
    hooks_dir.mkdir()
    (hooks_dir / "dead.py").write_text(
        'BINDING = {"events": {"SessionEnd": []}, "harness": "codex"}\n'
    )
    settings = tmp_path / "settings.json"
    config = tmp_path / "config.toml"
    profiles = tmp_path / "profiles"
    settings.write_text("{}\n")
    config.write_text("[[hooks.old]]\n")
    profiles.mkdir()
    before = config.read_bytes()

    with pytest.raises(ValueError, match="codex has no SessionEnd event"):
        hooks.generate(hooks_dir, settings, config, profiles)

    assert config.read_bytes() == before


def test_codex_groups_carry_tool_matchers():
    """Pins every codex hook starting on every tool call because no group had a matcher."""
    rendered = hooks.render_codex(
        "[[hooks.old]]\n",
        {"shell": {"events": {"PreToolUse": ["Bash"]}, "harness": "codex"},
         "patch": {"events": {"PreToolUse": ["Write"]}, "harness": "codex"}},
    )

    assert 'matcher = "Bash"' in rendered
    assert 'matcher = "Write"' in rendered


def test_one_tool_call_starts_one_runner():
    """Pins a separate interpreter per quick hook on every tool call."""
    bindings = {
        "a": {"events": {"PreToolUse": ["Bash"]}, "timeout": 5},
        "b": {"events": {"PreToolUse": ["Bash"]}, "timeout": 5},
        "c": {"events": {"PreToolUse": ["Bash", "Write"]}, "timeout": 5},
        "slow": {"events": {"PreToolUse": ["Bash"]}, "standalone": True},
    }

    groups = hooks.render_claude({}, bindings)["hooks"]["PreToolUse"]
    bash = [hook["command"] for group in groups if group["matcher"] == "Bash"
            for hook in group["hooks"]]

    assert bash == ["python3 ~/.agents/hooks/slow.py",
                    "python3 ~/.agents/hooks/combine_hooks.py a b c"]


def test_shipped_bindings_only_name_events_their_harness_fires():
    """Pins silently inert shipped hooks bound to events their harness never fires."""
    invalid = []
    for module, binding in hooks.read_bindings(HOOKS_DIR).items():
        events = set(binding.get("events", {}))
        harness = binding.get("harness", "all")
        if harness in {"all", "claude"}:
            invalid.extend((module, "claude", event) for event in events - hooks.CLAUDE_EVENT)
        if harness in {"all", "codex"}:
            invalid.extend((module, "codex", event) for event in events - hooks.CODEX_EVENT.keys())

    assert invalid == []
