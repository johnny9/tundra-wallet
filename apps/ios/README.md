# Tundra iOS starter

Native SwiftUI views call the shared Rust core via generated UniFFI bindings. This is an
initial source shell, not a compiled or published iOS app. Current UI scope: public test
wallet import by file/paste, list/switch wallet, unknown balance, Activity/Coins, unverified
receive-address derivation, dark/light. Advanced labels, coin selection and signing UI
still need porting; the core already exposes relevant metadata/draft APIs.

From repository root run `scripts/build-ios.sh` on a Mac with Xcode, Rust Apple targets and
XcodeGen installed. Then run `xcodegen generate` here and open the project. The generated
Swift file is compiled into the app; its `TundraCoreFFI` Clang module comes from the generated
XCFramework. Do not add an `import TundraCore` expecting a separate prebuilt Swift framework.

`CoreService` is an actor so synchronous database/BDK calls do not run on the main UI actor.
The app excludes its Application Support directory from backup and requests complete iOS
file protection. This is not an audited encryption strategy. User-facing receipt addresses
are unverified and must not be funded. No camera, USB, network or broadcast path is provided.

The first Xcode build must verify generated names and the FFI lifetime model. A Swift syntax
parse on Linux, where available, does not typecheck SwiftUI or certify an iOS build.
