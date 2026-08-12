"""Generate hook wiring for both harnesses from each hook's BINDING declaration.

Each Python hook in packages/agents/hooks owns a module-level BINDING constant
declaring the lifecycle events and tool matchers it attaches to, and whether it
runs on both harnesses or Claude only:

    BINDING = {
        "events": {"<Event>": ["<matcher>", ...], ...},
        "harness": "all" | "claude",
        "roots": "all",            # optional
    }

`roots: "all"` puts a hook in every Claude config root — the default settings.json
and each profile's — instead of the default root alone. An optional
`"except": ["<profile>"]` beside it keeps named profiles out — for a profile
whose own machinery replaces the guard. A profile is a hand-kept
copy, so a guard that must hold everywhere would otherwise depend on someone
remembering to paste it into each one. Everything without the key stays in the
default root only, which is where all the workflow hooks belong: a profile that
declares `"hooks": {}` means it, and is left with only the hooks that opt in.

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

# Every event codex has a field for, snake_case label keyed on the shared Event
# name. This is the whole set — codex's hooks struct sets no deny_unknown_fields,
# so a table it has no field for is dropped without a warning, which is how the
# entire codex wiring sat inert. An Event absent here is one codex cannot fire, so
# emitting it would ship that same silence; `render_codex` raises instead.
# `SessionEnd` was listed here and is not one of codex's.
# Every event Claude Code fires, read from the harness binary at 2.1.226. Claude
# ignores a hook wired to a name it does not know, in silence — the same failure
# mode CODEX_EVENT exists to stop, so the Claude side is checked the same way.
CLAUDE_EVENT = {
    "PreToolUse", "PostToolUse", "PostToolUseFailure", "PostToolBatch", "Notification",
    "UserPromptSubmit", "UserPromptExpansion", "SessionStart", "SessionEnd", "Stop",
    "StopFailure", "SubagentStart", "SubagentStop", "PreCompact", "PostCompact",
    "PermissionRequest", "PermissionDenied", "Setup", "TeammateIdle", "TaskCreated",
    "TaskCompleted", "Elicitation", "ElicitationResult", "ConfigChange", "WorktreeCreate",
    "WorktreeRemove", "InstructionsLoaded", "CwdChanged", "FileChanged", "DirectoryAdded",
    "MessageDisplay",
}

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
    """Map (Event, matcher_key) -> [(command, timeout, async_rewake), ...] for
    hooks whose harness is in `harnesses`, in stable module order. matcher_key is
    None for non-tool events; timeout and async_rewake are None when omitted.
    """
    groups = {}
    for module in sorted(bindings):
        binding = bindings[module]
        if binding.get("harness", "all") not in harnesses:
            continue
        command = f"python3 {hook_dir}/{module}.py"
        timeout = binding.get("timeout")
        async_rewake = binding.get("asyncRewake")
        for event, matchers in binding.get("events", {}).items():
            key = (event, _matcher_key(matchers))
            groups.setdefault(key, []).append((command, timeout, async_rewake))
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
    for event, _matcher_key in groups:
        if event not in CLAUDE_EVENT:
            raise ValueError(
                "Claude has no %s event; a hook wired to it never fires and "
                "nothing says so. Fix the BINDING." % event)
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
        hooks = [_claude_hook(command, timeout, async_rewake)
                 for command, timeout, async_rewake in commands]
        group = {"hooks": hooks} if matcher_key is None else {"matcher": matcher_key, "hooks": hooks}
        out.append(group)
    return out


def _claude_hook(command, timeout, async_rewake):
    entry = {"type": "command", "command": command}
    if timeout is not None:
        entry["timeout"] = timeout
    if async_rewake is not None:
        entry["asyncRewake"] = async_rewake
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
    """The [[hooks.<Event>]] TOML text plus, per Event, the ordered
    (command, timeout) list (codex fires per event without tool matchers; the
    Python hook self-filters, matching the existing config). timeout is omitted
    from the block when the BINDING omits it.

    The table name is the Event verbatim — codex deserializes the `hooks` table
    into a struct whose fields are renamed to the PascalCase event names, with no
    deny_unknown_fields. A snake_case table is an unknown key: it parses to an
    empty struct, no handler is ever discovered, and nothing warns. Every codex
    hook was silently inert until this used the Event name.
    """
    hooks_by_event = {}
    for (event, _matcher_key), commands in groups.items():
        if event not in CODEX_EVENT:
            raise ValueError(
                "codex has no %s event; a BINDING declaring it with harness "
                "all/codex would be dropped silently. Bind it to claude." % event)
        hooks_by_event.setdefault(event, [])
        for command, timeout, _ in commands:
            if (command, timeout) not in hooks_by_event[event]:
                hooks_by_event[event].append((command, timeout))

    blocks = []
    for event in sorted(hooks_by_event):
        lines = [f"[[hooks.{event}]]"]
        for command, timeout in hooks_by_event[event]:
            lines += [f"[[hooks.{event}.hooks]]", 'type = "command"', f'command = "{command}"']
            if timeout is not None:
                lines.append(f"timeout = {timeout}")
        blocks.append("\n".join(lines))
    return "\n\n".join(blocks), hooks_by_event


# A handler codex resolves but we never write: an omitted timeout resolves to
# codex's own default, and the hash covers the resolved value, not the file's.
_CODEX_DEFAULT_TIMEOUT = 600


def _codex_trust_hash(event, command, timeout):
    """The trust hash codex computes for one handler.

    Not a hash of the command. Codex builds a normalized identity — the event's
    snake_case label plus the matcher group reduced to this one handler, with the
    timeout resolved — serializes it, sorts every key, and hashes the compact
    JSON bytes. Absent fields (matcher, statusMessage, commandWindows) drop out
    rather than serializing as null.
    """
    identity = {
        "event_name": CODEX_EVENT[event],
        "hooks": [{
            "type": "command",
            "command": command,
            "async": False,
            "timeout": _CODEX_DEFAULT_TIMEOUT if timeout is None else timeout,
        }],
    }
    canonical = json.dumps(identity, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def _codex_state_table(hooks_by_event):
    """The [hooks.state] trust table: one entry per emitted command, keyed
    config.toml:<snake_event>:<group_index>:<hook_index>. The key keeps the
    snake_case label even though the table above it is PascalCase.

    A hash codex disagrees with reads as Modified, a missing one as Untrusted,
    and either suppresses the handler as silently as a misnamed table does.
    """
    entries = []
    for event in sorted(hooks_by_event):
        for hook_index, (command, timeout) in enumerate(hooks_by_event[event]):
            key = f"/Users/jordan/.codex/config.toml:{CODEX_EVENT[event]}:0:{hook_index}"
            digest = _codex_trust_hash(event, command, timeout)
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

def generate(hooks_dir, claude_settings_path, codex_config_path, profiles_dir):
    bindings = read_bindings(hooks_dir)

    _write_claude(claude_settings_path, bindings)
    for path in _profile_settings(profiles_dir):
        profile = os.path.basename(os.path.dirname(path))
        _write_claude(path, {m: b for m, b in bindings.items()
                             if b.get("roots") == "all"
                             and profile not in b.get("except", [])})

    with open(codex_config_path, encoding="utf-8") as f:
        config_text = f.read()
    files.write_if_changed(codex_config_path, render_codex(config_text, bindings))


def _profile_settings(profiles_dir):
    """Every profile's own settings.json. A profile slot that is a symlink back to
    the default settings.json is skipped — writing through it would double-render
    the default root's hooks into itself."""
    found = []
    for name in sorted(os.listdir(profiles_dir)):
        path = os.path.join(profiles_dir, name, "settings.json")
        if os.path.isfile(path) and not os.path.islink(path):
            found.append(path)
    return found


def _write_claude(path, bindings):
    with open(path, encoding="utf-8") as f:
        settings = json.load(f)
    settings = render_claude(settings, bindings)
    files.write_if_changed(path, json.dumps(settings, indent=2) + "\n")
