# Milestones and acceptance gates

These are ordered work packages, not time estimates.

| Milestone | Source status | Done only when |
|---|---|---|
| M0: product/design baseline | Local rendering and all reference integrity checks passed; attribution retained | Reference renders locally, attribution retained, decisions recorded |
| M1: offline Rust/native foundation | Rust/native builds, host FFI and initial virtual runtime checks pass; broader device qualification remains | Rust tests pass; both generated bindings compile; Android and iOS import a disposable public descriptor, survive restart, derive distinct persistent addresses, show unknown balances |
| M2: sync + durable coin state | Core reorg checks and both native regtest sync scenarios pass | Explicit test endpoint syncs/reorgs correctly, no keys/labels leak, balance freshness represented, spent/reserved/frozen transitions tested |
| M3: native coin control + review | All four payment modes pass on both virtual platforms; complete layout/device qualification remains | Native exact-input, automatic, max and consolidation flows round-trip through Rust; no UI-only validation; layouts match prototype; interrupted drafts persist |
| M4: QR external signing | PSBT verification and QR codecs pass software tests; native camera builds/device qualification open | Bounded UR/BBQr sessions interoperate on devices; wrong payloads/transactions rejected; per-input signatures verified; receive/policy comparison works |
| M5: Android USB / bhwi | Bounded pinned protocol and Android adapter compile; software and mobile FFI tests pass, physical qualification open | Pinned bhwi revision; documented model/firmware/transport matrix; permission, cancellation, reconnect and app lifecycle tests pass |
| M6: signet end-to-end | Finalization and explicit broadcast pass software tests; native/hardware end-to-end open | 2-of-3 QR + USB with restart signs, validates, finalizes and explicitly broadcasts to a configured test backend |
| M7: hardened release candidate | Core protection/migration, native vaults, system backup/restore, generation switching and recovery UX pass; physical storage and release review open | Protected storage, backup/recovery/migration tests, dependency/license review, independent security review and a clean supported-device matrix |

## M1 backlog before declaring it done

- Rust compilation, rustfmt and reviewed Cargo.lock completed on Rust 1.93.1; keep required checks passing.
- Both adapters compile and execute the actual mobile Rust library; Android packages both
  arm64/x86_64 ABIs. Keep the recorded host/mobile FFI and process-restart checks passing.
- Long-list, dark/light and enlarged-text scenarios pass on both virtual platforms; broader
  accessibility, device sizes and cancellation during active network IO on physical phones
  remain to qualify.
- Public key ordering, duplicate accounts/origins, branch/policy mismatches and hardened/public
  derivation constraints have regression coverage. Descriptor/private-key rejection and
  bounded parser fuzzing pass; keep these gates and extend the corpus when supported
  policies or observed failures change.
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
- Bounded UR/BBQr codecs pass Rust tests. Native scanner/display builds and independent
  barcode tests pass; iOS uses Vision revision 2, with default revision 4 and physical
  interoperability still unqualified.
- Explicit bounded Esplora test-network sync and signed-response validation are implemented.
  Native QR and broadcast qualification remain separate acceptance gates.
- Native 100-output tab-scroll and large-text scenarios are authored for both platforms;
  Apple passes independent scroll restoration, light appearance and measured large-text controls.
  Android also passes independent offsets, light appearance and enlarged-text controls in CI.
  The contrast/system-icon corrections also pass the final native rerun. The data comes from the existing keyless
  regtest node, with isolated native vaults and no production balance-injection API.
- Native coin selection, review, PSBT file export and saved drafts are implemented; all four
  modes and interrupted-draft restart pass on both platforms. Broader layouts/accessibility
  and physical file-provider qualification remain.
- Apple layout screenshots exposed inherited orange coin text and weak light-theme contrast.
  Explicit neutral coin text and the approved accent/secondary/error/primary-button colors
  now compile and render in the passing native suite. Source color-pair calculations and
  partial rendered comparison pass; broader accessibility remains open. See the
  [public screenshots and limits](../validation/native-2026-09-09/README.md).
- Observed submissions now apply payment labels to history and wallet-owned outputs once,
  retaining their saved input provenance through edits/reorgs. Rust, migration and rollback
  tests pass; both native provenance/label-edit screens now pass. A real-device submission
  remains to qualify.

## Release blockers

Public distribution still requires an owner-selected license, complete upstream/distribution
review and reproducibility review beyond the checked dependency locks. Real-funds support
additionally requires physical qualification of protected storage and backup recovery,
verified hardware receive addresses and multisig policies, the supported-device matrix and
the end-to-end adversarial trials in `05-testing.md`. Passing signature/reorg/restore software
tests does not close those release gates.
