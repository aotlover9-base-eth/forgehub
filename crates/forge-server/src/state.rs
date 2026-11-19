//! Shared application state handed to handlers.

use crate::config::Config;
use crate::repo::Repos;
use sqlx::SqlitePool;
use std::sync::Arc;

/// Everything a handler needs, behind one `Arc`-friendly struct.
#[derive(Clone)]
pub struct AppState {
    /// Validated configuration.
    pub config: Arc<Config>,
    /// Repositories (the only path to the database for handlers).
    pub repos: Arc<Repos>,
    /// The pool, for the health check's `SELECT 1` only.
    pub pool: SqlitePool,
}
