# Dependency notices

This directory supplements the design notices and public fixture licenses. It is not a
complete release license bundle. Original Tundra source still has no distribution license
selected by its owner; third-party code retains its own terms.

The development USB implementation links bhwi's Ledger-only core at
`edfac42caf0d67693660cf1f04c2d24ca6b8f7a1` (BSD 3-Clause, Wizardsardine 2025). Its license
is retained in `licenses/bhwi.txt`. The selected upstream miniscript revision
`ff4732e5f75aa555682343cb180fa72ee3e8e9d5` declares CC0-1.0; its license is retained in
`licenses/rust-miniscript.txt`. Other packages newly resolved with bhwi are base64ct 1.8.3
and thiserror/thiserror-impl 1.0.69, each Apache-2.0 OR MIT. The lockfile records checksums.

The upstream bhwi async HID implementation was reviewed but not copied or linked. Tundra's
framing requires full reports and never treats old receive-buffer bytes as fresh input.
The app uses Android's USB API directly through its native adapter.

A complete dependency/design/fixture inventory, binary notice packaging and distribution
review remain release gates. These notes do not grant rights to project branding or imply
hardware-vendor endorsement.

SQLCipher 4.19.0's reviewed source and BSD 3-Clause license are retained under
[`crates/tundra-sqlcipher`](../crates/tundra-sqlcipher/README.md), including source hashes and
reproduction instructions. Linux/Android link OpenSSL 3.6.3 through openssl-sys 0.9.117 and
openssl-src 300.6.1+3.6.3. The OpenSSL Apache-2.0 license and Rust wrapper licenses are
retained in `licenses/openssl*.txt`. Apple builds use CommonCrypto/Security.framework.
