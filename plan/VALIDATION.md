# Validation status

Planning baseline: September 7, 2026. Recorded validation: **September 9, 2026**.
This is development source with test-network sync, native payments and external PSBT validation.
**The full plan is not complete. No physical hardware or real-funds readiness is claimed.**

## Current software checkpoint

Encrypted backup/restore at `de6b619` passes **179 Rust tests including real keyless
regtest**, **26 offline checks**, formatting, strict Clippy and both binding generators.
Restore verifies a private encrypted copy, retains the source, migrates the pinned schema-7
fixture to schema 8 and installs only into a new protected path. Cached balances become
unknown, reviews are suspended and unresolved submission inputs remain held across reorgs.
Explicit resumption revalidates freshness, signatures, approved bytes and the latest attempt.
Actual process kills cover three restore boundaries. Native restore assertions are authored;
store switching, document exchange and recovery UI remain open. See
[`backup-restore-checks.json`](../validation/backup-restore-checks.json) and [backup rules](14-backup.md).

The [protected-startup run at 679119f](https://github.com/johnny9/tundra-wallet/actions/runs/34304964407)
passes Apple builds, **6 host FFI checks and 10 runtime tests**, including normal Keychain
protected startup, all payment modes/restart, and Linux/OpenSSL backup inspection plus native
CommonCrypto export. Android builds and passes **8 JVM and 10 of 11 instrumentation tests**,
including backup/Keystore checks and initial protected app startup. Its wallet scenario timed
out at descriptor review; `0011762` scrolls that button into view.

The [next run at 0011762](https://github.com/johnny9/tundra-wallet/actions/runs/34305883074)
passes iOS, Rust and parser/audit jobs. Android again passes 10 of 11 instrumentation tests:
the wallet scenario now passes import, receive, recreation and appearance, then correctly
fails a test-only plaintext database read against the protected file. `042c4ce` changes all
five such reads to the app's Keystore vault. Cold restart did not run after these failures.
See [`protected-startup-checks.json`](../validation/protected-startup-checks.json).

The earlier [fully green vault run at d441d6c](https://github.com/johnny9/tundra-wallet/actions/runs/34303410689)
passes **7 Android JVM tests, 10 instrumentation tests and cold restart**; Apple builds,
**5 host FFI checks and 8 runtime tests**; Rust, all sanitizer targets and audit. It tested
isolated native vaults before normal protected startup was connected.
See [`storage-vault-passing-checks.json`](../validation/storage-vault-passing-checks.json).

## Previous provenance and vault checkpoints

Output provenance at `eea54c5` passes **159 Rust tests including real keyless regtest**,
**24 offline checks**, formatting, strict Clippy and generation of both bindings. Schema 7
applies the saved payment label to history and wallet-owned outputs only after sync observes
the exact submitted transaction. A durable marker prevents later scans from overwriting
user edits or restoring removed labels. Input history, rollback, wallet isolation and schema-6
upgrade are tested. Native coin-detail presentation is authored and awaits its build/runtime
gate. See [`output-provenance-checks.json`](../validation/output-provenance-checks.json).

The [native vault run at 6be8612](https://github.com/johnny9/tundra-wallet/actions/runs/34301737203)
passes Rust, all sanitizer targets and audit. Android Rust ABI builds pass, but Kotlin stops
on an unavailable `OsConstants.O_DIRECTORY`; no JVM/instrumentation tests run. `98f01ed`
uses supported descriptor inspection before directory synchronization. Apple host/FFI and
iOS builds pass, as do **six runtime tests**, including protected storage, plaintext upgrade
and the existing all-mode/restart UI scenario. **Two Keychain tests fail** with a bounded
storage error. `93931ac` adds app-scoped Keychain entitlements, ad hoc simulator signing and
a non-secret status probe; the unsigned host is a suspected cause, not a confirmed diagnosis.
Both fixes await the next run. Normal startup still uses legacy plaintext storage.
[`storage-vault-native-checks.json`](../validation/storage-vault-native-checks.json) retains
the exact source, results and log hashes.

## Previous storage checkpoints

The SQLCipher storage boundary and atomic plaintext migration pass **155 Rust tests including real regtest**,
23 offline checks, formatting, strict Clippy and generation of both bindings. Added checks
cover encrypted DB/WAL, wrong/missing keys, tampering, symlink refusal, rollback, keyed
schema migration, live-connection exclusion, directory aliases and actual process kills
during encrypted writes and plaintext upgrade. The current apps
still use legacy plaintext storage; native platform keys and recovery are not implemented.
Native key retention is the next implementation gate. See [13-storage.md](13-storage.md).

The [migration-native run at 4fbbb67](https://github.com/johnny9/tundra-wallet/actions/runs/34300425464)
passes Rust, all sanitizer targets, audit, Android ABI/app builds and **7 JVM tests**.
Android instrumentation fails both storage tests because Rust 1.93's standard file locking
is unsupported there; the wallet test subsequently times out on failed startup. Five other
instrumentation tests pass, and cold restart does not run. Apple host linking/bindings now
pass, but arm64 iOS linking fails on a stack-probe symbol because C and Rust chose different
minimum OS versions. `efa7296` uses Android-supported rustix locks and explicitly aligns
Apple deployment targets; all 155 local Rust tests and required checks pass with the fix.
[`storage-migration-native-checks.json`](../validation/storage-migration-native-checks.json)
preserves the failures and corrections. The new native Keystore/Keychain adapters and
isolated tests at `555f1d5` also await CI; normal app startup still uses legacy storage.

The [first native storage run at b8d4bf7](https://github.com/johnny9/tundra-wallet/actions/runs/34298929142)
passes Rust, parser/audit, both Android ABI builds and 6 host JVM tests. Android passes six
existing instrumentation tests but fails the new protected open; cold restart did not run.
iOS fails during macOS host-library linking because the Apple provider needs CoreFoundation;
Swift/iOS builds and tests did not run. That link is added in `721bd76`. The migration change
`ccfd24a` also fixes a reproduced directory-alias failure while retaining final-file symlink
refusal. Both corrections and the expanded migration FFI tests await the next native run.
Exact initial failures remain in
[`storage-native-initial-checks.json`](../validation/storage-native-initial-checks.json).

The migration checks at `ccfd24a` include three actual child-kill boundaries and ten repeated
concurrent storage runs after an explicit-unlock fix. No data reset is used to recover.
[`storage-migration-checks.json`](../validation/storage-migration-checks.json) records the
software evidence and its limits.
At storage source `a13a0ff`, all three sanitizer targets passed: 1,626,888 parser, 1,256
USB protocol and 642,399 signature/finalization cases in 61 seconds per target. Cargo audit
reported no advisories or warnings; SQLCipher C review is a separate gate. Source reproduction
passed byte for byte. [`storage-core-checks.json`](../validation/storage-core-checks.json)
records commands, source/lock identities and log hashes.

The preceding native run at source `959224a159bef4ab18465f2565d139db6229c1ad` is fully green:
[CI run 34296820197](https://github.com/johnny9/tundra-wallet/actions/runs/34296820197).
Android built both ABIs, passed 5 JVM and **6 instrumentation tests**, then passed the cold
restart check without launcher ANR recovery. All four payment modes, QR, USB boundaries and
the explicit broadcast-consent dialog passed. iOS built and passed **4 tests**, including
all payment modes and process restart. Its independent QR decoder still used Vision
revision 2. Rust (138 tests at that source), all three fuzz targets and audit also passed.
Exact reports are retained in
[`broadcast-native-passing-checks.json`](../validation/broadcast-native-passing-checks.json).
This run predates SQLCipher and does not qualify positive signed submission or hardware.

## Preceding broadcast checkpoints

Explicit test-network broadcast now passes **14 additional Rust tests**, bringing the total
to **138 including real regtest**. Schema v6 records an uncertain attempt before POST and
keeps endpoint receipt separate from synchronized chain observation. Formatting, strict
Clippy, 23 offline checks and both binding generators pass. All three sanitizer targets
pass: 1,552,791 parser, 1,232 USB protocol and 684,839 signature/finalization cases, each in
61 seconds. See [broadcast rules](12-broadcast.md) and `validation/broadcast-checks.json`.
The HTTP fixtures are local; no real-network broadcast or hardware signing is claimed.

The [payment-mode run at 1376c10](https://github.com/johnny9/tundra-wallet/actions/runs/34292170172)
passed Rust/parser/audit and both native builds, but both expanded UI scenarios failed.
Android completed automatic Send/discard, then timed out waiting for a two-input review;
the old log does not distinguish Max from consolidation. iOS entered `2.52` instead of
`2.5` in the fee field and correctly failed its exact-value assertion. Three iOS runtime
tests and four Android QR/USB instrumentation tests passed; Android cold restart did not
run. Fixes/diagnostics are authored and retain the original assertions.
`validation/payment-modes-checks.json` preserves exact failures and artifact identities.
These failures preceded the passing `959224a` run above. No milestone is complete from
source alone.

The [broadcast-era native run at ec3a699](https://github.com/johnny9/tundra-wallet/actions/runs/34295706542)
passes Rust, all three sanitizer targets and iOS. **All four iOS tests pass**, including the
expanded payment modes and restart. Android builds and passes five JVM tests plus all five
wallet/QR/USB instrumentation tests; every payment mode and unsigned-broadcast refusal now
pass. Its sixth, standalone broadcast-dialog test failed to launch because the Compose test
activity manifest was missing. The debug test-host dependency is now added. Cold restart
was not run after that failure. `validation/broadcast-native-checks.json` records this partial
checkpoint; the corrected run above passes the dialog test and final Android restart.

## Earlier executed checkpoints

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

Finalization adds **6 passing Rust tests**, for **108 including regtest**, plus coverage of
finalized-draft sync invalidation. Schema v4 persists exact bytes and freezes signature changes.
[Finalization details](10-finalization.md) and `validation/finalization-checks.json` distinguish
local checks from pending native final-review qualification.

The first [QR native run](https://github.com/johnny9/tundra-wallet/actions/runs/34284043840)
passed Rust and parser/audit. Android compiled; descriptor scanning and reviewed-draft QR
export tests passed, while one public BBQr image failed ZXing's single finder. iOS failed
Swift compilation on error handling. Both fixes are authored; their native rerun is pending.
A host ZXing probe reproduced the finder failure and decoded all 26 public frames using the
multi-detector fallback now shared by the camera and instrumentation paths.

The [QR/finalization run at 579e418](https://github.com/johnny9/tundra-wallet/actions/runs/34285430052)
passed Android's three instrumentation tests and process-restart check, including all public
UR/BBQr barcode vectors and reviewed-draft export. iOS built and passed two XCTest tests;
its Vision barcode test and import UI test failed. The follow-up at
[d68cc23](https://github.com/johnny9/tundra-wallet/actions/runs/34287302455) passed Android,
Rust and parser/audit, but iOS compilation failed on an unexposed preview field. The focused
fix is pushed in `650b4d9`. Its iOS import, explicit sync, consolidation, signature progress
and restart UI scenario now passes, along with the two non-QR XCTest tests. The Vision
barcode test still failed; its exact attached PNG decodes correctly with host ZXing. The
next test exercises supported Vision revisions and records the actual decoder used; this
does not qualify the production camera. Android instrumentation again passed, but the
cold-restart check was blocked by a captured Quickstep launcher ANR. The test runner now
retains that evidence, closes only that specific launcher dialog once and rechecks the real
Tundra state. Its probe passed checks against saved successful/failed hierarchies and refused
to dismiss a Tundra ANR or unrelated package. Native recovery validation is pending.
Earlier failures remain recorded.

The development [USB adapter](11-usb.md) adds **16 passing Rust tests**, bringing the full
total to **124 including real regtest**. Formatting, strict Clippy, 23 offline checks and
both binding generations pass. The reviewed bhwi/miniscript Git pins compile and cargo-audit
reports no advisories or warnings. HID parser and APDU fuzzing evidence is recorded in
`validation/usb-checks.json`. Android USB source/tests await their first native run; no
hardware model is qualified and USB signing remains blocked.

An initial USB-era regtest run failed at initial sync; a retry passed. Inspection found that
its test backend published the readiness file before writing the port, allowing a reader
to see an empty port. Readiness now uses atomic rename; three independent fresh-chain runs
and the final regtest check passed. This race is a plausible cause of that failure, not a
confirmed reconstruction. The failed log remains retained alongside the successful runs.

The latest [complete passing run at dc54fe9](https://github.com/johnny9/tundra-wallet/actions/runs/34290175840)
passed all four jobs. Android passed five JVM and five instrumentation tests, including USB
public-account transcripts and permission-filter matching. The restart check recorded and
recovered one Quickstep launcher ANR, then verified the same saved balance/draft. iOS passed
all four XCTest/XCUITest tests. Its independent barcode oracle used **Vision revision 2**;
default revision 4 remains unqualified, and this is not a physical camera test. APK hashes,
test names, simulator details and artifact IDs are in `validation/usb-native-checks.json`.

Newer source adds every payment mode to both native UI scenarios, an iOS keyboard Done
control and an Android fallback for runtimes without USB. Their native rerun is pending.

The September 9 public-signature fuzz pass completed **658,421 cases in 61 seconds**
without crashes, alongside 1,623,390 parser and 1,246 USB protocol cases. The full 124-test
Rust/regtest suite, strict Clippy, formatting, 23 offline checks and binding generation pass
after the fixture refactor. See `validation/signature-fuzz-checks.json`; these are local
software results and do not change native/device qualification.

## Local executed checks

| Check | Actual result |
|---|---|
| Rust workspace | **131 core unit tests + 1 process-kill integration test + 5 HTTP integration tests passed**; all targets/features compiled with Rust 1.93.1 |
| Real Bitcoin Core integration | **1 separate regtest test passed** using Core 31.1, wallets disabled; maturity at 99/100 confirmations, persistence, shorter/replacement reorgs, invalidated drafts and durable freezes |
| Formatting / Clippy | `cargo fmt --all --check` and strict all-target/all-feature Clippy passed |
| Offline schema/fixture/source checks | **23 passed**, 0 failures/errors |
| UniFFI | Kotlin and Swift bindings generated from the same compiled Rust library |
| Sanitizer fuzzing | **1,552,791 parser + 1,232 USB protocol + 684,839 signature/finalization executions**, each target ran 61 seconds with no crashes; pinned tools and unchanged reviewed fuzz lock |
| Dependency audit | cargo-audit 0.22.2: **0 reported vulnerabilities, no warnings** against the recorded RustSec database; license inventory collected, distribution-license review incomplete |
| Native test-chain repeatability | Two fresh starts produced height 103; both refused a duplicate start and removed their own ready signal/data on termination |
| Approved design integrity | Four SHA-256 references pass; rebuilding HTML from editable sources is byte-identical |
| Approved design rendering | Chrome for Testing headless shell 152.0.7977.82 rendered at 420×900; screenshot visually inspected; no browser/hardware interaction retested |

That is **138 executed Rust tests**, including the separate regtest run. The default suite
intentionally ignores the real-node test and the process-kill child helper; the regtest
script runs the former explicitly, and the parent crash test invokes the latter.

Current machine-readable local evidence is in `validation/broadcast-checks.json`; earlier
checkpoints and dependency audit remain in `validation/local-checks.json` and
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

QR/UR/BBQr codecs and both native independent barcode tests pass; the iOS test uses
Vision revision 2, with default-detector and physical-camera qualification still open. Final assembly/persistence passes Rust tests; the development bhwi/USB
adapter passes software and Android native checks, with physical device qualification pending.
Explicit broadcast passes local software checks; its native and real-network qualification remain open. Physical phones, hardware address/policy verification, all signing
pairs and the full adversarial device matrix have not been tested. Native coverage still
needs every payment mode, accessibility/text sizes and complete visual comparison.
Protected production metadata storage, recovery/migration qualification, reproducibility,
license selection/review and independent security review remain release blockers.

Logical implementation commits are pushed to the configured `johnny9/tundra-wallet` origin.
The [original handoff](https://github.com/johnny9/tundra-wallet/blob/995026c/plan/VALIDATION.md)
is preserved in Git. Its 25 offline checks became 23 after obsolete publisher tests were
removed; neither suite alone certifies Rust/native behavior or security.
