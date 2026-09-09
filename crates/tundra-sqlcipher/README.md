# SQLCipher build boundary

This crate compiles the reviewed SQLCipher amalgamation for Tundra. It contains no wallet
logic or key-handling API. The C source is third-party code; Rust's `unsafe_code = forbid`
does not validate that implementation.

- Upstream: <https://github.com/sqlcipher/sqlcipher>, tag `v4.19.0`.
- Git revision: `c4b275a47932888216bade83aff2bbc73df0ff85`.
- SQLCipher: `4.19.0 community`; underlying SQLite: `3.53.4`.
- Source manifest: `bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc`.
- `vendor/sqlite3.c` SHA-256: `8640c653acadf665cce6331646f60b5b74a4690746f2c4a2d8f688a0570a0c0c`.
- `vendor/sqlite3.h` SHA-256: `8a9d1bff44d75174ca6dea3ea9bac50a6104d86facb566647b8bb839375b7b3a`.
- The upstream BSD 3-Clause license is retained in [vendor/LICENSE.md](vendor/LICENSE.md).

`build.rs` verifies both hashes before compiling. Apple targets use CommonCrypto and
Security.framework. Other targets use pinned `openssl-sys 0.9.117` with vendored OpenSSL
(`openssl-src 300.6.1+3.6.3` in both locks). No system development package is needed.
Both providers compile SQLCipher's documented initialization/shutdown hooks, serialized
SQLite threading and memory-only temporary storage; extension loading is disabled.

rusqlite `0.37.0` / libsqlite3-sys `0.35.0` remain pinned. Their `in_gecko` feature emits
the bundled modern bindings without compiling or linking another SQLite implementation.
Tundra supplies this native library, and every core connection verifies the exact runtime
SQLCipher and SQLite versions before wallet access. This non-default linkage is covered
by the Rust suite and native FFI tests; preserve those checks on dependency upgrades.

To reproduce the generated files, obtain a clean checkout at the revision above, then run
`scripts/check-sqlcipher-source.sh /path/to/sqlcipher`. It uses the upstream generator in
a disposable build directory and compares both files byte for byte. A C compiler, make
and the upstream bundled Tcl interpreter generator suffice; OpenSSL headers are not
required for this source-generation check. The September 9 Linux reproduction passed.

The earlier build probes found SQLCipher 4.6.1 in rusqlite 0.37's bundled feature and
4.14.0 in rusqlite 0.40.2's. The current upstream maintenance release fixes migration and
export escaping issues, so those older embedded copies were not adopted.
See the [upstream release notes](https://www.zetetic.net/blog/) and
[API documentation](https://www.zetetic.net/sqlcipher/sqlcipher-api/).

Cargo audit covers Rust advisories, including the OpenSSL wrapper. It does not independently
audit the vendored SQLCipher C code or the Apple provider. Upstream advisory monitoring,
native builds and independent security review remain release requirements.
