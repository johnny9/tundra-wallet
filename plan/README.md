# Tundra implementation plan

Planning baseline: **September 7, 2026**. Maintained alongside implementation, not a promise
of unattended future work.

## Product decision

Build Tundra, not another HTML simulation. A Rust application core owns Bitcoin behavior,
with Kotlin/Compose and SwiftUI presenting that behavior natively. One person manages
single-sig or 2-of-3 hardware wallets. No private signing keys, signer directory, shared
approval system, or cloud account.

## Current status — September 9, 2026

**Test-network sync and native coin control implemented; mobile qualification in progress.**

- Source: descriptor-first watch-only engine, metadata, unsigned-draft core, UniFFI API,
  Android native import shell, iOS native starter, tests and CI.
- Verified: 168 Rust tests including real regtest, 25 offline checks, parser fuzzing/audit,
  Android debug APK and iOS simulator builds, host FFI smoke tests, all four payment modes on both virtual platforms and initial process-restart checks.
  See [VALIDATION.md](VALIDATION.md) for the exact results and remaining gates.
- Added: bounded opt-in Esplora scans, cancellation, cached timestamps, coinbase maturity,
  reorg-safe draft invalidation, and a keyless Bitcoin Core regtest integration test.
- Added: native search/filter/selection, payment/max/consolidation review and saved drafts;
  decimal fee rates and atomic bulk coin metadata. Broader native qualification remains.
- Added: bounded external PSBT validation, per-input cryptographic signature progress,
  immutable approvals and atomic signature persistence. Native file integration and both initial payment/restart UI scenarios pass; broader coverage remains.
- Added: bounded UR/BBQr codecs and native camera/display source; both native barcode tests pass; iOS uses Vision revision 2 as its independent image decoder.
- Added: immutable finalized transaction bytes and native final-review source; software tests pass.
- Added: pinned bhwi Ledger protocol, registration persistence and Android USB adapter source; Android build/FFI/runtime checks pass; physical device qualification remains open.
- Added: explicit test-network broadcast, durable uncertainty and observed-spend handling; Rust tests and native builds pass; positive submission UI qualification remains open.
- Added: observed payment/output labels and durable input provenance; local rollback,
  reorg and migration tests pass; native coin-detail presentation awaits validation.
- Added: pinned SQLCipher, protected Rust storage and atomic plaintext migration with
  process-kill tests; both native vaults pass platform key-retention tests. Protected
  normal startup is now authored and awaits its runtime gate.
- Added: bounded encrypted backup export/inspection and a portable public fixture; Rust
  checks pass. Native provider checks, restore and backup/recovery UX remain.
- Logical implementation commits are pushed to the owner's `johnny9/tundra-wallet` origin.
- Not performed locally: Android build or Xcode build.
- **No real-funds use. No supported hardware devices yet.**

## Plan documents

| Document | Purpose |
|---|---|
| [01-product.md](01-product.md) | Stable scope and UX decisions from the prototype |
| [02-architecture.md](02-architecture.md) | Rust/native boundary and persistence model |
| [03-milestones.md](03-milestones.md) | Ordered deliverables and acceptance criteria |
| [04-security.md](04-security.md) | Threats, invariants, privacy and release blockers |
| [05-testing.md](05-testing.md) | Test layers and real-device qualification |
| [06-build.md](06-build.md) | Dependency choices, native builds and reproducibility |
| [13-storage.md](13-storage.md) | Protected database boundary, native keys and recovery gates |
| [14-backup.md](14-backup.md) | Encrypted snapshot format, inspection and restore gates |
| [12-broadcast.md](12-broadcast.md) | Explicit submission, durable uncertainty and chain observation |
| [11-usb.md](11-usb.md) | Bounded Ledger protocol and Android USB qualification |
| [10-finalization.md](10-finalization.md) | Exact final bytes, persistence and review |
| [09-qr.md](09-qr.md) | QR formats, bounds and native qualification |
| [08-signing.md](08-signing.md) | External response rules and software validation gates |
| [07-waydroid.md](07-waydroid.md) | Reuse CI APKs on a disposable local Android runtime |
| [VALIDATION.md](VALIDATION.md) | What was actually run during this handoff |

## Immediate next implementation pass

1. Keep `scripts/check.sh` passing against the reviewed lock and pinned Rust 1.93.1.
   Close the remaining descriptor, persistence and FFI regression gaps in M1.
2. Extend the passing Android exact-input and iOS consolidation/restart scenarios to every
   payment mode, metadata file exchange, accessibility and appearance. Qualify physical phones separately.
3. Keep the implemented opt-in test-network sync, cancellation and reorg checks passing.
   Never convert an unsynced balance into a displayed zero.
4. Complete visual comparison of native selection/review/draft screens with the approved design.
5. Add QR exchange, then one qualified bhwi/Android USB device. Validate signatures and
   approved-transaction invariants before exposing finalization or broadcast.

Do not mark a milestone complete because source files exist. Each milestone has a
compile/test/device gate below.
