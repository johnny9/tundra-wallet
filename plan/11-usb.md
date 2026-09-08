# Development Android USB / bhwi adapter

No model or firmware is qualified. USB signing remains blocked in Rust. The development
adapter can inspect a public account, register a wallet policy and compare an already-issued
test-network receive address. These operations do not mark an address hardware-verified.
They are available for a future explicit physical qualification session.

## Boundary and protocol

Rust owns the bounded Ledger interpreter, HID framing, public-account/policy comparison and
registration persistence. Android owns enumeration, user permission and USB handles. The
native UI sees typed UniFFI sessions and 64-byte reports; it cannot invoke arbitrary bhwi
commands, restore a device, enter a seed/PIN/passphrase, or invoke a software signer.

The Ledger-only bhwi dependency is pinned to
`edfac42caf0d67693660cf1f04c2d24ca6b8f7a1`; its miniscript dependency resolves to
`ff4732e5f75aa555682343cb180fa72ee3e8e9d5`. Both revisions are in the reviewed lockfiles.
BDK continues using miniscript 12.3.7. Tests compare materialized scripts between the two
versions for single-sig and 2-of-3, receive/change and multiple derivation indices. The
miniscript 13 policy API requires full key derivation expressions when setting key info;
bhwi strips their suffixes only when serializing BIP-388 wire key info.

Before each action, the core requires the exact `Bitcoin Test` app name, bounded version
metadata, a matching wallet fingerprint and equality of the full public account. It accepts
only the application's existing narrow descriptor policies. Mainnet is blocked. Ledger's
test app uses testnet/signet addresses; regtest address display is not qualified.

Policy names sent to hardware are always `Tundra`, never the user's label. A registration
response must contain exactly the expected 32-byte policy ID and a 32-byte device HMAC.
Schema v5 stores that token by wallet, fingerprint and policy ID, with wallet deletion cascade.
A response cannot publish completion until a short database transaction revalidates the
wallet and commits its metadata. A receive response must exactly equal the Rust-derived,
already-issued address. Public-account matching is not device attestation; the user must
compare policy/address details on the hardware's own screen during qualification.

The implemented signing interpreter path requires an existing device-specific registration
and sends any completed PSBT back through the independent cryptographic draft validator.
`prepare_usb(SignDraft)` currently refuses at the device qualification gate, and Android
exposes no signing button. No finalization or broadcast gate was removed.

## Bounds, permission and cancellation

- Ledger channel `0x0101`, tag `0x05`, strictly ordered full 64-byte reports. Short reads,
  wrong/repeated sequences, invalid lengths and late responses close the exchange. Padding
  cannot become response bytes; the Android reader uses the actual completed byte count.
- APDUs are at most 260 bytes; one response is at most 4 KiB. A session permits at most
  4,096 exchanges, 4 MiB received and 180 seconds, including polling without new input.
- Android selects vendor `0x2c97` and a vendor HID application on usage page `0xffa0`,
  with 64-byte interrupt IN/OUT reports and no report ID. U2F/other interfaces are rejected.
- Permission follows an explicit action and is bound to the device and a unique request.
  Denial, detach, leaving the screen and cancellation require a new operation. Reconnect
  creates fresh handles and repeats the public-account handshake.
- One IO coroutine acquires, uses and closes each USB handle. Reads remain queued across
  500 ms timeouts; writes/control transfers are bounded to 1,500 ms. Cancellation checks
  precede consuming responses, and actions remain disabled until cleanup finishes.
- Dependency `log` output is statically disabled. App errors contain bounded descriptions,
  never APDU bytes, labels, descriptors, xpubs, addresses or PSBTs.

## Executed checks and remaining gates

Sixteen Rust USB tests cover framing, metadata, wrong accounts, Merkle continuation,
policy/script parity, rejection/cancellation/expiry/budgets, registration persistence,
rollback, migration, wallet isolation and the signing gate. Full-suite evidence is in
`validation/usb-checks.json`. Separate fuzz targets exercise arbitrary HID framing and
APDU responses after a public test handshake; no device or Bitcoin private key is used.

Android compiled and passed five JVM tests, five instrumentation tests and a process-restart
check at `dc54fe9`. This includes the HID parser, a public transcript through the real mobile
FFI, permission-filter matching and the empty-device UI. One unrelated Quickstep launcher ANR
was recorded and closed before rechecking the same persisted app state. The iOS framework,
app and four runtime tests also passed; see `validation/usb-native-checks.json`.
Permission prompts with hardware, cancellation during device approval, detach/reconnect,
phone rotation/background, cable/OTG behavior and firmware interoperability remain physical
qualification gates. Emulator or Waydroid results cannot fill those device-matrix cells.

Upstream API references: [bhwi at the pinned revision](https://github.com/wizardsardine/bhwi/tree/edfac42caf0d67693660cf1f04c2d24ca6b8f7a1),
[Android USB host](https://developer.android.com/develop/connectivity/usb/host) and
[UsbRequest byte-count semantics](https://developer.android.com/reference/android/hardware/usb/UsbRequest).
