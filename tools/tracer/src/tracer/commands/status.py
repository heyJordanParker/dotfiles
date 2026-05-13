"""`trace status` — working-tree dirty set with code intelligence.

Replaces the `git status --short` → `git diff --stat` → per-file `git diff`
cluster agents run before commits. One call returns every uncommitted-state
file annotated with ccn rank, downstream caller count, deploy-branch
presence, and the standard lifecycle shoulder — ordered by blast radius so
the most load-bearing dirty files surface first.
"""

from __future__ import annotations

import json
from pathlib import Path

import click

from tracer import architecture, cache, file_facts, git_activity, passive_context
from tracer.deps import require_dependencies


# Order the working-state buckets so the human-readable output groups
# semantically: brand-new code first, then renames, modifications, deletions,
# then untracked at the bottom (most likely intentional, least urgent).
_STATE_ORDER = ("added", "renamed", "modified", "deleted", "untracked")


def _graph_counts(relative: str, graph) -> dict | None:
    """Caller/dependent counts for the file's module, or None if not in graph."""
    if graph is None:
        return None
    module_id = graph.file_to_module_id.get(relative)
    if not module_id:
        return None
    return {
        "callers": len(architecture.dependents_of(graph, module_id)),
        "depended_on_by_modules": len(architecture.dependencies_of(graph, module_id)),
    }


def _entries_for_state(
    repo_root: Path,
    states: dict[str, str],
    graph,
) -> list[dict]:
    """Build one annotated entry per dirty file."""
    entries: list[dict] = []
    for relative, state in states.items():
        abs_path = repo_root / relative
        facts = file_facts.get(abs_path, repo_root=repo_root) if abs_path.exists() else None
        gc = _graph_counts(relative, graph) if facts else None
        shoulder = passive_context.render(facts, graph=gc) if facts else None
        entry = {
            "path": relative,
            "state": state,
            "shoulder": shoulder,
            "callers": (gc["callers"] if gc else 0),
            "depended_on_by_modules": (gc["depended_on_by_modules"] if gc else 0),
            "ccn_total": (facts.cyclomatic_complexity_total if facts else 0),
            "ccn_rank": (facts.rank if facts else "unknown"),
            "present_in": list(facts.present_in) if facts else [],
            "last_subject": facts.last_subject if facts else None,
            "top_author": facts.top_author if facts else None,
        }
        entries.append(entry)
    return entries


def _sort_key(entry: dict) -> tuple:
    """Sort by blast radius first, complexity second, then path.

    Files with many downstream callers are load-bearing; show first.
    Negate counts so higher values sort earlier under default ascending sort.
    """
    state_rank = _STATE_ORDER.index(entry["state"]) if entry["state"] in _STATE_ORDER else len(_STATE_ORDER)
    return (
        -entry["callers"],
        -entry["ccn_total"],
        state_rank,
        entry["path"],
    )


@click.command()
@click.option("--json", "as_json", is_flag=True, help="Machine-parseable JSON output")
@click.option(
    "--state",
    "state_filter",
    type=click.Choice(_STATE_ORDER, case_sensitive=False),
    default=None,
    help="Restrict to a single working state",
)
def command(as_json: bool, state_filter: str | None) -> None:
    """Repo-wide dirty file set with code intelligence."""
    require_dependencies()
    repo_root = cache.repo_root_for(".")
    states = git_activity._working_tree_state(repo_root)
    if state_filter:
        states = {p: s for p, s in states.items() if s == state_filter}

    graph = architecture.load_cached(repo_root)
    entries = _entries_for_state(repo_root, states, graph)
    entries.sort(key=_sort_key)

    if as_json:
        click.echo(
            json.dumps(
                {
                    "repo_root": str(repo_root),
                    "count": len(entries),
                    "entries": entries,
                },
                indent=2,
            )
        )
        return

    if not entries:
        click.echo("(working tree clean)")
        return

    click.echo(f"{len(entries)} files with uncommitted state:")
    click.echo("")
    current_state: str | None = None
    for entry in entries:
        if entry["state"] != current_state:
            current_state = entry["state"]
            click.echo(f"## {current_state}")
        path = entry["path"]
        callers = entry["callers"]
        ccn = entry["ccn_total"]
        rank = entry["ccn_rank"]
        shoulder = entry["shoulder"] or ""
        suffix = f"  (callers={callers}, ccn={ccn} {rank})"
        click.echo(f"  {path}{suffix}")
        if shoulder:
            click.echo(f"    {shoulder}")
