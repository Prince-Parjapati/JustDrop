//! SQLite persistence layer for JustDrop.
//!
//! Stores device identities, peer trust relationships, transfer history,
//! and resume state. Schema is versioned with forward-only migrations.

use crate::error::JustDropError;
use crate::trust::TrustLevel;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use tracing::{debug, info};

/// Current schema version. Bump when adding migrations.
const SCHEMA_VERSION: u32 = 1;

/// Thread-safe database handle.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create the database at the given path and run migrations.
    pub fn open(path: &Path) -> Result<Self, JustDropError> {
        let conn = Connection::open(path)
            .map_err(|e| std::io::Error::other(format!("sqlite open: {e}")))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| std::io::Error::other(format!("sqlite pragma: {e}")))?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;

        info!(path = %path.display(), "database opened");
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    #[cfg(test)]
    pub fn open_memory() -> Result<Self, JustDropError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| std::io::Error::other(format!("sqlite memory: {e}")))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| std::io::Error::other(format!("sqlite pragma: {e}")))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Run forward-only schema migrations.
    fn migrate(&self) -> Result<(), JustDropError> {
        let conn = self.conn.lock();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );",
        )
        .map_err(|e| std::io::Error::other(format!("migrate init: {e}")))?;

        let current: u32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .optional()
            .map_err(|e| std::io::Error::other(format!("migrate query: {e}")))?
            .unwrap_or(0);

        if current < 1 {
            debug!("applying migration v1");
            conn.execute_batch(include_str!("migrations/v001.sql"))
                .map_err(|e| std::io::Error::other(format!("migration v1 failed: {e}")))?;
        }

        // Upsert version
        conn.execute(
            "INSERT OR REPLACE INTO schema_version (rowid, version) VALUES (1, ?1)",
            params![SCHEMA_VERSION],
        )
        .map_err(|e| std::io::Error::other(format!("version update: {e}")))?;

        info!(version = SCHEMA_VERSION, "schema at version");
        Ok(())
    }

    // ── Peer Trust ──────────────────────────────────────────────────────

    /// Set trust level for a peer identified by fingerprint hex.
    pub fn set_trust(
        &self,
        fingerprint: &str,
        name: &str,
        platform: &str,
        level: TrustLevel,
    ) -> Result<(), JustDropError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO peers (fingerprint, name, platform, trust_level, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(fingerprint) DO UPDATE SET
                name = excluded.name,
                platform = excluded.platform,
                trust_level = excluded.trust_level,
                updated_at = excluded.updated_at",
            params![fingerprint, name, platform, level.as_str()],
        )
        .map_err(|e| std::io::Error::other(format!("set_trust: {e}")))?;
        Ok(())
    }

    /// Get trust level for a peer. Returns `Unknown` if not in DB.
    pub fn get_trust(&self, fingerprint: &str) -> TrustLevel {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT trust_level FROM peers WHERE fingerprint = ?1",
            params![fingerprint],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .map(|s| TrustLevel::parse_str(&s))
        .unwrap_or(TrustLevel::Unknown)
    }

    /// Check if a peer is blocked.
    pub fn is_blocked(&self, fingerprint: &str) -> bool {
        self.get_trust(fingerprint) == TrustLevel::Blocked
    }

    /// Get all peers with a given trust level.
    pub fn peers_with_trust(&self, level: TrustLevel) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let mut stmt = match conn
            .prepare("SELECT fingerprint, name FROM peers WHERE trust_level = ?1 ORDER BY name")
        {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![level.as_str()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // ── Transfer History ────────────────────────────────────────────────

    /// Record a completed or failed transfer.
    pub fn record_transfer(
        &self,
        transfer_id: &str,
        peer_fingerprint: &str,
        direction: &str,
        file_count: u32,
        total_bytes: u64,
        status: &str,
    ) -> Result<(), JustDropError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO transfer_history
             (transfer_id, peer_fingerprint, direction, file_count, total_bytes, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![
                transfer_id,
                peer_fingerprint,
                direction,
                file_count,
                total_bytes,
                status
            ],
        )
        .map_err(|e| std::io::Error::other(format!("record_transfer: {e}")))?;
        Ok(())
    }

    // ── Resume State ────────────────────────────────────────────────────

    /// Save resume state for an interrupted transfer.
    pub fn save_resume_state(
        &self,
        transfer_id: &str,
        peer_fingerprint: &str,
        state_blob: &[u8],
    ) -> Result<(), JustDropError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO resume_state
             (transfer_id, peer_fingerprint, state_blob, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            params![transfer_id, peer_fingerprint, state_blob],
        )
        .map_err(|e| std::io::Error::other(format!("save_resume: {e}")))?;
        Ok(())
    }

    /// Load resume state. Returns None if no state exists.
    pub fn load_resume_state(&self, transfer_id: &str) -> Option<Vec<u8>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT state_blob FROM resume_state WHERE transfer_id = ?1",
            params![transfer_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten()
    }

    /// Delete resume state after successful completion.
    pub fn clear_resume_state(&self, transfer_id: &str) -> Result<(), JustDropError> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM resume_state WHERE transfer_id = ?1",
            params![transfer_id],
        )
        .map_err(|e| std::io::Error::other(format!("clear_resume: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_creates_tables() {
        let db = Database::open_memory().unwrap();
        // Verify tables exist by inserting
        db.set_trust("abc123", "Test Device", "macOS", TrustLevel::Trusted)
            .unwrap();
        assert_eq!(db.get_trust("abc123"), TrustLevel::Trusted);
    }

    #[test]
    fn unknown_peer_returns_unknown() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.get_trust("nonexistent"), TrustLevel::Unknown);
    }

    #[test]
    fn blocked_peer_detected() {
        let db = Database::open_memory().unwrap();
        db.set_trust("evil", "BadDevice", "Android", TrustLevel::Blocked)
            .unwrap();
        assert!(db.is_blocked("evil"));
        assert!(!db.is_blocked("good"));
    }

    #[test]
    fn trust_upgrade_path() {
        let db = Database::open_memory().unwrap();
        db.set_trust("dev1", "Phone", "Android", TrustLevel::Unknown)
            .unwrap();
        assert_eq!(db.get_trust("dev1"), TrustLevel::Unknown);

        db.set_trust("dev1", "Phone", "Android", TrustLevel::Trusted)
            .unwrap();
        assert_eq!(db.get_trust("dev1"), TrustLevel::Trusted);

        db.set_trust("dev1", "Phone", "Android", TrustLevel::Favorite)
            .unwrap();
        assert_eq!(db.get_trust("dev1"), TrustLevel::Favorite);
    }

    #[test]
    fn resume_state_roundtrip() {
        let db = Database::open_memory().unwrap();
        let blob = vec![1, 2, 3, 4, 5];
        db.save_resume_state("tx-001", "peer-abc", &blob).unwrap();

        let loaded = db.load_resume_state("tx-001").unwrap();
        assert_eq!(loaded, blob);

        db.clear_resume_state("tx-001").unwrap();
        assert!(db.load_resume_state("tx-001").is_none());
    }

    #[test]
    fn transfer_history_insert() {
        let db = Database::open_memory().unwrap();
        db.set_trust("peer-xyz", "TestPeer", "macOS", TrustLevel::Trusted)
            .unwrap();
        db.record_transfer("tx-002", "peer-xyz", "send", 3, 1024 * 1024, "completed")
            .unwrap();
    }

    #[test]
    fn peers_filtered_by_trust() {
        let db = Database::open_memory().unwrap();
        db.set_trust("a", "DevA", "macOS", TrustLevel::Trusted)
            .unwrap();
        db.set_trust("b", "DevB", "Android", TrustLevel::Blocked)
            .unwrap();
        db.set_trust("c", "DevC", "macOS", TrustLevel::Trusted)
            .unwrap();

        let trusted = db.peers_with_trust(TrustLevel::Trusted);
        assert_eq!(trusted.len(), 2);

        let blocked = db.peers_with_trust(TrustLevel::Blocked);
        assert_eq!(blocked.len(), 1);
    }
}
