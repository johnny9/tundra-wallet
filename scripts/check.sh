#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'Rust/Cargo is required. No Rust tests have been run.' >&2; exit 2; }
python3 scripts/check_offline.py
if [[ ! -f Cargo.lock ]]; then
  echo 'First resolution: generating Cargo.lock. Review and commit this file.'
  cargo generate-lockfile
fi
cargo fmt --all
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked
