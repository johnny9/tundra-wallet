#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build/fuzz-corpus build/fuzz-artifacts
cargo fetch --locked --manifest-path fuzz/Cargo.toml
cp fuzz/Cargo.lock build/fuzz-lock-before
cp tests/fixtures/single-sig.txt tests/fixtures/two-of-three.txt tests/fixtures/core-style-pair.json build/fuzz-corpus/
cp tests/fixtures/hwi-signed-wpkh.psbt tests/fixtures/ledger-wpkh-two-inputs.psbt build/fuzz-corpus/
base64 --decode tests/fixtures/hwi-signed-wpkh.psbt > build/fuzz-corpus/hwi-binary
base64 --decode tests/fixtures/ledger-wpkh-two-inputs.psbt > build/fuzz-corpus/ledger-binary
printf '%s\n' '{"type":"output","ref":"test:0","label":"Unicode 🧊","spendable":false}' > build/fuzz-corpus/label.jsonl
CARGO_NET_OFFLINE=true cargo +nightly-2026-09-07 fuzz run public_parsers build/fuzz-corpus -- \
  -max_total_time="${TUNDRA_FUZZ_SECONDS:-60}" -max_len=32769 -rss_limit_mb=2048 \
  -artifact_prefix=build/fuzz-artifacts/
cmp fuzz/Cargo.lock build/fuzz-lock-before
