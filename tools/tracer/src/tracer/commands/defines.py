"""`trace defines <symbol>` — where a symbol is defined, via the architecture graph."""

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
    """Definitions of `symbol` from the architecture graph."""
    require_dependencies()
    graph = architecture.get()
    matches = architecture.find_symbols(graph, symbol)

    if not matches:
        click.echo(f"Symbol '{symbol}' not found in architecture graph.", err=True)
        sys.exit(2)

    if as_json:
        click.echo(
            json.dumps(
                {
                    "symbol": symbol,
                    "definitions": [
                        {
                            "node_id": m.id,
                            "label": m.label,
                            "kind": m.kind,
                            "source_file": m.source_file,
                            "source_line": m.source_line,
                        }
                        for m in matches
                    ],
                    "definition_count": len(matches),
                },
                indent=2,
            )
        )
        return

    click.echo(f"Definitions of '{symbol}' ({len(matches)}):")
    for m in matches:
        click.echo(f"  [{m.kind}] {m.label} @ {m.source_file}:{m.source_line}")
