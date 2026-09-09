# Tundra iOS development app

Native SwiftUI views call the shared Rust core via generated UniFFI bindings. The simulator
target builds in CI; no iOS app is published. Current UI scope: public test
wallet import by file/paste, list/switch wallet, unknown balance, Activity/Coins, unverified
receive-address derivation, dark/light, opt-in sync, labels, coin selection and payment review.
Hardware qualification remains open. The exact build, payment, restart, recovery and
runtime results are recorded in [the validation report](../../plan/VALIDATION.md).

From repository root run `scripts/build-ios.sh` on a Mac with Xcode, Rust Apple targets and
XcodeGen installed. Then run `xcodegen generate` here and open the project. The generated
Swift file is compiled into the app; its `TundraCoreFFI` Clang module comes from the generated
XCFramework. Do not add an `import TundraCore` expecting a separate prebuilt Swift framework.

`CoreService` is an actor so synchronous database/BDK calls do not run on the main UI actor.
The app excludes its Application Support directory from backup and requests complete iOS
file protection. This is not an audited encryption strategy. User-facing receipt addresses
are unverified and must not be funded with real coins. Test-network sync requires explicit
endpoint consent. QR capture/display, signed-file validation, final review and explicit
test-network broadcast are implemented; device interoperability remains unqualified.
The USB adapter is Android-only.

Run `scripts/check-swift-ffi.sh` from the repository root on macOS for host Swift/Rust
roundtrips: database reopen, Unicode labels, unknown balances, typed errors and integer
amounts. These checks complement the Xcode build; iOS simulator/device runtime and lifecycle
tests are still required. A Swift syntax parse alone does not typecheck SwiftUI or certify a build.

`TundraTestHost` and `TundraSigningUITests` are separate test targets. The host compiles the
production wallet/payment views and model, with a unique protected storage directory and
Keychain service for each public scenario. Its setup controls supply the pinned published
response or start the synthetic fixture scan; signing, recovery, submission and coin-detail
controls use production views. Relaunch retains the prepared store and never resets it.
The normal Tundra target contains neither the fixture entry point nor the public databases,
PSBT or backup. CI checks the generated Xcode source/resource phases before compiling.
These new screen tests and that generated-project check await their first native run.

The complete simulator suite needs fresh public fixture servers on ports 3003 and 3004,
started with `scripts/start-published-native-fixture.sh` and its `--screen` option. Port 3004
is isolated from the vault tests' earlier submission. Neither server is a valid Signet
chain or a real broadcast. System PSBT picker and physical transport qualification remain
separate from the public-response handoff in this test host.

The separate public test host also has an authored `layout` scenario. It imports the public
2-of-3 fixture and scans the real keyless regtest server's 100 outputs. New Android/iOS
scenarios check independent tab scroll offsets and large-text controls; the Apple host
injects SwiftUI `accessibility3` into the shared wallet view. These tests await native
compilation/execution and do not qualify physical accessibility settings or screen readers.
