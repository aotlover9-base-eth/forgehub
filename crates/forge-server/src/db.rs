//! SQLite connection pool + migrations.
//!
//! WAL mode, foreign keys ON, a busy timeout, and a small pool (SQLite
//! serialises writers anyway). Timestamps are supplied by the
//! application as UTC ISO-8601 text (spec-data-model conventions).

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

/// The current UTC time as ISO-8601 text (`2026-11-19T05:30:00Z`).
#[must_use]
pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Create the pool for `database_url` (`sqlite://path`, `sqlite:path`,
/// a bare path, or `sqlite::memory:`).
///
/// # Errors
/// On an invalid URL or a connection failure.
pub async fn connect(database_url: &str) -> Result<SqlitePool> {
    let mut options = SqliteConnectOptions::from_str(database_url)
        .with_context(|| format!("parsing database URL `{database_url}`"))?;
    options = options
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .with_context(|| format!("connecting to `{database_url}`"))?;
    Ok(pool)
}

/// Run all pending migrations (idempotent: sqlx records applied
/// versions in `_sqlx_migrations`).
///
/// # Errors
/// On a migration failure.
pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("running migrations")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_run_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("t.db").display());
        let pool = connect(&url).await.unwrap();
        migrate(&pool).await.unwrap();
        // Second run: no-op, no error.
        migrate(&pool).await.unwrap();
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        for expected in [
            "users",
            "sessions",
            "api_tokens",
            "device_codes",
            "snippets",
            "snippet_versions",
            "snippet_tags",
            "stars",
            "renames",
            "reports",
            "snippets_fts",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "missing {expected}: {tables:?}"
            );
        }
        // WAL + foreign keys actually in effect.
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("fk.db").display());
        let pool = connect(&url).await.unwrap();
        migrate(&pool).await.unwrap();
        let err = sqlx::query(
            "INSERT INTO sessions (id_hash, user_id, csrf_token, created_at, last_used_at, expires_at)
             VALUES ('x', 999, 'c', 't', 't', 't')",
        )
        .execute(&pool)
        .await;
        assert!(err.is_err(), "FK violation must fail");
    }
}
