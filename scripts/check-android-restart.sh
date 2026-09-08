#!/usr/bin/env bash
# Run after instrumentation, on its disposable emulator with the real saved draft.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build
if ! adb shell pm path dev.johnny9.tundra.dev | grep -q '^package:'; then
  echo 'Tundra is not installed. Preserve the instrumented app with -Pandroid.injected.androidTest.leaveApksInstalledAfterRun=true.' >&2
  exit 1
fi
adb shell am force-stop dev.johnny9.tundra.dev
if ! adb shell am start -W -n dev.johnny9.tundra.dev/dev.johnny9.tundra.MainActivity > build/android-restart-launch.log 2>&1; then
  cat build/android-restart-launch.log >&2
  exit 1
fi
launcher_recovered=0
for attempt in $(seq 1 15); do
  adb shell uiautomator dump /sdcard/tundra-runtime.xml >/dev/null
  adb pull /sdcard/tundra-runtime.xml build/android-restart.xml >/dev/null 2>&1
  state="$(python3 scripts/android_restart_probe.py build/android-restart.xml)"
  if [[ "$state" == ready ]]; then
    adb shell rm /sdcard/tundra-runtime.xml
    echo 'Android force-stop/relaunch retained the synced balance and exact-input draft.'
    exit 0
  fi
  if [[ "$state" == launcher-anr* && "$launcher_recovered" == 0 ]]; then
    cp build/android-restart.xml build/android-restart-launcher-anr.xml
    echo 'Hosted emulator Quickstep ANR observed; closing that launcher dialog once, then rechecking Tundra.' | tee build/android-restart-environment.log
    read -r _ tap_x tap_y <<< "$state"
    adb shell input tap "$tap_x" "$tap_y"
    adb shell am start -W -n dev.johnny9.tundra.dev/dev.johnny9.tundra.MainActivity >> build/android-restart-launch.log 2>&1
    launcher_recovered=1
  fi
  sleep 1
done
echo 'Android restart UI did not show the persisted fixture state.' >&2
exit 1
