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

`qr-registry-psbt.ur` is the public PSBT test vector from Blockchain Commons' UR type
registry (© 2020 Blockchain Commons; Wolf McNally and Christopher Allen).
`qr-bbqr-vectors.json` independently encodes the existing public HWI response with Python
base64/hex/raw zlib following the BBQr specification. `qr-provenance.json` records sources
and hashes. None of these QR vectors contains a private signing key.

`backup-v1.tundra` is an encrypted SQLCipher snapshot generated on Linux with OpenSSL
from the public single-sig descriptor, one issued address and a synthetic label. It contains
no transactions, drafts, signatures or Bitcoin signing keys. `backup-provenance.json` records
its public test password (including the trailing space), hash, provider and schema hash.
The explicit ignored Rust generator requires `TUNDRA_PUBLIC_BACKUP_OUTPUT` and refuses an
existing output. SQLCipher uses a random salt, so regeneration changes the ciphertext.
The offline check verifies the committed hash; actual decryption and supported descriptor
checks belong to Rust/native tests. This is not a recoverable personal wallet or a safe
password for personal data. Native tests use it to check provider interoperability.
