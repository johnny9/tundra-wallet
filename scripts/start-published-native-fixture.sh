#!/usr/bin/env bash
# Disposable CI test fixture only. No upstream network or Bitcoin signing keys.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build
test ! -e build/published-native-port || { echo 'Public fixture port record already exists; use a fresh test run.' >&2; exit 1; }
python3 tests/esplora_published.py --port 3003 --port-file build/published-native-port > build/published-native-server.log 2>&1 &
TUNDRA_FIXTURE_PID=$!
for attempt in $(seq 1 20); do
  if test -f build/published-native-port; then
    test "$(cat build/published-native-port)" = 3003
    exit 0
  fi
  kill -0 "$TUNDRA_FIXTURE_PID" 2>/dev/null || { echo 'Public fixture server exited.' >&2; exit 1; }
  sleep 1
done
kill "$TUNDRA_FIXTURE_PID" 2>/dev/null || true
echo 'Public fixture server did not start.' >&2
exit 1
