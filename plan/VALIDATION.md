# Validation status

Planning baseline: September 7, 2026. Latest local validation: **September 11, 2026**.
This is development source with test-network sync, native payments and external PSBT validation.
**The full plan is not complete. No physical hardware or real-funds readiness is claimed.**

## September 11 local Android run — not fully passing

Waydroid now has a DHCP lease and authorized ADB access. The current working tree,
including the September 10 native wallet-creation changes, builds both Android Rust ABIs
and the app/test APKs. Fresh execution passes **10 JVM tests**, **34 offline checks**, all
locked Gradle dependency configurations and their refusal checks, **20,220 SDK payload
files**, native-library inspection and the exact **177-text notice resource**.

The complete Waydroid API 33 / x86_64 instrumentation suite ran: **17 passed, 1 failed,
0 skipped**. The passing wallet scenario includes the new import/review navigation,
all four payment modes, saved drafts, QR codecs and Activity recreation. The failing
`BackupDocumentRuntimeTest` reaches the system Save action, then Tundra reports that the
saved backup could not be verified. A focused diagnostic rerun reproduces this in both
Android users 10 and 0. The test now reports the bounded app error directly instead of
waiting only for a success message. The underlying write/flush/readback failure remains
unresolved; production backup verification was not relaxed.

The pre-existing Waydroid app has a different signing key. The runtime run therefore uses
the same application sources under an isolated validation package and a fresh Android
user, preserving the existing development installation and its wallet data. Normal APK
outputs were rebuilt afterward. Exact APK/source hashes, test names, logs and limits are
in the [Waydroid Android record](../validation/waydroid-android-checks-2026-09-11.json).
This is working-tree evidence, not a newly passing published source revision.

An explicit cold launch succeeds, but the separate post-force-stop saved-state probe
stalled without reporting test status and was stopped. Its resulting `Process crashed`
instrumentation message records that explicit termination; it is not evidence of a
diagnosed application crash. **Saved-state cold-restart validation is incomplete for
this run.** No UI tree was dumped. Android user 0 and the original animation settings
were restored; the isolated validation data was retained.

## September 10 local macOS VM rerun

Follow-up [CI run 34534457918](https://github.com/johnny9/tundra-wallet/actions/runs/34534457918)
passed Rust, dependency checks, Android and **16 of 17 iOS tests**. The wallet UI test
failed at backup restore on iOS 26.2: its recording and accessibility snapshot show
the exported public backup still in the system picker after the cell tap. The test
then swiped the picker while looking for the restore button in the underlying form.
This is separate from the earlier simulator Metal-service crash.

The test now waits for the picker to dismiss before scrolling the restore form. If
the picker remains, it retries once on the visible, enabled public file's icon and
requires dismissal; it still requires the full review, consent, restore and restart
assertions. The modified complete wallet flow passes locally on iOS 27 in **224.313
seconds**, with exit 0, and all 34 offline checks pass. CI's iOS 26.2 confirmation is
pending. Retained summaries: [CI failure](../validation/ios-backup-picker-2026-09-10/ci-failure-summary.json)
and [local pass](../validation/ios-backup-picker-2026-09-10/local-summary.json).

The unchanged `WalletUITests.testImportRestartAndReceive` passed in **228.795 seconds**
on a fresh iPhone 18 Pro simulator with iOS 27.0 (24A434), Xcode 27.0 (27A266a),
and macOS 26.6.2. It covered import, receive, restart, local regtest sync, all four
payment modes, saved drafts, system backup export/readback/restore and restart after
restore. The keyless regtest fixture server was restarted before the run.

The previous interruption was an app abort at 15:22:53 on September 10. Its crash
report records `EXC_CRASH / SIGABRT`, termination namespace `METAL`, code `102`,
and loss of the `SimMetalHost` XPC connection. It was not a coin-selection assertion
failure. The underlying reason the graphics service disconnected is not established.
The earlier test process also stalled finalizing its report and was terminated.
No production behavior, assertion, timeout in the test, or safety gate was changed.

The first successful rerun also stalled collecting verbose diagnostics. A stack sample
located the wait in `collectSimulatorDiagnostics` / `simCtlDiagnose`; stopping only
that optional collector allowed Xcode to finalize a passing result and exit 0.
The runner now accepts `TUNDRA_SIM_ID`, `TUNDRA_RESULT_BUNDLE`, test-selection arguments,
and `TUNDRA_TEST_DIAGNOSTICS=never` for affected VMs. Its CI default remains `on-failure`.
A second fresh-simulator run through the updated script passed in **223.485 seconds**,
reported **TEST SUCCEEDED**, exited **0 without intervention**, and produced a readable
[xcresult summary](../validation/ios-wallet-2026-09-10/summary.json) and
[test completion excerpt](../validation/ios-wallet-2026-09-10/result.txt).
All 34 offline checks passed again; shell syntax and invalid diagnostics-option rejection
were checked. Xcode still reports an `Invalid frame dimension` runtime warning; it did
not fail the test and is not claimed fixed here.

The earlier local run passed 13 hosted native tests and 3 signing/layout UI tests;
34 offline checks and 6 Swift/Rust host checks also passed. The wallet rerun completes
coverage across separate runs; it does not establish one clean full-suite run.
Local logs: `build/ios-runtime.log`, `build/ios-wallet-repro.log`, and
`build/ios-wallet-verified.log`. The final bundle is `build/ios-wallet-verified.xcresult`.

## Recorded hosted CI checkpoint

**All four CI jobs pass at [1a9499c](https://github.com/johnny9/tundra-wallet/actions/runs/34332720770).**
This includes **189 Rust tests / 34 offline checks**, parser fuzzing/audit and dependency/notice
checks. Exact results and hashes are in the [final native record](../validation/final-native-checks.json).
The preceding [cd44448 baseline](../validation/locked-native-passing-checks.json) also passes.

Android passes both Rust mobile ABIs, the app/test APK builds, **10 JVM / all 18 instrumentation
tests** and cold restart. The new recovered-payment screen now passes explicit reapproval,
separate submission consents, one exact published-fixture POST, observed input provenance and
retained output-label edits. Its 100-output scenario passes independent Activity/Coins offsets,
fixed tabs, light appearance and enlarged-text controls. Strict complete dependency checks,
actual refusal cases, all **20,220 installed SDK payload files/links** and the exact **177-text
APK notice resource** pass too. The document test needed one bounded recovery of the hosted
Quickstep launcher ANR; its public diagnostic screenshot is retained in the CI artifact.

Apple passes its app build, **6 host FFI / all 17 simulator tests** with no skips. The normal
wallet scenario passes all four payment modes, filters, saved-draft restart and real system
Files export/readback/inspection/restore/restart. Both public signing/recovery UI scenarios
and the complete long-list/light/large-text scenario pass. Pinned XcodeGen 2.46.0 executes,
and the exact **177-text notice resource** is present in the built app. The persistent,
read-only loopback RPC correction fixes the preceding fixture scan timeout without changing
production networking or the UI's wait limits. This run uses the arm64 iPhone 17 Pro simulator
on iOS 26.2; the app is built with the pinned Xcode 16.4.

**The presentation and packaging corrections now pass their native rerun.** iOS semantic
colors compile and render with neutral coin text; Android's window icon-mode assertions pass
in both appearance modes, and its light screenshot shows readable status icons. Exact public
screenshots and comparison limits are [retained in the repository](../validation/native-2026-09-09/README.md).
The Android builder removes two unused BBQr cdylibs; all ten remaining APK libraries pass
correct-ABI, 16 KiB ELF/ZIP alignment, nonexecutable-stack and relocation checks. The trimmed
APK passes the complete ordinary runtime suite; the separate 16 KiB result follows below. Earlier
source/local-check records are retained at
[iOS colors](../validation/ios-semantic-color-checks.json),
[Android system icons](../validation/android-system-bar-checks.json) and
[native packaging](../validation/android-native-packaging-checks.json); their pending statements
are superseded by this run.

The additional experimental **16 KiB Android runtime gate now passes** in
[run 34337878825](https://github.com/johnny9/tundra-wallet/actions/runs/34337878825): the exact
same app/test APKs pass **all 18 instrumentation tests and retained-state cold restart**.
The emulator reports 16384-byte pages, with compatibility fallback explicitly disabled and
checked. The image's default 2.5 GiB allocation caused Android to terminate instrumentation
with `LOW_MEMORY`; the unchanged tests pass with a 4 GiB guest. Setup failures and diagnostic
corrections remain in the [initial](../validation/android-16k-first-checks.json) and
[diagnostic](../validation/android-16k-diagnostic-checks.json) records. Exact passing hashes,
counts and limits are in [the 16 KiB result](../validation/android-16k-passing-checks.json).
This qualifies the recorded experimental x86_64 environment, not physical ARM64 page-size or
low-memory behavior. No application gate or test expectation changed.

The three 61-second AddressSanitizer runs complete **1,469,122 parser / 1,220 USB protocol /
503,608 signature cases** without crashes. The recorded RustSec audit reports zero
vulnerabilities and no warning categories. These are bounded checks, not exhaustive parser
testing or independent security review. Eight Rust fixture/helper entry points are intentionally
ignored by the ordinary workspace run; the real regtest is separately executed and passes.

Local user-owned Android tools now build both APKs and pass strict **offline** Gradle builds,
10 real host FFI JVM tests and exact native notice checks. No sudo installation was needed.
Coverage remains **440 locked components / 588 verified metadata components / 1,267 artifact
checksums**, with all **588 retained Maven POMs** matched. See
[local toolchain evidence](../validation/local-android-toolchain-checks.json).

At the September 9 checkpoint Waydroid reported **IP UNKNOWN with no ADB device**;
that networking blocker is resolved by the September 11 run above. There is no local KVM
device. The new local run has the explicit failure and restart limits above. Broader visual and
accessibility qualification, physical phones, QR/USB transports, real-network hardware signing
and independent release review remain open. Command-line utilities, emulator/host images and
remaining upstream/distribution review have separate limits; see the
[notice scope](../third-party/README.md). No hardware or real-funds readiness is claimed.

## Earlier document and fixture checkpoints

The following entries describe the results and pending work at each earlier source revision.
The current checkpoint above supersedes their then-pending validation statements.

The [next native run at ffc08e6](https://github.com/johnny9/tundra-wallet/actions/runs/34314049044)
passes Rust (**189 tests / 29 offline**), parser/audit and the notice-regeneration gate.
Both native app builds and published-signature/vault scenarios pass. Android passes **10 JVM
and 14 of 15 instrumentation tests**, including all payment modes after the focus correction.
The sole failure is its system picker: a public screenshot shows the exact Quickstep ANR
dialog covering DocumentsUI. A bounded one-time launcher recovery is now authored, with
a screenshot/artifact flag; a second launcher failure fails the test. Cold restart did not run.

iOS passes **6 host FFI and all 13 hosted runtime tests**, including the new Keychain signature,
restore, sync, explicit reapproval/submission and provenance scenario. Its UI scenario passes
payment modes/restart and actual local Files backup export/readback, then fails finding the
Restore-mode picker by its label. An explicit `backupAction` identifier corrects that selector.
**Complete system import/reviewed restore and positive payment UI controls remain open.**
See [`published-native-platform-passing-checks.json`](../validation/published-native-platform-passing-checks.json).

Workflow action pins and both complete Cargo license/notice inventories pass their local
checks: **29 offline tests**, deterministic inventory regeneration for **243 packages** and
**165 retained texts**. The CI action SHAs exactly match those already executed in run
34311969023; the new CI inventory comparison still awaits execution. This does not complete
native dependency, binary distribution or independent security review. See
[`reproducibility-checks.json`](../validation/reproducibility-checks.json).

The [published-fixture native run at f86c34e](https://github.com/johnny9/tundra-wallet/actions/runs/34312982396)
passes both app builds, **10 Android JVM / 13 Android instrumentation** checks and **6 Apple
host FFI** checks. Android’s new published-signature/vault scenario passes: both signatures,
exact finalization, retained generation restore, actual fixture HTTP sync, recovery review,
separate submission consent, reopen and observed provenance with retained edits. Both Android
UI failures now occur while waiting for keyboard dismissal. The sheets obtained the parent
activity focus manager; the correction resolves it inside each modal’s composition.
Apple runtime tests did **not run** because fixture startup exceeded 20 seconds. The fixture
now avoids reverse DNS during loopback bind and records bounded startup markers. Its two Rust
integration tests still pass. Android screenshot artifact collection also needs the corrected
development package path. These native corrections await their next run. See
[`published-native-platform-checks.json`](../validation/published-native-platform-checks.json).

The [next picker run at d89cba6](https://github.com/johnny9/tundra-wallet/actions/runs/34311969023)
passes both builds, **10 Android JVM / 12 Android instrumentation**, **6 Apple host FFI /
12 iOS runtime** checks, Rust and parser/audit. Android still fails at the system filename
control after export preparation; its wallet import preview also fails without a core error.
iOS passes payment modes/restart and now opens Settings and the actual Files export picker,
but cannot find the unindexed Tundra/Backups folder. Its recording shows a writable empty
local root. Pending corrections flush Compose before native picker polling, dismiss the
descriptor IME, accept the available local Files root and name the exported wrapper.
Neither complete picker flow nor Android cold restart passes this run. See
[`backup-picker-second-checks.json`](../validation/backup-picker-second-checks.json).

New native vault scenarios are authored using the pinned published-signature fixtures and
a separate local synthetic HTTP server: both inputs/signatures, exact final bytes, protected
restore/generation switching, sync, explicit reapproval, separate submission consent, reopen
and observed provenance with preserved user edits. Their first native run remains pending;
these tests do not exercise the corresponding positive UI controls or real network broadcast.

Published native fixtures now pass the local **189 Rust / 27 offline** gate, strict Clippy,
formatting and both binding generators. Two new integration tests verify the published
Ledger response and exact final bytes, then protected restore, actual loopback HTTP sync,
explicit recovery review, separate exact submission consent, reopen and observed output
provenance with retained user label edits. The server is a synthetic Merkle-consistent test
chain, **not valid Signet or a real network broadcast**. The separate actual keyless Bitcoin
Core regtest also passes. Native use of these pinned fixtures remains untested. See
[`published-native-fixture-checks.json`](../validation/published-native-fixture-checks.json).

The [first system-picker run at b9434ec](https://github.com/johnny9/tundra-wallet/actions/runs/34310420076)
passes Rust/parser/audit, both native builds, **10 Android JVM tests**, **12 Android
instrumentation tests**, **6 Apple host FFI checks** and **12 iOS runtime tests**. Two Android
UI tests fail: the payment review button is not displayed after scrolling, and the system
backup filename control is not found. The iOS payment modes/restart portion passes, then
Settings fails XCTest’s AX scroll action before the backup flow. Its public-fixture recording
shows the header control visibly on screen. Corrections add explicit Android IME dismissal,
document-window selection and export-preparation assertions; iOS taps the verified on-screen
Settings center and requires the menu to open. **These corrections and both complete system
picker flows await validation.** This does not qualify accessibility. Android cold restart
did not run. See [`backup-picker-native-checks.json`](../validation/backup-picker-native-checks.json).

The [document-helper run at 56ffb89](https://github.com/johnny9/tundra-wallet/actions/runs/34309306089)
passes both builds, **10 Android JVM tests** and **13 iOS runtime tests**. Both generation
recovery tests now pass, including lost keys/selectors and missing databases. Apple host FFI,
all payment modes and restart pass; bounded file helpers pass on both platforms. Android
passes **12 of 13 instrumentation tests**: its existing payment scenario times out at a
review request. `ef1259f` accounts for the keyboard in the payment sheet and adds actionable
button assertions plus bounded diagnostics; a keyboard cause is not yet confirmed.
Android cold restart did not run. See
[`backup-document-native-checks.json`](../validation/backup-document-native-checks.json).

System document-picker scenarios at `8c29fba` were first exercised in the failing run above.
Android uses an isolated real ViewModel, Keystore and Rust core. iOS will save/import through
Files, confirm recovery, check unknown balances/suspended drafts/restart, and exercise the
stated limit for unused receive addresses issued after a backup. Native source also adds
coordinated Apple provider reads, keyboard controls and a local Documents/Backups destination;
private wallet databases and keys remain outside the shared Documents directory.

Native backup document exchange at `3fc9a83` is authored: secure password/confirmation,
bounded staging, completed ciphertext export with destination readback, reviewed restore
and cleared old native state. Its helper tests and system picker flows await native CI.
A sync callback regression at `ae0f4a9` makes operation IDs unique across cores within the
process: stale run/status/cancel requests cannot select a restored store's new operation.
The local gate passes **187 Rust tests including keyless regtest**, **26 offline checks**,
formatting, strict Clippy and both bindings. See
[`backup-document-checks.json`](../validation/backup-document-checks.json).

The [generation run at 2890d70](https://github.com/johnny9/tundra-wallet/actions/runs/34308278331)
passes both native builds, host FFI, Rust and parser/audit. Android passes **12 of 13
instrumentation tests**; iOS passes **10 of 11 runtime tests**, including payment/restart.
Each new generation test compares ciphertext across a legitimate reopen of the old core,
so that preservation baseline is invalid. `7d03aca` now checks failed-restore preservation
before reopening, then captures the baseline used for subsequent restore assertions after
that explicit reopen. The remaining assertions stay in place and await the next run.
See [`store-selection-native-checks.json`](../validation/store-selection-native-checks.json).

## Earlier fully passing native checkpoint

The [restore run at 03d3732](https://github.com/johnny9/tundra-wallet/actions/runs/34307207286)
is **fully green**. Android passes **8 JVM tests, all 11 instrumentation tests and cold
restart**, without launcher ANR recovery. iOS passes builds, **6 host FFI checks and all
10 runtime tests**. Both normal apps open protected storage, retain payment/receive state
across restart, and execute the backup restore assertions. Apple CommonCrypto successfully
restores the Linux/OpenSSL fixture. Rust, all three sanitizer targets and audit pass too.
See [`backup-restore-native-passing-checks.json`](../validation/backup-restore-native-passing-checks.json).

Atomic store selection at `19b8152` passes **186 Rust tests including real keyless regtest**,
**26 offline checks**, formatting, strict Clippy and both binding generators. Four actual
process-kill boundaries preserve a complete old or new selected store. Missing/corrupt
selectors fail closed; explicit restore can establish a new selection without deleting the
old data. Native key retention for restored generations (`9cc8500`) and explicit recovered
submission review (`dacfcae`) are authored and await the next CI run. Native backup document
exchange and the positive recovery-review UI scenario remain open. See
[`store-selection-checks.json`](../validation/store-selection-checks.json).

## Preceding restore and protected-startup checkpoints

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
