#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v bitcoind >/dev/null
command -v bitcoin-cli >/dev/null
command -v python3 >/dev/null
cargo test -p tundra-core --test regtest --locked -- --ignored --nocapture
