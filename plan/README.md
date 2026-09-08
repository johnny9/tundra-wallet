# Tundra implementation plan

Planning baseline: **September 7, 2026**. Maintained alongside implementation, not a promise
of unattended future work.

## Product decision

Build Tundra, not another HTML simulation. A Rust application core owns Bitcoin behavior,
with Kotlin/Compose and SwiftUI presenting that behavior natively. One person manages
single-sig or 2-of-3 hardware wallets. No private signing keys, signer directory, shared
approval system, or cloud account.

## Current status — September 8, 2026

**Rust foundation compiled and tested; native app validation pending. M1 remains open.**

- Source: descriptor-first watch-only engine, metadata, unsigned-draft core, UniFFI API,
  Android native import shell, iOS native starter, tests and CI.
- Verified: 60 Rust tests; Kotlin and Swift binding generation from the compiled library.
  See [VALIDATION.md](VALIDATION.md) for the exact results and remaining gates.
- Not implemented: chain synchronization, real QR/USB transport, bhwi device integration,
  signed-response validation, finalization/broadcast, production storage protection.
- Git is initialized with the owner's configured `johnny9/tundra-wallet` origin.
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
| [VALIDATION.md](VALIDATION.md) | What was actually run during this handoff |

## Immediate next implementation pass

1. Keep `scripts/check.sh` passing against the reviewed lock and pinned Rust 1.93.1.
   Close the remaining descriptor, persistence and FFI regression gaps in M1.
2. Build Android debug and an iOS simulator target using both generated bindings.
3. Implement an explicit opt-in test-network sync source and reorg tests. Never convert an
   unsynced balance into a displayed zero.
4. Port native selection/review/draft screens from the approved design using the Rust API.
5. Add QR exchange, then one qualified bhwi/Android USB device. Validate signatures and
   approved-transaction invariants before exposing finalization or broadcast.

Do not mark a milestone complete because source files exist. Each milestone has a
compile/test/device gate below.
