# Encrypted backup and recovery

## Implemented export and inspection

The Rust core exports a consistent snapshot of every wallet and its metadata into a new
SQLCipher file. It uses a password separate from the platform-retained storage key. A
versioned prefix preserves exact UTF-8 password bytes and prevents raw-key literal handling.
Passwords must have at least 16 characters, at most 1024 UTF-8 bytes and no controls;
this length check is not a password-strength guarantee. Native UX still needs password
confirmation and clear advice to retain a strong, unique password separately.

Profile 1 explicitly sets 4096-byte pages, PBKDF2-HMAC-SHA512 at 256,000 iterations,
HMAC-SHA512 authenticated pages and no plaintext header, using the pinned SQLCipher provider.
No separate encryption primitive or custom cipher is introduced. See the
[SQLCipher API](https://www.zetetic.net/sqlcipher/sqlcipher-api/).

An immediate SQLite transaction copies the live snapshot, schema version and authenticated
manifest. The output uses DELETE journaling and full synchronization. After detach, the
core independently decrypts and validates the file, syncs it, installs it without replacing
an existing path and syncs its directory. Temporary exports are encrypted. Failures leave
the live wallet untouched; errors after installation may leave a complete export which can
be inspected. A native document writer must copy only the completed ciphertext and handle
document-provider partial writes explicitly.

Inspection opens read-only within a read transaction. It refuses wrong credentials, final
symlinks, non-self-contained files with journal sidecars, files outside 4 KiB–256 MiB, changed
schemas and unsupported versions. It checks authenticated pages, SQLite integrity, foreign
keys, bounded schema/wallet counts (1,000 wallets and 10,000 drafts), a 16 MiB per-wallet
state bound and supported public descriptors checked before loading BDK state. Returned summaries make no cached-balance or freshness claim. Inspection is
not a reusable authorization token and does not restore a wallet.

The versioned schema file preserves the initial schema-7 backup contract when the live app
schema advances. Future readers must explicitly validate and migrate supported old versions.
The first format has no older Tundra backup version to upgrade.

## Implemented protected restore

Restore copies bounded ciphertext to a private temporary file and validates that copy. The
original is never modified. It exports into a new SQLCipher store using a caller-provided
32-byte storage key, migrates supported schema 7 to schema 8, removes the backup manifest,
verifies the result independently and installs without replacing an existing destination.
The native caller must durably retain the new storage key before calling restore.

Restored cached chain data becomes unsynced and every old review is invalidated. Labels,
freezes, input provenance, signatures, final bytes and submission history remain. Separate
recovery holds keep inputs of unresolved submissions unavailable even when a reorg clears
ordinary reservations. Observing the exact transaction resolves its holds. Explicit recovery
resumption requires a fresh chain view, current attempt/transaction identity, acknowledgement,
and full signature, approval and eligibility validation. It changes no final bytes and
performs no network request. Competing resumption requests cannot both succeed.

## Executed checks and pending native gate

Nine export tests and eleven restore tests cover passwords, encryption, published public
signatures, uncertainty, bounded hostile input, no-clobber races and actual process kills
at three export and three restore boundaries. The complete local gate passes 179 Rust tests
including keyless regtest, 26 offline checks, formatting, strict Clippy and both binding
generators. No Bitcoin signing key is generated or imported. See
[restore evidence](../validation/backup-restore-checks.json).

Kotlin/Swift host and mobile export/inspection tests pass, including the pinned Linux/OpenSSL
fixture read on Apple CommonCrypto. Native restore assertions, normal protected startup and all payment/restart flows pass
on both virtual platforms at `03d3732`. See
[native restore evidence](../validation/backup-restore-native-passing-checks.json).

## Active store selection and native recovery

The core resolves a bounded, metadata-free selector in app-private storage. An explicit
restore holds an exclusive selection lock, records the existing default before allocating
a unique candidate, and keeps every old/candidate directory. Only a successful restore can
activate. Activation reopens the installed protected database, syncs it and its directory,
then atomically replaces the selector and syncs the parent. Errors after replacement may
mean the new selection won: native callers must reopen selection rather than continue an
old core handle. Missing/corrupt selectors and absent/aliased generations fail closed.
An explicit successful restore can replace a missing or malformed regular selector.

Android and iOS vault methods retain a new generation's storage key before restoration and
mark it initialized before activation. The selected generation must have both an existing
protected database and initialized key record; opening it cannot create either. Native apps
serialize wallet operations and must finish active work and release the old core before
switching. Arbitrary concurrent app-data writers are unsupported. The selector contains no
wallet metadata and is not an anti-rollback mechanism: a privileged attacker replaying old
app data remains outside the storage freshness guarantee.

Seven selection tests pass, including injected faults and four actual process-kill
boundaries. The complete gate now passes 186 Rust tests, 26 offline checks and both bindings.
Native lost-key/selector/generation tests and explicit submission recovery controls are
authored and await CI. See [selection evidence](../validation/store-selection-checks.json).

Native document exchange and the positive recovery-review UI scenario remain to implement
and validate before exposing backup as a recoverable product. Password confirmation,
provider partial-write handling and interrupted active operations need coverage. Recovery
must explain that an old backup cannot know receive addresses issued after it was made:
a new scan cannot discover previously issued but unused addresses. Hardware receive/policy
verification remains separate. M7 remains incomplete.
