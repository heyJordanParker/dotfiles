"""`trace survey` — repo-wide complexity distribution.

Wraps scc for fast language + LOC + complexity sweep. Adds top-N most-complex
files for orientation.
"""

from __future__ import annotations

import json
import statistics
import subprocess
import sys
from pathlib import Path

import click

from tracer.deps import require_dependencies


def _scc_by_file(path: Path) -> list[dict]:
    result = subprocess.run(
        ["scc", "--format", "json", "--by-file", str(path)],
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
    )
    if result.returncode != 0:
        click.echo(f"Error: scc failed: {result.stderr.strip()}", err=True)
        sys.exit(1)
    return json.loads(result.stdout)


def _summary(by_file: list[dict]) -> dict:
    languages: dict[str, dict] = {}
    files: list[dict] = []
    for lang_block in by_file:
        lang = lang_block["Name"]
        languages[lang] = {
            "files": lang_block["Count"],
            "loc": lang_block["Code"],
            "complexity": lang_block["Complexity"],
        }
        for f in lang_block.get("Files", []):
            files.append(
                {
                    "path": f["Location"],
                    "language": f["Language"],
                    "loc": f["Code"],
                    "complexity": f["Complexity"],
                }
            )

    complexities = [f["complexity"] for f in files]
    if not complexities:
        return {"languages": languages, "files": [], "distribution": {}, "top_complex": []}

    sorted_c = sorted(complexities)
    n = len(sorted_c)

    def pct(p: float) -> int:
        idx = max(0, int(n * p) - 1)
        return sorted_c[idx]

    distribution = {
        "median": int(statistics.median(complexities)),
        "p75": pct(0.75),
        "p90": pct(0.90),
        "p95": pct(0.95),
        "max": sorted_c[-1],
    }
    top = sorted(files, key=lambda x: x["complexity"], reverse=True)[:10]
    return {
        "total_files": n,
        "languages": languages,
        "distribution": distribution,
        "top_complex": top,
    }


@click.command()
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option("--json", "as_json", is_flag=True)
def command(path: str, as_json: bool) -> None:
    """Repo-wide language + LOC + complexity distribution."""
    require_dependencies()
    by_file = _scc_by_file(Path(path).resolve())
    summary = _summary(by_file)

    if as_json:
        click.echo(json.dumps(summary, indent=2))
        return

    click.echo(f"Files: {summary.get('total_files', 0)}")
    click.echo("")
    click.echo("Languages:")
    for lang, stats in sorted(summary["languages"].items(), key=lambda kv: -kv[1]["loc"])[:15]:
        click.echo(f"  {lang:<20} files={stats['files']:<6} loc={stats['loc']:<8} complexity={stats['complexity']}")
    click.echo("")
    dist = summary["distribution"]
    if dist:
        click.echo("Complexity distribution (per file):")
        click.echo(f"  median={dist['median']}  p75={dist['p75']}  p90={dist['p90']}  p95={dist['p95']}  max={dist['max']}")
    click.echo("")
    click.echo("Top 10 most-complex files:")
    for f in summary["top_complex"]:
        click.echo(f"  {f['complexity']:>5}  {f['loc']:>5} loc  {f['path']}")
