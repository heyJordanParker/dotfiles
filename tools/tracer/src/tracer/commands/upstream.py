"""`trace upstream` — what a symbol or path depends on.

Default mode: positional `<symbol>` → transitive dependencies of that symbol
(BFS over forward edges).

Path mode: `--path <path>` → top-N nodes in that path ranked by outgoing
edge count (the highest-coupling code — the symbols that depend on the
most other things).

Both modes return ranked lists from the architecture graph.
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path

import click

from tracer import architecture
from tracer.deps import require_dependencies


@click.command()
@click.argument("symbol", required=False)
@click.option(
    "--path",
    "path",
    type=click.Path(exists=True),
    default=None,
    help="Switch to path mode: rank top-N most-coupling symbols in this path.",
)
@click.option("--depth", default=3, show_default=True, help="Transitive BFS depth.")
@click.option("--limit", default=10, show_default=True, help="Top-N limit in --path mode.")
@click.option("--json", "as_json", is_flag=True)
def command(
    symbol: str | None,
    path: str | None,
    depth: int,
    limit: int,
    as_json: bool,
) -> None:
    """What a symbol (default) or a path (`--path`) depends on.

    Symbol mode: transitive dependencies of one symbol.
    Path mode: top-N highest-coupling symbols across the path's graph.
    """
    require_dependencies()

    if path is not None:
        _path_mode(path, depth, limit, as_json)
        return

    if not symbol:
        click.echo("Error: pass a SYMBOL or --path <path>.", err=True)
        sys.exit(2)
    _symbol_mode(symbol, depth, as_json)


def _symbol_mode(symbol: str, depth: int, as_json: bool) -> None:
    graph = architecture.get()
    matches = architecture.find_symbols(graph, symbol)
    if not matches:
        click.echo(f"Symbol '{symbol}' not found in architecture graph.", err=True)
        sys.exit(2)

    output: dict[str, dict] = {}
    for match in matches:
        chain = architecture.transitive_dependencies(graph, match.id, max_depth=depth)
        output[match.id] = {
            "symbol": match.label,
            "kind": match.kind,
            "source_file": match.source_file,
            "source_line": match.source_line,
            "dependencies": [
                {
                    "node_id": node.id,
                    "label": node.label,
                    "kind": node.kind,
                    "source_file": node.source_file,
                    "depth": d,
                }
                for node, d in chain
            ],
        }

    if as_json:
        click.echo(json.dumps(output, indent=2))
        return

    for match in matches:
        chain = architecture.transitive_dependencies(graph, match.id, max_depth=depth)
        click.echo(
            f"\n{match.label} [{match.kind}] @ {match.source_file}:{match.source_line}"
        )
        click.echo(f"  depends on (depth ≤ {depth}):")
        if not chain:
            click.echo("    (no architecture-graph dependencies found)")
            continue
        for node, d in chain:
            location = (
                f"{node.source_file}:{node.source_line}"
                if node.source_file
                else "(external)"
            )
            click.echo(f"    [d={d}] {node.label} [{node.kind}] @ {location}")


def _path_mode(path: str, depth: int, limit: int, as_json: bool) -> None:
    """Top-N nodes ranked by outgoing-edge count (highest coupling)."""
    graph = architecture.get(repo_root=Path(path).resolve())
    outgoing = Counter(edge.source for edge in graph.edges)

    ranked = [
        (node_id, count)
        for node_id, count in outgoing.most_common()
        if not node_id.startswith("module::external::")
    ]
    top_ids = [node_id for node_id, _ in ranked[: limit * 3]]

    transitive_counts: dict[str, int] = {}
    for node_id in top_ids:
        transitive_counts[node_id] = len(
            architecture.transitive_dependencies(graph, node_id, max_depth=depth)
        )

    re_ranked = sorted(
        top_ids,
        key=lambda nid: (transitive_counts.get(nid, 0), outgoing.get(nid, 0)),
        reverse=True,
    )[:limit]

    rows = []
    for node_id in re_ranked:
        node = graph.nodes.get(node_id)
        if node is None:
            continue
        rows.append(
            {
                "rank": len(rows) + 1,
                "node_id": node.id,
                "label": node.label,
                "kind": node.kind,
                "source_file": node.source_file,
                "source_line": node.source_line,
                "direct_dependencies": outgoing.get(node_id, 0),
                "transitive_dependencies": transitive_counts.get(node_id, 0),
            }
        )

    if as_json:
        click.echo(
            json.dumps(
                {"path": path, "limit": limit, "depth": depth, "mode": "upstream", "results": rows},
                indent=2,
            )
        )
        return

    if not rows:
        click.echo(
            "(no nodes in the architecture graph — cache may be empty; run `trace cache build`)"
        )
        return

    click.echo(
        f"Top {len(rows)} highest-coupling nodes in {path} (transitive depth ≤ {depth}):"
    )
    click.echo(
        f"  {'#':<3} {'direct':>6}  {'transitive':>10}  {'kind':<10}  symbol @ source"
    )
    for row in rows:
        location = (
            f"{row['source_file']}:{row['source_line']}"
            if row["source_file"]
            else "(no source)"
        )
        click.echo(
            f"  {row['rank']:<3} {row['direct_dependencies']:>6}  "
            f"{row['transitive_dependencies']:>10}  {row['kind']:<10}  "
            f"{row['label']} @ {location}"
        )
