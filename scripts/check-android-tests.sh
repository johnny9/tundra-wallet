#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
collect_public_fixture_diagnostics() {
  mkdir -p build/android-public-test-diagnostics
  adb pull /sdcard/Android/data/dev.johnny9.tundra/files/public-test-diagnostics/. build/android-public-test-diagnostics/ >/dev/null 2>&1 || true
}
trap collect_public_fixture_diagnostics EXIT
adb reverse tcp:3002 tcp:3002
adb reverse tcp:3003 tcp:3003
./apps/android/gradlew --no-daemon -p apps/android -Pandroid.injected.androidTest.leaveApksInstalledAfterRun=true :app:connectedDebugAndroidTest
./scripts/check-android-restart.sh
