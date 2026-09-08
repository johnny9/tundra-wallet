# Milestones and acceptance gates

These are ordered work packages, not time estimates.

| Milestone | Source status | Done only when |
|---|---|---|
| M0: product/design baseline | Approved reference included | Reference renders locally, attribution retained, decisions recorded |
| M1: offline Rust/native foundation | Rust compiled/tested; both bindings generated; native gates open | Rust tests pass; both generated bindings compile; Android and iOS import a disposable public descriptor, survive restart, derive distinct persistent addresses, show unknown balances |
| M2: sync + durable coin state | Not implemented | Explicit test endpoint syncs/reorgs correctly, no keys/labels leak, balance freshness represented, spent/reserved/frozen transitions tested |
| M3: native coin control + review | Core draft source only | Native exact-input, automatic, max and consolidation flows round-trip through Rust; no UI-only validation; layouts match prototype; interrupted drafts persist |
| M4: QR external signing | Not implemented | Bounded UR/BBQr sessions interoperate on devices; wrong payloads/transactions rejected; per-input signatures verified; receive/policy comparison works |
| M5: Android USB / bhwi | Not implemented | Pinned bhwi revision; documented model/firmware/transport matrix; permission, cancellation, reconnect and app lifecycle tests pass |
| M6: signet end-to-end | Not implemented | 2-of-3 QR + USB with restart signs, validates, finalizes and explicitly broadcasts to a configured test backend |
| M7: hardened release candidate | Not implemented | Protected storage, backup/recovery/migration tests, dependency/license review, independent security review and a clean supported-device matrix |

## M1 backlog before declaring it done

- Rust compilation, rustfmt and reviewed Cargo.lock completed on Rust 1.93.1; keep required checks passing.
- Compile generated Kotlin/Swift binding names against the hand-authored adapters.
- Verify ABI packaging, native errors and Unicode label roundtrips through UniFFI.
- Run Android instrumentation and an iOS simulator build; validate accessibility and dark/light.
- Add exhaustive public-only descriptor tests (valid private extended keys/WIF rejected,
  duplicate account keys, same key with different origins, mismatched policies/branches,
  public-key order normalization, hardened/public derivation constraints).
- Add crash/reopen tests covering BDK state plus metadata atomicity.

## Scope limitations to close deliberately

- Origin-bearing BIP 329 imports are currently skipped, not treated as an authority to
  apply labels to a different wallet. Add canonical descriptor-origin matching.
- M1 excludes all coinbase outputs conservatively. Implement maturity at the current tip
  before accepting mature coinbase spends; do not call all such outputs immature forever.
- Fees are integer sat/vB and have a development cap. Add precise fractional fee-rate support
  and reviewed policy limits without floating-point satoshi accounting.
- No on-device scanner or animated-QR codecs yet. Do not relabel the prototype simulation
  as a native hardware feature.
- No chain provider, sync endpoint, signed-response parser or broadcast API yet.
- Native screens show the offline import/list experience; advanced draft UI is still to port.
- Label provenance is captured on draft inputs. New-output labeling after a real broadcast
  and resulting history linkage remain to implement.

## Release blockers

Public distribution should wait for a chosen license and reproducible dependency lock.
Real-funds support additionally requires protected metadata storage, verified hardware
receive addresses and multisig policies, actual signature validation, reorg-safe drafts,
backup/export recovery, and the end-to-end adversarial tests in `05-testing.md`.
