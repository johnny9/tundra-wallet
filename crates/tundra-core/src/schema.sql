CREATE TABLE IF NOT EXISTS wallets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 80),
    network TEXT NOT NULL,
    policy TEXT NOT NULL,
    state_json TEXT NOT NULL,
    synced_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS labels (
    wallet_id TEXT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('tx', 'addr', 'output')),
    reference TEXT NOT NULL,
    label TEXT NOT NULL CHECK(length(label) <= 255),
    PRIMARY KEY (wallet_id, kind, reference)
);
-- A user freeze is not a draft reservation. Removing a draft never removes a freeze.
CREATE TABLE IF NOT EXISTS freezes (
    wallet_id TEXT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    outpoint TEXT NOT NULL,
    PRIMARY KEY (wallet_id, outpoint)
);
CREATE TABLE IF NOT EXISTS drafts (
    id TEXT PRIMARY KEY,
    wallet_id TEXT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    psbt TEXT NOT NULL,
    review_json TEXT NOT NULL,
    label TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (wallet_id, id)
);
CREATE TABLE IF NOT EXISTS reservations (
    wallet_id TEXT NOT NULL,
    outpoint TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    PRIMARY KEY (wallet_id, outpoint),
    FOREIGN KEY (wallet_id, draft_id) REFERENCES drafts(wallet_id, id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS sync_state (
    wallet_id TEXT PRIMARY KEY REFERENCES wallets(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL DEFAULT 0,
    endpoint TEXT NOT NULL
);
-- The originally approved PSBT stays immutable in drafts. Only cryptographically
-- validated signatures are stored here; deleting a draft removes them atomically.
CREATE TABLE IF NOT EXISTS draft_signatures (
    wallet_id TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    psbt TEXT NOT NULL,
    PRIMARY KEY (wallet_id, draft_id),
    FOREIGN KEY (wallet_id, draft_id) REFERENCES drafts(wallet_id, id) ON DELETE CASCADE
);
-- A finalized transaction freezes the exact witness bytes for later explicit review.
CREATE TABLE IF NOT EXISTS finalized_drafts (
    wallet_id TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    transaction_bytes BLOB NOT NULL,
    PRIMARY KEY (wallet_id, draft_id),
    FOREIGN KEY (wallet_id, draft_id) REFERENCES drafts(wallet_id, id) ON DELETE CASCADE
);
-- Wallet-owned policy authentication tokens; this is not a signer directory.
CREATE TABLE IF NOT EXISTS hardware_registrations (
    wallet_id TEXT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL CHECK(length(fingerprint)=8),
    policy_id BLOB NOT NULL CHECK(length(policy_id)=32),
    hmac BLOB NOT NULL CHECK(length(hmac)=32),
    PRIMARY KEY (wallet_id, fingerprint, policy_id)
);
-- Record uncertainty durably before network submission. Acknowledgement is not confirmation.
CREATE TABLE IF NOT EXISTS broadcast_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_id TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    txid TEXT NOT NULL CHECK(length(txid)=64),
    wtxid TEXT NOT NULL CHECK(length(wtxid)=64),
    requested_at INTEGER NOT NULL,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1)),
    FOREIGN KEY (wallet_id,draft_id) REFERENCES finalized_drafts(wallet_id,draft_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS broadcast_draft_attempts ON broadcast_attempts(wallet_id,draft_id,id);
PRAGMA user_version = 6;
