# Build and dependency decisions

## Baseline

- Rust workspace; BDK `=3.1.0` for wallet/descriptor/PSBT operations.
- rusqlite `=0.37.0` (bundled SQLite) for the application-owned persistence transaction.
- UniFFI `=0.32.0`; generate Kotlin and Swift from the compiled library metadata.
- Android: Kotlin/Compose, AGP `8.13.2`, Gradle `8.13`, JDK 17+, compile/target SDK 36,
  min SDK 28, Kotlin `2.2.21`, Compose BOM `2025.12.00`.
- iOS: SwiftUI starter, iOS 17+, XcodeGen project. Swift 5 language mode initially to avoid
  pretending the not-yet-generated bridge has passed strict Swift 6 concurrency checks.
- bhwi: **not linked in M1**. Pin an explicit revision after a hardware spike and review
  license/API/device-transport implications. QR codecs also await selection/testing.

These are source selections, not a known-good compiled matrix. BDK source was checked, but
a local toolchain was unavailable. Compiler compatibility and FFI generation are first gates.
The workspace is edition 2024; no minimum supported compiler has been validated. CI uses
stable initially, to be replaced with the exact first validated toolchain before a release.

## Dependency resolution

Cargo.lock is intentionally not ignored. It is absent because Cargo/network resolution
could not run here; do not fabricate a lockfile or claim reproducibility without one.
The first connected `scripts/check.sh` resolves a lock, formats and tests; commit the
reviewed lock and formatter changes. Subsequent builds use `--locked`.

Initial CI can generate and upload the lock for inspection, but this is not an acceptable
release process until that lock is reviewed and committed. Production CI should then fail
rather than resolve an absent lock. Pin third-party CI action commit SHAs after review;
version tags in the starter workflow are a bootstrap convenience, not a supply-chain claim.

## Build commands

- `scripts/check.sh`: first Rust/format/test pass and offline checks.
- `scripts/generate-bindings.sh`: host library build and generated Kotlin/Swift API.
- `scripts/build-android.sh`: bindings + cargo-ndk libraries for arm64 and x86_64.
- `scripts/build-ios.sh`: Apple static libraries + XCFramework; macOS/Xcode required.
- `scripts/publish-github.sh`: deliberate private repository creation/push from an authenticated
  development machine; never overwrites a pre-existing repository.

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
