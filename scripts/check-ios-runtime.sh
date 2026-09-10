#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build
TUNDRA_TEST_DIAGNOSTICS="${TUNDRA_TEST_DIAGNOSTICS:-on-failure}"
case "$TUNDRA_TEST_DIAGNOSTICS" in
  on-failure|never) ;;
  *) echo 'TUNDRA_TEST_DIAGNOSTICS must be on-failure or never' >&2; exit 2 ;;
esac
if [[ -z "${TUNDRA_SIM_ID:-}" ]]; then
xcrun simctl list devices available --json > build/ios-devices.json
TUNDRA_SIM_ID="$(python3 - <<'PY'
import json
with open('build/ios-devices.json') as f:
    devices = json.load(f)['devices']
for runtime, rows in sorted(devices.items(), reverse=True):
    if '.iOS-' in runtime:
        for device in rows:
            if device['name'].startswith('iPhone') and device['isAvailable']:
                print(device['udid'])
                raise SystemExit(0)
raise SystemExit('No available iPhone simulator')
PY
)"
fi
xcodebuild -project apps/ios/Tundra.xcodeproj -scheme Tundra \
  -configuration Debug -destination "platform=iOS Simulator,id=$TUNDRA_SIM_ID" \
  -derivedDataPath build/ios-derived \
  -parallel-testing-enabled NO -resultBundlePath "${TUNDRA_RESULT_BUNDLE:-build/ios-runtime.xcresult}" \
  -collect-test-diagnostics "$TUNDRA_TEST_DIAGNOSTICS" \
  -test-timeouts-enabled YES -default-test-execution-time-allowance 300 \
  -maximum-test-execution-time-allowance 420 \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- test "$@"
