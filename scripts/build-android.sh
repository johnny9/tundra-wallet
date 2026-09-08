#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
: "${ANDROID_HOME:?Set ANDROID_HOME to your Android SDK}"
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an installed Android NDK}"
[[ -d "$ANDROID_NDK_HOME" ]] || { echo 'Android NDK directory is missing.' >&2; exit 2; }
command -v cargo >/dev/null || { echo 'Cargo is required.' >&2; exit 2; }
cargo ndk --version >/dev/null || { echo 'Install cargo-ndk using cargo install cargo-ndk --locked.' >&2; exit 2; }
./scripts/generate-bindings.sh
cargo ndk --platform 28 -t arm64-v8a -t x86_64 \
  -o apps/android/app/src/main/jniLibs build --locked --release -p tundra-ffi --lib
echo 'Rust JNI libraries built. Build the app using Android Studio or Gradle 8.13.'
