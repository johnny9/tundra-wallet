#!/usr/bin/env bash
# Pinned test tool, installed inside the ignored build directory without sudo.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build/tools
archive=build/tools/bitcoin-31.1-x86_64-linux-gnu.tar.gz
curl --fail --location --silent --show-error \
  https://bitcoincore.org/bin/bitcoin-core-31.1/bitcoin-31.1-x86_64-linux-gnu.tar.gz -o "$archive"
echo "b80d9c3e04da78fb6f0569685673418cf686fadba9042d926d13fb87ff503f9e  $archive" | sha256sum --check
tar -xzf "$archive" -C build/tools
if [[ -n "${GITHUB_PATH:-}" ]]; then
  realpath build/tools/bitcoin-31.1/bin >> "$GITHUB_PATH"
fi
