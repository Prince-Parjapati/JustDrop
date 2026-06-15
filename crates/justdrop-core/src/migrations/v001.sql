-- JustDrop Schema v1
-- Canonical schema for device trust, transfer history, and resume state.

CREATE TABLE IF NOT EXISTS peers (
    fingerprint TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    platform    TEXT NOT NULL,
    trust_level TEXT NOT NULL DEFAULT 'unknown',
    public_key  BLOB,
    first_seen  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS transfer_history (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    transfer_id      TEXT NOT NULL UNIQUE,
    peer_fingerprint TEXT NOT NULL,
    direction        TEXT NOT NULL CHECK (direction IN ('send', 'receive')),
    file_count       INTEGER NOT NULL,
    total_bytes      INTEGER NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'cancelled')),
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (peer_fingerprint) REFERENCES peers(fingerprint)
);

CREATE TABLE IF NOT EXISTS resume_state (
    transfer_id      TEXT PRIMARY KEY NOT NULL,
    peer_fingerprint TEXT NOT NULL,
    state_blob       BLOB NOT NULL,
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_peers_trust ON peers(trust_level);
CREATE INDEX IF NOT EXISTS idx_history_peer ON transfer_history(peer_fingerprint);
CREATE INDEX IF NOT EXISTS idx_history_status ON transfer_history(status);
