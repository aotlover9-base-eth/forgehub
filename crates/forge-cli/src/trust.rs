//! The trust store and run cache (Step 13).
//!
//! `trust.db` (SQLite in the `ForgeHub` home) records which
//! (owner, name, hash) triples the user has explicitly approved
//! for execution. The trust rules in `cloudrun.rs` consult it:
//! a third-party snippet needs a typed `yes` until its exact
//! hash is recorded here; any hash change invalidates the
//! approval and forces a fresh confirmation with a diff.
//!
//! The cache (`<home>/cache/`) stores immutable fetched version
//! bodies so a snippet can be replayed offline and so a changed
//! hash can be diffed against the previously fetched body.
//! Bodies in the cache are data, never executed from the cache
//! path itself (execution always goes through the runner's
//! private temp files).

use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::store::Store;

/// One trust record: the user approved this exact content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustRecord {
    /// Snippet owner username.
    pub owner: String,
    /// Snippet name.
    pub name: String,
    /// The approved content hash.
    pub hash: String,
    /// The approved version number.
    pub version: i64,
    /// When this hash was first approved (ISO-8601 UTC).
    pub first_trusted_at: String,
    /// When it last ran (ISO-8601 UTC).
    pub last_run_at: String,
    /// How many times it has run under this approval.
    pub runs: i64,
}

/// The trust database handle.
pub struct TrustDb {
    conn: Connection,
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

impl TrustDb {
    /// Open (creating if needed) the trust database.
    pub fn open(path: &PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening trust database {}", path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trust (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                hash TEXT NOT NULL,
                version INTEGER NOT NULL,
                first_trusted_at TEXT NOT NULL,
                last_run_at TEXT NOT NULL,
                runs INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (owner, name)
            );",
        )?;
        Ok(Self { conn })
    }

    /// The record for (owner, name), when one exists.
    pub fn get(&self, owner: &str, name: &str) -> Result<Option<TrustRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT owner, name, hash, version, first_trusted_at, last_run_at, runs
             FROM trust WHERE owner = ?1 AND name = ?2",
        )?;
        let record = stmt
            .query_row(rusqlite::params![owner, name], |row| {
                Ok(TrustRecord {
                    owner: row.get(0)?,
                    name: row.get(1)?,
                    hash: row.get(2)?,
                    version: row.get(3)?,
                    first_trusted_at: row.get(4)?,
                    last_run_at: row.get(5)?,
                    runs: row.get(6)?,
                })
            })
            .ok();
        Ok(record)
    }

    /// Record (or refresh) an approval after a confirmed run:
    /// a new hash resets the run counter and the first-trusted
    /// timestamp; the same hash increments the counter.
    pub fn record_run(&self, owner: &str, name: &str, hash: &str, version: i64) -> Result<()> {
        let now = now_iso();
        match self.get(owner, name)? {
            Some(existing) if existing.hash == hash => {
                self.conn.execute(
                    "UPDATE trust SET last_run_at = ?3, runs = runs + 1, version = ?4
                     WHERE owner = ?1 AND name = ?2",
                    rusqlite::params![owner, name, now, version],
                )?;
            }
            _ => {
                self.conn.execute(
                    "INSERT OR REPLACE INTO trust
                     (owner, name, hash, version, first_trusted_at, last_run_at, runs)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
                    rusqlite::params![owner, name, hash, version, now],
                )?;
            }
        }
        Ok(())
    }

    /// Record an approval without running (`forgehub trust …`).
    pub fn record_trust(&self, owner: &str, name: &str, hash: &str, version: i64) -> Result<()> {
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO trust (owner, name, hash, version, first_trusted_at, last_run_at, runs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, 0)
             ON CONFLICT(owner, name) DO UPDATE SET
                hash = excluded.hash, version = excluded.version,
                first_trusted_at = excluded.first_trusted_at,
                last_run_at = excluded.last_run_at, runs = 0",
            rusqlite::params![owner, name, hash, version, now],
        )?;
        Ok(())
    }

    /// All trust records, sorted by owner then name.
    pub fn list(&self) -> Result<Vec<TrustRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT owner, name, hash, version, first_trusted_at, last_run_at, runs
             FROM trust ORDER BY owner, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TrustRecord {
                owner: row.get(0)?,
                name: row.get(1)?,
                hash: row.get(2)?,
                version: row.get(3)?,
                first_trusted_at: row.get(4)?,
                last_run_at: row.get(5)?,
                runs: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Remove the record for (owner, name); returns whether one
    /// existed.
    pub fn revoke(&self, owner: &str, name: &str) -> Result<bool> {
        let changed = self.conn.execute(
            "DELETE FROM trust WHERE owner = ?1 AND name = ?2",
            rusqlite::params![owner, name],
        )?;
        Ok(changed > 0)
    }
}

// ---------------------------------------------------------------------------
// Run cache
// ---------------------------------------------------------------------------

/// The cache pointer for the latest fetched version of a
/// snippet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePointer {
    /// Latest fetched version.
    pub version: i64,
    /// Its content hash.
    pub hash: String,
    /// Its visibility at fetch time.
    pub visibility: String,
    /// When this version was fetched (ISO-8601 UTC; Peak B8 -
    /// older pointers predate the field and read as empty).
    #[serde(default)]
    pub fetched_at: String,
}

/// A snippet body recovered from the cache.
#[derive(Debug, Clone)]
pub struct CachedSnippet {
    /// Version the body belongs to.
    pub version: i64,
    /// Content hash (verified against the body on load).
    pub hash: String,
    /// Visibility at fetch time.
    pub visibility: String,
    /// The canonical forge-file body.
    pub body: String,
    /// When the latest version was fetched (may be empty for
    /// pointers written before Peak B8).
    pub fetched_at: String,
}

pub fn cache_body_path(store: &Store, owner: &str, name: &str, version: i64) -> PathBuf {
    body_path(store, owner, name, version)
}

fn body_path(store: &Store, owner: &str, name: &str, version: i64) -> PathBuf {
    store
        .cache_dir()
        .join(format!("{owner}--{name}--v{version}.yaml"))
}

fn pointer_path(store: &Store, owner: &str, name: &str) -> PathBuf {
    store.cache_dir().join(format!("{owner}--{name}.json"))
}

/// Store a fetched version body and advance the latest pointer.
/// Older version bodies are kept (they are the diff baseline).
pub fn cache_put(
    store: &Store,
    owner: &str,
    name: &str,
    version: i64,
    hash: &str,
    visibility: &str,
    body: &str,
) -> Result<()> {
    let dir = store.cache_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(body_path(store, owner, name, version), body)
        .with_context(|| "writing cached snippet body".to_string())?;
    let pointer = CachePointer {
        version,
        hash: hash.to_string(),
        visibility: visibility.to_string(),
        fetched_at: now_iso(),
    };
    std::fs::write(
        pointer_path(store, owner, name),
        serde_json::to_string(&pointer)?,
    )
    .with_context(|| "writing cache pointer".to_string())?;
    // Keep the cache under its configured cap (Step 18 LRU);
    // eviction failures never fail the run that wrote the entry.
    let cap = store
        .load_config()
        .map_or(100, |c| c.cache_max_mb)
        .saturating_mul(1_048_576);
    crate::cache::evict_lru(store, cap);
    Ok(())
}

/// Load a specific cached version body (no hash verification -
/// callers verify against the hash they expect).
#[must_use]
pub fn cache_body(store: &Store, owner: &str, name: &str, version: i64) -> Option<String> {
    std::fs::read_to_string(body_path(store, owner, name, version)).ok()
}

/// Load the latest cached version, verifying that the body's
/// computed hash matches the pointer (a tampered cache fails
/// closed).
#[must_use]
pub fn cache_latest(store: &Store, owner: &str, name: &str) -> Option<CachedSnippet> {
    let pointer: CachePointer =
        serde_json::from_str(&std::fs::read_to_string(pointer_path(store, owner, name)).ok()?)
            .ok()?;
    let body = cache_body(store, owner, name, pointer.version)?;
    let snippet = forge_core::Snippet::from_yaml(&body).ok()?;
    if forge_core::hash::content_hash(&snippet) != pointer.hash {
        return None;
    }
    Some(CachedSnippet {
        version: pointer.version,
        hash: pointer.hash,
        visibility: pointer.visibility,
        body,
        fetched_at: pointer.fetched_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, TrustDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = TrustDb::open(&dir.path().join("trust.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn trust_lifecycle() {
        let (_d, db) = db();
        assert!(db.get("anna", "tool").unwrap().is_none());
        db.record_run("anna", "tool", "hash1", 1).unwrap();
        let rec = db.get("anna", "tool").unwrap().unwrap();
        assert_eq!(rec.hash, "hash1");
        assert_eq!(rec.runs, 1);
        db.record_run("anna", "tool", "hash1", 1).unwrap();
        assert_eq!(db.get("anna", "tool").unwrap().unwrap().runs, 2);
        // A changed hash resets the approval.
        db.record_run("anna", "tool", "hash2", 2).unwrap();
        let rec = db.get("anna", "tool").unwrap().unwrap();
        assert_eq!(rec.hash, "hash2");
        assert_eq!(rec.runs, 1);
        assert_eq!(db.list().unwrap().len(), 1);
        assert!(db.revoke("anna", "tool").unwrap());
        assert!(!db.revoke("anna", "tool").unwrap());
        assert!(db.get("anna", "tool").unwrap().is_none());
    }

    #[test]
    fn record_trust_without_running() {
        let (_d, db) = db();
        db.record_trust("bob", "setup", "h", 3).unwrap();
        let rec = db.get("bob", "setup").unwrap().unwrap();
        assert_eq!(rec.runs, 0);
        assert_eq!(rec.version, 3);
    }
}
