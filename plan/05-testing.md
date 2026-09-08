# Test plan

## Layers

| Layer | Tests / execution |
|---|---|
| Offline repository checks | Run `python3 scripts/check_offline.py`; verifies actual SQLite constraints/transactions and public fixture checksums plus source/package guards |
| Rust unit tests | `cargo test --workspace --all-targets --all-features --locked`; see the current validation report for executed results |
| Static Rust quality | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; both are required |
| FFI contract | Generated Kotlin JVM tests and `scripts/check-swift-ffi.sh` on macOS call the real host library for Unicode/reopen, typed errors and u64 amounts; mobile lifecycle/cancellation remains pending |
| Native apps | Android JVM/instrumentation plus emulator; iOS simulator tests; small/large screens, accessibility text, dark/light, file permissions |
| Bitcoin integration | Regtest/signet synthetic funding, reorgs, conflicts, receive/change persistence, fee accounting, strict exact-input behavior |
| Hardware | Physical devices and qualified firmware, not just emulators or a README support list |
| Security | Fuzz descriptor/BIP329/PSBT/QR parsers, transaction mutation corpus, storage crash tests, privacy/network/log inspection |

`scripts/check-fuzz.sh` runs the checked-in libFuzzer harness with AddressSanitizer, public
descriptor/BIP329 seeds, checksum repair for deeper descriptor mutations, and a bounded
runtime (60 seconds by default). Toolchain: nightly-2026-09-07; cargo-fuzz 0.13.2;
libfuzzer-sys 0.4.12. Its separate lock is committed and checked for changes. It covers
descriptor, label and amount parsing; PSBT/QR fuzzing awaits those implementations.

The `crash` integration test kills a child process after a durable receive/label commit and
during an uncommitted snapshot/metadata write, then checks reopening, index non-reuse and
SQLite integrity. It is abrupt-process recovery evidence, not physical power-loss evidence.

The native runtime tests use a real Bitcoin Core 31.1 node with wallets disabled. It mines
regtest outputs directly to public fixtures; no private signing keys or native fake balance
API are used. Android forwards loopback port 3002 using ADB; the iOS simulator uses host
loopback. These tests qualify software behavior on the recorded virtual runtimes only.

## Existing Rust source tests

Amount parsing/formatting; policy import and checksums; test-network matching; missing-change
rejection; public descriptor fixtures; BIP 329 omissions/nulls/duplicates/limits; label preview
and atomicity; unknown balances; distinct receive indices; freezes; draft reservation
exclusivity; manual-only insufficient funds; selected maximum/consolidation accounting;
privacy acknowledgement; and hardware unavailable behavior.

The engine's funding helper is `#[cfg(test)]` only. The native app cannot fabricate a funded
wallet or mark a wallet synced using the exported API. Production code must not call test
helpers as a shortcut to a demo.

## First physical-device acceptance scenario

1. Import a disposable 2-of-3 public descriptor on a physical Android phone.
2. Verify its policy and receive address on all participating hardware before test funding.
3. Sync, label two outputs, select them and inspect exact fee/change/recipient review.
4. Sign with one hardware key over QR, close/restart the app and resume the same draft.
5. Sign with another hardware key over USB; validate all signatures and finalize.
6. Explicitly broadcast only to the test chain; observe confirmation and label provenance.
7. Repeat with all three possible signing pairs and with a single-sig wallet.

Test cancellation/reconnect, wrong network/device/key, duplicate response, wrong unsigned
transaction, excessive fees, modified change, incomplete per-input signatures, unexpected
sighash, incomplete/interleaved/oversized QR frames, spent coins and a chain reorganization.

## Device qualification matrix

No devices are qualified in M1. Fill each row with tested evidence, not inferred support:

| Model + firmware | Platform + OS | Transport + format | Policy registration | Address verification | Single-sig | 2-of-3 | Cancellation/restart |
|---|---|---|---|---|---|---|---|
| Not yet selected/tested | — | — | — | — | — | — | — |

Before enabling a model, record exact versions, test date, regression fixtures, failures and
unsupported paths. QR device compatibility is independent of bhwi USB coverage.
