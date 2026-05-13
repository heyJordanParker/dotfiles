"""`trace diff` — files (or symbols) changed between HEAD and a base ref.

Collapses the recurring scope-a-branch cluster (merge-base + name-only diff
+ per-file follow-ups + log) into a single enriched call. Each entry carries
the same code intelligence tracer attaches to file reads: complexity rank,
direct-dependent count, deploy-branch presence, lifecycle shoulder.

Default base ref is `origin/development`. Override with `--base`. Default
granularity is per-file (most load-bearing first); `--symbols` switches to
per-symbol output with added / removed / changed states resolved from the
existing tree-sitter extractor against the base blob.

Reuses cached pipelines:
  * `file_facts.get` for per-file complexity, rank, presence, lifecycle
  * `architecture.get` for the cross-file graph (already cached at the
    repo's current fingerprint; cold build only runs from cached per-file
    facts, not raw extraction)
  * `extraction.extract` for symbol-level diffs against base blobs
"""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import click

from tracer import architecture, cache, file_facts, passive_context
from tracer.deps import require_dependencies
from tracer.extraction import extract


DEFAULT_BASE = "origin/development"

# git diff --name-status status codes we surface. Renames carry a similarity
# percentage (R100, R087, …); we collapse those to "renamed" for display.
_STATUS_LABELS = {
    "A": "added",
    "M": "modified",
    "D": "deleted",
    "R": "renamed",
    "C": "copied",
    "T": "type-changed",
}


@dataclass(frozen=True)
class _Change:
    """One entry from `git diff --name-status -M base..HEAD`."""

    status: str          # one of "added" | "modified" | "deleted" | "renamed" | "copied" | "type-changed"
    path: str            # post-image path (or pre-image path for deletes)
    rename_from: str | None  # populated only for renames/copies


@click.command()
@click.option(
    "--base",
    "base",
    default=DEFAULT_BASE,
    show_default=True,
    help="Base ref to diff against (e.g. origin/development, main, a SHA).",
)
@click.option(
    "--symbols",
    "symbol_mode",
    is_flag=True,
    help="Switch to per-symbol granularity (added / removed / changed per export).",
)
@click.option("--json", "as_json", is_flag=True)
def command(base: str, symbol_mode: bool, as_json: bool) -> None:
    """Show files (or symbols) changed between HEAD and a base ref.

    File mode (default): one entry per changed file, ranked with the most
    load-bearing change first (direct dependents, then ccn).

    Symbol mode (`--symbols`): one entry per added / removed / changed
    module-level symbol across the diff.
    """
    require_dependencies()

    repo_root = cache.repo_root_for(".")
    _verify_base_ref(repo_root, base)
    merge_base = _merge_base(repo_root, base)
    changes = _name_status(repo_root, merge_base)

    if symbol_mode:
        _emit_symbol_mode(repo_root, base, merge_base, changes, as_json)
    else:
        _emit_file_mode(repo_root, base, merge_base, changes, as_json)


def _verify_base_ref(repo_root: Path, base: str) -> None:
    """Hard-fail with a clear stderr message + exit 2 when the base ref is
    not resolvable. Mirrors the `downstream`/`callers` error pattern."""
    try:
        subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", f"{base}^{{commit}}"],
            cwd=repo_root,
            capture_output=True,
            check=True,
            timeout=5,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        click.echo(
            f"Error: base ref '{base}' not found in this repository. "
            f"Pass --base <ref> with a ref that exists "
            f"(e.g. main, origin/main, a SHA).",
            err=True,
        )
        sys.exit(2)


def _merge_base(repo_root: Path, base: str) -> str:
    """Resolve the merge-base SHA between HEAD and `base`. Exits 2 when
    HEAD and `base` share no common ancestor (disjoint histories)."""
    try:
        result = subprocess.run(
            ["git", "merge-base", base, "HEAD"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=10,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        click.echo(
            f"Error: no common ancestor between HEAD and '{base}'.",
            err=True,
        )
        sys.exit(2)
    return result.stdout.strip()


def _name_status(repo_root: Path, merge_base: str) -> list[_Change]:
    """`git diff --name-status -M <merge_base>..HEAD` parsed into _Change rows."""
    try:
        result = subprocess.run(
            ["git", "diff", "--name-status", "-M", f"{merge_base}..HEAD"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired) as exc:
        click.echo(f"Error: git diff failed: {exc}", err=True)
        sys.exit(2)

    changes: list[_Change] = []
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        tokens = line.split("\t")
        raw_status = tokens[0]
        # R100, C075, etc. → letter only
        kind = raw_status[0]
        label = _STATUS_LABELS.get(kind, kind.lower())
        if kind in ("R", "C") and len(tokens) >= 3:
            changes.append(_Change(status=label, path=tokens[2], rename_from=tokens[1]))
        elif len(tokens) >= 2:
            changes.append(_Change(status=label, path=tokens[1], rename_from=None))
    return changes


# --- File-granularity mode ----------------------------------------------------

def _emit_file_mode(
    repo_root: Path,
    base: str,
    merge_base: str,
    changes: list[_Change],
    as_json: bool,
) -> None:
    graph = architecture.get(repo_root=repo_root)
    rows = [_file_row(repo_root, graph, change) for change in changes]
    rows.sort(key=_load_bearing_key, reverse=True)

    payload = {
        "base": base,
        "merge_base": merge_base,
        "granularity": "file",
        "file_count": len(rows),
        "files": rows,
    }

    if as_json:
        click.echo(json.dumps(payload, indent=2))
        return

    click.echo(
        f"Diff base={base}  merge_base={merge_base[:12]}  files={len(rows)}"
    )
    if not rows:
        click.echo("(no files differ between HEAD and base)")
        return
    click.echo("")
    click.echo(
        f"  {'#':<3} {'status':<10} {'direct':>6}  {'ccn':>5}  {'rank':<8}  path"
    )
    for index, row in enumerate(rows, start=1):
        click.echo(
            f"  {index:<3} {row['status']:<10} {row['direct_dependents']:>6}  "
            f"{row['cyclomatic_complexity_total']:>5}  {row['rank']:<8}  {row['path']}"
        )
        if row.get("rename_from"):
            click.echo(f"      renamed from: {row['rename_from']}")
        if row.get("passive_context"):
            click.echo(f"      {row['passive_context']}")


def _file_row(
    repo_root: Path,
    graph: architecture.Graph,
    change: _Change,
) -> dict:
    """Enriched per-file dict. Deleted files have no current FileFacts; we
    still include the entry with the load-bearing fields it carried on the
    base side, via the architecture graph's lingering node."""
    abs_path = (repo_root / change.path).resolve()
    facts = file_facts.get(abs_path) if abs_path.is_file() else None

    direct_dependents = _direct_dependent_count(graph, change.path)
    # Renames keep their pre-image's dependent count too — callers haven't
    # been updated for the new path yet.
    if change.rename_from:
        direct_dependents = max(
            direct_dependents,
            _direct_dependent_count(graph, change.rename_from),
        )

    return {
        "path": change.path,
        "status": change.status,
        "rename_from": change.rename_from,
        "language": facts.language if facts else None,
        "cyclomatic_complexity_total": facts.cyclomatic_complexity_total if facts else 0,
        "rank": facts.rank if facts else "absent",
        "loc": facts.loc if facts else 0,
        "direct_dependents": direct_dependents,
        "present_in": list(facts.present_in) if facts else [],
        "passive_context": passive_context.render(facts) if facts else None,
    }


def _direct_dependent_count(graph: architecture.Graph, relative_path: str) -> int:
    """Count edges in the architecture graph that target the module owning
    `relative_path`. Returns 0 when the file isn't represented (unsupported
    language, deleted, etc.) — load-bearing-ness is unknown, not zero, but
    the sort still produces a sensible ordering by falling back to ccn."""
    module_id = graph.file_to_module_id.get(relative_path)
    if module_id is None:
        return 0
    return sum(1 for edge in graph.edges if edge.target == module_id)


def _load_bearing_key(row: dict) -> tuple[int, int]:
    """Most-load-bearing first: dependents primary, complexity secondary."""
    return (row["direct_dependents"], row["cyclomatic_complexity_total"])


# --- Symbol-granularity mode --------------------------------------------------

def _emit_symbol_mode(
    repo_root: Path,
    base: str,
    merge_base: str,
    changes: list[_Change],
    as_json: bool,
) -> None:
    graph = architecture.get(repo_root=repo_root)
    rows: list[dict] = []
    for change in changes:
        rows.extend(_symbol_rows_for_change(repo_root, graph, merge_base, change))

    rows.sort(key=_symbol_load_bearing_key, reverse=True)

    payload = {
        "base": base,
        "merge_base": merge_base,
        "granularity": "symbol",
        "symbol_count": len(rows),
        "symbols": rows,
    }

    if as_json:
        click.echo(json.dumps(payload, indent=2))
        return

    click.echo(
        f"Diff base={base}  merge_base={merge_base[:12]}  symbols={len(rows)}"
    )
    if not rows:
        click.echo("(no symbol-level changes detected in supported languages)")
        return
    click.echo("")
    click.echo(
        f"  {'state':<8} {'direct':>6}  {'kind':<10}  symbol @ source"
    )
    for row in rows:
        location = (
            f"{row['source_file']}:{row['line']}" if row.get("line") else row["source_file"]
        )
        click.echo(
            f"  {row['state']:<8} {row['direct_dependents']:>6}  "
            f"{row['kind']:<10}  {row['name']} @ {location}"
        )


def _symbol_rows_for_change(
    repo_root: Path,
    graph: architecture.Graph,
    merge_base: str,
    change: _Change,
) -> list[dict]:
    """For one changed file, diff its module-level symbols between base and HEAD.

    Reuses the cached extractor: `file_facts.get(...).extraction.exports` on
    the HEAD side, one fresh `extract()` against the base blob for the
    pre-image. Renames carry the pre-image lookup on the rename_from path.
    """
    head_path = (repo_root / change.path).resolve()
    head_exports = _head_exports(head_path)
    pre_path = change.rename_from or change.path
    base_exports = _base_exports(repo_root, merge_base, pre_path)

    head_by_name = {(e["name"], e["kind"]): e for e in head_exports}
    base_by_name = {(e["name"], e["kind"]): e for e in base_exports}

    rows: list[dict] = []
    for key, head_export in head_by_name.items():
        base_export = base_by_name.get(key)
        if base_export is None:
            state = "added"
        elif base_export["line"] != head_export["line"]:
            state = "changed"
        else:
            state = "unchanged"
        if state == "unchanged":
            continue
        rows.append(
            _symbol_row(graph, change.path, head_export, state)
        )
    for key, base_export in base_by_name.items():
        if key in head_by_name:
            continue
        rows.append(
            _symbol_row(graph, pre_path, base_export, "removed")
        )
    return rows


def _head_exports(path: Path) -> list[dict]:
    if not path.is_file():
        return []
    facts = file_facts.get(path)
    if facts is None or facts.extraction is None:
        return []
    return [
        {"name": e.name, "kind": e.kind, "line": e.line}
        for e in facts.extraction.exports
    ]


def _base_exports(repo_root: Path, merge_base: str, relative_path: str) -> list[dict]:
    """Extract module-level symbols from the base blob via `git show`.

    Returns [] when the file didn't exist at the merge base (additions) or
    when its extension isn't supported by any extractor.
    """
    try:
        result = subprocess.run(
            ["git", "show", f"{merge_base}:{relative_path}"],
            cwd=repo_root,
            capture_output=True,
            check=True,
            timeout=10,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return []
    extraction = extract(result.stdout, relative_path)
    if extraction is None:
        return []
    return [
        {"name": e.name, "kind": e.kind, "line": e.line}
        for e in extraction.exports
    ]


def _symbol_row(
    graph: architecture.Graph,
    relative_path: str,
    export: dict,
    state: str,
) -> dict:
    """Per-symbol row enriched with the architecture-graph dependent count."""
    node_id = f"{relative_path}::{export['name']}"
    node = graph.nodes.get(node_id)
    direct_dependents = 0
    if node is not None:
        direct_dependents = len(architecture.dependents_of(graph, node.id))
    return {
        "state": state,
        "name": export["name"],
        "kind": export["kind"],
        "source_file": relative_path,
        "line": export["line"],
        "direct_dependents": direct_dependents,
    }


def _symbol_load_bearing_key(row: dict) -> tuple[int, int]:
    """Removed > added > changed when tied on dependents, since removed
    symbols are the most likely to be downstream-breaking."""
    state_weight = {"removed": 2, "added": 1, "changed": 0}.get(row["state"], 0)
    return (row["direct_dependents"], state_weight)
