# Build and dependency decisions

## Baseline

- Rust workspace; BDK `=3.1.0` for wallet/descriptor/PSBT operations.
- rusqlite `=0.37.0` (bundled SQLite) for the application-owned persistence transaction.
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
- bhwi: **not linked in M1**. Pin an explicit revision after a hardware spike and review
  license/API/device-transport implications. QR codecs also await selection/testing.

The Rust workspace and binding generator compile with Rust 1.93.1 on Linux. The pinned
`rust-toolchain.toml` and CI use that version; this is a tested compiler, not an MSRV claim.
The workspace is edition 2024. Android debug and iOS simulator-target builds plus host
Kotlin/Swift FFI tests and initial emulator/simulator runtime scenarios pass in CI.
Complete native coverage and physical hardware acceptance remain open.

## Dependency resolution

Cargo.lock was updated by Cargo on September 8, 2026 for sync and reviewed: checksummed crates.io
dependencies, two workspace packages, no Git dependency sources. Direct BDK/rusqlite/UniFFI
pins are unchanged. Build/check scripts require the committed lock and use `--locked`.

Formatting and Clippy warnings are required checks. Third-party CI actions still use version
tags; pin their commit SHAs after review before a release. The Gradle wrapper is pinned,
but Gradle dependencies, SDK downloads, cargo-ndk installation and the Xcode host still
need reproducibility review.

## Build commands

- `scripts/check.sh`: required Rust formatting, all-target/all-feature tests, strict Clippy,
  and offline checks; never rewrites sources or bootstraps a missing lock.
- `scripts/generate-bindings.sh`: host library build and generated Kotlin/Swift API.
- `scripts/check-swift-ffi.sh`: macOS host smoke tests through the generated Swift binding.
- `scripts/check-regtest.sh`: actual keyless Bitcoin Core chain, maturity and reorg tests.
- `scripts/install-bitcoin-ci.sh`: checksummed Bitcoin Core 31.1 user-local CI installation.
- `scripts/check-fuzz.sh`: bounded AddressSanitizer parser fuzzing with a pinned nightly/tool.
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
