# Architecture

## Layers

```text
Android Compose / Kotlin       iOS SwiftUI / Swift
          \                      /
            Tundra-owned UniFFI API
                      |
           Rust application core
       descriptors / labels / drafts / policy
              /         |          \
           BDK       SQLite      signing adapters
                                  /           \
                         QR payload codecs    bhwi protocol layer
                           native scanner     native USB transport
```

Two crates initially: `tundra-core` and `tundra-ffi`. Modules are enough; do not create a
service or a crate for every feature. There is no backend that coordinates owners.

The core owns amounts (integer satoshis), wallet eligibility, exact-input policy, fee/change,
drafts, persistence and validation. Native view models own focus, scrolling, navigation,
selection gestures, animations and short-lived edit state. Native code never constructs a
second version of a transaction for display.

## Persistence implemented in the source milestone

SQLite has application tables for wallets, labels, freezes, drafts and reservations.
A wallet row contains a JSON serialization of its accumulated BDK `ChangeSet`.

For each mutation: acquire the core's mutex -> `BEGIN IMMEDIATE` -> reconstruct a BDK wallet
from its snapshot -> perform the operation -> merge staged changes -> store the updated
snapshot with metadata/reservations -> commit. If anything fails, neither the DB nor a
long-lived mutated wallet object survives the rollback.

This deliberately uses **BDK's no-persist wallet API and an application-owned persister**,
not its rusqlite convenience adapter. It keeps wallet index changes and application
reservations in one SQLite transaction. The initial whole-snapshot approach is simple but
must be benchmarked with realistic histories; a later incremental journal must preserve
atomicity and have migrations. Do not serialize dependency structs as a forever-stable
format: `user_version` and dependency upgrades require restore/migration tests.

Foreign keys include `(wallet_id, draft_id)`, preventing cross-wallet reservations. A
coin has at most one reservation in a wallet. Freeze rows are independent of reservation
rows. Unused indexes and newly revealed receive addresses are persisted before being
returned. Development SQLite is **not encrypted**; production protection is a release gate.

## FFI

Expose only Tundra records/enums and `Arc<Tundra>`; never raw BDK/bhwi objects or DB handles.
The source milestone uses synchronous UniFFI methods and a mutex. Calls go off the UI thread:
Android `Dispatchers.IO`, iOS a dedicated actor. Do not hold this mutex while waiting for a
user, a device or the network.

Before long-running work is added, introduce cancellable operation IDs and typed progress
notifications. Build external scan/transport work outside a DB transaction, then validate
and apply its result in a short transaction. Test cancellation and lifecycle explicitly;
a cancelled coroutine is not automatically a cancelled native operation.

## Hardware boundary

`hardware.rs` currently advertises no capabilities and rejects signing. This is a gate,
not an implementation. Add a pinned bhwi protocol interpreter only after a device spike.
Its transport independence is useful for Android USB APIs; it does not make iPhone USB
compatibility automatic. Initial platform goal: Android QR + qualified USB, iPhone QR.

Treat wallet registration artifacts (e.g. device-specific tokens) as wallet metadata,
not signer profiles. They are not private Bitcoin keys but should be protected as metadata.
Keep QR codecs separate. Native decoders supply frame bytes/text; Rust reassembles and
validates bounded payloads. Never send camera images across FFI unnecessarily.

## Sync and draft lifecycle

No sync adapter exists in M1. Implement configurable test-network Esplora first with explicit
consent, then additional adapters as justified. No fixed public endpoint or automatic
network request at app launch. Cached balances need last-sync/stale state.

Build -> review -> request signature -> validate/merge -> request second key if needed ->
finalize -> explicit broadcast. A returned PSBT must match the immutable approved transaction.
Validate all fields used to derive values/policies and cryptographically check signatures
before counting them. A changed transaction invalidates prior approvals/signatures.
Reorgs/spends and fee replacement must invalidate or rebuild affected drafts explicitly.
