#!/usr/bin/env bash
# Pinned test tool, installed inside the ignored build directory without sudo.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build/tools
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) platform=x86_64-linux-gnu; checksum=b80d9c3e04da78fb6f0569685673418cf686fadba9042d926d13fb87ff503f9e ;;
  Darwin-arm64) platform=arm64-apple-darwin; checksum=16a097c09fbd7eb78b240ce1dae123663ea2e5e377cfd6a951e71e227e23cf2f ;;
  Darwin-x86_64) platform=x86_64-apple-darwin; checksum=bc506958d0f387c1ea770bdc7c7192a505fa645ff62cabcc7761fa7eb89e867e ;;
  *) echo 'Unsupported CI test host' >&2; exit 1 ;;
esac
archive="build/tools/bitcoin-31.1-$platform.tar.gz"
curl --fail --location --silent --show-error \
  "https://bitcoincore.org/bin/bitcoin-core-31.1/bitcoin-31.1-$platform.tar.gz" -o "$archive"
python3 - "$archive" "$checksum" <<'PY'
import hashlib, sys
with open(sys.argv[1], 'rb') as f:
    actual = hashlib.file_digest(f, 'sha256').hexdigest()
if actual != sys.argv[2]:
    raise SystemExit('Bitcoin Core checksum mismatch')
PY
tar -xzf "$archive" -C build/tools
if [[ -n "${GITHUB_PATH:-}" ]]; then
  realpath build/tools/bitcoin-31.1/bin >> "$GITHUB_PATH"
fi
