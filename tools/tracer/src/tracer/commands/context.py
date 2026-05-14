"""`trace context` — two modes off one command.

No-args: session-start primer. Eight sections covering environment, repo
identity, tech stack, top-level layout, content-classified common
directories, git state, project rules, and architectural spine. First
invocation warms the file and architecture caches.

File mode: `trace context <path>` — single-file enrichment for the
PreToolUse Read hook. One passive_context line plus graph counts.

Glob-mode enrichment moved to `trace glob --details`, which returns the
complete deterministically-sorted match list (the per-match enrichment
shape this command used to emit, uncapped and sortable).

What this command intentionally OMITS — the native Read/Glob path already
loads it via Claude Code's harness-level Claude.md walk:
- `CLAUDE.md` / `.claude/CLAUDE.md` / `CLAUDE.local.md` ancestors
- `.claude/rules/` matches (unconditional + path-conditional)
- `@include` references
"""

from __future__ import annotations

import os
import platform as _platform
import re
import subprocess
from collections import Counter
from datetime import datetime, timedelta, timezone
from pathlib import Path

import click

from tracer import architecture, cache, file_facts, git_activity, passive_context, repo_files
from tracer.deps import require_dependencies
from tracer.extraction.dispatch import supported_extensions
from tracer.repo_context import repo_context

# Re-export so existing references in this module continue to resolve.
SKIP_DIRS = repo_files.SKIP_DIRS

# Primer caps — output stays short but comprehensive.
PRIMER_LANGUAGE_LIMIT = 10
PRIMER_DIRTY_LIMIT = 10
PRIMER_COMMIT_LIMIT = 10
PRIMER_SPINE_LIMIT = 10
PRIMER_SPINE_DEPTH = 3

# Branches whose prefixes mark them as feature/topic work; filtered out of
# the primary-branch candidate list.
FEATURE_PREFIXES = (
    "feat/", "feature/", "fix/", "bugfix/", "hotfix/", "chore/",
    "refactor/", "docs/", "test/", "tests/", "ci/", "build/",
    "wip/", "experiment/", "spike/", "release/",
)

# Stale-branch cutoff for the primary-branch candidate list.
PRIMER_BRANCH_STALE_DAYS = 21

# Package-config files at repo root → (manager label, language family).
PACKAGE_CONFIGS = {
    "package.json": ("npm/node", "javascript"),
    "package-lock.json": ("npm", "javascript"),
    "yarn.lock": ("yarn", "javascript"),
    "pnpm-lock.yaml": ("pnpm", "javascript"),
    "bun.lock": ("bun", "javascript"),
    "bun.lockb": ("bun", "javascript"),
    "composer.json": ("composer", "php"),
    "composer.lock": ("composer", "php"),
    "Gemfile": ("bundler", "ruby"),
    "Gemfile.lock": ("bundler", "ruby"),
    "Cargo.toml": ("cargo", "rust"),
    "Cargo.lock": ("cargo", "rust"),
    "pyproject.toml": ("pip/poetry/hatch", "python"),
    "requirements.txt": ("pip", "python"),
    "Pipfile": ("pipenv", "python"),
    "Pipfile.lock": ("pipenv", "python"),
    "go.mod": ("go modules", "go"),
    "go.sum": ("go modules", "go"),
    "Brewfile": ("homebrew", "macos"),
    "build.gradle": ("gradle", "jvm"),
    "build.gradle.kts": ("gradle", "jvm"),
    "pom.xml": ("maven", "jvm"),
    "Package.swift": ("swift package manager", "swift"),
    "mix.exs": ("mix", "elixir"),
    "deno.json": ("deno", "javascript"),
}

# Build / test / lint / orchestration configs detected at repo root.
TOOL_CONFIGS = (
    "vite.config.js", "vite.config.ts",
    "webpack.config.js", "webpack.config.ts",
    "rollup.config.js", "rollup.config.ts",
    "esbuild.config.js", "esbuild.config.ts",
    "tsconfig.json", "jsconfig.json",
    "playwright.config.ts", "playwright.config.js",
    "vitest.config.ts", "vitest.config.js",
    "jest.config.js", "jest.config.ts",
    "phpunit.xml", "phpunit.xml.dist",
    "pytest.ini", "tox.ini",
    "biome.json", ".eslintrc.json", ".eslintrc.js",
    "prettier.config.js", ".prettierrc",
    "pint.json", ".rubocop.yml",
    "Dockerfile", "docker-compose.yml", "compose.yaml", "compose.yml",
    ".lando.yml", "wp-cli.yml",
    "Makefile",
)

# Continuous-integration markers — file or directory existence is the
# signal. (path, label) tuples.
CI_MARKERS = (
    (".github/workflows", "GitHub Actions"),
    (".gitlab-ci.yml", "GitLab CI"),
    (".circleci/config.yml", "CircleCI"),
    ("Jenkinsfile", "Jenkins"),
    ("azure-pipelines.yml", "Azure Pipelines"),
    ("bitbucket-pipelines.yml", "Bitbucket Pipelines"),
    (".drone.yml", "Drone"),
    (".travis.yml", "Travis"),
)

# Test-framework config filenames inside a directory mark it as a test dir.
TEST_CONFIG_NAMES = {
    "phpunit.xml", "phpunit.xml.dist", "pytest.ini", "tox.ini",
    "vitest.config.ts", "vitest.config.js",
    "jest.config.js", "jest.config.ts",
    "playwright.config.ts", "playwright.config.js",
}

# Common-directory category order for output.
COMMON_KINDS = (
    "frontend",
    "backend",
    "database-migrations",
    "tests",
    "scripts",
    "continuous-integration",
)


@click.command()
@click.argument("path", type=click.Path(), required=False)
@click.option("--directory", "force_directory", is_flag=True,
              help="Force directory mode (no per-file enrichment available).")
def command(path: str | None, force_directory: bool) -> None:
    """Tracer context: primer (no args) or file enrichment (path)."""
    require_dependencies()

    if not path:
        if force_directory:
            return
        _primer_mode()
        return

    p = Path(path).resolve()
    if not p.exists():
        return

    if force_directory or p.is_dir():
        return

    facts = file_facts.get(p)
    if not facts:
        return

    repo_root = cache.repo_root_for(p)
    graph_counts = _graph_counts(p, repo_root)
    context_line = passive_context.render(facts, graph=graph_counts)
    if context_line:
        click.echo(context_line)


# ---------------------------------------------------------------------------
# File enrichment helper.
# ---------------------------------------------------------------------------


def _graph_counts(file_path: Path, repo_root: Path) -> dict | None:
    """Architecture-graph caller/dependent counts (mirrors read.py helper)."""
    try:
        graph = architecture.load_cached(repo_root)
    except Exception:
        return None
    if graph is None:
        return None
    try:
        relative = str(file_path.relative_to(repo_root.resolve()))
    except ValueError:
        return None
    module_id = graph.file_to_module_id.get(relative)
    if not module_id:
        return None
    callers = len(architecture.dependents_of(graph, module_id))
    deps = len(architecture.dependencies_of(graph, module_id))
    return {"callers": callers, "depended_on_by_modules": deps}


# ---------------------------------------------------------------------------
# Primer mode (no args).
# ---------------------------------------------------------------------------


def _primer_mode() -> None:
    """Emit the session-start primer. Warms caches synchronously on first
    invocation — the primer's whole purpose is to show rich repo data, so
    blocking on the build is correct here. Orientation commands (`list`,
    `tree`, `info`) use `file_facts.get(cache_only=True)` which returns
    lite facts from scc + git_activity on cache miss — no extraction
    triggered, no warmup needed."""
    repo_root = cache.repo_root_for(".")
    try:
        architecture.get(repo_root=repo_root)
    except Exception:
        pass

    # Single repo-wide file discovery. Used by layout and rules sections —
    # one git ls-files call shared across every section that needs the
    # repo's file list.
    tracked = repo_files.tracked_files(repo_root) or []

    _emit_environment(repo_root)
    click.echo("")
    _emit_identity(repo_root)
    click.echo("")
    _emit_tech_stack(repo_root)
    click.echo("")
    _emit_layout(repo_root, tracked)
    click.echo("")
    _emit_common_directories(repo_root)
    click.echo("")
    _emit_git(repo_root)
    click.echo("")
    _emit_rules(repo_root, tracked)
    click.echo("")
    _emit_spine(repo_root)

    ctx = repo_context(str(repo_root))
    click.echo("")
    click.echo(
        f"repo_context: complexity_p95={ctx['complexity_p95']} "
        f"median={ctx['median_file_ccn']} files={ctx['total_files']}"
    )


# ---------------------------------------------------------------------------
# Section: Environment.
# ---------------------------------------------------------------------------


def _emit_environment(repo_root: Path) -> None:
    cwd = os.getcwd()
    is_git_repo = (repo_root / ".git").exists()
    is_worktree = _is_worktree(repo_root)
    shell = os.environ.get("SHELL", "").rsplit("/", 1)[-1] or "(unknown)"
    git_user = _git_user(repo_root)

    click.echo("## Environment")
    click.echo(f"  cwd: {cwd}")
    click.echo(f"  repo root: {repo_root}")
    click.echo(f"  git repository: {'yes' if is_git_repo else 'no'}")
    click.echo(f"  worktree: {'yes' if is_worktree else 'no'}")
    click.echo(f"  platform: {_platform.system().lower()}")
    click.echo(f"  shell: {shell}")
    click.echo(f"  os version: {_platform.system()} {_platform.release()}")
    click.echo(f"  git user: {git_user}")
    click.echo(f"  date: {datetime.now().strftime('%Y-%m-%d')}")


def _is_worktree(repo_root: Path) -> bool:
    """A linked worktree has a `.git` file (not directory) that points into
    the parent repo's `worktrees/<name>/` subdirectory."""
    git_path = repo_root / ".git"
    if not git_path.is_file():
        return False
    try:
        contents = git_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return False
    return "worktrees/" in contents


def _git_user(repo_root: Path) -> str:
    name = _git_config(repo_root, "user.name")
    email = _git_config(repo_root, "user.email")
    if name and email:
        return f"{name} <{email}>"
    return name or email or "(unset)"


def _git_config(repo_root: Path, key: str) -> str:
    try:
        result = subprocess.run(
            ["git", "config", "--get", key],
            cwd=repo_root, capture_output=True, text=True, timeout=5,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return ""
    return result.stdout.strip()


# ---------------------------------------------------------------------------
# Section: Identity.
# ---------------------------------------------------------------------------


def _emit_identity(repo_root: Path) -> None:
    languages = _scc_languages(repo_root)
    if not languages:
        click.echo("## Identity")
        click.echo("  (scc unavailable or empty result)")
        return

    total_files = sum(lang.get("Count", 0) for lang in languages)
    total_loc = sum(lang.get("Code", 0) for lang in languages)
    languages_sorted = sorted(languages, key=lambda x: -x.get("Code", 0))

    click.echo("## Identity")
    click.echo(f"  Files: {total_files}  Lines of code: {total_loc}")
    click.echo("  Languages:")
    for lang in languages_sorted[:PRIMER_LANGUAGE_LIMIT]:
        name = lang.get("Name", "?")
        count = lang.get("Count", 0)
        code = lang.get("Code", 0)
        complexity = lang.get("Complexity", 0)
        click.echo(
            f"    {name:<20} files={count:<5} loc={code:<8} complexity={complexity}"
        )
    extra = len(languages_sorted) - PRIMER_LANGUAGE_LIMIT
    if extra > 0:
        click.echo(f"    … {extra} more languages")


def _scc_languages(repo_root: Path) -> list[dict]:
    try:
        result = subprocess.run(
            ["scc", "--format", "json", str(repo_root)],
            capture_output=True, text=True, check=True, timeout=60,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return []
    import json as _json
    try:
        return _json.loads(result.stdout)
    except _json.JSONDecodeError:
        return []


# ---------------------------------------------------------------------------
# Section: Tech Stack.
# ---------------------------------------------------------------------------


def _emit_tech_stack(repo_root: Path) -> None:
    managers: dict[str, list[str]] = {}
    for filename, (manager, _stack) in PACKAGE_CONFIGS.items():
        if (repo_root / filename).exists():
            managers.setdefault(manager, []).append(filename)

    configs: list[str] = []
    for filename in TOOL_CONFIGS:
        if (repo_root / filename).exists():
            configs.append(filename)

    click.echo("## Tech Stack")
    if managers:
        click.echo("  Package managers:")
        for manager, files in sorted(managers.items()):
            click.echo(f"    {manager}: {', '.join(files)}")
    if configs:
        click.echo("  Build / test / lint configs:")
        for config in configs:
            click.echo(f"    {config}")
    if not managers and not configs:
        click.echo("  (no package or tool configs detected at repo root)")


# ---------------------------------------------------------------------------
# Section: Layout.
# ---------------------------------------------------------------------------


def _emit_layout(repo_root: Path, tracked: list[str]) -> None:
    """Top-level directory rollup. Partitions the pre-discovered file list
    by first path segment — one pass, no per-dir filesystem walks."""
    by_top_dir: dict[str, list[str]] = {}
    for rel in tracked:
        head, sep, _ = rel.partition("/")
        if not sep:
            continue  # file at repo root, not under a top-level dir
        if head.startswith(".") or head in SKIP_DIRS:
            continue
        by_top_dir.setdefault(head, []).append(rel)

    click.echo("## Layout")
    if not by_top_dir:
        click.echo("  (no source directories at top level)")
        return

    # Compute the git activity map and the supported-extensions set once;
    # both stay constant across every top-level directory aggregation.
    source_exts = supported_extensions()
    git_map = git_activity.bulk_cached(repo_root)

    for name in sorted(by_top_dir, key=str.lower):
        summary = repo_files.aggregate_paths(repo_root, by_top_dir[name], source_exts, git_map)
        bits = [f"{summary['file_count']} files", f"ccn={summary['ccn_total']}"]
        if summary["last_modified"]:
            bits.append(f"last: {summary['last_modified']}")
        if summary["has_uncommitted"]:
            bits.append("uncommitted")
        click.echo(f"  📁 {name}/  ({' · '.join(bits)})")


# ---------------------------------------------------------------------------
# Section: Common Directories (content-based detection).
# ---------------------------------------------------------------------------


def _emit_common_directories(repo_root: Path) -> None:
    classifications: dict[str, list[tuple[str, str]]] = {k: [] for k in COMMON_KINDS}

    # CI markers: existence at known paths.
    for marker, label in CI_MARKERS:
        if (repo_root / marker).exists():
            classifications["continuous-integration"].append((marker, label))

    # Walk top-level + one level deeper for nested patterns like
    # `database/migrations/`, `app/tests/`, etc.
    candidate_dirs: list[Path] = []
    for child in repo_root.iterdir():
        if not child.is_dir():
            continue
        if child.name.startswith(".") or child.name in SKIP_DIRS:
            continue
        candidate_dirs.append(child)
        try:
            for sub in child.iterdir():
                if not sub.is_dir():
                    continue
                if sub.name.startswith(".") or sub.name in SKIP_DIRS:
                    continue
                candidate_dirs.append(sub)
        except (OSError, PermissionError):
            continue

    seen: set[tuple[str, str]] = set()
    for d in candidate_dirs:
        try:
            rel = str(d.relative_to(repo_root))
        except ValueError:
            continue
        for kind, marker in _classify_directory(d):
            key = (kind, rel)
            if key in seen:
                continue
            seen.add(key)
            classifications[kind].append((rel, marker))

    click.echo("## Common Directories")
    any_found = False
    for kind in COMMON_KINDS:
        entries = classifications[kind]
        if not entries:
            continue
        any_found = True
        click.echo(f"  {kind}:")
        for path, marker in entries:
            click.echo(f"    {path}  ({marker})")
    if not any_found:
        click.echo("  (no common directories detected)")


def _classify_directory(directory: Path) -> list[tuple[str, str]]:
    """Return zero or more (kind, marker) tags for a directory's contents."""
    try:
        entries = list(directory.iterdir())
    except (OSError, PermissionError):
        return []

    file_names: list[str] = []
    extension_counts: Counter[str] = Counter()
    for entry in entries:
        if entry.is_file():
            file_names.append(entry.name)
            extension_counts[entry.suffix.lower()] += 1

    labels: list[tuple[str, str]] = []

    # Frontend: tsx/jsx/vue/svelte content.
    frontend_count = (
        extension_counts.get(".tsx", 0)
        + extension_counts.get(".jsx", 0)
        + extension_counts.get(".vue", 0)
        + extension_counts.get(".svelte", 0)
    )
    if frontend_count >= 3:
        labels.append(("frontend", f"{frontend_count} tsx/jsx/vue/svelte files"))

    # Backend: framework markers + bulk-of-backend-language files.
    backend_markers: list[str] = []
    name_set = set(file_names)
    if "artisan" in name_set:
        backend_markers.append("Laravel")
    if {"manage.py", "wsgi.py", "asgi.py"} & name_set:
        backend_markers.append("Django/Flask")
    if "config.ru" in name_set or (
        "Gemfile" in name_set and (directory / "config" / "application.rb").exists()
    ):
        backend_markers.append("Rails")
    php_count = extension_counts.get(".php", 0)
    if php_count >= 3 and any(
        n.endswith("Controller.php") or n.endswith("Model.php") or n.endswith("Service.php")
        for n in file_names
    ):
        backend_markers.append(f"{php_count} PHP files (controller/model/service)")
    if backend_markers:
        labels.append(("backend", ", ".join(backend_markers)))

    # Database migrations: timestamp-prefixed files (Laravel, Rails, Knex,
    # Alembic conventions all share this shape).
    timestamp_prefixed = sum(
        1 for n in file_names if re.match(r"^\d{4}[_-]\d{2}[_-]\d{2}", n)
    )
    if timestamp_prefixed >= 2:
        labels.append(
            ("database-migrations", f"{timestamp_prefixed} timestamp-prefixed files")
        )

    # Tests: framework config presence or many test-shaped filenames.
    test_configs = [n for n in file_names if n in TEST_CONFIG_NAMES]
    if test_configs:
        labels.append(("tests", f"config: {', '.join(test_configs)}"))
    else:
        test_count = sum(
            1 for n in file_names
            if "_test." in n
            or ".test." in n
            or n.startswith("test_")
            or n.endswith("Test.php")
            or n.endswith("Spec.php")
        )
        if test_count >= 2:
            labels.append(("tests", f"{test_count} test files"))

    # Scripts: shell file count, then fall back to shebangs on extensionless
    # executables.
    shell_count = sum(1 for n in file_names if n.endswith((".sh", ".bash", ".zsh")))
    if shell_count >= 2:
        labels.append(("scripts", f"{shell_count} shell scripts"))
    else:
        shebang_count = _count_shebangs(directory, entries)
        if shebang_count >= 2:
            labels.append(("scripts", f"{shebang_count} shebang scripts"))

    return labels


def _count_shebangs(directory: Path, entries: list[Path]) -> int:
    """Count extensionless executables in `directory` that start with `#!`."""
    count = 0
    for entry in entries:
        if not entry.is_file():
            continue
        if entry.suffix:
            continue
        try:
            with open(entry, "rb") as handle:
                head = handle.read(2)
        except OSError:
            continue
        if head == b"#!":
            count += 1
    return count


# ---------------------------------------------------------------------------
# Section: Git.
# ---------------------------------------------------------------------------


def _emit_git(repo_root: Path) -> None:
    click.echo("## Git")
    if not (repo_root / ".git").exists():
        click.echo("  (not a git repository)")
        return

    origin_head = _origin_head_branch(repo_root)
    candidates = _primary_branch_candidates(repo_root, origin_head)
    if candidates:
        click.echo("  Primary branch candidates:")
        for name, info_text in candidates:
            click.echo(f"    {name}  ({info_text})")

    current = _current_branch(repo_root)
    ahead_behind = _ahead_behind(repo_root, current, origin_head)
    suffix = f"  ({ahead_behind})" if ahead_behind else ""
    click.echo(f"  Current branch: {current}{suffix}")

    dirty = _working_tree_state_safe(repo_root)
    if dirty:
        click.echo(f"  Dirty files ({len(dirty)}):")
        for line in _render_dirty(repo_root, dirty):
            click.echo(f"    {line}")
    else:
        click.echo("  Working tree clean")

    commits = _recent_commit_subjects(repo_root, PRIMER_COMMIT_LIMIT)
    if commits:
        click.echo(f"  Recent commits ({len(commits)}):")
        for line in commits:
            click.echo(f"    {line}")


def _primary_branch_candidates(
    repo_root: Path, origin_head: str | None
) -> list[tuple[str, str]]:
    """List long-lived branch candidates after filtering features and stale."""
    try:
        result = subprocess.run(
            ["git", "for-each-ref",
             "--format=%(refname:short)\t%(committerdate:iso8601)",
             "refs/heads/", "refs/remotes/origin/"],
            cwd=repo_root, capture_output=True, text=True, check=True, timeout=10,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return []

    cutoff = datetime.now(timezone.utc) - timedelta(days=PRIMER_BRANCH_STALE_DAYS)

    seen_short: set[str] = set()
    candidates: list[tuple[str, str, bool]] = []
    for line in result.stdout.splitlines():
        parts = line.split("\t", 1)
        if len(parts) != 2:
            continue
        raw_name, date_text = parts[0], parts[1].strip()
        short = raw_name[len("origin/"):] if raw_name.startswith("origin/") else raw_name
        if short == "HEAD" or short in seen_short:
            continue
        is_feature = any(short.startswith(prefix) for prefix in FEATURE_PREFIXES)
        is_origin_head = (short == origin_head)
        if is_feature and not is_origin_head:
            continue
        branch_dt = _parse_iso(date_text)
        is_stale = branch_dt is not None and branch_dt < cutoff
        if is_stale and not is_origin_head:
            continue
        seen_short.add(short)
        date_short = date_text.split()[0] if date_text else "(unknown)"
        marker = " [origin/HEAD]" if is_origin_head else ""
        candidates.append((short, f"last: {date_short}{marker}", is_origin_head))

    candidates.sort(key=lambda entry: (0 if entry[2] else 1, entry[0]))
    return [(name, info) for name, info, _ in candidates]


def _origin_head_branch(repo_root: Path) -> str | None:
    try:
        result = subprocess.run(
            ["git", "symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
            cwd=repo_root, capture_output=True, text=True, check=True, timeout=5,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return None
    ref = result.stdout.strip()
    return ref[len("origin/"):] if ref.startswith("origin/") else (ref or None)


def _current_branch(repo_root: Path) -> str:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            cwd=repo_root, capture_output=True, text=True, check=True, timeout=5,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return "(unknown)"
    return result.stdout.strip() or "(detached)"


def _ahead_behind(repo_root: Path, current: str, base: str | None) -> str:
    if not base or current in {base, "(unknown)", "(detached)"}:
        return ""
    try:
        result = subprocess.run(
            ["git", "rev-list", "--left-right", "--count", f"origin/{base}...HEAD"],
            cwd=repo_root, capture_output=True, text=True, check=True, timeout=5,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return ""
    parts = result.stdout.strip().split()
    if len(parts) != 2:
        return ""
    behind, ahead = parts
    return f"ahead: {ahead}, behind: {behind} vs origin/{base}"


def _working_tree_state_safe(repo_root: Path) -> dict[str, str]:
    try:
        return git_activity._working_tree_state(repo_root)
    except Exception:
        return {}


def _render_dirty(repo_root: Path, dirty: dict[str, str]) -> list[str]:
    """Render dirty files sorted by blast radius (callers desc, then ccn desc)."""
    graph = architecture.load_cached(repo_root)
    scored: list[tuple[int, int, str, str]] = []
    for path, state in dirty.items():
        abs_path = repo_root / path
        facts = file_facts.get(abs_path, repo_root=repo_root) if abs_path.exists() else None
        callers = 0
        if graph is not None:
            module_id = graph.file_to_module_id.get(path)
            if module_id:
                callers = len(architecture.dependents_of(graph, module_id))
        ccn = facts.cyclomatic_complexity_total if facts else 0
        scored.append((callers, ccn, state, path))
    scored.sort(key=lambda entry: (-entry[0], -entry[1]))
    lines: list[str] = []
    for callers, ccn, state, path in scored[:PRIMER_DIRTY_LIMIT]:
        lines.append(f"{state:<10} {path}  (callers={callers}, ccn={ccn})")
    if len(scored) > PRIMER_DIRTY_LIMIT:
        lines.append(f"… {len(scored) - PRIMER_DIRTY_LIMIT} more")
    return lines


def _recent_commit_subjects(repo_root: Path, limit: int) -> list[str]:
    try:
        result = subprocess.run(
            ["git", "log", f"-n{limit}", "--pretty=format:%h %s"],
            cwd=repo_root, capture_output=True, text=True, check=True, timeout=10,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return []
    return [line for line in result.stdout.splitlines() if line.strip()]


def _parse_iso(text: str) -> datetime | None:
    if not text:
        return None
    cleaned = text.strip().replace(" ", "T", 1)
    try:
        dt = datetime.fromisoformat(cleaned)
    except ValueError:
        return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt


# ---------------------------------------------------------------------------
# Section: Rules.
# ---------------------------------------------------------------------------


def _emit_rules(repo_root: Path, tracked: list[str]) -> None:
    claude_md = _collect_claude_md(tracked)
    rules_files = _collect_rules_dir(tracked)

    click.echo("## Rules")
    if claude_md:
        click.echo(f"  Claude.md files ({len(claude_md)}):")
        for rel in claude_md:
            click.echo(f"    {rel}")
    if rules_files:
        click.echo(f"  Project rules ({len(rules_files)}):")
        for rel in rules_files:
            click.echo(f"    {rel}")
    if not claude_md and not rules_files:
        click.echo("  (no Claude.md or .claude/rules/ found)")


def _collect_claude_md(tracked: list[str]) -> list[str]:
    """Filter the tracked file list to Claude.md / CLAUDE.md entries. Dedupe
    by case-folded path because case-insensitive filesystems (APFS, HFS+)
    can surface the same physical file under either casing."""
    by_key: dict[str, str] = {}
    for rel in tracked:
        if any(part.startswith(".") or part in SKIP_DIRS for part in rel.split("/")):
            continue
        basename = rel.rsplit("/", 1)[-1]
        if basename.casefold() != "claude.md":
            continue
        by_key.setdefault(rel.casefold(), rel)
    return sorted(by_key.values())


def _collect_rules_dir(tracked: list[str]) -> list[str]:
    """Filter the tracked file list to .claude/rules/*.md entries."""
    prefix = ".claude/rules/"
    return sorted(rel for rel in tracked if rel.startswith(prefix) and rel.endswith(".md"))


# ---------------------------------------------------------------------------
# Section: Spine (architectural centrality).
# ---------------------------------------------------------------------------


def _emit_spine(repo_root: Path) -> None:
    """Top-N most-depended-on nodes ranked by direct dependent count.

    Direct count alone is a reliable load-bearing signal for the primer —
    the heavier transitive-walk ranking (BFS to depth 3 over 30 candidates)
    is reserved for `trace downstream --path <path>`, where the agent has
    explicitly asked for architectural centrality.
    """
    click.echo("## Spine")
    try:
        graph = architecture.load_cached(repo_root)
    except Exception:
        graph = None
    if graph is None or not graph.edges:
        click.echo("  (architecture graph empty — run `trace cache build` if you expect data)")
        return

    incoming = Counter(edge.target for edge in graph.edges)
    ranked: list[tuple[str, int]] = [
        (node_id, count)
        for node_id, count in incoming.most_common()
        if not node_id.startswith("module::external::")
    ][:PRIMER_SPINE_LIMIT]

    if not ranked:
        click.echo("  (no internal nodes in the architecture graph)")
        return

    click.echo(f"  Top {len(ranked)} most-depended-on nodes:")
    click.echo(f"    {'#':<3} {'direct':>6}  {'kind':<10} symbol @ source")
    for rank, (node_id, direct_count) in enumerate(ranked, 1):
        node = graph.nodes.get(node_id)
        if node is None:
            continue
        location = (
            f"{node.source_file}:{node.source_line}" if node.source_file else "(no source)"
        )
        click.echo(
            f"    {rank:<3} {direct_count:>6}  {node.kind:<10} {node.label} @ {location}"
        )
