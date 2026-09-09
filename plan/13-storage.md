# Protected storage and recovery

## Implemented storage boundary

`Core::open_protected` and the typed UniFFI constructor accept a 32-byte database encryption
key. These are storage keys, never Bitcoin signing keys. Native platforms own their
generation, wrapping and retention. Both key vaults pass isolated platform tests; `322bf83`
connects them to normal app startup and explicit plaintext upgrade. Protected startup
and all payment/restart flows now pass on both virtual platforms at `03d3732`. Legacy `open` remains for test fixtures and migration checks.

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

The next native run showed that Rust 1.93's standard file-lock API is unsupported on Android,
preventing every file-backed core open. The implementation now uses pinned rustix 1.1.4's
safe flock wrapper on Unix and distinguishes contention from other storage errors. This
retains shared/exclusive locking rather than bypassing it. The behavior is explicit in the
[pinned standard-library source](https://github.com/rust-lang/rust/blob/1.93.1/library/std/src/sys/fs/unix.rs).
Apple's host link succeeds after adding CoreFoundation, but iOS linking then exposed unequal
C/Rust deployment targets; build scripts now set iOS 17.0 and macOS 11.0 for both compilers.
The full Linux suite passes with these fixes; the corrected native run is pending.

## Native key retention and remaining gates

Android and iOS key-store adapters and isolated runtime tests pass at `d441d6c`. Android
wraps a random 32-byte storage key and initialization marker with a
non-exportable Keystore AES-GCM key; only the authenticated packet is written to private
storage with file/directory synchronization and atomic replacement. iOS stores the same
key/marker in a non-synchronizing Keychain item with `WhenUnlockedThisDeviceOnly` access.
Both retain a pending key before creating/upgrading the DB, mark initialization only after
the protected core opens, and refuse lost/corrupt key material or a missing DB after that
marker is set. Existing empty database files are still refused, including during interrupted
setup. Both are now connected to normal startup in source; the app shows a persistent
unavailable/retry state until protected open and the first wallet read succeed. It cannot
offer a fresh-wallet flow after a storage failure. The startup tests and complete payment/restart scenarios pass at `03d3732`.

Android uses `setUnlockedDeviceRequired(true)` on API 35+, following Google's documented
API 31–34 bugs; older targets use credential-encrypted app storage and an explicit keyguard
check. This does not claim hardware-backed key storage on an emulator, or removal of an
already-open SQLCipher key from memory when the screen locks. See
[Android's API guidance](https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec.Builder#setUnlockedDeviceRequired(boolean))
and [Apple's Keychain accessibility](https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly).

- Run protected normal startup, all payment modes and restart after adopting the tested
  vaults. Physical device interruption and lock qualification remain separate.
- Encrypted export/inspection/restore and native provider checks pass. Atomic generation
  selection passes core tests; native generation keys and explicit recovery review await
  CI. Complete document exchange and recovery UX; see [14-backup.md](14-backup.md).
- Native startup/restart/lock behavior with protected storage, accessibility and recovery UX.
- Dependency notice packaging, reproducible native builds and independent security review.

M7 remains incomplete. The app must not advertise recoverable backups or production storage
protection until the corresponding native and recovery gates pass.

The historical native failures above are retained in the validation report. The subsequent
run at `d441d6c` passes both platform vaults: Android uses supported `fstat`/`S_ISDIR` before
directory fsync; iOS Keychain access succeeds with app-scoped entitlements and ad hoc
simulator signing. Normal protected startup, all payment modes and restart pass at `03d3732`.
