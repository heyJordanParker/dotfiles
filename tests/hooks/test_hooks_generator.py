"""Failure contracts for generated Claude and codex hook wiring."""

import json
from pathlib import Path

import hooks
import pytest

REPO = Path(__file__).parents[2]
HOOKS_DIR = REPO / "packages" / "agents" / "hooks"
CLAUDE_SETTINGS = REPO / "packages" / "claude" / "settings.json"
CODEX_CONFIG = REPO / "packages" / "codex" / "config.toml"


def test_codex_event_tables_are_pascal_case():
    """Pins the inert-wiring failure caused by snake_case codex event tables."""
    rendered = hooks.render_codex(
        "[[hooks.old]]\n\n[hooks.state]\n",
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
    config.write_text("[[hooks.old]]\n\n[hooks.state]\n")
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
    config.write_text("[[hooks.old]]\n\n[hooks.state]\n")
    profiles.mkdir()
    before = config.read_bytes()

    with pytest.raises(ValueError, match="codex has no SessionEnd event"):
        hooks.generate(hooks_dir, settings, config, profiles)

    assert config.read_bytes() == before


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
