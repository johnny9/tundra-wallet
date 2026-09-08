#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin ]] || { echo 'iOS builds require macOS and Xcode.' >&2; exit 2; }
xcodebuild -version >/dev/null
./scripts/generate-bindings.sh
for TARGET in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  cargo build --locked --release -p tundra-ffi --lib --target "$TARGET"
done
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
mkdir -p build/ios/headers build/ios/simulator apps/ios/Frameworks
cp apps/ios/Generated/TundraCoreFFI.h build/ios/headers/
cp apps/ios/Generated/TundraCoreFFI.modulemap build/ios/headers/module.modulemap
xcrun lipo -create \
  "$TARGET_DIR/aarch64-apple-ios-sim/release/libtundra_ffi.a" \
  "$TARGET_DIR/x86_64-apple-ios/release/libtundra_ffi.a" \
  -output build/ios/simulator/libtundra_ffi.a
# This is generated output in a fixed directory, not user data.
rm -rf apps/ios/Frameworks/TundraCoreFFI.xcframework
xcodebuild -create-xcframework \
  -library "$TARGET_DIR/aarch64-apple-ios/release/libtundra_ffi.a" -headers build/ios/headers \
  -library build/ios/simulator/libtundra_ffi.a -headers build/ios/headers \
  -output apps/ios/Frameworks/TundraCoreFFI.xcframework
echo 'Generated bindings and XCFramework. Run xcodegen in apps/ios and open the project.'
