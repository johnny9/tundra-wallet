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

The Esplora adapter uses a bounded reqwest/rustls client and BDK wallet updates. A prepared
operation ID exists before IO; run, poll and cancel are separate typed calls. A worker scans
both keychains outside the DB mutex, then checks its snapshot/revision and commits chain,
freshness and draft invalidation atomically. Address/draft changes during the scan cause a
retry; independent label/freeze changes survive. Cancellation and commit serialize, so the
returned terminal state says which won. Schema v2 adds endpoint and sync revision metadata.

No fixed endpoint or launch requests. Each scan requires explicit privacy consent. HTTPS
is required except loopback HTTP (including Android emulator host 10.0.2.2). Redirects,
embedded credentials and implicit environment proxies are disabled. Cached balances show
the last successful scan; errors/cancellation preserve it. Limits: 20 unused scripts per
branch after the last revealed/active index, 2,000 scripts per branch, 5,000 transactions,
4 MB per response, 64 MB total, 20,000 requests and 180 seconds overall. Reaching a limit
fails without a partial commit. Larger histories and non-default discovery gaps need a
reviewed configuration flow before broad wallet support.

The server remains the trusted view of the test chain. Transaction IDs, block-header hashes
and Merkle inclusion proofs are checked; this is not independent chain-work/SPV validation.
The adapter replaces its validated checkpoint snapshot, including shorter tips, and expires
affected drafts permanently until the user rebuilds them. A reappearing coin retains its
label/freeze, but cannot silently restore an old approval.

Build -> review -> request signature -> validate/merge -> request second key if needed ->
finalize -> explicit broadcast. A returned PSBT must match the immutable approved transaction.
Validate all fields used to derive values/policies and cryptographically check signatures
before counting them. A changed transaction invalidates prior approvals/signatures.
Reorgs/spends and fee replacement must invalidate or rebuild affected drafts explicitly.
