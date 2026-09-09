# Tundra

**Your bitcoin. Your hardware.**

A single-owner Bitcoin wallet with a shared Rust core and native mobile UIs.
The product manages public descriptors, labeled coins and hardware-signed payments.
It never imports or generates Bitcoin private signing keys.

> **Development source, not a released wallet. Use disposable test descriptors only.**
> Rust tests and recorded native builds, payment modes and initial restart checks pass.
> Current backup document UI checks still fail; see the exact validation report below.
> Android's published-signature/vault submission test passes; positive submission UI remains open.
> Explicit test-network sync and external signature validation are implemented. Hardware
> signing remains unavailable. Explicit test-network broadcast has software validation;
> USB device and camera qualification remain open.
> Do not fund addresses from the fixtures or use this version with real savings.

## Start here

- [Implementation plan and actual status](plan/README.md)
- [Product requirements](plan/01-product.md)
- [Architecture and boundaries](plan/02-architecture.md)
- [Milestones and acceptance gates](plan/03-milestones.md)
- [Security / privacy model](plan/04-security.md)
- [Testing and hardware matrix](plan/05-testing.md)
- [Build and dependency decisions](plan/06-build.md)
- [Approved interactive design](design/prototype.html)

## Current implementation

| Area | Authored | Important limitation |
|---|---|---|
| `tundra-core` | BDK public descriptor validation, receive/change derivation, SQLite snapshots, bounded opt-in Esplora sync | Test networks only; chosen endpoint supplies the chain view |
| Metadata | Wallet-scoped labels, BIP 329, bulk edits, user freezes and observed-output input provenance | Core provenance tests and native builds pass; positive provenance UI validation pending |
| Transactions | Exact/manual and automatic eligible inputs, max/consolidation, saved reviews/reservations, verified external PSBT signatures and immutable final transaction bytes | All four native modes pass; positive signed submission and hardware qualification remain |
| QR exchange | Bounded UR/BBQr codecs, native camera/display and independent barcode tests | Both native barcode tests pass; physical interoperability pending |
| USB exchange | Pinned bhwi Ledger protocol, registration persistence and Android adapter source | Android native tests pass; physical USB qualification pending; signing remains blocked |
| `tundra-ffi` | Typed UniFFI API; Kotlin and Swift bindings generated and exercised against the host library | Mobile lifecycle/cancellation qualification remains |
| Android | Compose import, wallets, sync, coin selection/search, bulk metadata and payment review | Every payment mode and restart pass on the emulator; full device qualification remains |
| iOS | SwiftUI import, wallets, sync, coin selection/search, metadata and payment review | Every payment mode and restart pass on the simulator; full device qualification remains |
| Protected storage | Pinned SQLCipher, encrypted DB/WAL, atomic plaintext upgrade, platform key vaults and encrypted backup/restore | Protected startup and native generation restore pass; system document/recovery UX remain under validation |
| Design | Exact approved Tundra HTML reference and source | Simulation stays in `design/`, not in the Rust/native wallet |
| Quality | Rust tests, offline schema/fixture checks, CI and build scripts | See [validation report](plan/VALIDATION.md) for executed versus unexecuted checks |

The [development USB adapter](plan/11-usb.md) links a pinned bhwi Ledger interpreter.
It inspects public accounts, registers policies and compares issued test-network addresses.
No device is qualified; hardware signing remains gated. QR is a separate exchange path,
and all external signatures are independently validated against the approved draft.

## Layout

```text
crates/tundra-core/  Bitcoin decisions, persistence, metadata, unsigned drafts
crates/tundra-ffi/   App-owned bridge types; no BDK/bhwi types in native UI
apps/android/       Kotlin + Jetpack Compose
apps/ios/           SwiftUI starter + XcodeGen specification
plan/               Requirements, roadmap, risks, validation, build decisions
design/             Approved visual reference and third-party attribution
tests/fixtures/     Public descriptors and published signature vectors; NEVER FUND
scripts/            Verification and native binding/build helpers
```

## Build on a connected development machine

Install Rust 1.93.1 with `rustfmt` and `clippy`, Python 3 and Git. Rustup uses the committed
`rust-toolchain.toml`. The reviewed `Cargo.lock` is required; checks never resolve a missing
lock or silently update dependencies.

On Ubuntu 26.04 with the matching system Rust/Cargo already installed:

```sh
sudo apt update
sudo apt install rustfmt rust-clippy build-essential pkg-config openjdk-17-jdk curl unzip zip
```

Rustup, Android SDK/NDK and Gradle belong in user-owned directories; install those without sudo.

```sh
./scripts/check.sh
cargo run --locked -p tundra-core --example inspect -- tests/fixtures/single-sig.txt
```

`check.sh` runs actual Rust tests; it does not declare success if Cargo is absent.
It checks formatting, all workspace targets/features and Clippy with warnings as errors.
Run `cargo fmt --all` when editing Rust sources.
For the subset that can run without Rust, use `python3 scripts/check_offline.py`.

### Android

Use JDK 17+, Android SDK 36, build-tools 36.0.0 and NDK 27.2.12479018.
The committed wrapper supplies Gradle 8.13 and verifies its distribution checksum.
For local runtime checks using CI-built APKs, see [Waydroid validation](plan/07-waydroid.md).
The scripts do not silently accept SDK licenses or download executables with `curl | sh`.

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
./scripts/build-android.sh
# Open apps/android in Android Studio, or:
cd apps/android && ./gradlew --no-daemon :app:assembleDebug :app:testDebugUnitTest
```

The build script generates Kotlin bindings from this repository's compiled library and
builds the two Android ABIs. JVM tests call the host library built by the same script.
If using a custom `CARGO_TARGET_DIR`, pass its absolute `debug` path to Gradle with
`-Ptundra.hostLibraryDir=/path/to/target/debug`. Native runtime/device gates are tracked in
the [validation report](plan/VALIDATION.md).

### iOS (macOS / Xcode required)

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
./scripts/build-ios.sh
cd apps/ios && xcodegen generate
# Open Tundra.xcodeproj and select a simulator; configure your own signing team for a phone.
```

Install XcodeGen separately. No signing certificate, developer team or provisioning profile
is included. See [iOS notes](apps/ios/README.md).

## GitHub remote

This project is initialized with `origin` pointing to `git@github.com:johnny9/tundra-wallet.git`.
Use normal Git commits and pushes to that configured repository:

```sh
git push -u origin HEAD
```

No repository-creation helper is included. Repository visibility and a project license
remain owner decisions. Credentials do not belong in the project.

## Attribution and release status

Tundra uses the approved Bitcoin UI Kit-inspired design. Preserve
[third-party notices](design/THIRD-PARTY-NOTICES.md) when reusing the visual assets.
Original project code has no distribution license selected yet; dependency and design
licenses remain their respective authors'. A license review is a release gate.
