# Final transaction review

Finalization is implemented for test-network P2WPKH and 2-of-3 sorted P2WSH drafts. It does
not send a request to a hardware device or a network. [Broadcast](12-broadcast.md) is a
separate explicit action with its own consent and durable outcome.

Every finalization rechecks the immutable approval, current unspent inputs, confirmations,
maturity, reservations, freezes, public descriptor derivations and all stored signatures.
Every input must independently meet its threshold. Witnesses use the verified compressed
public key for P2WPKH, or the first two verified keys in script order plus the exact multisig
script for P2WSH. The assembled final witnesses pass the response validator again.
The unsigned transaction, fee, recipient/change and transaction ID cannot change.

Schema v4 adds `finalized_drafts`, scoped by wallet and draft with a composite foreign key.
Exact transaction bytes and the `finalized` review state commit atomically. The signature
aggregate cannot change after finalization. Reopening a final review reassembles and compares
the transaction with the saved bytes; missing/altered bytes, invalid signatures, freezes,
missing reservations and sync invalidation fail closed. Finalizing again is idempotent.
Discard removes the final record with its draft and releases only that draft's reservations.
It cannot revoke a signature or transaction already exported elsewhere.

The native review shows the transaction ID, fee and actual virtual size. Finalization leaves
inputs reserved. No success text implies a broadcast or confirmation.

## Executed software checks

Six additional Rust tests cover exact equality with a publicly confirmed 2-of-3 transaction,
published single-sig witnesses, every-input completeness, restart, atomic rollback, exact
reservation preservation, missing/altered persisted data, freezes and schema-v3 migration.
The existing sync invalidation test now covers both signed and finalized states.
The full suite, real keyless regtest, formatting, Clippy and binding generation pass locally;
`validation/finalization-checks.json` records counts and log hashes.

Both native positive signed-file and final-review UI tests pass, including per-input
signature progress and equality with the pinned published transaction. Unsigned-draft
finalization remains rejected. See [current native results](VALIDATION.md); this is not
a real-device signing/broadcast test.

A dedicated signature/finalization AddressSanitizer target completed 658,421 cases in
61 seconds with no crashes on September 9, 2026. It mutates published responses and final
witnesses, checks incomplete-finalization refusal, immutable transaction fields and final
witness revalidation. `validation/signature-fuzz-checks.json` records all three fuzz targets
and the full Rust/regtest/binding rerun. This target does not fuzz database persistence.
