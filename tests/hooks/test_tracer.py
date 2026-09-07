"""Distribution freshness contracts for scripts/tracer.py."""

import hashlib
import importlib.util
from pathlib import Path

import pytest

REPO = Path(__file__).parents[2]
SPEC = importlib.util.spec_from_file_location("tracer_sync", REPO / "scripts" / "tracer.py")
tracer_sync = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tracer_sync)
CRATE_STAMP = "8c3e50358c46b79f00f11255434efec52a62b056153500da0a87a7ff5e581ccd"


def _payload(tmp_path):
    root = tmp_path / "repo"
    source = root / "tools" / "tracer"
    mirror = root / "packages" / "claude" / "bin" / "tracer-dist" / "crate"
    source_src = source / "src"
    mirror_src = mirror / "src"
    source_src.mkdir(parents=True)
    mirror_src.mkdir(parents=True)

    source.joinpath("Cargo.toml").write_text(
        "[workspace]\nmembers = [\"xtask\"]\n\n[package]\nname = \"tracer\"\n"
    )
    source.joinpath("Cargo.lock").write_text("workspace lock\n")
    source_src.joinpath("main.rs").write_text("fn main() {}\n")
    mirror.joinpath("Cargo.toml").write_text("[package]\nname = \"tracer\"\n")
    mirror.joinpath("Cargo.lock").write_text("workspace lock\n")
    mirror_src.joinpath("main.rs").write_text("fn main() {}\n")

    bin_dir = root / "packages" / "claude" / "bin" / "tracer-dist" / "bin"
    for platform in tracer_sync.PLATFORMS:
        binary = bin_dir / platform / "trace"
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_bytes(b"trace")
    bin_dir.joinpath("source.sha256").write_text(_crate_stamp(mirror) + "\n")
    return root, source, mirror, bin_dir


def _crate_stamp(mirror):
    digest = hashlib.sha256()
    for rel in ("Cargo.lock", "Cargo.toml", "src/main.rs"):
        digest.update(rel.encode())
        digest.update(b"\0")
        digest.update((mirror / rel).read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def test_manifest_source_and_lock_changes_rebuild_the_right_distribution_step(tmp_path, monkeypatch):
    """Pins every canonical input that must not leave the shipped payload current."""
    cases = (
        (Path("Cargo.toml"), "description = \"changed\"\n", ("sync-dist", "build-bin")),
        (Path("Cargo.lock"), "changed lock\n", ("sync-dist", "build-bin")),
        (Path("src/main.rs"), "fn main() { println!(\"changed\"); }\n", ("sync-dist", "build-bin")),
    )

    for relative, contents, expected in cases:
        root, source, mirror, _ = _payload(tmp_path / relative.name)
        (source / relative).write_text(contents)
        calls = []
        monkeypatch.setattr(tracer_sync, "_xtask", lambda _, task: calls.append(task) or True)

        tracer_sync.sync(root)

        assert calls == list(expected)


@pytest.mark.parametrize(
    ("failure", "expected"),
    (
        ("missing", ["build-bin"]),
        ("interrupted", ["build-bin"]),
        ("missing-stamp", ["build-bin"]),
        ("invalid-stamp", ["build-bin"]),
        ("missing-lock", ["sync-dist", "build-bin"]),
        ("corrupt-lock", ["sync-dist", "build-bin"]),
    ),
)
def test_incomplete_prebuilt_payload_is_stale(tmp_path, monkeypatch, failure, expected):
    """Pins publication from recording a fresh stamp after incomplete output."""
    root, source, mirror, bin_dir = _payload(tmp_path)
    if failure == "missing":
        (bin_dir / tracer_sync.PLATFORMS[0] / "trace").unlink()
    elif failure == "interrupted":
        (bin_dir / "source.sha256").write_text("interrupted\n")
    elif failure == "missing-stamp":
        (bin_dir / "source.sha256").unlink()
    elif failure == "invalid-stamp":
        (bin_dir / "source.sha256").write_bytes(b"\xff\xfe\n")
    elif failure == "missing-lock":
        (mirror / "Cargo.lock").unlink()
    else:
        (mirror / "Cargo.lock").write_text("corrupted\n")

    calls = []
    monkeypatch.setattr(tracer_sync, "_xtask", lambda _, task: calls.append(task) or True)

    tracer_sync.sync(root)

    assert calls == expected


def test_unchanged_valid_payload_needs_no_cargo_work(tmp_path, monkeypatch):
    """Pins the normal pre-commit path to a filesystem and hash-only check."""
    root, source, mirror, _ = _payload(tmp_path)
    calls = []
    monkeypatch.setattr(tracer_sync, "_xtask", lambda _, task: calls.append(task) or True)

    tracer_sync.sync(root)

    assert calls == []


def test_python_stamp_matches_the_xtask_contract(tmp_path):
    """Pins the path-and-byte framing shared with xtask's crate_stamp producer."""
    _, source, mirror, _ = _payload(tmp_path)
    assert _crate_stamp(mirror) == CRATE_STAMP
    assert tracer_sync._crate_stamp(str(mirror)) == CRATE_STAMP
    assert tracer_sync._standalone_manifest((source / "Cargo.toml").read_bytes()) == (
        mirror / "Cargo.toml"
    ).read_bytes()
