# Public disposable fixtures — NEVER FUND

These checksummed public descriptors were included with the approved prototype. They contain
public extended keys only and use test-family serialization. Their private keys are not
provided. They are not recoverable wallets or backups. Do not send bitcoin or test funds to
their addresses expecting to spend it. Core tests fabricate chain data entirely in memory.

`single-sig.txt` and `two-of-three.txt` are multipath imports. `core-style-pair.json` is the
same two-of-three policy as an explicit receive/change pair. The single-branch files are
negative tests for incomplete onboarding.

The signing fixtures contain published public transactions, public account keys and existing
signatures only. `signing-provenance.json` records upstream revisions, source links and hashes;
license notices are retained under `upstream-licenses/`. HWI provides the single-input example;
Ledger provides two inputs, their funding transactions and deterministic expected signatures.
The confirmed public multisig transaction tests one actual 2-of-3 signing pair and witness
order. It does not qualify the other pairs, a hardware model or mainnet use.

The Rust persistence tests construct a test-only wallet snapshot from the Ledger account and
funding transactions with synthetic confirmation data, then save the fixture transaction's
review. No app API injects those snapshots, generates keys or manufactures signatures.
