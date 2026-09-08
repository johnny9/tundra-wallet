# External PSBT validation

The Rust core accepts a signed PSBT for an existing test-network draft. The original PSBT
and review stay immutable; schema v3 stores the validated signature aggregate separately.
Import, review-state update and signature persistence share one SQLite transaction.
Reopen/export revalidate signatures. There is no finalization or broadcast API yet.

## Accepted format and policy

- Binary PSBT v0 or one base64 value, with optional surrounding whitespace. Binary maximum
  1 MiB; file maximum 1,398,106 bytes; 200 inputs, 400 outputs, 64 fields per map and 256-byte
  keys. Transaction vector lengths are checked before typed parsing allocates them.
- Reject concatenated payloads, trailing bytes/maps, duplicate fields, nonminimal sizes,
  unsupported versions, unknown/proprietary fields, taproot, preimages and wrapped SegWit.
- Only the imported native P2WPKH or 2-of-3 sorted P2WSH policy, compressed public keys,
  strict DER, low-S ECDSA and SIGHASH_ALL. No private-key parser or software signer is used.
- The complete unsigned transaction must equal the approved one, including every outpoint,
  sequence, output value/script, version and locktime. Supplied prevouts, scripts, origins,
  xpubs and output metadata must agree with the approval and independently derived wallet data.
  A hardware response may omit metadata; exports restore it from the original approval only.
- Every supplied signature is verified for its input, public key, amount, script and approved
  transaction. Final witnesses are checked against the narrow policy, including multisig
  ordering and the empty CHECKMULTISIG dummy, then normalized to verified partial signatures.
  Mixed partial/final signatures and conflicting signatures for the same key are rejected.
- Progress counts distinct valid keys **per input**. Repeated responses cannot increase it;
  previously validated signatures survive subsequent partial responses and restart.

## Current-state checks

Import/export/progress recheck the wallet's known unspent outputs, confirmation/maturity,
reservation ownership, freezes, reviewed values/fees, public derivations and draft identity.
Mainnet, unsynced wallets and invalidated drafts are rejected. A successful sync invalidates
signed drafts on the same terms as unsigned drafts; returning coins cannot revive approval.
Freezing a reserved input blocks signing use until the user unfreezes it.

The supplied endpoint is still the trusted chain view. These checks do not defend against
an attacker who can coherently replace the plaintext database, descriptors and approval.
Protected storage and hardware policy/address comparison remain release gates.

## Executed software evidence and remaining gates

Fifteen signing tests verify published HWI, Ledger and confirmed native 2-of-3 signatures,
field mutations, high-S/wrong-key rejection, witness order, duplicate imports, per-input
progress, reopen, failed-write rollback, schema-v2 migration and permanent invalidation.
The parser participates in the AddressSanitizer fuzz harness. See [VALIDATION.md](VALIDATION.md)
and `validation/signing-checks.json` for exact execution evidence.

These tests do not qualify QR codecs, scanner sessions, a hardware device, every 2-of-3
signing pair or a signet end-to-end flow. Native signed-file interaction tests, QR exchange,
policy/address verification and bhwi/USB remain separate work.

The format and signature hashing follow [BIP 174](https://bips.dev/174/) and
[BIP 143](https://bips.dev/143/). Public fixture attribution is in
[the fixture manifest](../tests/fixtures/signing-provenance.json).
