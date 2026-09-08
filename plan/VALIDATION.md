# Validation status

Planning baseline: September 7, 2026. Recorded validation: **September 8, 2026**.
This is development source with test-network sync, native payments and external PSBT validation.
**The full plan is not complete. No physical hardware or real-funds readiness is claimed.**

The later signing implementation passes **15 additional Rust tests** and the full local suite:
published single-sig/2-of-3 verification, per-input progress, mutation rejection, persistence,
rollback, migration and invalidation. Its updated Kotlin/Swift bindings were generated locally.
[Signing CI at 2d6862b](https://github.com/johnny9/tundra-wallet/actions/runs/34280039963)
passed Rust, Android (including wrong-draft signed-file rejection), parser/audit, iOS build
and both iOS native tests (including binary signed-file bounds). The iOS UI test failed on
two attempts: initial simulator launch automation, then an explicit sync failure before
payment review. Neither attempt qualifies the new iOS signature UI. Diagnostics are retained;
the test server now indexes before advertising readiness, and a failed asynchronous sync
expectation stops subsequent payment actions. Native QR changes await the next CI run.
See [signing rules and limits](08-signing.md) and `validation/signing-checks.json`.

The [final CI run for bad8721](https://github.com/johnny9/tundra-wallet/actions/runs/34272130763)
passed all four jobs: Rust, Android, iOS, and parser/dependency checks. Android passed on
its second attempt after an unrelated launcher ANR. `validation/ci-checks.json` records the
exact commit, jobs, counts and artifact IDs. The downloaded debug APK is
`build/ci-final-app/app-debug.apk`; its matching instrumentation APK is under
`build/ci-final-tests/`. Both are development test artifacts.

The later QR implementation passes **11 additional Rust tests**, bringing the executed Rust
total to **102 including real regtest**. A host AddressSanitizer run completed 1,679,070 inputs
in 61 seconds with no crashes; the first sandbox run failed during LeakSanitizer shutdown
because ptrace was restricted. The dependency audit found no advisories or warnings.
Both bindings generated successfully. Native QR compilation and independent barcode tests
are pending. See [QR limits and gates](09-qr.md) and `validation/qr-checks.json`.

## Local executed checks

| Check | Actual result |
|---|---|
| Rust workspace | **95 core unit tests + 1 process-kill integration test + 5 HTTP integration tests passed**; all targets/features compiled with Rust 1.93.1 |
| Real Bitcoin Core integration | **1 separate regtest test passed** using Core 31.1, wallets disabled; maturity at 99/100 confirmations, persistence, shorter/replacement reorgs, invalidated drafts and durable freezes |
| Formatting / Clippy | `cargo fmt --all --check` and strict all-target/all-feature Clippy passed |
| Offline schema/fixture/source checks | **23 passed**, 0 failures/errors |
| UniFFI | Kotlin and Swift bindings generated from the same compiled Rust library |
| Parser fuzzing | **2,103,214 executions in 61 seconds**, no crashes; expanded PSBT/descriptor/label/amount harness; AddressSanitizer, cargo-fuzz 0.13.2, nightly-2026-09-07; reviewed fuzz lock unchanged during the run |
| Dependency audit | cargo-audit 0.22.2: **0 reported vulnerabilities, no warnings** against the recorded RustSec database; license inventory collected, distribution-license review incomplete |
| Native test-chain repeatability | Two fresh starts produced height 103; both refused a duplicate start and removed their own ready signal/data on termination |
| Approved design integrity | Four SHA-256 references pass; rebuilding HTML from editable sources is byte-identical |
| Approved design rendering | Chrome for Testing headless shell 152.0.7977.82 rendered at 420×900; screenshot visually inspected; no browser/hardware interaction retested |

That is **91 executed Rust tests**, including the separate regtest run. The default suite
intentionally ignores the real-node test and the process-kill child helper; the regtest
script runs the former explicitly, and the parent crash test invokes the latter.

Machine-readable local evidence is in `validation/local-checks.json` and
`validation/rustsec-audit.json`. Offline results describe only their own checker.
Full logs and generated artifacts remain in ignored `build/`: `hardening-check.log`,
`regtest.log`, `fuzz.log`, `prototype-local.png`, and the repeat-chain logs.

## Native CI evidence

The [foundation run at 6b20983](https://github.com/johnny9/tundra-wallet/actions/runs/34248953834)
passed Android debug and iOS simulator builds, three Kotlin/JVM native-library tests,
and three macOS Swift FFI smoke tests. The later
[iOS run at 287e305](https://github.com/johnny9/tundra-wallet/actions/runs/34264453998)
passed actual simulator XCTest and XCUITest import, unknown-balance, restart and distinct
receive-index checks.

The [payment run at 82b14db](https://github.com/johnny9/tundra-wallet/actions/runs/34265875226)
passed Android instrumentation using real keyless regtest funding: explicit consent,
150 BTC fixture balance, two selected inputs, fractional fee review, saved draft,
Activity recreation, and real Android FFI checks of reservations and exact accounting.
Its iOS UI run failed to enable the long-label consent switch and was cancelled during
subsequent failing assertions. The test now targets the trailing switch and verifies its
value before scanning; endpoint keyboard dismissal was added to the native form.

At [ab6772e](https://github.com/johnny9/tundra-wallet/actions/runs/34268702320), Rust,
parser/audit checks and Android instrumentation passed. The new Android process-restart
step failed because Gradle removes test APKs by default. The runner now explicitly keeps
them installed. At [7a1b614](https://github.com/johnny9/tundra-wallet/actions/runs/34271090797/job/102212568312),
Android instrumentation and the real force-stop/relaunch check both passed. The captured
UI hierarchy contains the retained 150 BTC regtest balance and unsigned two-input draft.
`validation/android-runtime.json` and its accompanying XML now record the newer final
passing job and APK checksum. The overall run was cancelled after Android completed when
the next iOS UI correction was pushed.

At [bad8721](https://github.com/johnny9/tundra-wallet/actions/runs/34272130763/job/102216268897),
iOS builds, three host Swift FFI smoke tests, one simulator XCTest and one XCUITest all
passed on an arm64 iPhone 17 Pro simulator running iOS 26.2. The UI test covers import,
unknown balance, distinct receive indices, explicit consent, real regtest sync, two-coin
consolidation and a saved draft after process termination/relaunch. The previous sole
review assertion failure was SwiftUI's uppercase section title; its lookup now ignores
casing. Inspection of the screen recording also found disappearing fee units in the
populated form. BTC and sat/vB now remain visible; the simulator test checks the fee label.
`validation/ios-runtime.json` contains the exported test summary.

The same commit's first Android attempt passed instrumentation and cold-launched Tundra,
but UI inspection was obscured by a **Quickstep launcher ANR**, captured in the hierarchy.
The Android-only retry at the identical source commit passed instrumentation and the
force-stop/relaunch check. Its captured UI contains the persisted regtest balance and
saved two-input draft. The environmental failure from the first attempt remains recorded.

Android and Xcode builds run on CI hosts, not this Ubuntu checkout. Both APK ABIs are
arm64-v8a and x86_64. Native test funding comes from actual Bitcoin Core regtest blocks;
there is no app API to fabricate balances or synchronization success.

## Waydroid attempt

Waydroid booted successfully after starting user-local Weston and PulseAudio. ADB was
extracted locally without sudo. Tundra Dev from `82b14db` installed and appeared in
Waydroid's app list, but Android received no IPv4 DHCP lease and ADB was unreachable.
UFW was enabled; the operator was asked for a narrow DHCP rule on `waydroid0`. This account
cannot use sudo and changed no firewall rule. **Waydroid instrumentation has not run.**
See [setup and reusable APK test commands](07-waydroid.md).

## What the checks establish

The Rust persistence tests reopen real SQLite databases, preserve Unicode labels,
receive/change indices, freezes and unsigned drafts, roll back failed snapshot/metadata
commits, and race reservations across independent connections. The crash test kills a
child during an uncommitted write after earlier durable state. This is process-crash
recovery evidence, not a physical power-loss or storage-fault qualification.

HTTP tests exercise malformed/oversized responses, redirects, wrong networks, prompt
cancellation during stalled IO, stale snapshots after address issuance, failed atomic
commits and retention of cached balances/timestamps on failure. Sync checks block hashes
and transaction inclusion against the chosen server view; it is not an independent
proof-of-work light client. Descriptor and BIP 329 origins are checked in Rust, with
strict manual selection and mainnet gates retained.

## Unfinished gates

QR/UR/BBQr codecs pass Rust tests; native camera/display source and barcode tests await CI.
Bhwi/USB interoperability, finalization and broadcast remain **unimplemented**. Physical phones, hardware address/policy verification, all signing
pairs and the full adversarial device matrix have not been tested. Native coverage still
needs every payment mode, accessibility/text sizes and complete visual comparison.
Protected production metadata storage, recovery/migration qualification, reproducibility,
license selection/review and independent security review remain release blockers.

Logical implementation commits are pushed to the configured `johnny9/tundra-wallet` origin.
The [original handoff](https://github.com/johnny9/tundra-wallet/blob/995026c/plan/VALIDATION.md)
is preserved in Git. Its 25 offline checks became 23 after obsolete publisher tests were
removed; neither suite alone certifies Rust/native behavior or security.
