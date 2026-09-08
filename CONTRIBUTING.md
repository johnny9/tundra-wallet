# Contributing

Read `plan/README.md` and the security invariants first. Keep wallet behavior in Rust and
native UI state native. Never add a demo success path to production code or a software
signer fallback. Test behavior rather than checking only that UI elements exist.

Run `scripts/check.sh`, regenerate bindings after FFI changes, and compile both native
adapters. Keep `plan/VALIDATION.md` honest about which tests ran. Use test descriptors only.
Do not introduce a dependency without reviewing its license and maintaining the lockfile.
The owner must choose a project distribution license before public contributions/release.
