#!/usr/bin/env bash
# Run after instrumentation, on its disposable emulator with the real saved draft.
set -euo pipefail
cd "$(dirname "$0")/.."
adb shell am force-stop dev.johnny9.tundra.dev
adb shell am start -W -n dev.johnny9.tundra.dev/dev.johnny9.tundra.MainActivity > build/android-restart-launch.log
for attempt in $(seq 1 15); do
  adb shell uiautomator dump /sdcard/tundra-runtime.xml >/dev/null
  adb pull /sdcard/tundra-runtime.xml build/android-restart.xml >/dev/null 2>&1
  if python3 - <<'PY'
import xml.etree.ElementTree as ET
root = ET.parse('build/android-restart.xml').getroot()
texts = {node.get('text') for node in root.iter('node')}
raise SystemExit(0 if {'150 BTC', 'Draft · unsigned · 2 inputs'} <= texts else 1)
PY
  then
    adb shell rm /sdcard/tundra-runtime.xml
    echo 'Android force-stop/relaunch retained the synced balance and exact-input draft.'
    exit 0
  fi
  sleep 1
done
echo 'Android restart UI did not show the persisted fixture state.' >&2
exit 1
