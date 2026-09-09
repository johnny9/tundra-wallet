# Protected storage and recovery

## Implemented storage boundary

`Core::open_protected` and the typed UniFFI constructor accept a 32-byte database encryption
key. These are storage keys, never Bitcoin signing keys. The native platforms will own their
generation, wrapping and retention. The legacy `open` path still serves the current apps;
adding the protected constructor does not make their existing databases encrypted.

The [pinned SQLCipher build](../crates/tundra-sqlcipher/README.md) supplies page and WAL
encryption with authenticated pages. Every connection checks the linked implementation;
plain SQLite silently ignoring a key pragma cannot produce a successful protected open.
The raw key is set before the first database read, and a real schema read validates it
before WAL setup or schema writes. Invalid-length keys, wrong keys, plaintext files,
existing empty files and tampered headers fail without deletion or reset. Protected opens
disable URI interpretation and refuse a final-path symlink.

SQLCipher logging is disabled, temporary storage stays in memory, and its memory-wiping
option is enabled. Rust clears its owned key and hex buffers on drop. This is not a claim
that all platform/UniFFI copies, process memory or OS caches are erased. Existing SQLite
transactions, full synchronization, wallet isolation and reservation rules remain in use.
The cryptographic design comes from [SQLCipher](https://www.zetetic.net/sqlcipher/design/);
Tundra does not implement a separate encryption format.

## Executed checks

Linux tests cover encrypted DB/WAL contents, Unicode labels and persistent receive indexes,
unknown balances, missing/wrong/invalid keys, tampered headers, plaintext refusal, symlink
refusal, independent connections, state/metadata rollback and keyed schema migration.
An actual child-process kill during an encrypted write preserves the last committed wallet
and label together. The complete Rust/regtest suite passes with this backend. The generated
amalgamation reproduces byte for byte from its pinned source.

Kotlin host encrypted-reopen checks pass in the first native CI run. Android's initial
protected open failed; iOS host linking failed on missing CoreFoundation symbols. The
Apple link fix is committed. A regression reproduced rejection of directory symlinks;
storage now canonicalizes the parent while still refusing a final-file symlink. This also
makes directory aliases share the same migration lock. The corrected native run and new
migration tests are pending. These use public storage-encryption test keys, not platform
key stores. No Bitcoin signing key is generated or imported.

## Explicit plaintext upgrade

The native caller must durably retain its storage key before calling `upgrade_storage`.
Every file-backed Core now holds a shared advisory lock for its lifetime; migration
requires the exclusive lock and refuses live clones or independent core connections.
The app owns the database directory and lock file. Arbitrary programs that ignore this
lock are not supported concurrent writers. A raw SQLite WAL reader also blocks the
required journal-mode transition in the regression test.

The upgrade checkpoints legacy WAL, switches to DELETE journaling, takes an exclusive
SQLite transaction and exports into a temporary encrypted database in the same directory.
It copies `user_version`, verifies authenticated pages, SQLite integrity and foreign keys,
and syncs the new file before an atomic replacement. It then syncs the containing directory.
No plaintext backup is made. Repeating after replacement validates the retained key and
does not rekey the database. An existing empty or unrecognized file never authorizes
native generation of a replacement key.

Tests retain labels, freezes, signatures, final bytes, reservations, registration metadata
and uncertain submissions. Errors and actual process kills before commit, after verification
and after replacement leave an original or complete encrypted database that reopens on retry.
Interrupted temporary files contain encrypted data; this does not prove old plaintext flash
blocks were securely erased. A complete device power-loss/storage-fault qualification remains.

The migration suite exposed a transient lock-retention failure during concurrent child
spawning. Lock guards now explicitly unlock on drop instead of depending only on descriptor
closure; the full suite passes with the child-process checks enabled.

## Next gates

- Android Keystore wrapping and iOS Keychain retention; missing/corrupt key material must
  preserve existing files and expose recovery rather than generate a replacement key.
- Native adoption of the tested plaintext migration and device interruption qualification.
- Encrypted export and restore, including wrong credentials, bounds, partial writes,
  migrations, labels/freezes/drafts and uncertain broadcasts. Restored chain state must not
  be treated as a fresh approval to spend.
- Native startup/restart/lock behavior with protected storage, accessibility and recovery UX.
- Dependency notice packaging, reproducible native builds and independent security review.

M7 remains incomplete. The app must not advertise recoverable backups or production storage
protection until the corresponding native and recovery gates pass.
