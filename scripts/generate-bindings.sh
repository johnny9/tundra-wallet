#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'Cargo is required.' >&2; exit 2; }
[[ -f Cargo.lock ]] || { echo 'The reviewed Cargo.lock is required. Restore it from Git.' >&2; exit 2; }
cargo build --locked -p tundra-ffi --lib
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
case "$(uname -s)" in
  Darwin) LIB="$TARGET/debug/libtundra_ffi.dylib" ;;
  Linux) LIB="$TARGET/debug/libtundra_ffi.so" ;;
  *) echo 'Use a Linux/macOS development host for these starter scripts.' >&2; exit 2 ;;
esac
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
for LANGUAGE in kotlin swift; do
  cargo run --locked -p tundra-ffi --features bindgen --bin uniffi-bindgen -- \
    generate "$LIB" --language "$LANGUAGE" --no-format --out-dir "$TMP/$LANGUAGE"
done
# Copy only generator output; never remove the hand-written app sources.
mkdir -p apps/android/app/src/main/java apps/ios/Generated
cp -R "$TMP/kotlin/." apps/android/app/src/main/java/
cp -R "$TMP/swift/." apps/ios/Generated/
echo 'Generated Kotlin and Swift from the same compiled Tundra API.'
