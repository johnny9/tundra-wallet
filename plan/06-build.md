# Build and dependency decisions

## Baseline

- Rust workspace; BDK `=3.1.0` for wallet/descriptor/PSBT operations.
- rusqlite `=0.37.0` with pinned SQLCipher `4.19.0` for the application-owned persistence
  transaction; [source/build provenance](../crates/tundra-sqlcipher/README.md).
- UniFFI `=0.32.0`; generate Kotlin and Swift from the compiled library metadata.
- Sync: reqwest `=0.12.28` with rustls (no native OpenSSL), Tokio `=1.49.0`.
  The small Esplora adapter bounds streamed bodies before parsing; BDK owns wallet state.
- Android: Kotlin/Compose, AGP `8.13.2`, Gradle `8.13`, JDK 17+, compile/target SDK 36,
  min SDK 28, Kotlin `2.2.21`, Compose BOM `2025.12.00`.
- The Gradle 8.13 wrapper was generated with the official distribution, then both the
  distribution and wrapper JAR were checked against Gradle's published SHA-256 values.
  `gradle-wrapper.properties` enforces the distribution checksum. No system Gradle is needed.
- iOS: SwiftUI starter, iOS 17+, XcodeGen project. Swift 5 language mode initially to avoid
  pretending the not-yet-generated bridge has passed strict Swift 6 concurrency checks.
- bhwi: Ledger-only core pinned to `edfac42caf0d67693660cf1f04c2d24ca6b8f7a1`;
  its Git miniscript 13 dependency is locked to `ff4732e5f75aa555682343cb180fa72ee3e8e9d5`.
  [USB implementation and qualification gates](11-usb.md) and [dependency notices](../third-party/README.md)
  record the software spike; no hardware is qualified. QR codec pins are in Cargo.toml.

The Rust workspace and binding generator compile with Rust 1.93.1 on Linux. The pinned
`rust-toolchain.toml` and CI use that version; this is a tested compiler, not an MSRV claim.
The workspace is edition 2024. Android debug and iOS simulator-target builds plus host
Kotlin/Swift FFI tests and initial emulator/simulator runtime scenarios pass in CI.
Complete native coverage and physical hardware acceptance remain open.

## Dependency resolution

Cargo.lock was updated by Cargo on September 9, 2026 and reviewed: checksummed crates.io
packages, three workspace packages and the two immutable Git revisions recorded above. Direct BDK/rusqlite/UniFFI
pins are unchanged. Build/check scripts require the committed lock and use `--locked`.

CI caches Cargo registries/Git checkouts and build artifacts by OS, architecture, job,
Rust toolchain and lockfile hash, with a fallback for the same OS/architecture/job/toolchain
when the lock changes. Cargo revalidates changed dependency/build inputs. Every build/test
command still runs with the reviewed lock; cache hits are not validation results.

Formatting and Clippy warnings are required checks. Workflow actions are now pinned to the
exact commit SHAs already executed in run 34311969023; `.github/action-pins.json` records
upstream versions and revisions, and an offline guard rejects unreviewed/floating references.
The Rust action revision explicitly selects 1.93.1. This prevents tag movement, not malicious
behavior inside an action. The Gradle wrapper and dependency baseline are pinned. SDK
downloads, cargo-ndk installation and the Xcode host still need reproducibility review.

The manually dispatched `Collect Android dependency review` workflow prepares candidate
Gradle locks (including plugin classpaths) and SHA-256 verification metadata on an Android
SDK host. It resolves every configuration's graph and external module artifacts, failing
on dependency errors, and never commits or pushes generated files. The fourth collection
at `4570492` passes. Earlier failures and corrections remain in the validation evidence.

Normal builds now require committed locks and strict checksum verification. The baseline
locks **440 distinct components** across plugin and application configurations. Verification
covers **584 metadata components / 1,006 artifacts**, including parent/BOM metadata not itself
in a resolved graph. Three metadata artifacts missing from the CI bootstrap were separately
fetched from Maven Central, identity-checked and byte-compared with the local cache; the
pinned Compose BOM was independently compared with Google Maven. These are checksums of the
reviewed downloads, not publisher signature authentication or a completed license review.

Local Gradle 8.13 configures the real Android project successfully. Isolated copies of the
same build configuration pass a baseline and refuse missing locks, disabled verification and
an altered plugin checksum. The offline checker also rejects mutable versions, unreviewed
components and broad verification exceptions. The local all-configuration check reaches the
missing Android SDK and cannot complete; normal native CI now runs that read-only gate before
building, then runs the same refusal checks. Its first full locked native run is pending.
See [exact lock/enforcement evidence](../validation/android-lock-enforcement-checks.json).

For an intentional update, dispatch the collector, inspect its artifacts and version changes,
review exact additional checksums, then commit the updated files. `verifyDependencyReview`
refuses lock/checksum-writing flags. See Gradle's [locking guide](https://docs.gradle.org/current/userguide/dependency_locking.html)
and [verification guide](https://docs.gradle.org/current/userguide/dependency_verification.html).

Both complete Cargo locks now have a deterministic declared-license/notice inventory,
including all features and the fuzz graph. CI regenerates it from locked package metadata;
offline checks verify graph coverage and retained bytes. See the [notice scope and remaining
distribution gates](../third-party/README.md). Original Tundra's license remains an owner decision.

## Build commands

- `scripts/check.sh`: required Rust formatting, all-target/all-feature tests, strict Clippy,
  and offline checks; never rewrites sources or bootstraps a missing lock.
- `scripts/generate-bindings.sh`: host library build and generated Kotlin/Swift API.
- `scripts/check-swift-ffi.sh`: macOS host smoke tests through the generated Swift binding.
- `scripts/check-regtest.sh`: actual keyless Bitcoin Core chain, maturity and reorg tests.
- `scripts/check-sqlcipher-source.sh`: reproduce and compare the vendored SQLCipher files
  from a clean checkout at the reviewed upstream revision.
- `scripts/install-bitcoin-ci.sh`: checksummed Bitcoin Core 31.1 user-local CI installation.
- `scripts/check-fuzz.sh`: bounded AddressSanitizer parser, USB protocol and signature/finalization fuzzing.
- CI audits the lock with cargo-audit 0.22.2 and the current RustSec database; a clean audit
  is not an independent review or a guarantee that dependencies have no vulnerabilities.
- `scripts/build-android.sh`: bindings + cargo-ndk libraries for arm64 and x86_64.
- `scripts/check-android-apks.sh`: reuse matching CI app/test APKs on an explicitly
  disposable ADB target; see [Waydroid setup and limits](07-waydroid.md).
- `scripts/build-ios.sh`: Apple static libraries + XCFramework; macOS/Xcode required.
- Publish commits using normal Git pushes to the configured origin; no creation helper exists.

Native generated code, binaries, local.properties, databases, label exports containing user
data, signing material and provisioning profiles do not belong in source control. Test
fixtures are explicitly public and disposable. The repository has no private keys and no
application-level analytics/cloud account.

## Upstream references consulted

These are authoritative design/API references, not evidence that this source compiled:

- BDK 3.1.0: https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/
- BDK ChangeSet persistence: https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.ChangeSet.html
- BDK TxBuilder: https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.TxBuilder.html
- bhwi: https://github.com/wizardsardine/bhwi
- UniFFI: https://mozilla.github.io/uniffi-rs/latest/
- BIP 329: https://bips.dev/329/
- Android USB host: https://developer.android.com/develop/connectivity/usb/host
- Android AGP 8.13: https://developer.android.com/build/releases/past-releases/agp-8-13-0-release-notes
- Bitcoin UI Kit: https://www.bitcoinuikit.com/

No Google/Apple account, package-name ownership, App Store name, trademark clearance or
open-source license choice is implied by the development identifiers.
