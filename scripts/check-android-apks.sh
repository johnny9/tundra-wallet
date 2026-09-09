#!/usr/bin/env bash
# Reuse matching CI APKs on an explicitly disposable Waydroid/emulator instance.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${ANDROID_SERIAL:?Set ANDROID_SERIAL to the disposable Waydroid/emulator ADB target}"
if [[ ${TUNDRA_DISPOSABLE_ANDROID:-0} != 1 || $# != 2 ]]; then
  echo 'Usage: TUNDRA_DISPOSABLE_ANDROID=1 ANDROID_SERIAL=<target> scripts/check-android-apks.sh <app.apk> <androidTest.apk>' >&2
  echo 'Use a fresh disposable installation; the test refuses existing wallet storage and never resets it.' >&2
  exit 1
fi
test -f "$1"
test -f "$2"
mkdir -p build
adb get-state
adb reverse tcp:3002 tcp:3002
adb reverse tcp:3003 tcp:3003
adb reverse tcp:3004 tcp:3004
adb install -r "$1"
adb install -r "$2"
python3 - <<'PY'
import re
import subprocess
from pathlib import Path

result = subprocess.run([
    'adb', 'shell', 'am', 'instrument', '-w', '-r',
    'dev.johnny9.tundra.dev.test/androidx.test.runner.AndroidJUnitRunner',
], capture_output=True, text=True, timeout=300)
output = result.stdout + result.stderr
Path('build/android-apk-instrumentation.log').write_text(output)
print(output)
# adb itself can return zero even when instrumentation fails.
passed = re.search(r'^OK \(([1-9][0-9]*) tests?\)\s*$', output, re.MULTILINE)
if result.returncode or not passed or 'FAILURES!!!' in output or 'INSTRUMENTATION_FAILED' in output:
    raise SystemExit('Android instrumentation did not report a passing test run.')
PY
./scripts/check-android-restart.sh
