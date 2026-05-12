"""`trace context` — tracer-only enrichment for hook injection.

Surfaces what Claude Code's native Read/Glob/Grep do NOT already provide:
- Passive-context shoulder: lifecycle (commits, age, deploy-branch presence,
  rename history), complexity rank
- Architecture-graph caller/dependent counts (file mode only)

What this command intentionally OMITS — the native Read/Glob path already
loads it via Claude Code's harness-level Claude.md walk:
- `CLAUDE.md` / `.claude/CLAUDE.md` / `CLAUDE.local.md` ancestors
- `.claude/rules/` matches (unconditional + path-conditional)
- `@include` references

Three modes:
- File mode (default): `trace context <path>` for a concrete file. One
  passive_context line, plus graph caller/dependent counts.
- Directory mode (`--directory`): silent no-op. Per-file lifecycle requires
  a specific file; aggregate summaries duplicate `trace list`/`trace info`.
- Glob mode (`--glob <pattern>`): resolves matched files under `<path>`
  (default cwd) and emits one passive_context line per match, capped at
  the top 30 by complexity. Used by the PreToolUse hook on Glob to
  give the agent per-file triage before native Glob's bare-paths output.
"""

from __future__ import annotations

from pathlib import Path

import click

from tracer import architecture, cache, file_facts, passive_context
from tracer.deps import require_dependencies

# Skip dirs that Glob/find typically excludes — keep enrichment cheap and
# focused on real source files.
SKIP_DIRS = {".git", "node_modules", ".venv", "venv", "__pycache__", "dist",
             "build", ".next", "vendor"}

# Cap on matched files to enrich. Larger result sets (~thousands) would blow
# context for marginal benefit — agent gets the top-N by complexity ranked.
GLOB_MATCH_CAP = 30


@click.command()
@click.argument("path", type=click.Path(), required=False)
@click.option("--directory", "force_directory", is_flag=True,
              help="Force directory mode (no per-file enrichment available).")
@click.option("--glob", "glob_pattern", default=None,
              help="Glob mode: enrich each file matching this pattern under <path>.")
def command(path: str | None, force_directory: bool, glob_pattern: str | None) -> None:
    """Print tracer-only enrichment for a path. No file body, no Claude.md."""
    require_dependencies()

    if glob_pattern:
        _glob_mode(glob_pattern, path or ".")
        return

    if not path:
        return

    p = Path(path).resolve()
    if not p.exists():
        return

    if force_directory or p.is_dir():
        return

    facts = file_facts.get(p)
    if not facts:
        return

    repo_root = cache.repo_root_for(p)
    graph_counts = _graph_counts(p, repo_root)
    context_line = passive_context.render(facts, graph=graph_counts)
    if context_line:
        click.echo(context_line)


def _glob_mode(pattern: str, base: str) -> None:
    """Resolve pattern under base, enrich top-N matches by complexity."""
    base_path = Path(base).resolve()
    if not base_path.exists() or not base_path.is_dir():
        return

    try:
        # Path.glob supports *, **, ?, [...]. For the common Glob patterns
        # (`**/*.tsx`, `src/**/*.py`, `*.md`) this matches Claude Code's
        # native Glob output. Exotic patterns may drift — acceptable risk.
        matches = list(base_path.glob(pattern))
    except (OSError, ValueError):
        return

    enriched: list[tuple[str, str, int, str]] = []  # (rel_path, line, ccn_total, rank)
    for match in matches:
        if not match.is_file():
            continue
        if any(part.startswith(".") or part in SKIP_DIRS for part in match.parts):
            continue
        facts = file_facts.get(match)
        if not facts:
            continue
        try:
            rel = str(match.relative_to(base_path))
        except ValueError:
            rel = str(match)
        compact = passive_context.render_compact(facts)
        line = f"{rel}  [ccn={facts.cyclomatic_complexity_total} {facts.rank}] {compact}"
        enriched.append((rel, line, facts.cyclomatic_complexity_total, facts.rank))

    if not enriched:
        return

    # Sort by complexity descending, cap at GLOB_MATCH_CAP. Agent sees the
    # hottest matches first — the ones most likely to need careful reading.
    enriched.sort(key=lambda x: -x[2])
    total = len(enriched)
    shown = enriched[:GLOB_MATCH_CAP]

    header = f"matched {total} files under {base_path}"
    if total > GLOB_MATCH_CAP:
        header += f" (showing top {GLOB_MATCH_CAP} by complexity)"
    click.echo(header)
    for _, line, _, _ in shown:
        click.echo(line)


def _graph_counts(file_path: Path, repo_root: Path) -> dict | None:
    """Architecture-graph caller/dependent counts (mirrors read.py helper)."""
    try:
        graph = architecture.load_cached(repo_root)
    except Exception:
        return None
    if graph is None:
        return None
    try:
        relative = str(file_path.relative_to(repo_root.resolve()))
    except ValueError:
        return None
    module_id = graph.file_to_module_id.get(relative)
    if not module_id:
        return None
    callers = len(architecture.dependents_of(graph, module_id))
    deps = len(architecture.dependencies_of(graph, module_id))
    return {"callers": callers, "depended_on_by_modules": deps}
