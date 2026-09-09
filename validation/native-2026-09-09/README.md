# Native appearance evidence

Exact, unedited public-fixture screenshots from [run 34332720770](https://github.com/johnny9/tundra-wallet/actions/runs/34332720770)
at `1a9499c`. The 100 outputs come from a disposable, keyless Bitcoin Core regtest node.
These are test balances, not funds. Hashes and test results are in
[the final native record](../final-native-checks.json).

| Platform | Light appearance | Enlarged text in dark appearance |
|---|---|---|
| Android, Pixel 6 / API 35 | [Screenshot](android-light.png) | [Screenshot](android-large-text.png) |
| iOS, iPhone 17 Pro / iOS 26.2 simulator | [Screenshot](ios-light.png) | [Screenshot](ios-large-text.png) |

The iOS correction removes inherited orange coin text and applies the approved semantic
accent/secondary colors. Android now shows dark status icons over its light surface.
Both tests verify independent Activity/Coins scroll offsets, fixed tabs and nonoverlapping
enlarged-text selection controls through the production native views and real mobile FFI.

These screenshots are a partial comparison with `design/prototype.html`, not pixel parity
or whole-app accessibility qualification. Native platform controls differ. iOS abbreviates
the large-text sync timestamp, and the coin list is partly scrolled. Android's dark capture
shows dim navigation buttons despite passing window icon-mode assertions; settled system
animation and physical navigation-mode appearance remain to inspect. The iOS fixture-only
controls at the bottom are excluded from the normal app target by the CI boundary check.
