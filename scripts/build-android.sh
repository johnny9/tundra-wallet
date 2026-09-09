#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
: "${ANDROID_HOME:?Set ANDROID_HOME to your Android SDK}"
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an installed Android NDK}"
[[ -d "$ANDROID_NDK_HOME" ]] || { echo 'Android NDK directory is missing.' >&2; exit 2; }
command -v cargo >/dev/null || { echo 'Cargo is required.' >&2; exit 2; }
[[ "$(cargo ndk --version)" == 'cargo-ndk 4.1.2' ]] || { echo 'Install cargo-ndk 4.1.2 using cargo install cargo-ndk --version 4.1.2 --locked.' >&2; exit 2; }
python3 - <<'PY'
import os
from pathlib import Path
properties = dict((key.strip(), value.strip()) for key, value in
                  (line.split('=', 1) for line in
                   (Path(os.environ['ANDROID_NDK_HOME']) / 'source.properties').read_text().splitlines()
                   if '=' in line))
if properties.get('Pkg.Revision') != '27.2.12479018':
    raise SystemExit('Set ANDROID_NDK_HOME to the reviewed NDK 27.2.12479018.')
PY
./scripts/generate-bindings.sh
cargo ndk --platform 28 -t arm64-v8a -t x86_64 \
  -o build/android-rust-libs build --locked --release -p tundra-ffi --lib
# cargo-ndk also copies dependency cdylibs. The app calls only tundra-ffi;
# BBQr is linked into that library as Rust code and has no dynamic load entry.
for ABI in arm64-v8a x86_64; do
  DESTINATION="apps/android/app/src/main/jniLibs/$ABI"
  mkdir -p "$DESTINATION"
  install -m 755 "build/android-rust-libs/$ABI/libtundra_ffi.so" "$DESTINATION/libtundra_ffi.so"
  # Remove only the previously generated, unused BBQr cdylib from this output.
  for PREVIOUS in "$DESTINATION"/libbbqr-*.so; do
    if [[ -f "$PREVIOUS" ]]; then rm -- "$PREVIOUS"; fi
  done
done
echo 'Rust JNI libraries built. Run apps/android/gradlew -p apps/android :app:assembleDebug.'
