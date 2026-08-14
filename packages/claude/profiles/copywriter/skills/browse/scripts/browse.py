#!/usr/bin/env python3
"""Read one web page for research: fetch, validate, clean, cache, and record.

A plain-HTTP fetch with a browser User-Agent is the primary reader — it passes
bot walls that block headless browsers (tested: 13/13 review quotes recovered on
a Cloudflare-gated page where agent-browser and Playwright both failed).
agent-browser is the one fallback, tried only when the direct fetch cannot read
the page, its session closed inside this command.

Reading is this script's job. The registry database has one owner, sources.py,
whose write helpers this imports — this never touches the schema itself.

Two files are written per readable page, so a wrong cleaner guess loses nothing:
    <hash>.txt      readability-cleaned main content (nav/footer/cookie stripped)
    <hash>.raw.txt  full stripped visible text
Prints the cleaned path; --raw prints the raw path.

Stdlib only.

Usage:
    browse.py <url> [--raw] [--db PATH]

Exit codes: 0 read, 2 blocked, 3 gone/network, 4 invalid URL, 5 thin, 6 dead URL.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "scripts"))
import sources  # noqa: E402  the registry database owner; write helpers only


CACHE = Path("/tmp/browse-cache")
TIMEOUT = 20
USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0 Safari/537.36"
)
INTERSTITIAL_MARKERS = (
    "access denied",
    "just a moment",
    "verify you are human",
    "enable javascript",
    "captcha",
)

VOID_TAGS = {
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "param", "source", "track", "wbr",
}
INVISIBLE_TAGS = {"script", "style", "head", "noscript", "template"}
CHROME_TAGS = INVISIBLE_TAGS | {
    "svg", "nav", "footer", "header", "aside", "form", "button", "iframe",
}
CHROME_KEYWORDS = (
    "nav", "menu", "sidebar", "footer", "header", "cookie", "consent", "gdpr",
    "banner", "popup", "modal", "dialog", "overlay", "subscribe", "newsletter",
    "social", "share", "comment", "related", "promo", "advert", "sponsor",
    "breadcrumb", "pagination", "masthead", "toolbar", "skip", "widget",
)
CANDIDATE_TAGS = {"div", "article", "section", "main", "td", "ul", "ol", "body"}
BLOCK_TAGS = {
    "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "br", "tr",
    "blockquote",
}


# --- readability-style DOM cleaner -----------------------------------------


class Node:
    __slots__ = ("tag", "attrs", "children")

    def __init__(self, tag: str, attrs: dict[str, str]) -> None:
        self.tag = tag
        self.attrs = attrs
        self.children: list = []  # str tokens and child Nodes, in document order


class DomBuilder(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.root = Node("root", {})
        self.stack = [self.root]

    def handle_starttag(self, tag: str, attrs) -> None:
        node = Node(tag, {k: (v or "") for k, v in attrs})
        self.stack[-1].children.append(node)
        if tag not in VOID_TAGS:
            self.stack.append(node)

    def handle_startendtag(self, tag: str, attrs) -> None:
        self.stack[-1].children.append(Node(tag, {k: (v or "") for k, v in attrs}))

    def handle_endtag(self, tag: str) -> None:
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                del self.stack[index:]
                return

    def handle_data(self, data: str) -> None:
        self.stack[-1].children.append(data)


def normalize_text(text: str) -> str:
    return "\n".join(
        re.sub(r"[^\S\n]+", " ", line).strip()
        for line in text.splitlines()
        if line.strip()
    )


def is_chrome(node: Node) -> bool:
    if node.tag in CHROME_TAGS:
        return True
    marker = f"{node.attrs.get('class', '')} {node.attrs.get('id', '')}".lower()
    return any(keyword in marker for keyword in CHROME_KEYWORDS)


def is_discussion_chrome(node: Node) -> bool:
    if node.tag in CHROME_TAGS and node.tag != "form":
        return True
    marker = f"{node.attrs.get('class', '')} {node.attrs.get('id', '')}".lower()
    return any(
        keyword in marker for keyword in CHROME_KEYWORDS if keyword != "comment"
    )


def collect_text(node: Node, drop) -> str:
    parts = []
    for index, child in enumerate(node.children):
        if isinstance(child, str):
            parts.append(child)
        elif not drop(child):
            text = collect_text(child, drop)
            if (
                drop is is_chrome
                and not normalize_text(text)
                and index + 1 < len(node.children)
                and isinstance(node.children[index + 1], Node)
                and len(normalize_text(
                    collect_text(node.children[index + 1], drop)
                )) >= 100
            ):
                visible = normalize_text(collect_text(
                    child, lambda node: node.tag in INVISIBLE_TAGS
                ))
                if len(visible) <= 60 and re.fullmatch(
                    r"[^\W\d_][\w'’.-]*(?:\s+[^\W\d_][\w'’.-]*){1,4}",
                    visible,
                ):
                    text = visible
            if child.tag in BLOCK_TAGS:
                parts.extend(("\n", text, "\n"))
            else:
                parts.append(text)
    return "".join(parts)


def raw_visible_text(root: Node) -> str:
    return normalize_text(collect_text(root, lambda node: node.tag in INVISIBLE_TAGS))


def link_text_length(node: Node, drop, inside_link: bool = False) -> int:
    total = 0
    for child in node.children:
        if isinstance(child, str):
            if inside_link:
                total += len(normalize_text(child))
        elif not drop(child):
            total += link_text_length(child, drop, inside_link or child.tag == "a")
    return total


def walk(node: Node):
    yield node
    for child in node.children:
        if isinstance(child, Node):
            yield from walk(child)


def is_discussion_container(node: Node) -> bool:
    classes = node.attrs.get("class", "").lower().split()
    marker = f"{node.attrs.get('class', '')} {node.attrs.get('id', '')}".lower()
    return "content" in classes or any(
        keyword in marker
        for keyword in (
            "comments-page", "commentarea", "comment-tree", "discussion",
            "message-list", "post-list", "postlist", "thread", "topic",
        )
    )


def discussion_content_text(root: Node) -> str:
    best_text = ""
    best_body_text = ""
    for node in walk(root):
        if (
            node.tag not in CANDIDATE_TAGS
            or is_discussion_chrome(node)
            or not is_discussion_container(node)
        ):
            continue
        text = normalize_text(collect_text(node, is_discussion_chrome))
        if node.tag == "body" and len(text) > len(best_body_text):
            best_body_text = text
        elif node.tag != "body" and len(text) > len(best_text):
            best_text = text
    return best_text or best_body_text


def main_content_text(root: Node, discussion: bool = False) -> str:
    """Highest text-density block: strip chrome, score each candidate container
    by its own text minus three times its link text, take the maximum. The best
    container sits just below the point where footer/nav links drag the score
    down, so sibling content stays and page chrome is left out."""
    drop = is_discussion_chrome if discussion else is_chrome
    best_node = None
    best_score = 0
    for node in walk(root):
        if node.tag not in CANDIDATE_TAGS or drop(node):
            continue
        text_length = len(collect_text(node, drop))
        if text_length < 200:
            continue
        score = text_length - 3 * link_text_length(node, drop)
        if score > best_score:
            best_score = score
            best_node = node
    if best_node is None:
        return ""
    return normalize_text(collect_text(best_node, drop))


def texts_from_html(markup: str, discussion: bool = False) -> tuple[str, str]:
    builder = DomBuilder()
    builder.feed(markup)
    builder.close()
    raw = raw_visible_text(builder.root)
    discussion_text = discussion_content_text(builder.root) if discussion else ""
    cleaned = discussion_text or main_content_text(builder.root, discussion) or raw
    if discussion and len(cleaned) * 2 < len(raw):
        cleaned = raw
    return raw, cleaned


# --- fetching ---------------------------------------------------------------


def valid_url(url: str) -> bool:
    try:
        parsed = urlparse(url)
    except ValueError:
        return False
    return (
        parsed.scheme in {"http", "https"}
        and bool(parsed.hostname)
        and parsed.username is None
        and parsed.password is None
    )


def is_discussion_url(url: str) -> bool:
    parsed = urlparse(url)
    host = (parsed.hostname or "").lower()
    path = parsed.path.lower()
    return (
        (host == "news.ycombinator.com" and path == "/item")
        or (host.endswith("reddit.com") and "/comments/" in path)
        or any(
            marker in path
            for marker in (
                "/discussion/", "/forum/", "/forums/", "/showthread",
                "/t/", "/thread/", "/threads/", "/topic/",
            )
        )
    )


def content_is_readable(
    status: int, content_type: str, raw: str, cleaned: str
) -> bool:
    media_type = content_type.split(";", 1)[0].strip().lower()
    allowed = (
        media_type == "text/html"
        or media_type.startswith("text/")
        or media_type == "application/json"
        or media_type.endswith("+json")
    )
    lowered = raw.lower()
    return (
        status == 200
        and allowed
        and len(raw) >= 500
        and len(cleaned) >= 500
        and not any(marker in lowered for marker in INTERSTITIAL_MARKERS)
    )


def content_is_thin(result: tuple[int, str, str, str]) -> bool:
    status, content_type, raw, cleaned = result
    media_type = content_type.split(";", 1)[0].strip().lower()
    allowed = (
        media_type == "text/html"
        or media_type.startswith("text/")
        or media_type == "application/json"
        or media_type.endswith("+json")
    )
    return (
        status == 200
        and allowed
        and len(raw) >= 500
        and len(cleaned) < 500
        and not any(marker in raw.lower() for marker in INTERSTITIAL_MARKERS)
    )


def read_direct(url: str) -> tuple[int, str, str, str]:
    request = Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urlopen(request, timeout=TIMEOUT) as response:
            status = response.status
            content_type = response.headers.get_content_type()
            charset = response.headers.get_content_charset()
            body = response.read()
    except HTTPError as error:
        status = error.code
        content_type = error.headers.get_content_type()
        charset = error.headers.get_content_charset()
        body = error.read()
    return build_result(status, content_type, body, charset, is_discussion_url(url))


def build_result(
    status: int, content_type: str, body: bytes, charset, discussion: bool
) -> tuple[int, str, str, str]:
    markup = body.decode(charset or "utf-8", errors="replace")
    if content_type.split(";", 1)[0].strip().lower() == "text/html":
        raw, cleaned = texts_from_html(markup, discussion)
    else:
        raw = cleaned = normalize_text(markup)
    return status, content_type, raw, cleaned


def read_browser(url: str) -> tuple[int, str, str, str] | None:
    executable = shutil.which("agent-browser")
    if not executable:
        return None
    session = f"browse-{os.getpid()}-{hashlib.sha256(url.encode()).hexdigest()[:12]}"
    command = [executable, "--session", session]
    script = (
        'JSON.stringify({status:performance.getEntriesByType("navigation")[0]'
        '?.responseStatus||0,contentType:document.contentType||"",'
        'text:document.body?.innerText||""})'
    )
    try:
        opened = subprocess.run(
            [*command, "--json", "open", url],
            capture_output=True, text=True, timeout=TIMEOUT + 10, check=False,
        )
        if opened.returncode:
            return None
        rendered = subprocess.run(
            [*command, "eval", "--stdin"],
            input=script, capture_output=True, text=True,
            timeout=TIMEOUT + 10, check=False,
        )
        if rendered.returncode:
            return None
        payload = json.loads(json.loads(rendered.stdout))
        text = normalize_text(str(payload.get("text", "")))
        return (
            int(payload.get("status", 0)),
            str(payload.get("contentType", "")),
            text,
            text,
        )
    except (json.JSONDecodeError, OSError, subprocess.TimeoutExpired, ValueError):
        return None
    finally:
        try:
            subprocess.run(
                [*command, "close"],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                timeout=10, check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            pass


def unreadable_outcome(results) -> tuple[str, int, str]:
    statuses = [result[0] for result in results if result is not None]
    status = str(statuses[0]) if statuses else "-"
    if any(value in {401, 403, 429} for value in statuses):
        return "blocked", 2, status
    if not statuses or any(value in {404, 410} or value >= 500 for value in statuses):
        return "gone/network", 3, status
    return "thin/interstitial", 5, status


def is_dead(connection, url: str) -> bool:
    row = connection.execute(
        "SELECT note FROM sources WHERE url = ?", (url,)
    ).fetchone()
    return bool(row and re.search(r"(^|\n)dead \d{4}-\d{2}-\d{2}", row["note"]))


def browse(connection, url: str, want_raw: bool) -> int:
    if not valid_url(url):
        print("invalid URL: expected http(s) scheme and host", file=sys.stderr)
        return 4
    if is_dead(connection, url):
        print("dead URL", file=sys.stderr)
        return 6

    direct = None
    try:
        direct = read_direct(url)
    except (OSError, URLError, ValueError):
        pass
    served_url = url
    parsed = urlparse(url)
    if (
        parsed.hostname == "www.reddit.com"
        and direct
        and not content_is_readable(*direct)
        and not content_is_thin(direct)
        and (direct[0] in {401, 403, 429} or not direct[2])
    ):
        served_url = parsed._replace(netloc="old.reddit.com").geturl()
        old_direct = None
        try:
            old_direct = read_direct(served_url)
        except (OSError, URLError, ValueError):
            pass
        direct = old_direct
    result = (
        direct
        if direct and (content_is_readable(*direct) or content_is_thin(direct))
        else read_browser(served_url)
    )

    if result and content_is_readable(*result):
        _, _, raw, cleaned = result
        digest = hashlib.sha256(served_url.encode()).hexdigest()
        CACHE.mkdir(parents=True, exist_ok=True)
        cleaned_path = CACHE / f"{digest}.txt"
        raw_path = CACHE / f"{digest}.raw.txt"
        cleaned_path.write_text(f"{cleaned}\n", encoding="utf-8")
        raw_path.write_text(f"{raw}\n", encoding="utf-8")
        sources.record_url(connection, served_url)
        print(raw_path if want_raw else cleaned_path)
        return 0

    outcome, exit_code, status = unreadable_outcome((result, direct))
    sources.record_unreadable(connection, served_url, outcome, status)
    print(f"unreadable: {outcome} {status}", file=sys.stderr)
    return exit_code


def main() -> int:
    argument_parser = argparse.ArgumentParser(
        description="Read one page for research: fetch, clean, cache, and record."
    )
    argument_parser.add_argument("url")
    argument_parser.add_argument(
        "--raw", action="store_true", help="print the raw stripped-text path"
    )
    argument_parser.add_argument("--db", help="path to the registry database")
    arguments = argument_parser.parse_args()
    with sources.connect(sources.database_path(arguments.db)) as connection:
        sources.migrate(connection)
        return browse(connection, arguments.url, arguments.raw)


if __name__ == "__main__":
    sys.exit(main())
