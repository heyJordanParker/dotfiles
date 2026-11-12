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

# The aws package stows the r2 profile's endpoint into ~/.aws/config. Its keys stay
# out of the repo, so this writes them into ~/.aws/credentials.
mkdir -p ~/.aws
umask 077
cat > ~/.aws/credentials <<EOF
[r2]
aws_access_key_id = $(op read "op://Development/Cloudflare R2/access key id")
aws_secret_access_key = $(op read "op://Development/Cloudflare R2/secret access key")
EOF
echo "  ✓ Cloudflare R2 (AWS profile r2)"

op read "op://Development/TypeSafe/apiKey" | cmod secret store rulebook jev
echo "  ✓ TypeSafe Jev (rulebook)"

echo ""
echo "Done. All secrets configured."
