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
before WAL setup or schema writes. Invalid-length keys, wrong keys, plaintext files and
tampered headers fail without deletion, reset or implicit migration. Protected opens
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

Matching Kotlin host, Swift host, Android runtime and iOS runtime encrypted-reopen tests
are authored; their first CI build/run is pending. These use public storage-encryption test
keys, not platform key stores. No Bitcoin signing key is generated or imported.

## Next gates

- Android Keystore wrapping and iOS Keychain retention; missing/corrupt key material must
  preserve existing files and expose recovery rather than generate a replacement key.
- An explicit, atomic plaintext migration with no live competing core connection, verified
  encrypted output and interruption tests. Do not claim old flash blocks are securely erased.
- Encrypted export and restore, including wrong credentials, bounds, partial writes,
  migrations, labels/freezes/drafts and uncertain broadcasts. Restored chain state must not
  be treated as a fresh approval to spend.
- Native startup/restart/lock behavior with protected storage, accessibility and recovery UX.
- Dependency notice packaging, reproducible native builds and independent security review.

M7 remains incomplete. The app must not advertise recoverable backups or production storage
protection until the corresponding native and recovery gates pass.
