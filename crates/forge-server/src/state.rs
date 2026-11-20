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
    /// Pull-counter dedup (Step 10): `(ip hash, snippet id)` ->
    /// last counted instant; a pull counts once per pair per
    /// hour. In-memory and single-node, like the other limiters.
    pub pull_dedup: Arc<PullDedup>,
    /// Explore feed cache (Step 10): feed name -> (fetched at,
    /// rows). 60-second TTL per the roadmap.
    pub explore_cache: Arc<ExploreCache>,
    /// Outgoing mail (password resets): SMTP, the dev console
    /// mailer, or disabled — from the configuration.
    pub mailer: Arc<dyn crate::mail::Mailer>,
}

/// Pull dedup map: `(ip hash, snippet id) -> last counted at`.
pub type PullDedup = std::sync::Mutex<std::collections::HashMap<(String, i64), std::time::Instant>>;

/// Explore cache: `feed -> (fetched at, rows)`.
pub type ExploreCache = std::sync::Mutex<
    std::collections::HashMap<String, (std::time::Instant, Vec<crate::repo::SnippetRow>)>,
>;

impl AppState {
    /// Build state from configuration + storage, deriving the
    /// throttles from the config so production and tests agree.
    #[must_use]
    pub fn new(config: Config, repos: Repos, pool: SqlitePool) -> Self {
        let mailer = crate::mail::mailer_from_config(&config);
        Self::with_mailer(config, repos, pool, mailer)
    }

    /// Build state with an explicit mailer (tests capture mail
    /// instead of sending it).
    #[must_use]
    pub fn with_mailer(
        config: Config,
        repos: Repos,
        pool: SqlitePool,
        mailer: Arc<dyn crate::mail::Mailer>,
    ) -> Self {
        let login_throttle = Arc::new(LoginThrottle::new(Duration::from_millis(
            config.login_backoff_base_ms,
        )));
        Self {
            config: Arc::new(config),
            repos: Arc::new(repos),
            pool,
            login_throttle,
            device_limiter: Arc::new(DeviceLimiter::new()),
            pull_dedup: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            explore_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            mailer,
        }
    }

    /// Should this pull be counted? Records the pair when it
    /// should. The IP arrives already hashed (raw IPs are never
    /// stored). Entries older than the window are evicted
    /// opportunistically.
    #[must_use]
    pub fn should_count_pull(&self, ip_hash: &str, snippet_id: i64) -> bool {
        let now = std::time::Instant::now();
        let window = Duration::from_secs(3600);
        let mut map = self
            .pull_dedup
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() > 10_000 {
            map.retain(|_, seen| now.duration_since(*seen) < window);
        }
        let key = (ip_hash.to_string(), snippet_id);
        if map
            .get(&key)
            .is_some_and(|seen| now.duration_since(*seen) < window)
        {
            return false;
        }
        map.insert(key, now);
        true
    }
}
