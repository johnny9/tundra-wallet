#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'Rust/Cargo is required. No Rust tests have been run.' >&2; exit 2; }
cargo fmt --version >/dev/null || { echo 'Install rustfmt for the pinned Rust toolchain.' >&2; exit 2; }
cargo clippy --version >/dev/null || { echo 'Install clippy for the pinned Rust toolchain.' >&2; exit 2; }
[[ -f Cargo.lock ]] || { echo 'The reviewed Cargo.lock is required. Restore it from Git.' >&2; exit 2; }
python3 scripts/check_offline.py
cargo fmt --all --check
cargo test --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
