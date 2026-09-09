#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# == 1 ]] || { echo 'Pass a clean local SQLCipher source checkout.' >&2; exit 2; }
SOURCE="$(cd "$1" && pwd)"
[[ "$(git -C "$SOURCE" rev-parse HEAD)" == c4b275a47932888216bade83aff2bbc73df0ff85 ]] || {
  echo 'SQLCipher source revision does not match the reviewed pin.' >&2; exit 2;
}
[[ -z "$(git -C "$SOURCE" status --porcelain)" ]] || {
  echo 'SQLCipher source checkout must be clean.' >&2; exit 2;
}
mkdir -p "$ROOT/build"
OUTPUT="$(mktemp -d "$ROOT/build/sqlcipher-verify.XXXXXX")"
trap 'rm -rf "$OUTPUT"' EXIT
cd "$OUTPUT"
env -u CPPFLAGS -u LDFLAGS CCACHE=none LC_ALL=C \
  CFLAGS='-DSQLITE_HAS_CODEC -DSQLITE_EXTRA_INIT=sqlcipher_extra_init -DSQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown' \
  "$SOURCE/configure" --disable-tcl --with-tempstore=yes --fts5
make sqlite3.c sqlite3.h
cmp sqlite3.c "$ROOT/crates/tundra-sqlcipher/vendor/sqlite3.c"
cmp sqlite3.h "$ROOT/crates/tundra-sqlcipher/vendor/sqlite3.h"
echo 'Both SQLCipher amalgamation files match the pinned source byte for byte.'
