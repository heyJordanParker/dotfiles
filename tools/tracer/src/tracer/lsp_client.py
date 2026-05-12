"""multilspy wrapper — sync facade over multilspy's SyncLanguageServer.

multilspy ships its own SyncLanguageServer that runs the async LSP client on
a background event loop and exposes blocking methods. We use that directly —
no manual asyncio.run, no async-to-sync bridging.

Files must be opened (`server.open_file(...)`) before any request to that
file. Paths passed to multilspy are relative to the repo root.
"""

from __future__ import annotations

import os
import subprocess
import sys
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator


def _ensure_venv_on_path() -> None:
    """Prepend our Python venv's bin/ to PATH so multilspy can spawn its LSP
    servers (jedi-language-server, typescript-language-server, etc.).

    pipx shims execute the venv's Python directly but do NOT augment PATH
    for child processes. multilspy spawns LSP servers via PATH lookup
    (`subprocess.Popen("jedi-language-server", ...)`), which silently fails
    when the binary isn't on PATH — the spawn returns, but the LSP IPC
    blocks forever waiting for handshake messages that will never come.
    Prepending sys.executable's dir fixes both pipx-installed and
    development (editable) invocations.
    """
    # Don't .resolve() — pipx venv pythons are symlinks to the system Python,
    # and resolving leads us to the system bin/ instead of the venv bin/ where
    # jedi-language-server (and other LSP servers) actually live.
    bin_dir = str(Path(sys.executable).parent)
    current = os.environ.get("PATH", "")
    if bin_dir not in current.split(os.pathsep):
        os.environ["PATH"] = bin_dir + os.pathsep + current


def repo_root_for(path: str | Path) -> Path:
    p = Path(path).resolve()
    start = p.parent if p.is_file() else p
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=start,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        return Path(result.stdout.strip())
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return start


@contextmanager
def _server_for(file_path: str | Path, lsp_language: str, timeout: int = 120) -> Iterator[tuple[Any, Path, str]]:
    """Yield (sync_server, repo_root, relative_path) with the file opened.

    Encapsulates SyncLanguageServer create + start + open_file so commands
    don't repeat the dance. multilspy auto-downloads the language server
    binary on first use of each language; that download adds latency the
    first time but is one-shot per machine.
    """
    _ensure_venv_on_path()

    # multilspy imports psutil, which has a C extension (_psutil_osx.so on
    # macOS). C extensions can't load from inside a Python zipapp — so when
    # tracer is invoked via the bundled plugin binary, LSP commands fail at
    # import. Surface a clear error pointing to the install path that works.
    try:
        from multilspy import SyncLanguageServer
        from multilspy.multilspy_config import MultilspyConfig
        from multilspy.multilspy_logger import MultilspyLogger
    except ImportError as e:
        raise SystemExit(
            "LSP commands need a real Python install of tracer (the bundled "
            "zipapp can't load multilspy's C extension dependencies).\n"
            "Run: pipx install tracer\n"
            f"(import error: {e})"
        )

    repo = repo_root_for(file_path)
    abs_path = Path(file_path).resolve()
    rel = str(abs_path.relative_to(repo))

    config = MultilspyConfig.from_dict({"code_language": lsp_language})
    logger = MultilspyLogger()
    server = SyncLanguageServer.create(config, logger, str(repo), timeout=timeout)
    with server.start_server():
        with server.open_file(rel):
            yield server, repo, rel


def find_references(file: str, line: int, col: int, lsp_language: str) -> list[dict[str, Any]]:
    """LSP textDocument/references.

    Returns [] when the language server returns null (no references found
    or the position is not a resolvable symbol). multilspy raises
    AssertionError for null responses; we treat that as empty result, which
    matches LSP semantics — null means "couldn't compute," not "error."
    """
    with _server_for(file, lsp_language) as (server, _repo, rel):
        try:
            result = server.request_references(rel, line, col)
        except AssertionError:
            return []
        return result or []


def find_definition(file: str, line: int, col: int, lsp_language: str) -> list[dict[str, Any]]:
    """LSP textDocument/definition. Empty list on null response."""
    with _server_for(file, lsp_language) as (server, _repo, rel):
        try:
            result = server.request_definition(rel, line, col)
        except AssertionError:
            return []
        return result or []


def document_symbols(file: str, lsp_language: str) -> list[dict[str, Any]]:
    """LSP textDocument/documentSymbol — returns flat symbol list (not the tree).

    multilspy returns Tuple[List[UnifiedSymbolInformation], Union[List[TreeRepr], None]];
    we return the flat list. The tree representation is sometimes None depending
    on the language server, so callers can't depend on it.
    """
    with _server_for(file, lsp_language) as (server, _repo, rel):
        symbols, _tree = server.request_document_symbols(rel)
        return symbols


def resolve_symbol_position(symbol: str, repo_root: Path) -> tuple[Path, int, int] | None:
    """Find a symbol's most LSP-resolvable position in the repo via ripgrep.

    Returns (file, line, col) — both 0-indexed (LSP convention).

    Strategy: prefer matches that look like definitions (`def <symbol>`,
    `class <symbol>`, `function <symbol>`, `<symbol> =`) — LSP servers
    resolve those most reliably. Fall back to any match.
    """
    import json

    try:
        result = subprocess.run(
            ["rg", "--json", "-w", symbol, str(repo_root)],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None

    definition_hits: list[tuple[Path, int, int]] = []
    fallback_hits: list[tuple[Path, int, int]] = []

    for line in result.stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") != "match":
            continue
        data = event["data"]
        path = data["path"]["text"]
        line_num = data["line_number"] - 1
        match_text = data["lines"]["text"]
        col = match_text.find(symbol)
        if col == -1:
            col = 0
        hit = (Path(path), line_num, col)

        looks_like_def = (
            f"def {symbol}" in match_text
            or f"class {symbol}" in match_text
            or f"function {symbol}" in match_text
            or f"const {symbol}" in match_text
            or f"let {symbol}" in match_text
            or f"var {symbol}" in match_text
        )
        if looks_like_def:
            definition_hits.append(hit)
        else:
            fallback_hits.append(hit)

    if definition_hits:
        return definition_hits[0]
    if fallback_hits:
        return fallback_hits[0]
    return None
