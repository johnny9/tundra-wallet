# Test plan

## Layers

| Layer | Tests / execution |
|---|---|
| Offline repository checks | Run `python3 scripts/check_offline.py`; verifies actual SQLite constraints/transactions and public fixture checksums plus source/package guards |
| Rust unit tests | `cargo test --workspace --all-targets`; tests are authored but were not executable in the authoring environment |
| Static Rust quality | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`; format/fix after first connected compilation |
| FFI contract | Generate both bindings from the same compiled library; test records/errors, Unicode, u64 amounts, cancellation/lifetime handling |
| Native apps | Android JVM/instrumentation plus emulator; iOS simulator tests; small/large screens, accessibility text, dark/light, file permissions |
| Bitcoin integration | Regtest/signet synthetic funding, reorgs, conflicts, receive/change persistence, fee accounting, strict exact-input behavior |
| Hardware | Physical devices and qualified firmware, not just emulators or a README support list |
| Security | Fuzz descriptor/BIP329/PSBT/QR parsers, transaction mutation corpus, storage crash tests, privacy/network/log inspection |

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
