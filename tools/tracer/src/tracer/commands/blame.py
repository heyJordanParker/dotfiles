"""`trace blame` — scoped, symbol-aware blame.

Returns blame information for a file, scoped to either the whole file,
an explicit line range (`--lines L1:L2`), or a named symbol resolved to
its line range via lizard. Consecutive lines sharing the same commit
are collapsed into one region. Each region carries the commit subject
so the caller never needs a follow-up `git show` to learn the "why".
"""

from __future__ import annotations

import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import click
import lizard

from tracer import cache, file_facts
from tracer.deps import require_dependencies


def _parse_lines(spec: str) -> tuple[int, int]:
    """Parse 'L1:L2' into (start, end). Inclusive on both ends."""
    if ":" not in spec:
        raise click.BadParameter("--lines must be in form L1:L2")
    raw_start, raw_end = spec.split(":", 1)
    try:
        start = int(raw_start)
        end = int(raw_end)
    except ValueError as exc:
        raise click.BadParameter("--lines must be integers, e.g. 10:42") from exc
    if start < 1 or end < start:
        raise click.BadParameter("--lines start must be >= 1 and <= end")
    return start, end


def _resolve_symbol_range(file: Path, symbol: str) -> tuple[int, int] | None:
    """Resolve a symbol name to its (start_line, end_line) range via lizard.

    Matches either an exact function name or a qualified suffix
    (e.g. `Class.method`) — same matcher shape as `trace read`.
    """
    try:
        parsed = lizard.analyze_file(str(file))
    except Exception:
        return None
    target = next(
        (
            function
            for function in parsed.function_list
            if function.name == symbol or function.name.endswith(f".{symbol}")
        ),
        None,
    )
    if target is None:
        return None
    return target.start_line, target.end_line


UNTRACKED_MARKERS = ("no such path", "is outside repository", "not in a git repository")


def _run_blame(file: Path, line_range: tuple[int, int] | None) -> str:
    """Run `git blame --line-porcelain`, optionally scoped to a range.

    For untracked files git blame fails with "no such path … in HEAD";
    we synthesize a porcelain stream that marks every line uncommitted
    so the command returns valid output instead of crashing.
    """
    command = ["git", "blame", "--line-porcelain"]
    if line_range is not None:
        start, end = line_range
        command += ["-L", f"{start},{end}"]
    command += ["--", str(file)]
    result = subprocess.run(
        command,
        cwd=file.parent,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    if result.returncode == 0:
        return result.stdout
    stderr_lower = result.stderr.lower()
    if any(marker in stderr_lower for marker in UNTRACKED_MARKERS):
        return _synthesize_untracked_porcelain(file, line_range)
    message = result.stderr.strip() or "git blame failed"
    click.echo(f"Error: {message}", err=True)
    sys.exit(1)


def _synthesize_untracked_porcelain(
    file: Path, line_range: tuple[int, int] | None
) -> str:
    """Build porcelain output for an untracked file: every line uncommitted."""
    with open(file, encoding="utf-8", errors="replace") as handle:
        source_lines = handle.read().splitlines()
    if line_range is None:
        start, end = 1, len(source_lines)
    else:
        start, end = line_range
        end = min(end, len(source_lines))
    zero_sha = "0" * 40
    now = int(datetime.now(tz=timezone.utc).timestamp())
    chunks: list[str] = []
    for line_number in range(start, end + 1):
        content = source_lines[line_number - 1] if line_number <= len(source_lines) else ""
        chunks.append(
            f"{zero_sha} {line_number} {line_number} 1\n"
            f"author Not Committed Yet\n"
            f"author-time {now}\n"
            f"author-tz +0000\n"
            f"summary (uncommitted)\n"
            f"\t{content}\n"
        )
    return "".join(chunks)


def _parse_porcelain(output: str) -> list[dict]:
    """Parse `git blame --line-porcelain` output into per-line records.

    Porcelain groups: each line begins with a header
    `<sha> <orig-line> <final-line> [<group-size>]`, followed by
    key/value attribute lines, then a tab-prefixed content line.
    The full attribute block only appears the first time a commit is
    seen; subsequent lines carry only the header + content. We track
    commit attributes in a dict keyed by sha to fill in repeats.
    """
    commits: dict[str, dict] = {}
    lines: list[dict] = []
    current_sha: str | None = None
    current_line: int | None = None
    pending: dict[str, str] = {}

    for raw in output.splitlines():
        if raw.startswith("\t"):
            # Content line — closes the current per-line record.
            if current_sha is None or current_line is None:
                continue
            attributes = commits.setdefault(current_sha, {})
            attributes.update({k: v for k, v in pending.items() if k not in attributes})
            lines.append(
                {
                    "line": current_line,
                    "sha": current_sha,
                    "author": attributes.get("author", "unknown"),
                    "author_time": int(attributes.get("author-time", "0")),
                    "author_tz": attributes.get("author-tz", "+0000"),
                    "summary": attributes.get("summary", ""),
                }
            )
            current_sha = None
            current_line = None
            pending = {}
            continue

        parts = raw.split(" ", 3)
        if current_sha is None and len(parts) >= 3 and len(parts[0]) >= 7:
            # Header line: <sha> <orig> <final> [<group-size>]
            current_sha = parts[0]
            try:
                current_line = int(parts[2])
            except ValueError:
                current_sha = None
                current_line = None
            continue

        # Attribute line within the current group.
        if " " in raw:
            key, _, value = raw.partition(" ")
            pending[key] = value

    return lines


def _collapse_regions(lines: list[dict]) -> list[dict]:
    """Collapse consecutive lines sharing the same commit into regions."""
    regions: list[dict] = []
    for entry in lines:
        if (
            regions
            and regions[-1]["sha"] == entry["sha"]
            and regions[-1]["line_end"] + 1 == entry["line"]
        ):
            regions[-1]["line_end"] = entry["line"]
            continue
        regions.append(
            {
                "line_start": entry["line"],
                "line_end": entry["line"],
                "sha": entry["sha"],
                "author": entry["author"],
                "author_time": entry["author_time"],
                "author_tz": entry["author_tz"],
                "subject": entry["summary"],
            }
        )
    return regions


def _is_uncommitted(sha: str) -> bool:
    return sha.startswith("0" * 8)


def _short_sha(sha: str) -> str:
    return "uncommitted" if _is_uncommitted(sha) else sha[:8]


def _subject(region: dict) -> str:
    """Subject of the region's commit; replaces git's verbose placeholder for uncommitted lines."""
    if _is_uncommitted(region["sha"]):
        return "(uncommitted change)"
    return region["subject"] or "(no subject)"


def _isoformat_date(author_time: int, author_tz: str) -> str:
    """Render author_time as YYYY-MM-DD in the author's timezone."""
    if author_time == 0:
        return "—"
    moment = datetime.fromtimestamp(author_time, tz=timezone.utc)
    return moment.strftime("%Y-%m-%d")


def _humanize_age(author_time: int) -> str:
    if author_time == 0:
        return "uncommitted"
    delta = datetime.now(tz=timezone.utc) - datetime.fromtimestamp(
        author_time, tz=timezone.utc
    )
    seconds = int(delta.total_seconds())
    if seconds < 60:
        return f"{seconds}s ago"
    minutes = seconds // 60
    if minutes < 60:
        return f"{minutes}m ago"
    hours = minutes // 60
    if hours < 24:
        return f"{hours}h ago"
    days = hours // 24
    if days < 30:
        return f"{days}d ago"
    months = days // 30
    if months < 12:
        return f"{months}mo ago"
    years = days // 365
    return f"{years}y ago"


def _format_region(region: dict) -> str:
    line_start = region["line_start"]
    line_end = region["line_end"]
    span = f"L{line_start}" if line_start == line_end else f"L{line_start}-L{line_end}"
    sha = _short_sha(region["sha"])
    date = _isoformat_date(region["author_time"], region["author_tz"])
    age = _humanize_age(region["author_time"])
    subject = _subject(region)
    author = region["author"]
    return f"  {span:<14} {sha:<11} {date}  {age:<10}  {author:<22} {subject}"


@click.command()
@click.argument("file", type=click.Path(exists=True, dir_okay=False))
@click.argument("symbol", required=False)
@click.option(
    "--lines",
    "lines_spec",
    metavar="L1:L2",
    help="Blame only this inclusive line range; mutually exclusive with SYMBOL.",
)
@click.option("--json", "as_json", is_flag=True, help="Machine-parseable JSON output.")
def command(file: str, symbol: str | None, lines_spec: str | None, as_json: bool) -> None:
    """Blame a file, line range, or symbol — collapsed into regions.

    Three scoping modes:

    \b
      trace blame <file>                  whole file
      trace blame <file> <symbol>         just the symbol's line range
      trace blame <file> --lines L1:L2    just that explicit line range
    """
    require_dependencies()

    if symbol is not None and lines_spec is not None:
        click.echo("Error: SYMBOL and --lines are mutually exclusive", err=True)
        sys.exit(2)

    path = Path(file).resolve()

    line_range: tuple[int, int] | None = None
    scope = "file"
    if symbol is not None:
        resolved = _resolve_symbol_range(path, symbol)
        if resolved is None:
            click.echo(f"Error: symbol '{symbol}' not found in {file}", err=True)
            sys.exit(2)
        line_range = resolved
        scope = "symbol"
    elif lines_spec is not None:
        line_range = _parse_lines(lines_spec)
        scope = "lines"

    porcelain = _run_blame(path, line_range)
    blame_lines = _parse_porcelain(porcelain)
    regions = _collapse_regions(blame_lines)

    facts = file_facts.get(path)
    repo_root = cache.repo_root_for(path)
    try:
        display_file = str(path.relative_to(repo_root.resolve()))
    except ValueError:
        display_file = str(path)

    if as_json:
        click.echo(
            json.dumps(
                {
                    "file": display_file,
                    "language": facts.language if facts else None,
                    "scope": scope,
                    "symbol": symbol,
                    "line_range": (
                        {"start": line_range[0], "end": line_range[1]}
                        if line_range
                        else None
                    ),
                    "regions": [
                        {
                            "line_start": region["line_start"],
                            "line_end": region["line_end"],
                            "sha": _short_sha(region["sha"]),
                            "author": region["author"],
                            "date": _isoformat_date(
                                region["author_time"], region["author_tz"]
                            ),
                            "age": _humanize_age(region["author_time"]),
                            "subject": _subject(region),
                        }
                        for region in regions
                    ],
                    "region_count": len(regions),
                    "line_count": len(blame_lines),
                },
                indent=2,
            )
        )
        return

    header = f"# {display_file}"
    if scope == "symbol":
        header += f" :: {symbol}  (L{line_range[0]}-L{line_range[1]})"
    elif scope == "lines":
        header += f"  L{line_range[0]}-L{line_range[1]}"
    click.echo(header)
    click.echo(
        f"Regions: {len(regions)}  Lines blamed: {len(blame_lines)}"
    )
    click.echo("")
    click.echo(
        f"  {'lines':<14} {'commit':<11} {'date':<10}  {'age':<10}  "
        f"{'author':<22} subject"
    )
    for region in regions:
        click.echo(_format_region(region))
