# Milestones and acceptance gates

These are ordered work packages, not time estimates.

| Milestone | Source status | Done only when |
|---|---|---|
| M0: product/design baseline | Local rendering and all reference integrity checks passed; attribution retained | Reference renders locally, attribution retained, decisions recorded |
| M1: offline Rust/native foundation | Rust/native builds, host FFI and initial virtual runtime checks pass; broader device qualification remains | Rust tests pass; both generated bindings compile; Android and iOS import a disposable public descriptor, survive restart, derive distinct persistent addresses, show unknown balances |
| M2: sync + durable coin state | Core reorg checks and both native regtest sync scenarios pass | Explicit test endpoint syncs/reorgs correctly, no keys/labels leak, balance freshness represented, spent/reserved/frozen transitions tested |
| M3: native coin control + review | Android exact-input and iOS consolidation/restart checks pass; full mode/layout qualification remains | Native exact-input, automatic, max and consolidation flows round-trip through Rust; no UI-only validation; layouts match prototype; interrupted drafts persist |
| M4: QR external signing | PSBT verification and QR codecs pass software tests; native camera builds/device qualification open | Bounded UR/BBQr sessions interoperate on devices; wrong payloads/transactions rejected; per-input signatures verified; receive/policy comparison works |
| M5: Android USB / bhwi | Not implemented | Pinned bhwi revision; documented model/firmware/transport matrix; permission, cancellation, reconnect and app lifecycle tests pass |
| M6: signet end-to-end | Final transaction assembly/persistence passes software tests; hardware/broadcast end-to-end open | 2-of-3 QR + USB with restart signs, validates, finalizes and explicitly broadcasts to a configured test backend |
| M7: hardened release candidate | Not implemented | Protected storage, backup/recovery/migration tests, dependency/license review, independent security review and a clean supported-device matrix |

## M1 backlog before declaring it done

- Rust compilation, rustfmt and reviewed Cargo.lock completed on Rust 1.93.1; keep required checks passing.
- Both adapters compile and execute the actual mobile Rust library; Android packages both
  arm64/x86_64 ABIs. Keep the recorded host/mobile FFI and process-restart checks passing.
- Extend initial emulator/simulator coverage to accessibility, dark/light appearance and
  cancellation during active network IO on physical phones.
- Public key ordering, duplicate accounts/origins, branch/policy mismatches and hardened/public
  derivation constraints now have regression coverage. Expand private-key/WIF rejection and
  parser fuzz coverage before declaring descriptor validation complete.
- File reopen, snapshot-write rollback, competing reservations and abrupt-process WAL
  recovery tests pass. Physical power-loss/storage-fault testing remains open.

## Scope limitations to close deliberately

- BIP 329 abbreviated origins now match canonical wallet policy/origins (including reordered
  multisig origins and hardened notation aliases). Known-reference checks remain mandatory;
  duplicate matching records roll back atomically. Both apps expose label file import/export.
- Coinbase maturity now follows confirmation depth at the current tip; verified against
  Bitcoin Core regtest at 99 and 100 confirmations and after reorgs.
- Decimal fee rates now normalize upward to BDK's integer sat/kwu precision; the normalized
  rate is included in new saved reviews. Development fee limits still need release review.
- Bounded UR/BBQr codecs pass Rust tests. Native scanner/display and independent barcode
  tests are authored; native QR compilation and physical interoperability remain open.
- Explicit bounded Esplora test-network sync and signed-response validation are implemented.
  Native QR qualification and broadcast remain separate acceptance gates.
- Native coin selection, review, PSBT file export and saved drafts are implemented; validate
  every mode on both platforms and compare layouts/accessibility against the reference.
- Label provenance is captured on draft inputs. New-output labeling after a real broadcast
  and resulting history linkage remain to implement.

## Release blockers

Public distribution should wait for a chosen license and reproducible dependency lock.
Real-funds support additionally requires protected metadata storage, verified hardware
receive addresses and multisig policies, actual signature validation, reorg-safe drafts,
backup/export recovery, and the end-to-end adversarial tests in `05-testing.md`.
