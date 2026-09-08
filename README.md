# Tundra

**Your bitcoin. Your hardware.**

A single-owner Bitcoin wallet with a shared Rust core and native mobile UIs.
The product manages public descriptors, labeled coins and hardware-signed payments.
It never imports or generates Bitcoin private signing keys.

> **Development source, not a released wallet. Use disposable test descriptors only.**
> This initial implementation has not been compiled in the authoring environment.
> No hardware integration, blockchain sync, signature acceptance or broadcast is enabled.
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

## What's in the first source milestone

| Area | Authored | Important limitation |
|---|---|---|
| `tundra-core` | BDK public descriptor validation, receive/change derivation, SQLite snapshots | Deliberately narrow native-SegWit policies; no sync backend |
| Metadata | Wallet-scoped transaction/address/output labels, BIP 329 patch import/export, user freezes | Known references only; origin-bearing imports are skipped; development DB is not encrypted |
| Transactions | Exact/manual and automatic eligible inputs, selected-max, consolidation, unsigned PSBT drafts and atomic reservations | Core source only; test networks; not exposed as an enabled native Send flow |
| `tundra-ffi` | Typed UniFFI API for the apps | Generated bindings require the first Rust build |
| Android | Native Compose import-by-file/paste, wallet list, unknown balance, Activity/Coins, labels and appearance | No APK built here; QR, Send, sync and USB remain disabled/unavailable |
| iOS | SwiftUI starter and actor adapter for import/list/receive with generated bindings | Not an iOS release; build and UI parity still required |
| Design | Exact approved Tundra HTML reference and source | Simulation stays in `design/`, not in the Rust/native wallet |
| Quality | Rust tests, offline schema/fixture checks, CI and build scripts | See [validation report](plan/VALIDATION.md) for executed versus unexecuted checks |

**bhwi is the selected candidate, not an implemented dependency in this milestone.**
The hardware boundary returns an explicit unavailable result. The next hardware milestone
will pin and test a bhwi revision against specific devices. QR exchange remains a separate
path. There is no stub that returns a fake signature.

## Layout

```text
crates/tundra-core/  Bitcoin decisions, persistence, metadata, unsigned drafts
crates/tundra-ffi/   App-owned bridge types; no BDK/bhwi types in native UI
apps/android/       Kotlin + Jetpack Compose
apps/ios/           SwiftUI starter + XcodeGen specification
plan/               Requirements, roadmap, risks, validation, build decisions
design/             Approved visual reference and third-party attribution
tests/fixtures/     Public test descriptors only; NEVER FUND
scripts/            Verification, binding/build and GitHub publishing helpers
```

## First build on a connected development machine

Install a current stable Rust toolchain with `rustfmt` and `clippy`, Python 3 and Git.
The first run resolves dependencies and creates `Cargo.lock`; review and commit it.
No lockfile was fabricated or copied from an unrelated project.

```sh
./scripts/check.sh
# After the first successful build: commit Cargo.lock and any cargo fmt changes.
cargo run -p tundra-core --example inspect -- tests/fixtures/single-sig.txt
```

`check.sh` runs actual Rust tests; it does not declare success if Cargo is absent.
For the subset that can run without Rust, use `python3 scripts/check_offline.py`.

### Android

Use JDK 17+, Android SDK 36, a configured Android NDK, and Gradle 8.13.
The scripts do not silently accept SDK licenses or download executables with `curl | sh`.

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
./scripts/build-android.sh
# Open apps/android in Android Studio, or:
cd apps/android && gradle --no-daemon :app:assembleDebug
```

The build script generates Kotlin bindings from this repository's compiled library and
builds the two Android ABIs. A Gradle wrapper is intentionally not faked: when Gradle 8.13
is installed, run `gradle wrapper --gradle-version 8.13`, review its distribution checksum,
and commit the wrapper. The first CI run is a compile gate, not a previously passing build.

### iOS (macOS / Xcode required)

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
./scripts/build-ios.sh
cd apps/ios && xcodegen generate
# Open Tundra.xcodeproj and select a simulator; configure your own signing team for a phone.
```

Install XcodeGen separately. No signing certificate, developer team or provisioning profile
is included. See [iOS notes](apps/ios/README.md).

## GitHub publishing

The authenticated GitHub connection identified `johnny9`, but did not expose write or
repository-creation actions. **This source has not been pushed to GitHub.**
With the GitHub CLI authenticated on your own development machine:

```sh
./scripts/publish-github.sh
```

This creates **`johnny9/tundra` as private**, initializes `main` if necessary, and pushes
this source. It refuses to overwrite an existing remote/repository. It requires your own
Git commit identity and does not place credentials in the project. Public visibility and
a project license are deliberate owner decisions, not assumed here.

## Attribution and release status

Tundra uses the approved Bitcoin UI Kit-inspired design. Preserve
[third-party notices](design/THIRD-PARTY-NOTICES.md) when reusing the visual assets.
Original project code has no distribution license selected yet; dependency and design
licenses remain their respective authors'. A license review is a release gate.
