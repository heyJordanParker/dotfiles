#!/bin/bash
set -e

if ! command -v op &>/dev/null; then
  echo "Error: 1Password CLI (op) not found. Install it first."
  exit 1
fi

if ! op account list &>/dev/null 2>&1; then
  echo "==> Sign in to 1Password CLI first: eval \$(op signin)"
  exit 1
fi

echo "==> Setting up secrets from 1Password..."

mkdir -p ~/intelephense
op read "op://Private/Intelephense/license key" > ~/intelephense/licence.txt
echo "  ✓ Intelephense license"

echo ""
echo "Done. All secrets configured."
