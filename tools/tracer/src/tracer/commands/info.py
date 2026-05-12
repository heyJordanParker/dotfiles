"""`trace info` — complexity structure + architectural overview.

For a file: function list with cyclomatic complexity (lizard, cached via
file_facts), language, LOC, nearest doc, and the file's per-file facts
from the file/ cache.

For a directory: aggregated per-file stats sourced from the file/ cache
(no per-call re-extraction).
"""

from __future__ import annotations

import json
from pathlib import Path

import click
import lizard

from tracer import architecture, cache, digest, file_facts, passive_context
from tracer.deps import require_dependencies
from tracer.enrich import nearest_doc
from tracer.repo_context import repo_context


def _file_info(path: Path) -> dict:
    """Per-file info; uses file_facts for cached aggregates plus a fresh
    lizard pass for per-function detail (not stored in file_facts)."""
    facts = file_facts.get(path)
    try:
        parsed = lizard.analyze_file(str(path))
        functions = [
            {
                "name": f.name,
                "cyclomatic_complexity": f.cyclomatic_complexity,
                "nloc": f.nloc,
                "params": f.parameter_count,
                "start_line": f.start_line,
                "end_line": f.end_line,
            }
            for f in parsed.function_list
        ]
    except Exception:
        functions = []

    repo_root = cache.repo_root_for(path)
    leading = digest.leading_comment(path)
    callers: list[dict] = []
    deps: list[dict] = []
    graph = architecture.load_cached(repo_root)
    if graph is not None:
        try:
            relative = str(path.resolve().relative_to(repo_root.resolve()))
        except ValueError:
            relative = str(path)
        callers = digest.top_callers(graph, relative, repo_root=repo_root)
        deps = digest.immediate_dependencies(graph, relative)

    return {
        "file": str(path),
        "language": facts.language if facts else None,
        "loc": facts.loc if facts else 0,
        "function_count": facts.function_count if facts else len(functions),
        "cyclomatic_complexity_total": facts.cyclomatic_complexity_total if facts else 0,
        "cyclomatic_complexity_max": facts.cyclomatic_complexity_max if facts else 0,
        "rank": facts.rank if facts else "unknown",
        "functions": functions,
        "nearest_doc": nearest_doc(path),
        "passive_context": passive_context.render(facts) if facts else None,
        "leading_comment": leading,
        "top_callers": callers,
        "dependencies": deps,
    }


def _dir_info(path: Path) -> dict:
    files: list[dict] = []
    for f in sorted(path.rglob("*")):
        if not f.is_file():
            continue
        if any(part.startswith(".") or part in {"node_modules", "__pycache__", "dist", "build", "vendor"} for part in f.parts):
            continue
        facts = file_facts.get(f)
        if facts is None:
            continue
        files.append(
            {
                "file": str(f.relative_to(path)),
                "abs_path": str(f),
                "loc": facts.loc,
                "cyclomatic_complexity_total": facts.cyclomatic_complexity_total,
                "function_count": facts.function_count,
                "rank": facts.rank,
                "passive_context": passive_context.render_compact(facts),
            }
        )
    return {
        "directory": str(path),
        "file_count": len(files),
        "cyclomatic_complexity_total": sum(f["cyclomatic_complexity_total"] for f in files),
        "loc_total": sum(f["loc"] for f in files),
        "files": files,
        "nearest_doc": nearest_doc(path),
    }


@click.command()
@click.argument("path", type=click.Path(exists=True))
@click.option("--json", "as_json", is_flag=True)
@click.option("--brief", is_flag=True, help="Show only top-3 most-complex functions instead of all.")
def command(path: str, as_json: bool, brief: bool) -> None:
    """Complexity structure + architectural overview of a file or directory.

    Default output is full: purpose, top callers, dependencies, lifecycle,
    and the complete function table. `--brief` truncates the function table
    to the 3 hottest functions.
    """
    full = not brief
    require_dependencies()
    p = Path(path).resolve()
    info = _file_info(p) if p.is_file() else _dir_info(p)
    info["repo_context"] = repo_context(str(p))

    if as_json:
        click.echo(json.dumps(info, indent=2))
        return

    if p.is_file():
        click.echo(f"File: {info['file']}")
        if info.get("passive_context"):
            click.echo(info["passive_context"])
        click.echo(f"Language: {info.get('language')}")
        click.echo(
            f"LOC: {info['loc']}  Functions: {info['function_count']}  "
            f"CCN total: {info['cyclomatic_complexity_total']}  "
            f"CCN max: {info['cyclomatic_complexity_max']}  Rank: {info['rank']}"
        )
        click.echo(f"Nearest doc: {info.get('nearest_doc') or '(none)'}")
        if info.get("leading_comment"):
            click.echo("")
            click.echo("Purpose (from leading comment):")
            for line in info["leading_comment"].splitlines():
                click.echo(f"  {line}")
        if info.get("top_callers"):
            click.echo("")
            click.echo("Top callers (modules depending on this file):")
            for caller in info["top_callers"]:
                if caller.get("source_line"):
                    where = f" — {caller['source_file']}:{caller['source_line']}"
                elif caller.get("source_file"):
                    where = f" — {caller['source_file']}"
                else:
                    where = ""
                summary = caller.get("summary")
                summary_suffix = f"  {summary}" if summary else ""
                click.echo(f"  {caller['label']} ({caller['kind']}){where}{summary_suffix}")
        if info.get("dependencies"):
            click.echo("")
            click.echo("Immediate dependencies (modules this file imports):")
            for dep in info["dependencies"]:
                click.echo(f"  {dep['module']}  [{dep['confidence']}]")
        click.echo("")
        # Brief default: top-3 hottest functions; --full shows the whole table.
        top_funcs = sorted(info["functions"], key=lambda f: -f["cyclomatic_complexity"])
        shown = top_funcs if full else top_funcs[:3]
        click.echo(f"Functions ({'all' if full else 'top 3 by complexity'} of {len(info['functions'])}):")
        for f in shown:
            click.echo(
                f"  {f['cyclomatic_complexity']:>3}  {f['nloc']:>4} loc  "
                f"{f['name']}({f['params']} params)  L{f['start_line']}-{f['end_line']}"
            )
        if not full and len(info["functions"]) > 3:
            click.echo(f"  … {len(info['functions']) - 3} more (use --full to see all)")
    else:
        click.echo(f"Directory: {info['directory']}")
        click.echo(
            f"Files: {info['file_count']}  LOC: {info['loc_total']}  "
            f"CCN total: {info['cyclomatic_complexity_total']}"
        )
        click.echo(f"Nearest doc: {info.get('nearest_doc') or '(none)'}")
        click.echo("")
        click.echo("Files (top 20 by complexity, with file digest):")
        for f in sorted(info["files"], key=lambda x: -x["cyclomatic_complexity_total"])[:20]:
            click.echo(
                f"  {f['cyclomatic_complexity_total']:>5}  {f['loc']:>5} loc  "
                f"{f['function_count']:>3} fn  [{f['rank']:<8}]  "
                f"{f['file']}  ({f.get('passive_context','')})"
            )
            abs_path = f.get("abs_path")
            if abs_path:
                purpose = digest.leading_comment(Path(abs_path))
                if purpose:
                    first_line = purpose.splitlines()[0].strip()
                    if first_line:
                        click.echo(f"        Purpose: {first_line}")
                try:
                    parsed = lizard.analyze_file(abs_path)
                    fns = sorted(parsed.function_list, key=lambda x: -x.cyclomatic_complexity)[:3]
                    if fns:
                        sig_line = "  ".join(
                            f"{fn.name}() L{fn.start_line}" for fn in fns
                        )
                        click.echo(f"        Top fns: {sig_line}")
                except Exception:
                    pass

    ctx = info["repo_context"]
    click.echo("")
    click.echo(
        f"repo_context: complexity_p95={ctx['complexity_p95']} "
        f"median={ctx['median_file_ccn']} files={ctx['total_files']}"
    )
