# Product requirements

## Users and wallet policy

Tundra is a personal Bitcoin savings/coin-management application. One owner controls all
hardware required for signing. Both wallet types use the same workflow:

- Native SegWit single-sig `wpkh`: one hardware signature per input.
- Native SegWit 2-of-3 `wsh(sortedmulti(2,...))`: any valid pair, not a designated pair.

The phone stores public descriptors, wallet state and private metadata, **not Bitcoin
private signing keys**. Public keys are necessarily stored. Importing a descriptor does
not establish that the owner's hardware possesses matching keys; policy and receive
addresses must be verified on hardware in a later milestone.

## Descriptor onboarding

Scan a public descriptor QR or import a supported file -> review detected policy and network
-> name the wallet. No collection of saved signer profiles. Public key origin and policy
information belong to a wallet. Hardware-specific registration metadata can also belong
there quietly, without creating a signer-management UI.

The foundation implements file/paste parsing; native QR scanning is now added with physical
qualification still open. Require checksummed public
extended keys with origin information and receive/change branches. Accept one `/<0;1>/*`
multipath expression or a matching receive/change pair. Explicitly choose network: testnet,
signet and regtest keys cannot disambiguate the intended chain on their own. Never guess a
missing branch. Warn without logging rejected descriptor content.

## Main screen

Preserve the approved `design/prototype.html` as a visual/interaction reference:
wallet name/switcher -> compact balance -> fixed Activity/Coins tabs -> actions/content.
No repeated brand toolbar. Keep the same balance component in the coin picker. Total
wallet balance and selected-input balance are distinct. A selection or draft must not
move the tabs. Each tab retains scroll position.

Dark by default, explicit locally remembered light option. Cool neutrals, orange actions.
Use ordinary words: Wallet, Activity, Coins, Send, Receive and Sign; no themed terminology.
No fake balances, transactions or hardware success in native production code. An imported
but unsynced wallet shows **Not connected / —**, not zero.

## Coins and labels

Browse a label-first coin list. Tap a coin to edit its label; put address/outpoint in details.
Enter Select mode explicitly; no essential operation hidden behind a gesture. Search labels,
addresses and outpoints. Keep filter/sort under one control with active-filter indication.
After selecting, show count/amount, Send selected and More. More includes bulk label,
consolidation and freeze. Ineligible outputs remain visible with reasons.

Persist transaction/address/output labels and user freezes. Support previewed BIP 329
imports/exports. Do not send labels to hardware or a chain server by default. Preserve
original input labels in draft review and observed output provenance.
Consolidated/change outputs may receive new labels without erasing input history.

## Spending

Automatic selection operates only on eligible outputs. Explicit selection is strict:
insufficient selected funds is an error, never a reason to add other inputs silently.
`Some([])` is not automatic selection. Frozen, reserved, immature and unsupported outputs
are rejected again in Rust even if the UI erroneously selects them.

Send selected max subtracts the fee from exactly those inputs. Consolidation requires at
least two eligible coins and explicit acknowledgement that combining inputs links them.
Its destination is a fresh internal address; the net wallet loss is the fee, not the amount
moved. No `drain_wallet()` for a selected-only operation.

Review exact inputs, recipient, change, fee, network and warnings before signing. Persist
an immutable draft plus reservations atomically. Keep a draft in the wallet, not a shared
approval inbox. Cancelling/discarding releases only that draft's reservations, not freezes.

## Hardware / release

QR exchange and USB sessions enter one Rust validation path but have independent transports.
Native APIs handle camera, USB permission/I/O, files and application lifecycle. Rust handles
payloads, hardware protocols and all transaction decisions. No software-signing fallback.
Only signatures verified against required keys for each input count toward completion.
The final broadcast is explicit and must use exactly the reviewed, finalized transaction.
