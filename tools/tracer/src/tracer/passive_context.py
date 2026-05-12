"""Render passive architectural context for any file the agent reads.

One short line per file. Format:

    [git: <state> · age: <age> · ccn: <total> <rank>]

`state` collapses the lifecycle signals (working_state, rename_from,
commit_count) to a single label so the agent doesn't have to interpret
multiple fields. The whole line is under 30 tokens for a typical file —
cheap enough to attach to every read or directory listing.
"""

from __future__ import annotations

from datetime import date, datetime
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from tracer.file_facts import FileFacts


def render(facts: "FileFacts", graph: dict | None = None) -> str:
    """Single-line passive context shoulder. Pure function — no I/O.

    Optional `graph` dict carries architecture-layer counts:
        {"callers": int, "depended_on_by_modules": int}
    Surfaced inline so the agent doesn't need a separate `trace callers` call
    to assess how load-bearing the file is.
    """
    state = _state_label(facts)
    age = _age(facts)
    complexity = f"ccn: {facts.cyclomatic_complexity_total} {facts.rank}"
    parts = [f"git: {state}"]
    if age:
        parts.append(f"age: {age}")
    if facts.present_in:
        parts.append(f"presence: {', '.join(facts.present_in)}")
    else:
        parts.append("presence: local-only")
    if graph:
        graph_part = f"callers: {graph['callers']} · dependents: {graph['depended_on_by_modules']}"
        parts.append(graph_part)
    parts.append(complexity)
    return "[" + " · ".join(parts) + "]"


def render_compact(facts: "FileFacts") -> str:
    """Tighter form for callers that already display complexity elsewhere
    (info-on-dir, tree). Drops `ccn` to avoid duplication.

    Format: `<state> · <age>`.
    """
    state = _state_label(facts)
    age = _age(facts) or "—"
    return f"{state} · {age}"


def _state_label(facts: "FileFacts") -> str:
    ws = facts.working_state
    if ws == "untracked":
        return "untracked"
    if ws == "added":
        return "added (uncommitted)"
    if ws == "renamed":
        return "renamed (uncommitted)"
    if ws == "modified":
        # Committed history exists; show count alongside the dirt flag so the
        # agent knows whether this is a small tweak to mature code or churn
        # on a young file.
        if facts.commit_count <= 1:
            return "modified (new file)"
        return f"modified ({facts.commit_count} commits)"
    if facts.rename_from:
        return f"renamed-from {facts.rename_from}"
    if facts.commit_count == 0:
        return "no-history"
    if facts.commit_count == 1:
        return "new (1 commit)"
    return f"{facts.commit_count} commits"


def _age(facts: "FileFacts") -> str | None:
    """Human-readable age from last_modified. Returns None when missing."""
    if not facts.last_modified:
        return None
    try:
        last = datetime.strptime(facts.last_modified, "%Y-%m-%d").date()
    except ValueError:
        return None
    days = (date.today() - last).days
    if days < 0:
        return None
    if days == 0:
        return "today"
    if days == 1:
        return "1d"
    if days < 14:
        return f"{days}d"
    if days < 60:
        return f"{days // 7}w"
    if days < 365:
        return f"{days // 30}mo"
    years = days // 365
    months = (days % 365) // 30
    if months:
        return f"{years}y{months}mo"
    return f"{years}y"
