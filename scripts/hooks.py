"""Generate hook wiring for both harnesses from each hook's BINDING declaration.

Each Python hook in packages/agents/hooks owns a module-level BINDING constant
declaring the lifecycle events and tool matchers it attaches to, and whether it
runs on both harnesses or Claude only:

    BINDING = {
        "events": {"<Event>": ["<matcher>", ...], ...},
        "harness": "all" | "claude",
    }

This reads every hook's BINDING statically (ast.literal_eval — never imports or
runs the hook, the way frontmatter.py reads agent files), then rewrites the
managed entries of packages/claude/settings.json and packages/codex/config.toml.

Managed = a `type: command` hook whose command invokes ~/.agents/hooks/<module>.py.
Every other entry — inline `type: prompt` gates, third-party shell glue (tmux,
herdr, superset) — is unmanaged and preserved byte-for-byte; non-hook sections
(permissions, env, model, MCP servers, project trust, desktop) are never touched.
For codex it also regenerates the [hooks.state] trust hashes for the exact
commands it emits.
"""

import ast
import glob
import hashlib
import json
import os
import re

import files

CLAUDE_HOOK_DIR = "~/.agents/hooks"
CODEX_HOOK_DIR = "/Users/jordan/.agents/hooks"

# Codex lifecycle event name (snake_case) keyed on the shared Event name.
CODEX_EVENT = {
    "PreToolUse": "pre_tool_use",
    "PostToolUse": "post_tool_use",
    "PermissionRequest": "permission_request",
    "UserPromptSubmit": "user_prompt_submit",
    "SessionStart": "session_start",
    "Stop": "stop",
    "PreCompact": "pre_compact",
    "PostCompact": "post_compact",
    "SubagentStart": "subagent_start",
    "SubagentStop": "subagent_stop",
    "SessionEnd": "session_end",
}


# --- BINDING discovery -------------------------------------------------------

def read_bindings(hooks_dir):
    """Map module name -> BINDING dict for every hook that declares one.

    Parses each file's AST and literal-evaluates its top-level BINDING
    assignment; never imports or executes the hook.
    """
    out = {}
    for path in sorted(glob.glob(os.path.join(hooks_dir, "*.py"))):
        binding = _binding_of(path)
        if binding is not None:
            out[os.path.splitext(os.path.basename(path))[0]] = binding
    return out


def _binding_of(path):
    with open(path, encoding="utf-8") as f:
        tree = ast.parse(f.read(), filename=path)
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id == "BINDING":
                    return ast.literal_eval(node.value)
    return None


# --- event/matcher expansion -------------------------------------------------

def _matcher_key(matchers):
    """The matcher string a group carries for this matcher list.

    [] (non-tool event) -> None (group has no matcher key); ["*"] -> "*";
    a tool list -> the pipe-joined regex Claude expects ("Write|Edit").
    """
    if not matchers:
        return None
    return "|".join(matchers)


def _commands_by_group(bindings, hook_dir, harnesses):
    """Map (Event, matcher_key) -> [(command, timeout), ...] for hooks whose
    harness is in `harnesses`, in stable module order. matcher_key is None for
    non-tool events; timeout is None when the BINDING omits it.
    """
    groups = {}
    for module in sorted(bindings):
        binding = bindings[module]
        if binding.get("harness", "all") not in harnesses:
            continue
        command = f"python3 {hook_dir}/{module}.py"
        timeout = binding.get("timeout")
        for event, matchers in binding.get("events", {}).items():
            key = (event, _matcher_key(matchers))
            groups.setdefault(key, []).append((command, timeout))
    return groups


# --- Claude settings.json ----------------------------------------------------

_MANAGED_RE = re.compile(r"~/\.agents/hooks/[\w\-]+\.py")


def _is_managed_claude(entry):
    return (
        entry.get("type") == "command"
        and _MANAGED_RE.search(entry.get("command", "")) is not None
    )


def render_claude(settings, bindings):
    """Return a new settings dict with managed hook entries regenerated from
    BINDING; unmanaged entries and every non-hook section stay identical.
    """
    groups = _commands_by_group(bindings, CLAUDE_HOOK_DIR, {"all", "claude"})
    new = dict(settings)
    new["hooks"] = _merge_claude_hooks(settings.get("hooks", {}), groups)
    return new


def _merge_claude_hooks(existing, groups):
    """For each event, strip managed command entries from existing groups, append
    the regenerated managed groups, and drop groups left empty.
    """
    events = list(existing.keys())
    for event, _ in groups:
        if event not in events:
            events.append(event)

    out = {}
    for event in events:
        kept = _strip_managed_claude(existing.get(event, []))
        generated = _generated_claude_groups(event, groups)
        merged = kept + generated
        if merged:
            out[event] = merged
    return out


def _strip_managed_claude(event_groups):
    """Drop managed command entries from each group; drop groups left with no
    hooks. Unmanaged entries (inline prompts, shell glue) are preserved verbatim.
    """
    kept = []
    for group in event_groups:
        hooks = [h for h in group.get("hooks", []) if not _is_managed_claude(h)]
        if hooks:
            kept.append({**group, "hooks": hooks})
    return kept


def _generated_claude_groups(event, groups):
    out = []
    for (ev, matcher_key), commands in groups.items():
        if ev != event:
            continue
        hooks = [_claude_hook(command, timeout) for command, timeout in commands]
        group = {"hooks": hooks} if matcher_key is None else {"matcher": matcher_key, "hooks": hooks}
        out.append(group)
    return out


def _claude_hook(command, timeout):
    entry = {"type": "command", "command": command}
    if timeout is not None:
        entry["timeout"] = timeout
    return entry


# --- codex config.toml -------------------------------------------------------

def render_codex(config_text, bindings):
    """Return config.toml text with the [[hooks.<Event>]] blocks and the
    [hooks.state] trust table regenerated from BINDING; every other section
    (mcp_servers, features, plugins, marketplaces, projects, desktop, ...) stays
    byte-identical.
    """
    groups = _commands_by_group(bindings, CODEX_HOOK_DIR, {"all", "codex"})
    blocks, hooks_by_event = _codex_hook_blocks(groups)
    state = _codex_state_table(hooks_by_event)
    return _replace_codex_hook_region(config_text, blocks + "\n\n" + state)


def _codex_hook_blocks(groups):
    """The [[hooks.<Event>]] TOML text plus, per codex event, the ordered
    (command, timeout) list (codex fires per event without tool matchers; the
    Python hook self-filters, matching the existing config). timeout is omitted
    from the block when the BINDING omits it.
    """
    hooks_by_event = {}
    for (event, _matcher_key), commands in groups.items():
        codex_event = CODEX_EVENT[event]
        hooks_by_event.setdefault(codex_event, [])
        for command, timeout in commands:
            if (command, timeout) not in hooks_by_event[codex_event]:
                hooks_by_event[codex_event].append((command, timeout))

    blocks = []
    for codex_event in sorted(hooks_by_event):
        lines = [f"[[hooks.{codex_event}]]"]
        for command, timeout in hooks_by_event[codex_event]:
            lines += [f"[[hooks.{codex_event}.hooks]]", 'type = "command"', f'command = "{command}"']
            if timeout is not None:
                lines.append(f"timeout = {timeout}")
        blocks.append("\n".join(lines))
    return "\n\n".join(blocks), hooks_by_event


def _codex_state_table(hooks_by_event):
    """The [hooks.state] trust table: one entry per emitted command, keyed
    config.toml:<event>:<group_index>:<hook_index>, hashed over the command.
    """
    entries = []
    for codex_event in sorted(hooks_by_event):
        for hook_index, (command, _timeout) in enumerate(hooks_by_event[codex_event]):
            key = f"/Users/jordan/.codex/config.toml:{codex_event}:0:{hook_index}"
            digest = hashlib.sha256(command.encode("utf-8")).hexdigest()
            entries.append(f'[hooks.state."{key}"]\ntrusted_hash = "sha256:{digest}"')
    return "[hooks.state]\n\n" + "\n\n".join(entries)


# The managed codex region runs from the first [[hooks. block through the end of
# the [hooks.state] table — everything between is generator-owned. Markers below
# bound it precisely so the surrounding hand-authored TOML stays byte-identical.
_CODEX_BEGIN = re.compile(r"^\[\[hooks\.", re.M)
_CODEX_STATE = re.compile(r"^\[hooks\.state\]", re.M)


def _replace_codex_hook_region(text, generated):
    """Splice the generated hook region in place of the existing one.

    The region is [first [[hooks. block .. end of [hooks.state] table]. The
    [hooks.state] table ends at the next top-level [section] that is not a
    hooks.state subtable, or end of file.
    """
    begin = _CODEX_BEGIN.search(text)
    state = _CODEX_STATE.search(text)
    if begin is None or state is None:
        raise ValueError("config.toml has no [[hooks.*]] / [hooks.state] region to regenerate")
    end = _state_region_end(text, state.end())
    return text[: begin.start()] + generated + "\n\n" + text[end:]


def _state_region_end(text, search_from):
    """Offset just past the [hooks.state] table — the start of the next top-level
    section that is not a [hooks.state."..."] subtable, or len(text).
    """
    for match in re.finditer(r"^\[(?!hooks\.state)[^\]]+\]", text[search_from:], re.M):
        return search_from + match.start()
    return len(text)


# --- entry point -------------------------------------------------------------

def generate(hooks_dir, claude_settings_path, codex_config_path):
    bindings = read_bindings(hooks_dir)

    with open(claude_settings_path, encoding="utf-8") as f:
        settings = json.load(f)
    settings = render_claude(settings, bindings)
    files.write_if_changed(claude_settings_path, json.dumps(settings, indent=2) + "\n")

    with open(codex_config_path, encoding="utf-8") as f:
        config_text = f.read()
    files.write_if_changed(codex_config_path, render_codex(config_text, bindings))
