# Dependency notices

This directory supplements the design notices and public fixture licenses. It is not a
complete release license bundle. Original Tundra source still has no distribution license
selected by its owner; third-party code retains its own terms.

[`cargo-inventory.json`](cargo-inventory.json) now covers **all 243 third-party packages**
in the application and fuzz locks with all features, including build/test tools and every
resolved platform. It records package checksums, declared licenses, authors, graph membership
and hashes/links to **165 distinct retained notice texts**. This is a source inventory, not
a claim that every package ships in every mobile binary.

`python3 scripts/cargo-notices.py` regenerates and compares the inventory using cached,
locked Cargo metadata. Add `--fetch` to permit missing package downloads or `--write` to
write a reviewable update. CI compares it with the actual resolved graphs. The offline
checker independently requires coverage of both complete locks and exact retained bytes.
Notice files keep their original line endings and copyright/license text.

Some archives omit license files. [`cargo-notice-supplements.json`](cargo-notice-supplements.json)
records the alternatives precisely: BDK and UniFFI root licenses at their packaged VCS
revisions, the shared rust-bitcoin CC0 text (without asserting an unavailable package VCS
revision), and the standard SPDX MITNFA text for `hex_lit`, whose package author remains in
the inventory. These supplements do not reconstruct a missing copyright notice or choose
Tundra's license. Packaged `AUTHORS` and `LICENSES/` texts are retained where applicable.

The development USB implementation links bhwi's Ledger-only core at
`edfac42caf0d67693660cf1f04c2d24ca6b8f7a1` (BSD 3-Clause, Wizardsardine 2025). Its license
is retained in `licenses/bhwi.txt`. The selected upstream miniscript revision
`ff4732e5f75aa555682343cb180fa72ee3e8e9d5` declares CC0-1.0; its license is retained in
`licenses/rust-miniscript.txt`. Other packages newly resolved with bhwi are base64ct 1.8.3
and thiserror/thiserror-impl 1.0.69, each Apache-2.0 OR MIT. The lockfile records checksums.

The upstream bhwi async HID implementation was reviewed but not copied or linked. Tundra's
framing requires full reports and never treats old receive-buffer bytes as fresh input.
The app uses Android's USB API directly through its native adapter.

[`android-inventory.json`](android-inventory.json) covers all **585 Gradle verification
components** and retains **585 exact Maven POMs**, including their parent metadata. License
declarations are followed through explicit Maven parent inheritance. All retained POM hashes
are now included in Gradle verification: the initial 328 matched that baseline, and 257
supplemental POMs were independently collected from the official configured repositories,
retained for review and then added as exact checksum entries. This covers fresh-cache parent
metadata resolution without trusting whole groups or skipping verification.
Four parent-only metadata components (`jvnet-parent` 1/3 and `oss-parent` 7/9) have no effective
license declaration; the inventory records that absence explicitly.

`python3 scripts/android-notices.py` compares the inventory and retained bytes offline.
`--fetch --write` prepares an intentional update for review. This declared-license inventory
does not choose an alternative license or grant distribution permission for Tundra.

[`android-artifact-inventory.json`](android-artifact-inventory.json) separately covers **138
checksum-verified JAR/AAR variants** for the **150 release-runtime graph components**. It retains
**five distinct embedded notice texts** and **116 verified Gradle module documents** used to
resolve logical artifact names to their actual repository filenames. Nested AAR JARs are
inspected too. **54 archives have no embedded notices; 48 of those contain compiled classes
or native libraries.** Those absences remain explicit upstream/license review work. This
set includes known variants such as metadata and desktop JNA, not a claim that every archive
ships in the APK.

`python3 scripts/android-artifact-notices.py` checks the graph, hashes and retained notices
offline. `--collect --fetch --write` downloads pinned archives and extracts notices for review;
`--collect` can repeat extraction from the ignored local archive cache. SDK/build-tool review,
remaining upstream notices and binary notice packaging remain release gates.

A complete SDK/design/fixture inventory, other nested vendored notices, binary notice
packaging and distribution review remain release gates. These notes do not grant rights to
project branding or imply hardware-vendor endorsement.

SQLCipher 4.19.0's reviewed source and BSD 3-Clause license are retained under
[`crates/tundra-sqlcipher`](../crates/tundra-sqlcipher/README.md), including source hashes and
reproduction instructions. Linux/Android link OpenSSL 3.6.3 through openssl-sys 0.9.117 and
openssl-src 300.6.1+3.6.3. The OpenSSL Apache-2.0 license and Rust wrapper licenses are
retained in `licenses/openssl*.txt`. Apple builds use CommonCrypto/Security.framework.
