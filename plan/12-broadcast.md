# Explicit test-network broadcast

Rust submits only the exact persisted finalized transaction, after another check of the
approval, signatures, current wallet inputs, reservations, freezes and fee accounting.
Mainnet remains blocked. Native confirmation forms and typed UniFFI calls are authored;
their new build/runtime gate is pending. Hardware signet end-to-end qualification remains open.

The adapter uses Esplora's hex-body `POST /tx` and expects the exact transaction ID in its
successful response. See the [upstream Esplora API](https://github.com/Blockstream/esplora/blob/master/API.md#post-tx).
The endpoint is explicit, with fresh transaction/endpoint consent. Before submission it
must report the selected test network's genesis hash. Only HTTPS or local/emulator loopback
HTTP is allowed; credentials, query strings, fragments, redirects and environment proxies
are disabled. Each request has a 15-second timeout, response bodies are capped at 128 bytes,
and the client disables automatic retries. No labels, descriptors or xpubs are uploaded.
The complete transaction and connection remain visible to the chosen server.

## Durable uncertainty

Schema v6 adds wallet/draft-scoped submission attempts. An uncertain attempt commits before
POST begins, so a crash cannot turn an attempted submission into an apparently untouched
payment. Only an exact successful acknowledgement updates that attempt's receipt flag.
Timeouts, refusal, wrong transaction IDs, truncated/oversized replies and interrupted
acknowledgement persistence retain uncertainty. The app never echoes a server response body.
An acknowledgement does not prove mempool acceptance, propagation or confirmation.

Every request includes the finalized transaction ID and the latest attempt ID shown to the
user. A compare-and-insert after network preflight rejects stale screens and competing
initial requests. Retrying requires explicit acknowledgement of the previous attempt and
revalidates the same immutable bytes. Each draft is limited to 64 attempts. Late completion
updates only its own record and cannot overwrite the latest attempt's result. Opening or
resuming a wallet never transmits anything. Submitted records cannot be discarded: deletion
would not recall an already transmitted transaction and would hide its uncertainty.

An explicit sync supplies chain observation. The exact own transaction can be unconfirmed
or confirmed in the cached wallet view; those observations are separate from endpoint
receipt. Sync releases its reservations and preserves the saved review as `observed`.
Eviction/reorg removes that observation and invalidates the approval, without automatically
recreating reservations or retrying. Other conflicts retain the existing invalidation rules.
This is still a chosen-server chain view, not an independent proof-of-work light client.

## Software evidence and remaining gates

Fourteen Rust tests cover exact public transaction bytes, unknown outcomes, endpoint and
retry consent, mainnet/wrong-network refusal, unsigned/incomplete/frozen/tampered drafts,
changes during preflight, independent-connection races, late acknowledgement, persistence
failure, actual process death during POST, schema-v5 migration, wallet isolation, own-spend
observation and confirmation/reorg handling. `validation/broadcast-checks.json` records
executed checks. The chain-observation unit tests use public signatures in a test-only graph.
The HTTP servers are local protocol fixtures; they do not prove Bitcoin network acceptance.

Both native forms show the transaction ID and endpoint and require fresh consent. Changing
the endpoint clears consent; a retry has an additional acknowledgement. No automatic
broadcast follows signing, finalization, startup or sync. Android dialog and mobile FFI
refusal tests are authored. Positive native signed-review/submission coverage, output-label
provenance and a hardware-funded signet round trip remain to qualify/complete.
