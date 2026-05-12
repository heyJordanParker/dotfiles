"""`trace history` — git log + blame summary for a file."""

from __future__ import annotations

import json
import subprocess
from collections import Counter
from datetime import datetime, timedelta, timezone
from pathlib import Path

import click

from tracer.deps import require_dependencies


def _recent_commits(file: Path, n: int = 10) -> list[dict]:
    result = subprocess.run(
        ["git", "log", f"-{n}", "--pretty=format:%h|%an|%ad|%s", "--date=short", "--", str(file)],
        cwd=file.parent,
        capture_output=True,
        text=True,
        check=False,
        timeout=10,
    )
    commits: list[dict] = []
    for line in result.stdout.splitlines():
        if "|" in line:
            sha, author, date, subject = line.split("|", 3)
            commits.append({"sha": sha, "author": author, "date": date, "subject": subject})
    return commits


def _commit_count_30d(file: Path) -> int:
    since = (datetime.now(timezone.utc) - timedelta(days=30)).strftime("%Y-%m-%d")
    result = subprocess.run(
        ["git", "log", f"--since={since}", "--oneline", "--", str(file)],
        cwd=file.parent,
        capture_output=True,
        text=True,
        check=False,
        timeout=10,
    )
    return len([l for l in result.stdout.splitlines() if l.strip()])


def _blame_summary(file: Path) -> list[tuple[str, int]]:
    result = subprocess.run(
        ["git", "blame", "--line-porcelain", str(file)],
        cwd=file.parent,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    counts: Counter[str] = Counter()
    for line in result.stdout.splitlines():
        if line.startswith("author "):
            counts[line[len("author "):]] += 1
    return counts.most_common(5)


@click.command()
@click.argument("file", type=click.Path(exists=True, dir_okay=False))
@click.option("--json", "as_json", is_flag=True)
def command(file: str, as_json: bool) -> None:
    """Git log + blame summary for a file."""
    require_dependencies()
    p = Path(file).resolve()
    commits = _recent_commits(p)
    count_30d = _commit_count_30d(p)
    blame = _blame_summary(p)

    if as_json:
        click.echo(
            json.dumps(
                {
                    "file": str(p),
                    "recent_commits": commits,
                    "commits_30d": count_30d,
                    "top_authors": [{"author": a, "lines": n} for a, n in blame],
                },
                indent=2,
            )
        )
        return

    click.echo(f"File: {p}")
    click.echo(f"Commits in last 30 days: {count_30d}")
    click.echo("")
    click.echo("Recent commits:")
    for c in commits:
        click.echo(f"  {c['sha']} {c['date']} {c['author']}: {c['subject']}")
    click.echo("")
    click.echo("Top blame authors (lines of current file):")
    for author, lines in blame:
        click.echo(f"  {lines:>5}  {author}")
