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
keys, bounded schema/wallet counts, a 16 MiB per-wallet state bound and supported public
descriptors. Returned summaries make no cached-balance or freshness claim. Inspection is
not a reusable authorization token and does not restore a wallet.

The versioned schema file preserves the initial schema-7 backup contract when the live app
schema advances. Future readers must explicitly validate and migrate supported old versions.
The first format has no older Tundra backup version to upgrade.

## Executed checks and pending native gate

Nine Rust tests cover password handling, encryption, public signatures/final bytes, uncertain
submissions, bounds, hostile schema changes, no-clobber installation, failure injection and
three actual process-kill boundaries. The complete local gate passes 168 Rust tests including
keyless regtest, 25 offline checks, formatting, strict Clippy and both binding generators.
The public encrypted fixture is generated on Linux/OpenSSL, pinned by hash and inspected by
Rust. New Kotlin/Swift host and mobile tests will read that same fixture and export their own
snapshot; Apple CommonCrypto interoperability is still pending. No Bitcoin signing key is
generated or imported. See [recorded checks](../validation/backup-export-checks.json).

## Restore still to implement

Do not expose export as a recoverable backup product until restore and native document
exchange pass. Restore must verify a fresh private copy, never mutate the source backup or
overwrite an unreadable existing wallet, and install into a new protected store with a
durably retained new storage key. Switching the active store must be atomic and leave the
old protected data available if interrupted.

Restored cached chain data must become unsynced. Old approvals must remain suspended until
an explicit fresh review; restoring or syncing must not silently authorize signing,
finalization or broadcast. Preserve labels, freezes, original input provenance, signatures,
final bytes and all submission history. Inputs of unresolved restored submissions need
separate durable holds; an old reservation disappearing during a reorg must not make those
coins selectable. Observing the exact transaction or an explicit validated recovery action
must resolve those holds deliberately.

An old backup cannot know receive addresses issued after it was made. A new scan cannot
discover previously issued but unused addresses; recovery UX must explain that limit instead
of promising no address reuse. Hardware receive/policy verification remains a separate gate.

Wrong passwords, corrupt/unsupported backups, disk faults, interruption at each installation
boundary, lost native keys and active network operations must be covered before exposing
restore. M7 remains incomplete.
