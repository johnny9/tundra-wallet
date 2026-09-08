# Handoff validation

Planning baseline: September 7, 2026. This report distinguishes executed checks from
source that has not been compiled. **No published repository or installable app is claimed.**

| Check | Actual result |
|---|---|
| Offline SQLite / public fixture / source / publisher-safeguard tests | **25 passed**, 0 failures, 0 errors; `python3 scripts/check_offline.py` |
| Approved HTML integrity | Byte-for-byte match with supplied Tundra prototype |
| Rebuild of HTML from included editable sources | Byte-for-byte match with approved HTML |
| Included browser JavaScript | 8 files passed Node syntax checks; no browser/hardware behavior was retested in this handoff |
| Swift sources | 3 files passed `swiftc -frontend -parse`; grammar only, not SwiftUI/FFI typechecking or an iOS build |
| Build specifications | Cargo TOML and 2 YAML files parsed; shell scripts passed `bash -n` |
| Rust unit tests | **49 authored, NOT RUN**; no Cargo/rustc in the environment |
| Cargo lockfile | NOT generated; dependency resolution was unavailable |
| Generated UniFFI bindings | NOT generated; require a compiled Rust library |
| Android build / emulator / physical phone | NOT RUN; Android toolchain absent |
| iOS Xcode build / simulator / physical phone | NOT RUN; no Xcode host |
| QR / USB / bhwi device interoperability | NOT IMPLEMENTED OR TESTED |
| Signing, finalization and broadcast | NOT IMPLEMENTED; no success fallback |
| GitHub repository creation / push | NOT PERFORMED; connector had no write/create actions and no authenticated CLI was available |

## What the offline checks prove—and do not prove

The SQLite checks execute the actual `schema.sql` and exercise wallet isolation,
reservation uniqueness, composite foreign keys, deletion semantics, label bounds, and
state/metadata rollback. They do not exercise the Rust transaction code.

The fixture checks independently verify descriptor checksums and test xpub Base58Check
serialization/origin depth. They are not a replacement for BDK parser or derivation tests.

Source guards flag accidental removal of the manual-selection/mainnet gates and check
platform file layout/permissions. They do not prove an application is secure. The publisher
refusal checks use fake CLI executables and perform **no GitHub writes**.

## Environment and first follow-up gate

Git and a Swift syntax parser were available. Cargo/rustc, GitHub CLI and the Android
build toolchain were not. Dependency fetching could not be performed in this environment.
The next developer action is an actual connected build via `scripts/check.sh`, followed
by both native binding/app builds and review/commit of Cargo.lock. Expect the first
compiler pass to identify integration or API issues; this is not a validated release.

Machine-readable offline results are stored in `validation/offline-checks.json`.
