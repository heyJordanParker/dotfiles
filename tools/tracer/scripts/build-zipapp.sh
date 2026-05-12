#!/usr/bin/env bash
# Build the tracer zipapp and commit it to the plugin's bin/.
#
# Run from anywhere; resolves repo root via git. Bundles tracer + its
# pure-Python deps (click, lizard, multilspy) into a single executable
# zipapp at packages/claude/bin/trace. Plugin users get this on PATH
# automatically when the plugin is enabled.

set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOL_DIR="$REPO_ROOT/tools/tracer"
OUT_DIR="$REPO_ROOT/packages/claude/bin"
OUT_FILE="$OUT_DIR/trace"
BUILD_DIR="$(mktemp -d)"

trap 'rm -rf "$BUILD_DIR"' EXIT

echo "==> Building tracer zipapp"
echo "    source : $TOOL_DIR"
echo "    output : $OUT_FILE"

mkdir -p "$OUT_DIR"

# Install package + pure-Python deps into the build dir
python3 -m pip install \
  --target "$BUILD_DIR" \
  --no-compile \
  --quiet \
  "$TOOL_DIR" \
  click \
  lizard \
  multilspy

# Build the zipapp
python3 -m zipapp \
  "$BUILD_DIR" \
  --main "tracer.__main__:main" \
  --python "/usr/bin/env python3" \
  --output "$OUT_FILE" \
  --compress

chmod +x "$OUT_FILE"

echo "==> Built $(du -h "$OUT_FILE" | cut -f1)  →  $OUT_FILE"
