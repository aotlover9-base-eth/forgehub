//! Shared application state handed to handlers.

use crate::auth::LoginThrottle;
use crate::config::Config;
use crate::device::DeviceLimiter;
use crate::repo::Repos;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;

/// Everything a handler needs, behind one `Arc`-friendly struct.
#[derive(Clone)]
pub struct AppState {
    /// Validated configuration.
    pub config: Arc<Config>,
    /// Repositories (the only path to the database for handlers).
    pub repos: Arc<Repos>,
    /// The pool, for the health check's `SELECT 1` only.
    pub pool: SqlitePool,
    /// Per-IP / per-account login throttling (Step 7).
    pub login_throttle: Arc<LoginThrottle>,
    /// Per-IP device-code lookup throttling (Step 8).
    pub device_limiter: Arc<DeviceLimiter>,
}

impl AppState {
    /// Build state from configuration + storage, deriving the
    /// throttles from the config so production and tests agree.
    #[must_use]
    pub fn new(config: Config, repos: Repos, pool: SqlitePool) -> Self {
        let login_throttle = Arc::new(LoginThrottle::new(Duration::from_millis(
            config.login_backoff_base_ms,
        )));
        Self {
            config: Arc::new(config),
            repos: Arc::new(repos),
            pool,
            login_throttle,
            device_limiter: Arc::new(DeviceLimiter::new()),
        }
    }
}
