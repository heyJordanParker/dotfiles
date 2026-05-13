"""`trace history` — three archaeology modes in one command.

Mode selection is mutually exclusive:

  trace history <file>                whole-file mode
  trace history <file> <symbol>       function-level mode (git log -L)
  trace history --contains <pattern>  content-search mode (git pickaxe)

Whole-file mode reuses the cached bulk git pipeline (`git_activity`) for
commits_30d, top_author, last_subject, co_changed, and the immediate
rename_from. The full transitive rename chain is computed via one
`git log --follow --name-status --diff-filter=R` invocation.

Function-level mode shells out to `git log -L :<symbol>:<file>`, which is
git's native function-range history — each commit that touched the symbol,
with the diff hunk inline.

Content-search mode runs `git log -S<pattern>` (pickaxe). For each commit
and path, the enclosing symbol is resolved with universal-ctags against
the commit's blob.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import click

from tracer import cache as _cache
from tracer import git_activity
from tracer.deps import require_dependencies


# Whole-file mode: how many recent commits to show by default.
_RECENT_COMMITS = 10
# Function-level mode: how many commits of -L history to show.
_FUNCTION_COMMITS = 20
# Pickaxe mode: how many matching commits to show.
_PICKAXE_COMMITS = 30


# ---------- mode 1: whole-file ----------


def _recent_commits(file: Path, repo_root: Path, n: int) -> list[dict]:
    try:
        relative = str(file.resolve().relative_to(repo_root.resolve()))
    except ValueError:
        relative = str(file)
    result = subprocess.run(
        [
            "git", "log", f"-{n}",
            "--pretty=format:%h|%an|%ad|%s",
            "--date=short",
            "--", relative,
        ],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=15,
    )
    commits: list[dict] = []
    for line in result.stdout.splitlines():
        if "|" not in line:
            continue
        sha, author, date, subject = line.split("|", 3)
        commits.append({"sha": sha, "author": author, "date": date, "subject": subject})
    return commits


def _blame_top_authors(file: Path, repo_root: Path, top: int = 5) -> list[dict]:
    from collections import Counter
    result = subprocess.run(
        ["git", "blame", "--line-porcelain", str(file)],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    counts: Counter[str] = Counter()
    for line in result.stdout.splitlines():
        if line.startswith("author "):
            counts[line[len("author "):]] += 1
    return [{"author": a, "lines": n} for a, n in counts.most_common(top)]


def _rename_chain(file: Path, repo_root: Path) -> list[str]:
    """Full transitive name lineage of a file, newest → oldest.

    `git log --follow --name-status --diff-filter=R` emits every rename
    affecting this file. Walks the chain by stitching `R<X>\\told\\tnew`
    rows. Returns the historical names other than the current one — empty
    list when the file has no rename history.
    """
    try:
        relative = str(file.resolve().relative_to(repo_root.resolve()))
    except ValueError:
        relative = str(file)
    result = subprocess.run(
        [
            "git", "log", "--follow",
            "--name-status",
            "--diff-filter=R",
            "--pretty=format:",
            "--", relative,
        ],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    chain: list[str] = []
    current = relative
    # Output is newest → oldest. For each `R<X>\told\tnew` where new ==
    # current, advance current = old.
    for line in result.stdout.splitlines():
        if not line.strip() or not line.startswith("R"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        old, new = parts[1], parts[2]
        if new == current:
            chain.append(old)
            current = old
    return chain


def _whole_file_payload(file: Path, repo_root: Path) -> dict:
    try:
        relative = str(file.resolve().relative_to(repo_root.resolve()))
    except ValueError:
        relative = str(file)

    activity_map = git_activity.bulk_cached(repo_root)
    activity = activity_map.get(relative, git_activity.empty())

    return {
        "mode": "file",
        "file": relative,
        "commit_count": activity.commit_count,
        "commits_30d": activity.commits_30d,
        "first_seen": activity.first_seen,
        "last_modified": activity.last_modified,
        "last_author": activity.last_author,
        "last_subject": activity.last_subject,
        "top_author": activity.top_author,
        "working_state": activity.working_state,
        "present_in": list(activity.present_in),
        "recent_commits": _recent_commits(file, repo_root, _RECENT_COMMITS),
        "top_blame_authors": _blame_top_authors(file, repo_root),
        "rename_chain": _rename_chain(file, repo_root),
        "co_changed": [
            {"path": path, "commits": n} for path, n in activity.co_changed
        ],
    }


def _render_whole_file(payload: dict) -> None:
    click.echo(f"File: {payload['file']}")
    state = payload["working_state"]
    state_part = f", working_state={state}" if state else ""
    click.echo(
        f"Commits: {payload['commit_count']} total, "
        f"{payload['commits_30d']} in last 30 days{state_part}"
    )
    if payload["first_seen"] or payload["last_modified"]:
        click.echo(
            f"First seen: {payload['first_seen']}  "
            f"Last modified: {payload['last_modified']} "
            f"({payload['last_author']})"
        )
    if payload["last_subject"]:
        click.echo(f"Last subject: {payload['last_subject']}")
    if payload["top_author"]:
        click.echo(f"Top author (by commits): {payload['top_author']}")
    if payload["present_in"]:
        click.echo(f"Present on: {', '.join(payload['present_in'])}")
    click.echo("")

    if payload["rename_chain"]:
        click.echo("Rename chain (newest -> oldest):")
        click.echo(f"  {payload['file']}")
        for old in payload["rename_chain"]:
            click.echo(f"  <- {old}")
        click.echo("")

    if payload["recent_commits"]:
        click.echo("Recent commits:")
        for c in payload["recent_commits"]:
            click.echo(f"  {c['sha']} {c['date']} {c['author']}: {c['subject']}")
        click.echo("")

    if payload["top_blame_authors"]:
        click.echo("Top blame authors (lines in current file):")
        for entry in payload["top_blame_authors"]:
            click.echo(f"  {entry['lines']:>5}  {entry['author']}")
        click.echo("")

    if payload["co_changed"]:
        click.echo("Files that change together:")
        for entry in payload["co_changed"]:
            click.echo(f"  {entry['commits']:>4}  {entry['path']}")


# ---------- mode 2: function-level ----------


def _function_history(file: Path, repo_root: Path, symbol: str, n: int) -> list[dict]:
    """Use `git log -L :<symbol>:<file>` for function-line history.

    Each commit produces a header block (sha, author, date, subject) and a
    diff hunk. Parsed below into one dict per commit with `hunk` as the raw
    diff text — keeping the diff verbatim is the value of this mode.
    """
    try:
        relative = str(file.resolve().relative_to(repo_root.resolve()))
    except ValueError:
        relative = str(file)
    result = subprocess.run(
        [
            "git", "log",
            f"-L:{symbol}:{relative}",
            f"-{n}",
            "--pretty=format:%x00COMMIT%x00%H%x00%an%x00%ad%x00%s",
            "--date=short",
            "--no-color",
        ],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
    )
    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        raise click.ClickException(
            f"git log -L failed for symbol '{symbol}' in {relative}: {stderr}"
        )

    commits: list[dict] = []
    current: dict | None = None
    hunk_lines: list[str] = []

    def _flush() -> None:
        if current is not None:
            current["hunk"] = "\n".join(hunk_lines).rstrip()
            commits.append(current)

    for line in result.stdout.splitlines():
        if line.startswith("\x00COMMIT\x00"):
            _flush()
            parts = line.split("\x00")
            # ['', 'COMMIT', sha, author, date, subject]
            sha = parts[2] if len(parts) > 2 else ""
            author = parts[3] if len(parts) > 3 else ""
            date = parts[4] if len(parts) > 4 else ""
            subject = parts[5] if len(parts) > 5 else ""
            current = {
                "sha": sha[:12],
                "author": author,
                "date": date,
                "subject": subject,
            }
            hunk_lines = []
            continue
        if current is not None:
            hunk_lines.append(line)
    _flush()
    return commits


def _function_payload(file: Path, repo_root: Path, symbol: str) -> dict:
    try:
        relative = str(file.resolve().relative_to(repo_root.resolve()))
    except ValueError:
        relative = str(file)
    return {
        "mode": "function",
        "file": relative,
        "symbol": symbol,
        "commits": _function_history(file, repo_root, symbol, _FUNCTION_COMMITS),
    }


def _render_function(payload: dict) -> None:
    click.echo(f"File: {payload['file']}")
    click.echo(f"Symbol: {payload['symbol']}")
    click.echo(f"Commits touching symbol: {len(payload['commits'])}")
    click.echo("")
    for commit in payload["commits"]:
        click.echo(
            f"{commit['sha']} {commit['date']} {commit['author']}: {commit['subject']}"
        )
        if commit["hunk"]:
            for line in commit["hunk"].splitlines():
                click.echo(f"  {line}")
        click.echo("")


# ---------- mode 3: pickaxe ----------


def _pickaxe_commits(pattern: str, repo_root: Path, n: int) -> list[dict]:
    """`git log -S<pattern> --name-only` — commits that add or remove the
    literal string, with the files touched per commit.
    """
    result = subprocess.run(
        [
            "git", "log", f"-{n}",
            f"-S{pattern}",
            "--name-only",
            "--pretty=format:%x00COMMIT%x00%H%x00%an%x00%ad%x00%s",
            "--date=short",
        ],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
    )
    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        raise click.ClickException(f"git log -S failed: {stderr}")

    commits: list[dict] = []
    current: dict | None = None

    def _flush() -> None:
        if current is not None:
            commits.append(current)

    for line in result.stdout.splitlines():
        if line.startswith("\x00COMMIT\x00"):
            _flush()
            parts = line.split("\x00")
            sha = parts[2] if len(parts) > 2 else ""
            author = parts[3] if len(parts) > 3 else ""
            date = parts[4] if len(parts) > 4 else ""
            subject = parts[5] if len(parts) > 5 else ""
            current = {
                "sha": sha,
                "short_sha": sha[:12],
                "author": author,
                "date": date,
                "subject": subject,
                "files": [],
            }
            continue
        stripped = line.strip()
        if stripped and current is not None:
            current["files"].append(stripped)
    _flush()
    return commits


def _commit_line_for_pattern(commit_sha: str, path: str, pattern: str, repo_root: Path) -> int | None:
    """Find the line in `path` at `commit_sha` where `pattern` appears.

    Used to anchor the enclosing-symbol resolution. Returns None when the
    pattern isn't found in the blob at that revision (which happens when
    the commit *removed* the string — the line existed in the parent, not
    this commit's blob).
    """
    result = subprocess.run(
        ["git", "show", f"{commit_sha}:{path}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=15,
    )
    if result.returncode != 0:
        return None
    for idx, line in enumerate(result.stdout.splitlines(), start=1):
        if pattern in line:
            return idx
    return None


def _enclosing_symbol(commit_sha: str, path: str, line: int, repo_root: Path) -> str | None:
    """Use universal-ctags on the file's blob at `commit_sha` to find the
    enclosing symbol for `line`. Returns None when ctags can't resolve.
    """
    show = subprocess.run(
        ["git", "show", f"{commit_sha}:{path}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
        timeout=15,
    )
    if show.returncode != 0 or not show.stdout:
        return None

    suffix = Path(path).suffix or ".txt"
    with tempfile.NamedTemporaryFile("w", suffix=suffix, delete=False) as tmp:
        tmp.write(show.stdout)
        tmp_path = tmp.name
    try:
        ctags = subprocess.run(
            [
                "ctags",
                "--output-format=json",
                "--fields=+ne",
                "--sort=no",
                "-f", "-",
                tmp_path,
            ],
            capture_output=True,
            text=True,
            check=False,
            timeout=15,
        )
    finally:
        try:
            Path(tmp_path).unlink()
        except OSError:
            pass

    if ctags.returncode != 0:
        return None

    enclosing: tuple[int, str] | None = None
    for entry_line in ctags.stdout.splitlines():
        if not entry_line.strip():
            continue
        try:
            entry = json.loads(entry_line)
        except json.JSONDecodeError:
            continue
        start = entry.get("line")
        end = entry.get("end") or start
        name = entry.get("name")
        if not isinstance(start, int) or not isinstance(end, int) or not name:
            continue
        if start <= line <= end:
            if enclosing is None or start > enclosing[0]:
                enclosing = (start, name)
    return enclosing[1] if enclosing else None


def _pickaxe_payload(pattern: str, repo_root: Path) -> dict:
    commits = _pickaxe_commits(pattern, repo_root, _PICKAXE_COMMITS)
    annotated: list[dict] = []
    for commit in commits:
        entries: list[dict] = []
        for path in commit["files"]:
            line = _commit_line_for_pattern(commit["sha"], path, pattern, repo_root)
            symbol = (
                _enclosing_symbol(commit["sha"], path, line, repo_root)
                if line is not None
                else None
            )
            entries.append({"path": path, "line": line, "enclosing_symbol": symbol})
        annotated.append(
            {
                "sha": commit["short_sha"],
                "date": commit["date"],
                "author": commit["author"],
                "subject": commit["subject"],
                "matches": entries,
            }
        )
    return {
        "mode": "contains",
        "pattern": pattern,
        "commit_count": len(annotated),
        "commits": annotated,
    }


def _render_pickaxe(payload: dict) -> None:
    click.echo(f"Pattern: {payload['pattern']}")
    click.echo(f"Commits introducing or removing the pattern: {payload['commit_count']}")
    click.echo("")
    for commit in payload["commits"]:
        click.echo(
            f"{commit['sha']} {commit['date']} {commit['author']}: {commit['subject']}"
        )
        for match in commit["matches"]:
            line_part = f"L{match['line']}" if match["line"] is not None else "L?"
            symbol_part = (
                f" [in {match['enclosing_symbol']}]"
                if match["enclosing_symbol"]
                else ""
            )
            click.echo(f"  {line_part:<7} {match['path']}{symbol_part}")
        click.echo("")


# ---------- entry point ----------


@click.command()
@click.argument("file", required=False)
@click.argument("symbol", required=False)
@click.option("--contains", "contains", default=None, help="Pickaxe: commits introducing or removing this literal string anywhere in the repo.")
@click.option("--json", "as_json", is_flag=True)
def command(file: str | None, symbol: str | None, contains: str | None, as_json: bool) -> None:
    """Git archaeology across three mutually exclusive modes.

    \b
    trace history <file>                whole-file history
    trace history <file> <symbol>       line-history of one symbol
    trace history --contains <pattern>  commits adding/removing a string
    """
    require_dependencies()

    if contains is not None:
        if file is not None or symbol is not None:
            click.echo(
                "Error: --contains is mutually exclusive with <file>/<symbol> arguments.",
                err=True,
            )
            sys.exit(2)
        repo_root = _cache.repo_root_for(".")
        payload = _pickaxe_payload(contains, repo_root)
        if as_json:
            click.echo(json.dumps(payload, indent=2))
        else:
            _render_pickaxe(payload)
        return

    if file is None:
        click.echo(
            "Error: provide <file>, <file> <symbol>, or --contains <pattern>.",
            err=True,
        )
        sys.exit(2)

    file_path = Path(file)
    if not file_path.exists() or not file_path.is_file():
        click.echo(f"Error: file not found: {file}", err=True)
        sys.exit(2)
    file_path = file_path.resolve()
    repo_root = _cache.repo_root_for(file_path)

    if symbol is not None:
        payload = _function_payload(file_path, repo_root, symbol)
        if as_json:
            click.echo(json.dumps(payload, indent=2))
        else:
            _render_function(payload)
        return

    payload = _whole_file_payload(file_path, repo_root)
    if as_json:
        click.echo(json.dumps(payload, indent=2))
    else:
        _render_whole_file(payload)
