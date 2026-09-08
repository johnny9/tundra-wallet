# Tundra development rules

- Start with `plan/README.md`; update status when changing scope.
- This project is hardware-only. Never generate/import Bitcoin private signing keys.
- Native apps call Tundra's typed UniFFI API, not BDK or bhwi directly.
- No fake transactions, signatures, sync success, or balances outside `design/` and tests.
- `Some([])` input selection must not become automatic selection.
- User freezes are not draft reservations; keep them independently durable.
- Labels/descriptors/PSBTs are private metadata. Do not log them or add analytics.
- Existing `design/prototype.html` is the approved interaction reference, not production code.
- Dark is the initial default. Keep Activity/Coins fixed below the compact balance.
- Do not enable signing/broadcast/mainnet by removing a gate before its acceptance tests pass.
- Run and report actual tests. Missing toolchains are not passing tests.
