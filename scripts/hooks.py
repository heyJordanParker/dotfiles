"""Generate hook wiring for both harnesses from each hook's BINDING declaration.

Each Python hook in packages/agents/hooks owns a module-level BINDING constant
declaring the lifecycle events and tool matchers it attaches to, and whether it
runs on both harnesses or Claude only:

    BINDING = {
        "events": {"<Event>": ["<matcher>", ...], ...},
        "harness": "all" | "claude",
        "roots": "all",            # optional
        "standalone": True,        # optional
        "additionalContextLimit": 0,  # optional, codex only
    }

Every hook without `standalone` joins its group's `combine_hooks` runner, one
process for all of them. A hook that injects large context or calls the network
or a model declares `standalone`: its answer keeps its own ceiling and its wait
holds up no quick check. On a tool event the runner is formed per tool, so one
tool call starts one runner whatever mix of matcher lists its hooks declare.
`additionalContextLimit` is codex's own per-handler field, written verbatim.

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
managed entries of packages/claude/settings.json and packages/codex-system/config.toml.

Managed = a `type: command` hook whose command invokes ~/.agents/hooks/<module>.py.
Every other entry — inline `type: prompt` gates, third-party shell glue (tmux,
herdr, superset) — is unmanaged and preserved byte-for-byte; non-hook sections
(permissions, env, model, MCP servers, plugins) are never touched.
"""

import ast
import glob
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


RUNNER = "combine_hooks"

# Events whose matcher names tools. A quick hook bound to one joins the runner of
# every tool it names.
TOOL_EVENTS = {"PreToolUse", "PostToolUse", "PostToolUseFailure", "PermissionRequest",
               "PermissionDenied"}


def _is_standalone(binding):
    return bool(binding.get("standalone") or binding.get("asyncRewake"))


def _entry(hook_dir, modules, bindings):
    """(command, modules, timeout, async_rewake, context_limit) running `modules`.

    One module runs as itself. Several run through the runner, whose ceiling is
    the sum of theirs, so no member loses the time it had alone."""
    if len(modules) == 1:
        binding = bindings[modules[0]]
        return (f"python3 {hook_dir}/{modules[0]}.py", tuple(modules), binding.get("timeout"),
                binding.get("asyncRewake"), binding.get("additionalContextLimit"))
    timeouts = [bindings[module].get("timeout") for module in modules]
    timeout = None if None in timeouts else sum(timeouts)
    return (f"python3 {hook_dir}/{RUNNER}.py {' '.join(modules)}", tuple(modules), timeout, None, None)


def _commands_by_group(bindings, hook_dir, harnesses):
    """Map (Event, matcher_key) -> [(command, modules, timeout, async_rewake, context_limit), ...]
    for hooks whose harness is in `harnesses`, in stable module order. matcher_key
    is None for a non-tool event; the other fields are None when omitted.

    Standalone hooks keep their own entry under their own matcher. Quick hooks on
    a tool event are regrouped per tool, then tools with the same members share
    one group, so the matcher each group carries is a plain tool list.
    """
    groups = {}
    quick = {}
    per_tool = {}
    for module in sorted(bindings):
        binding = bindings[module]
        if binding.get("harness", "all") not in harnesses:
            continue
        for event, matchers in binding.get("events", {}).items():
            if _is_standalone(binding):
                groups.setdefault((event, _matcher_key(matchers)), []).append(
                    _entry(hook_dir, [module], bindings))
            elif event in TOOL_EVENTS and matchers and "*" not in matchers:
                tools = per_tool.setdefault(event, {})
                for matcher in matchers:
                    for tool in matcher.split("|"):
                        tools.setdefault(tool, []).append(module)
            else:
                quick.setdefault((event, _matcher_key(matchers)), []).append(module)
    for event, tools in per_tool.items():
        by_members = {}
        for tool, modules in tools.items():
            by_members.setdefault(tuple(modules), []).append(tool)
        for modules, tool_names in by_members.items():
            quick[(event, "|".join(tool_names))] = list(modules)
    for key, modules in quick.items():
        groups.setdefault(key, []).append(_entry(hook_dir, modules, bindings))
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
                 for command, _, timeout, async_rewake, _ in commands]
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
    """Return config.toml text with the [[hooks.<Event>]] blocks regenerated
    from BINDING; every other section (mcp_servers, features, plugins,
    marketplaces, ...) stays byte-identical.
    """
    groups = _commands_by_group(bindings, CODEX_HOOK_DIR, {"all", "codex"})
    blocks = _codex_hook_blocks(_codex_groups(groups))
    return _replace_codex_hook_region(config_text, blocks)


# The events codex applies a group's matcher to (hooks/src/events/common.rs
# matcher_pattern_for_event). On the others it ignores the matcher, so it is not
# written there either.
_CODEX_MATCHER_EVENTS = {"PreToolUse", "PermissionRequest", "PostToolUse", "SessionStart",
                         "SubagentStart", "SubagentStop", "PreCompact", "PostCompact"}

# The events codex reads additionalContextLimit on (hooks/src/engine/discovery.rs).
# On the others it warns at every start and ignores it, so it is not written there.
_CODEX_CONTEXT_EVENTS = {"PreToolUse", "PostToolUse", "SessionStart", "UserPromptSubmit",
                         "SubagentStart"}

# The names one codex tool call is matched under: its own name first, then its
# aliases (core/src/tools/hook_names.rs). A hook reached through two of them in
# different groups would run twice for the one call.
_CODEX_TOOL_NAMES = (("Bash",), ("apply_patch", "Write", "Edit"), ("spawn_agent", "Agent"))


def _codex_groups(groups):
    """Event -> [(matcher or None, [(command, timeout, context_limit), ...])], in
    emission order. Raises when a BINDING names an event codex cannot fire, and
    when one codex tool call would run a hook twice."""
    groups_by_event = {}
    for (event, matcher_key), commands in groups.items():
        if event not in CODEX_EVENT:
            raise ValueError(
                "codex has no %s event; a BINDING declaring it with harness "
                "all/codex would be dropped silently. Bind it to claude." % event)
        matcher = matcher_key if event in _CODEX_MATCHER_EVENTS else None
        entries = [(command, modules, timeout, limit if event in _CODEX_CONTEXT_EVENTS else None)
                   for command, modules, timeout, _, limit in commands]
        groups_by_event.setdefault(event, []).append((matcher, entries))
    for event, event_groups in groups_by_event.items():
        if event not in TOOL_EVENTS:
            continue
        for names in _CODEX_TOOL_NAMES:
            seen = []
            for matcher, entries in event_groups:
                if matcher is not None and matcher != "*" and not set(matcher.split("|")) & set(names):
                    continue
                for _, modules, _, _ in entries:
                    seen.extend(modules)
            twice = sorted({module for module in seen if seen.count(module) > 1})
            if twice:
                raise ValueError(
                    "codex matches one %s call under %s, so %s would run twice. Give "
                    "those tools the same hooks." % (names[0], "/".join(names), ", ".join(twice)))
    return groups_by_event


def _codex_hook_blocks(groups_by_event):
    """The [[hooks.<Event>]] TOML text, one table per group. timeout and
    additionalContextLimit are omitted when the BINDING omits them.

    The table name is the Event verbatim — codex deserializes the `hooks` table
    into a struct whose fields are renamed to the PascalCase event names, with no
    deny_unknown_fields. A snake_case table is an unknown key: it parses to an
    empty struct, no handler is ever discovered, and nothing warns. Every codex
    hook was silently inert until this used the Event name.
    """
    blocks = []
    for event in sorted(groups_by_event):
        for matcher, entries in groups_by_event[event]:
            lines = [f"[[hooks.{event}]]"]
            if matcher is not None:
                lines.append(f"matcher = {json.dumps(matcher)}")
            for command, _, timeout, limit in entries:
                lines += [f"[[hooks.{event}.hooks]]", 'type = "command"', f'command = "{command}"']
                if timeout is not None:
                    lines.append(f"timeout = {timeout}")
                if limit is not None:
                    lines.append(f"additionalContextLimit = {limit}")
            blocks.append("\n".join(lines))
    return "\n\n".join(blocks)


# The managed codex region runs from the first [[hooks. block to the next
# top-level table outside hooks — everything between is generator-owned, so the
# surrounding hand-authored TOML stays byte-identical. Codex reads this file as
# its system layer, where every Hook is managed and needs no trust hash.
_CODEX_BEGIN = re.compile(r"^\[\[hooks\.", re.M)
_CODEX_END = re.compile(r"^\[(?!\[?hooks\.)", re.M)


def _replace_codex_hook_region(text, generated):
    """Splice the generated [[hooks.*]] blocks in place of the existing ones."""
    begin = _CODEX_BEGIN.search(text)
    if begin is None:
        raise ValueError("config.toml has no [[hooks.*]] region to regenerate")
    end = _CODEX_END.search(text, begin.end())
    end = len(text) if end is None else end.start()
    return text[: begin.start()] + generated + "\n\n" + text[end:]


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
