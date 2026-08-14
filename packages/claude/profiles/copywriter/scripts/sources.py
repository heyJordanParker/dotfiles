#!/usr/bin/env python3
"""Track every visited research URL and the trust it earns over time.

Each URL is recorded once (`log`, or implicitly by `check`) and judged once
(`judge`) with a trustworthiness and a usefulness score, both 1-100. Domain
and author numbers are recency-weighted averages of their judged URLs with a
~12-month half-life, so old judgments fade and the `trust` listing works as a
living whitelist/blacklist.

Stdlib only.

Page reading lives in the browse skill (skills/browse/scripts/browse.py); it
imports this module's write helpers so the registry has one owner. `record_url`
and `record_unreadable` are that shared surface.

Usage:
    sources.py check <url> [--db PATH]
    sources.py log <url> [--type T] [--topic T] [--published D] [--updated D]
                        [--author ID] [--note ...] [--db PATH]
    sources.py judge <url> --trust N --useful N [--note ...] [--force] [--db PATH]
    sources.py author add [--first-name ...] [--last-name ...] [--usernames csv]
                        [--twitter-url ...] [--linkedin-url ...] [--youtube-url ...]
                        [--instagram-url ...] [--website-url ...]
                        [--newsletter-url ...] [--note ...] [--db PATH]
    sources.py author link <url> <author-id> [--db PATH]
    sources.py trust [--authors] [--db PATH]

The database defaults to the SOURCES_DB environment variable, else the global
home at /Users/jordan/Developer/wiki/sources.db; override with --db.
"""

from __future__ import annotations

import argparse
import datetime as dt
import math
import os
import re
import sqlite3
import sys
from pathlib import Path
from urllib.parse import urlparse


DEFAULT_DB = "/Users/jordan/Developer/wiki/sources.db"
HALF_LIFE_DAYS = 365.0


def database_path(argument_db: str | None) -> Path:
    if argument_db:
        return Path(argument_db).expanduser()
    env = os.environ.get("SOURCES_DB")
    if env:
        return Path(env).expanduser()
    return Path(DEFAULT_DB)


def connect(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA foreign_keys = ON")
    return connection


def migrate(connection: sqlite3.Connection) -> None:
    connection.execute("BEGIN IMMEDIATE")
    try:
        version = connection.execute("PRAGMA user_version").fetchone()[0]
        if version < 2:
            for table in ("grade_events", "sources"):
                if connection.execute(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
                    (table,),
                ).fetchone():
                    connection.execute(
                        f"ALTER TABLE {table} RENAME TO {table}_legacy"
                    )
        connection.execute(
            """
            CREATE TABLE IF NOT EXISTS authors (
            id INTEGER PRIMARY KEY,
            first_name TEXT NOT NULL DEFAULT '',
            last_name TEXT NOT NULL DEFAULT '',
            usernames TEXT NOT NULL DEFAULT '',
            twitter_url TEXT NOT NULL DEFAULT '',
            linkedin_url TEXT NOT NULL DEFAULT '',
            youtube_url TEXT NOT NULL DEFAULT '',
            instagram_url TEXT NOT NULL DEFAULT '',
            website_url TEXT NOT NULL DEFAULT '',
            newsletter_url TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT ''
            )
            """
        )
        connection.execute(
            """
            CREATE TABLE IF NOT EXISTS sources (
            url TEXT PRIMARY KEY,
            domain TEXT NOT NULL,
            topic TEXT NOT NULL DEFAULT '',
            type TEXT NOT NULL DEFAULT '',
            author_id INTEGER REFERENCES authors(id),
            published TEXT,
            updated TEXT,
            first_seen TEXT NOT NULL,
            trustworthiness INTEGER,
            usefulness INTEGER,
            judged_at TEXT,
            note TEXT NOT NULL DEFAULT ''
            )
            """
        )
        if version < 3:
            columns = {
                row["name"] for row in connection.execute("PRAGMA table_info(sources)")
            }
            if "attempts" not in columns:
                connection.execute(
                    "ALTER TABLE sources ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0"
                )
        connection.execute(
            "CREATE INDEX IF NOT EXISTS sources_domain_idx ON sources (domain)"
        )
        connection.execute(
            "CREATE INDEX IF NOT EXISTS sources_author_idx ON sources (author_id)"
        )
        connection.execute("PRAGMA user_version = 3")
        connection.commit()
    except Exception:
        connection.rollback()
        raise


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def domain_of(url: str) -> str:
    parsed = urlparse(url if "://" in url else f"//{url}", scheme="https")
    host = (parsed.netloc or parsed.path).split("/", 1)[0].lower()
    return host[4:] if host.startswith("www.") else host


def record_unreadable(
    connection: sqlite3.Connection, url: str, outcome: str, status: str
) -> None:
    source = record_url(connection, url)
    date = dt.datetime.now(dt.timezone.utc).date().isoformat()
    entry = f"unreadable: {outcome} {status} {date}"
    note = "\n".join(part for part in (source["note"], entry) if part)
    failed_dates = set(
        re.findall(r"unreadable: [^\n]+ (\d{4}-\d{2}-\d{2})", note)
    )
    if len(failed_dates) >= 2 and not re.search(r"(^|\n)dead \d{4}-\d{2}-\d{2}", note):
        note = f"{note}\ndead {date}"
    with connection:
        connection.execute(
            "UPDATE sources SET attempts = attempts + 1, note = ? WHERE url = ?",
            (note, url),
        )


def age_days(timestamp: str) -> float:
    moment = dt.datetime.fromisoformat(timestamp)
    if moment.tzinfo is None:
        moment = moment.replace(tzinfo=dt.timezone.utc)
    return max((dt.datetime.now(dt.timezone.utc) - moment).total_seconds() / 86400, 0.0)


def weighted_scores(rows: list[sqlite3.Row]) -> tuple[float | None, float | None, int]:
    """Recency-weighted trust and usefulness over judged rows; ~12-month half-life."""
    trust_total = useful_total = weight_total = 0.0
    judged = 0
    for row in rows:
        if row["trustworthiness"] is None or row["attempts"]:
            continue
        judged += 1
        anchor = row["judged_at"] or row["first_seen"]
        weight = math.pow(0.5, age_days(anchor) / HALF_LIFE_DAYS)
        weight_total += weight
        trust_total += weight * row["trustworthiness"]
        useful_total += weight * row["usefulness"]
    if not judged:
        return None, None, 0
    return trust_total / weight_total, useful_total / weight_total, judged


def format_score(score: float | None) -> str:
    return f"{score:.0f}" if score is not None else "-"


def record_url(
    connection: sqlite3.Connection,
    url: str,
    type_: str = "",
    topic: str = "",
    published: str | None = None,
    updated: str | None = None,
    author_id: int | None = None,
) -> sqlite3.Row:
    """Insert the URL if unseen, else fold in any newly supplied fields. Idempotent."""
    if author_id is not None and not connection.execute(
        "SELECT id FROM authors WHERE id = ?", (author_id,)
    ).fetchone():
        raise SystemExit(f"unknown author id: {author_id}")
    with connection:
        connection.execute(
            """
            INSERT INTO sources (url, domain, type, topic, published, updated, author_id, first_seen)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(url) DO UPDATE SET
                type = CASE WHEN excluded.type != '' THEN excluded.type ELSE sources.type END,
                topic = CASE WHEN excluded.topic != '' THEN excluded.topic ELSE sources.topic END,
                published = COALESCE(excluded.published, sources.published),
                updated = COALESCE(excluded.updated, sources.updated),
                author_id = COALESCE(excluded.author_id, sources.author_id)
            """,
            (url, domain_of(url), type_, topic, published, updated, author_id, now()),
        )
    return connection.execute("SELECT * FROM sources WHERE url = ?", (url,)).fetchone()


def log_url(connection: sqlite3.Connection, arguments: argparse.Namespace) -> None:
    for field in ("published", "updated"):
        value = getattr(arguments, field)
        if value is None:
            continue
        parts = value.split("-")
        try:
            if (
                len(parts) not in (1, 2, 3)
                or len(parts[0]) != 4
                or any(len(part) != 2 for part in parts[1:])
                or not all(part.isdigit() for part in parts)
            ):
                raise ValueError
            dt.date(*map(int, parts), *([1] * (3 - len(parts))))
        except ValueError:
            raise SystemExit(
                f"invalid --{field} date: {value} "
                "(expected YYYY, YYYY-MM, or YYYY-MM-DD)"
            )
    source = record_url(
        connection,
        arguments.url,
        type_=arguments.type or "",
        topic=arguments.topic or "",
        published=arguments.published,
        updated=arguments.updated,
        author_id=arguments.author,
    )
    if arguments.note and not (
        source["note"] == arguments.note
        or source["note"].endswith(f"\n{arguments.note}")
    ):
        note = "\n".join(part for part in (source["note"], arguments.note) if part)
        with connection:
            connection.execute(
                "UPDATE sources SET note = ? WHERE url = ?",
                (note, arguments.url),
            )
    print(f"logged {arguments.url} [{source['domain']}]")
    if not source["published"] and not source["updated"]:
        print("warning: neither published nor updated is set for this URL")


def domain_rows(connection: sqlite3.Connection, domain: str) -> list[sqlite3.Row]:
    return connection.execute(
        "SELECT * FROM sources WHERE domain = ?", (domain,)
    ).fetchall()


def author_rows(connection: sqlite3.Connection, author_id: int) -> list[sqlite3.Row]:
    return connection.execute(
        "SELECT * FROM sources WHERE author_id = ?", (author_id,)
    ).fetchall()


def author_name(author: sqlite3.Row) -> str:
    name = f"{author['first_name']} {author['last_name']}".strip()
    return name or author["usernames"].split(",")[0].strip() or f"author {author['id']}"


def check_url(connection: sqlite3.Connection, url: str) -> None:
    source = record_url(connection, url)
    domain = source["domain"]
    trust, useful, judged = weighted_scores(domain_rows(connection, domain))
    total = len(domain_rows(connection, domain))
    print(f"{url} [{domain}]")
    status = (
        f"judged {source['judged_at']} trust {source['trustworthiness']} useful {source['usefulness']}"
        if source["judged_at"]
        else "unjudged"
    )
    print(f"  url: {status}")
    print(
        f"  domain: trust {format_score(trust)} useful {format_score(useful)}"
        f" ({judged}/{total} urls judged)"
    )
    if source["author_id"] is not None:
        author = connection.execute(
            "SELECT * FROM authors WHERE id = ?", (source["author_id"],)
        ).fetchone()
        rows = author_rows(connection, author["id"])
        trust, useful, judged = weighted_scores(rows)
        print(
            f"  author {author['id']} ({author_name(author)}):"
            f" trust {format_score(trust)} useful {format_score(useful)}"
            f" ({judged}/{len(rows)} urls judged)"
        )


def judge_url(connection: sqlite3.Connection, arguments: argparse.Namespace) -> None:
    for score in (arguments.trust, arguments.useful):
        if not 1 <= score <= 100:
            raise SystemExit("scores run 1-100")
    source = record_url(connection, arguments.url)
    if source["judged_at"] and not arguments.force:
        raise SystemExit(
            f"already judged {source['judged_at']} (trust {source['trustworthiness']},"
            f" useful {source['usefulness']}) — a URL is judged once; use --force to overwrite"
        )
    with connection:
        connection.execute(
            """
            UPDATE sources
            SET trustworthiness = ?, usefulness = ?, judged_at = ?,
                note = CASE WHEN ? != '' THEN ? ELSE note END
            WHERE url = ?
            """,
            (arguments.trust, arguments.useful, now(), arguments.note, arguments.note, arguments.url),
        )
    print(f"judged {arguments.url} trust {arguments.trust} useful {arguments.useful}")


def add_author(connection: sqlite3.Connection, arguments: argparse.Namespace) -> None:
    with connection:
        cursor = connection.execute(
            """
            INSERT INTO authors (first_name, last_name, usernames, twitter_url,
                linkedin_url, youtube_url, instagram_url, website_url,
                newsletter_url, note)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                arguments.first_name,
                arguments.last_name,
                arguments.usernames,
                arguments.twitter_url,
                arguments.linkedin_url,
                arguments.youtube_url,
                arguments.instagram_url,
                arguments.website_url,
                arguments.newsletter_url,
                arguments.note,
            ),
        )
    author = connection.execute(
        "SELECT * FROM authors WHERE id = ?", (cursor.lastrowid,)
    ).fetchone()
    print(f"author {author['id']} added: {author_name(author)}")


def link_author(connection: sqlite3.Connection, url: str, author_id: int) -> None:
    if not connection.execute("SELECT id FROM authors WHERE id = ?", (author_id,)).fetchone():
        raise SystemExit(f"unknown author id: {author_id}")
    record_url(connection, url)
    with connection:
        connection.execute(
            "UPDATE sources SET author_id = ? WHERE url = ?", (author_id, url)
        )
    print(f"linked {url} to author {author_id}")


def print_trust(connection: sqlite3.Connection, authors: bool) -> None:
    if authors:
        entries = []
        for author in connection.execute("SELECT * FROM authors ORDER BY id").fetchall():
            rows = author_rows(connection, author["id"])
            trust, useful, judged = weighted_scores(rows)
            entries.append((f"{author['id']} {author_name(author)}", trust, useful, judged, len(rows)))
        label = "author"
    else:
        entries = []
        for row in connection.execute("SELECT DISTINCT domain FROM sources ORDER BY domain"):
            rows = domain_rows(connection, row["domain"])
            trust, useful, judged = weighted_scores(rows)
            entries.append((row["domain"], trust, useful, judged, len(rows)))
        label = "domain"
    if not entries:
        print(f"no {label}s recorded")
        return
    entries.sort(key=lambda entry: (-(entry[1] if entry[1] is not None else -1), entry[0]))
    print(f"{label:<40} trust  useful  judged/urls")
    for name, trust, useful, judged, total in entries:
        print(
            f"{name:<40} {format_score(trust):>5}  {format_score(useful):>6}"
            f"  {judged}/{total}"
        )


def parser() -> argparse.ArgumentParser:
    command_parser = argparse.ArgumentParser(
        description="Track visited research URLs and the trust their domains and authors earn."
    )
    command_parser.add_argument("--db", help="path to the SQLite database")
    subcommands = command_parser.add_subparsers(dest="command", required=True)

    check = subcommands.add_parser("check", help="show domain/author trust; record unseen URLs")
    check.add_argument("url")

    log = subcommands.add_parser("log", help="record a visited URL (idempotent)")
    log.add_argument("url")
    log.add_argument("--type", help="plain label: review site, forum thread, vendor blog, ...")
    log.add_argument("--topic")
    log.add_argument("--published", help="publication date, YYYY-MM-DD")
    log.add_argument("--updated", help="last-updated date, YYYY-MM-DD")
    log.add_argument("--author", type=int, help="author id to link")
    log.add_argument("--note", default="")

    judge = subcommands.add_parser("judge", help="one-time judgment of a URL")
    judge.add_argument("url")
    judge.add_argument("--trust", type=int, required=True, help="trustworthiness 1-100")
    judge.add_argument("--useful", type=int, required=True, help="usefulness 1-100")
    judge.add_argument("--note", default="")
    judge.add_argument("--force", action="store_true", help="overwrite an existing judgment")

    author = subcommands.add_parser("author", help="manage authors")
    author_commands = author.add_subparsers(dest="author_command", required=True)
    add = author_commands.add_parser("add", help="create an author row")
    for field in (
        "first-name", "last-name", "usernames", "twitter-url", "linkedin-url",
        "youtube-url", "instagram-url", "website-url", "newsletter-url", "note",
    ):
        add.add_argument(f"--{field}", default="")
    link = author_commands.add_parser("link", help="link a URL to an author")
    link.add_argument("url")
    link.add_argument("author_id", type=int)

    trust = subcommands.add_parser("trust", help="every domain (or author) with its numbers")
    trust.add_argument("--authors", action="store_true")
    return command_parser


def main() -> int:
    arguments = parser().parse_args()
    with connect(database_path(arguments.db)) as connection:
        migrate(connection)
        if arguments.command == "check":
            check_url(connection, arguments.url)
        elif arguments.command == "log":
            log_url(connection, arguments)
        elif arguments.command == "judge":
            judge_url(connection, arguments)
        elif arguments.command == "author":
            if arguments.author_command == "add":
                add_author(connection, arguments)
            else:
                link_author(connection, arguments.url, arguments.author_id)
        elif arguments.command == "trust":
            print_trust(connection, arguments.authors)
    return 0


if __name__ == "__main__":
    sys.exit(main())
