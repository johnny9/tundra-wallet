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
fixture read on Apple CommonCrypto. Native restore assertions are authored and await CI.
Normal iOS protected startup and payment/restart pass; Android's full scenario needs its
remaining test-only plaintext reads changed to the vault. See
[native startup evidence](../validation/protected-startup-checks.json).

## Native recovery still to implement

Do not expose export as a recoverable backup product until native document exchange and
restore pass. Restore must use a new protected generation with a durably retained storage
key. Active-store selection must switch atomically and retain the old protected data if
interrupted, including when its native key is unavailable. Lost or malformed selectors must
not silently create an empty wallet. Active network work must finish before switching.

Native recovery must show suspended reviews and make resumption an explicit fresh review.
An old backup cannot know receive addresses issued after it was made. A new scan cannot
discover previously issued but unused addresses; recovery UX must explain that limit instead
of promising no address reuse. Hardware receive/policy verification remains a separate gate.

Wrong passwords, corrupt/unsupported backups, disk faults, interruption at installation and
selection boundaries, lost native keys and active network operations need native coverage
before exposing restore. M7 remains incomplete.
