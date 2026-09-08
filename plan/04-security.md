# Security and privacy model

## Invariants

1. Import only public descriptor keys. No seed input, private-key generation, secret-key
   import, mnemonic flow or software-signing fallback exists in the app.
2. Never infer a missing change policy. Never turn unknown balances into zero.
3. Validate selected inputs in Rust immediately before transaction construction, even when
   the UI disables them. Manual-only selection cannot silently gain inputs.
4. User freezes and draft reservations are separate durable facts.
5. A signing response is untrusted input. No signature is counted until cryptographically
   verified for the correct input/key/policy and approved transaction.
6. No automatic broadcast, and no broadcast endpoint in this source milestone.
7. Never log descriptors, xpubs, addresses, labels, PSBTs or full native errors. Errors presented
   across FFI use bounded, input-independent descriptions.

## Threats and mitigations to implement/test

| Threat | Design |
|---|---|
| Malicious descriptor/JSON/QR file | Size/line limits, checksum and public-only typed parsing, narrow policy support, fuzzing |
| Changed recipient/change/fee in returned PSBT | Compare immutable approved transaction plus prevouts/policies; verify signatures; no blind combine/finalize |
| Misleading multisig progress | Track valid distinct keys per input, not device response count |
| Double-use of selected inputs | Atomic reservations, sync/reorg conflict handling, short transactions, no stale UI authority |
| Private metadata exposure | App-private storage, no backup by default, no logs/analytics, reviewed encryption and export UX before mainnet |
| Public-server address correlation | Explicit endpoint choice/consent, no labels/xpub upload, self-hosted option, documented request privacy |
| USB or camera lifecycle failure | Native permissions, bounded sessions, operation cancellation, recoverable drafts; never downgrade to software keys |
| Dependency/API drift | Pinned/tested device revision, reviewed Cargo.lock, CI, reproducible builds, audit/license checks |

## M1 limitations, not promises

SQLite state is plaintext in the app sandbox. Android uses `noBackupFilesDir`, disables
Android backup and sets FLAG_SECURE. iOS excludes the support directory from backup and
requests complete file protection. Neither measure equals an audited encrypted wallet
metadata design. iOS screenshot prevention is not asserted.

The database contains public descriptors and sensitive labels/history. An attacker who
can tamper with it can mislead the UI. Hardware address/policy verification is therefore a
required user interaction, not replaced by successful descriptor parsing.

Android now requests INTERNET for explicitly initiated sync. No endpoint is configured by
default, and opening/resuming the app makes no chain requests. The native Rust HTTP adapter
enforces transport restrictions; Android's Java networking policy is not treated as a
substitute for those checks.
Mainnet transaction construction is blocked in Rust. Offline addresses are clearly
unverified and must not be funded. Test fixture private keys are not supplied and should
be assumed unavailable.

## Signing acceptance

The software validator and atomic persistence are implemented and exercised with public signed
fixtures and adversarial tests. [08-signing.md](08-signing.md) defines the accepted format and
remaining native, hardware, finalization and broadcast gates.

Bound input size, parse PSBT versions/types explicitly, match the draft's unsigned transaction,
check outpoints/prevout scripts and values against independently known wallet state, match
all outputs/amounts and recognized change derivations, enforce allowed sequence/locktime and
sighash modes, reject conflicting fields, verify signatures for each input, and only then
persist their incorporation. Preserve already validated signatures on repeat imports.
Review finalized bytes/txid before explicit broadcast. Reject an operation that cannot prove
these invariants instead of attempting a best-effort signing shortcut.

An independent review and a test-only real-hardware trial are release gates. No claims of
security, hardware compatibility or money safety are made by the existence of this source.
