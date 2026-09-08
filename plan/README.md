# Tundra implementation plan

Planning baseline: **September 7, 2026**. Maintained alongside implementation, not a promise
of unattended future work.

## Product decision

Build Tundra, not another HTML simulation. A Rust application core owns Bitcoin behavior,
with Kotlin/Compose and SwiftUI presenting that behavior natively. One person manages
single-sig or 2-of-3 hardware wallets. No private signing keys, signer directory, shared
approval system, or cloud account.

## Status at handoff

**Initial source milestone authored; compiled validation pending.**

- Source: descriptor-first watch-only engine, metadata, unsigned-draft core, UniFFI API,
  Android native import shell, iOS native starter, tests and CI.
- Available verification: see [VALIDATION.md](VALIDATION.md) for the exact results.
- Not implemented: chain synchronization, real QR/USB transport, bhwi device integration,
  signed-response validation, finalization/broadcast, production storage protection.
- Not performed: repository creation/push, Cargo build/test, Android build, or Xcode build.
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

1. Run `scripts/check.sh` on a connected Rust host; fix compile/API issues before expanding
   scope, format the sources, review and commit the generated `Cargo.lock`.
2. Generate both language bindings; build Android debug and an iOS simulator target.
3. Implement an explicit opt-in test-network sync source and reorg tests. Never convert an
   unsynced balance into a displayed zero.
4. Port native selection/review/draft screens from the approved design using the Rust API.
5. Add QR exchange, then one qualified bhwi/Android USB device. Validate signatures and
   approved-transaction invariants before exposing finalization or broadcast.

Do not mark a milestone complete because source files exist. Each milestone has a
compile/test/device gate below.
