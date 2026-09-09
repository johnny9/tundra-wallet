#!/usr/bin/env bash
# Disposable CI test fixture only. No upstream network or Bitcoin signing keys.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build
case "${1:-}" in
  '') TUNDRA_FIXTURE_PORT=3003; TUNDRA_FIXTURE_NAME=published-native ;;
  --screen) TUNDRA_FIXTURE_PORT=3004; TUNDRA_FIXTURE_NAME=published-screen ;;
  *) echo 'Usage: start-published-native-fixture.sh [--screen]' >&2; exit 2 ;;
esac
test "$#" -le 1 || exit 2
TUNDRA_FIXTURE_RECORD="build/$TUNDRA_FIXTURE_NAME-port"
test ! -e "$TUNDRA_FIXTURE_RECORD" || { echo 'Public fixture port record already exists; use a fresh test run.' >&2; exit 1; }
python3 tests/esplora_published.py --port "$TUNDRA_FIXTURE_PORT" --port-file "$TUNDRA_FIXTURE_RECORD" > "build/$TUNDRA_FIXTURE_NAME-server.log" 2>&1 &
TUNDRA_FIXTURE_PID=$!
for attempt in $(seq 1 20); do
  if test -f "$TUNDRA_FIXTURE_RECORD"; then
    test "$(cat "$TUNDRA_FIXTURE_RECORD")" = "$TUNDRA_FIXTURE_PORT"
    exit 0
  fi
  kill -0 "$TUNDRA_FIXTURE_PID" 2>/dev/null || { echo 'Public fixture server exited.' >&2; exit 1; }
  sleep 1
done
kill "$TUNDRA_FIXTURE_PID" 2>/dev/null || true
echo 'Public fixture server did not start.' >&2
exit 1
