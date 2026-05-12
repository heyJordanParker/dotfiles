"""`trace cache clear|stats` — manage the on-disk cache."""

from __future__ import annotations

import json

import click

from tracer import cache as cache_module


@click.group()
def command() -> None:
    """Manage the .tracer-cache/ disk cache."""


@command.command()
@click.argument("path", type=click.Path(exists=True), default=".")
def build(path: str) -> None:
    """Prebuild the cache for a repo so the first agent query is fast.

    Discovers all source files via `git ls-files`, populates per-file
    facts in parallel, then builds the architecture graph. Idempotent —
    re-running on a warm cache does only the cheap mtime fast-path
    validation. Useful before running an agent that will issue many
    `trace` queries.
    """
    from time import perf_counter

    from tracer import architecture

    start = perf_counter()
    graph = architecture.get(repo_root=__import__("pathlib").Path(path).resolve())
    elapsed = perf_counter() - start
    rows = cache_module.stats()
    click.echo(f"Built in {elapsed:.2f}s")
    click.echo(f"Architecture graph: {len(graph.nodes)} nodes, {len(graph.edges)} edges")
    click.echo("")
    for row in rows:
        size_kb = row.total_bytes / 1024
        click.echo(
            f"  {row.namespace:<14}  {row.entry_count:>5} entries  {size_kb:>8.1f} KB"
        )


@command.command()
@click.option(
    "--namespace",
    type=click.Choice([cache_module.NAMESPACE_FILE, cache_module.NAMESPACE_ARCHITECTURE]),
    default=None,
    help="Limit clear to one namespace; default clears both.",
)
@click.option("--all", "clear_all", is_flag=True, help="Remove .tracer-cache/ entirely.")
def clear(namespace: str | None, clear_all: bool) -> None:
    """Delete cache entries."""
    if clear_all:
        removed = cache_module.clear_all()
        click.echo(f"Removed {removed} cache entries (entire .tracer-cache/).")
        return
    removed = cache_module.clear(namespace)
    target = namespace or "all namespaces"
    click.echo(f"Removed {removed} cache entries from {target}.")


@command.command()
@click.option("--json", "as_json", is_flag=True, help="Machine-parseable output.")
def stats(as_json: bool) -> None:
    """Show cache size and entry count per namespace."""
    rows = cache_module.stats()
    if as_json:
        click.echo(
            json.dumps(
                {row.namespace: {"entries": row.entry_count, "bytes": row.total_bytes} for row in rows},
                indent=2,
            )
        )
        return

    for row in rows:
        size_kb = row.total_bytes / 1024
        click.echo(
            f"  {row.namespace:<14}  {row.entry_count:>5} entries  {size_kb:>8.1f} KB"
        )
