"""`trace callers <symbol>` — what depends directly on a symbol, via the architecture graph."""

from __future__ import annotations

import json
import sys

import click

from tracer import architecture
from tracer.deps import require_dependencies


@click.command()
@click.argument("symbol")
@click.option("--json", "as_json", is_flag=True)
def command(symbol: str, as_json: bool) -> None:
    """Direct callers / importers of `symbol` from the architecture graph.

    Uses the cached `architecture/` graph — never the per-file cache. For
    transitive callers use `trace dependents`.
    """
    require_dependencies()
    graph = architecture.get()
    matches = architecture.find_symbols(graph, symbol)

    if not matches:
        click.echo(f"Symbol '{symbol}' not found in architecture graph.", err=True)
        sys.exit(2)

    output: dict[str, dict] = {}
    for match in matches:
        edges = architecture.dependents_of(graph, match.id)
        callers = []
        for edge in edges:
            source_node = graph.nodes.get(edge.source)
            if source_node is None:
                continue
            callers.append(
                {
                    "node_id": source_node.id,
                    "label": source_node.label,
                    "kind": source_node.kind,
                    "source_file": source_node.source_file,
                    "source_line": source_node.source_line,
                    "relation": edge.relation,
                    "confidence": edge.confidence,
                }
            )
        output[match.id] = {
            "symbol": match.label,
            "kind": match.kind,
            "source_file": match.source_file,
            "source_line": match.source_line,
            "callers": callers,
        }

    if as_json:
        click.echo(json.dumps(output, indent=2))
        return

    for match in matches:
        click.echo(f"\n{match.label} [{match.kind}] @ {match.source_file}:{match.source_line}")
        callers = output[match.id]["callers"]
        if not callers:
            click.echo("  (no architecture-graph callers found)")
            continue
        click.echo(f"  callers ({len(callers)}):")
        for caller in callers:
            location = f"{caller['source_file']}:{caller['source_line']}" if caller["source_file"] else "(external)"
            click.echo(
                f"    [{caller['confidence']}] {caller['label']} [{caller['kind']}] @ {location}"
            )
