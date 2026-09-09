#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin ]] || { echo 'Swift FFI checks require a macOS host.' >&2; exit 2; }
command -v swiftc >/dev/null || { echo 'The Swift compiler is required.' >&2; exit 2; }
export MACOSX_DEPLOYMENT_TARGET=11.0
./scripts/generate-bindings.sh
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
mkdir -p build/ffi/swift/headers
cp apps/ios/Generated/TundraCoreFFI.h build/ffi/swift/headers/
cp apps/ios/Generated/TundraCoreFFI.modulemap build/ffi/swift/headers/module.modulemap
swiftc -swift-version 5 -I build/ffi/swift/headers -L "$TARGET_DIR/debug" -ltundra_ffi \
  -Xlinker -rpath -Xlinker "$TARGET_DIR/debug" \
  apps/ios/Generated/TundraCore.swift tests/ffi/swift/main.swift -o build/ffi/swift/check
build/ffi/swift/check tests/fixtures/single-sig.txt
