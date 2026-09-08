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
PRAGMA user_version = 1;
