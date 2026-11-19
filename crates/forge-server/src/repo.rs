//! Repository traits and their SQLite implementations.
//!
//! Handlers depend on the traits, never on SQL (ADR 0002), so a
//! Postgres implementation can replace `SqliteStore` later. Versions
//! are immutable: `SnippetRepo` deliberately has no version-update
//! method. FTS5 rows are maintained here, in the same transaction as
//! the write they belong to (spec-data-model §3).

use async_trait::async_trait;
use serde::Serialize;
use sqlx::SqlitePool;

/// Repository errors.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    /// The requested row does not exist (carries what was looked up).
    #[error("{0} not found")]
    NotFound(String),
    /// A uniqueness or state conflict (safe message).
    #[error("{0}")]
    Conflict(String),
    /// A database failure (details stay server-side).
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// Repository result alias.
pub type RepoResult<T> = Result<T, RepoError>;

/// A user row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct UserRow {
    /// Primary key.
    pub id: i64,
    /// Unique, stored lowercase.
    pub username: String,
    /// Unique, stored lowercase.
    pub email: String,
    /// Argon2id PHC string.
    pub password_hash: String,
    /// Optional display name.
    pub display_name: Option<String>,
    /// Optional bio.
    pub bio: Option<String>,
    /// ISO-8601 creation time.
    pub created_at: String,
    /// Set when an admin disables the account.
    pub disabled_at: Option<String>,
    /// Set on soft delete.
    pub deleted_at: Option<String>,
}

/// A web session row (the cookie value itself is never stored).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SessionRow {
    /// SHA-256 hex of the session id.
    pub id_hash: String,
    /// Owner.
    pub user_id: i64,
    /// Per-session CSRF token.
    pub csrf_token: String,
    /// Creation time.
    pub created_at: String,
    /// Last activity (idle expiry is enforced against this).
    pub last_used_at: String,
    /// Absolute expiry.
    pub expires_at: String,
    /// Display-only IP hint.
    pub ip_hint: Option<String>,
    /// Display-only user-agent hint.
    pub ua_hint: Option<String>,
}

/// A CLI API token row (only the hash is stored).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct TokenRow {
    /// Primary key.
    pub id: i64,
    /// Owner.
    pub user_id: i64,
    /// User-chosen label.
    pub name: String,
    /// First 8 chars after `fgh_`, for display.
    pub prefix: String,
    /// SHA-256 hex of the full token.
    pub token_hash: String,
    /// Space-separated scopes.
    pub scopes: String,
    /// Creation time.
    pub created_at: String,
    /// Last use, NULL until first use.
    pub last_used_at: Option<String>,
    /// Optional expiry.
    pub expires_at: Option<String>,
    /// Set when revoked.
    pub revoked_at: Option<String>,
}

/// A device-code flow row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DeviceCodeRow {
    /// Primary key.
    pub id: i64,
    /// SHA-256 hex of the device code.
    pub device_code_hash: String,
    /// The 8-char user code (uppercase, no hyphen).
    pub user_code: String,
    /// Client label from the device.
    pub client_name: String,
    /// pending | approved | denied | expired | consumed.
    pub status: String,
    /// Approving user, NULL until approved.
    pub user_id: Option<i64>,
    /// Minted token, NULL until approval.
    pub token_id: Option<i64>,
    /// Creation time.
    pub created_at: String,
    /// Expiry (600 s lifetime).
    pub expires_at: String,
    /// Last poll time (server-side poll-interval enforcement).
    pub last_polled_at: Option<String>,
    /// Failed lookup/approval attempts.
    pub attempts: i64,
}

/// A snippet row (metadata; bodies live in versions).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct SnippetRow {
    /// Primary key.
    pub id: i64,
    /// Owner.
    pub owner_id: i64,
    /// Name (unique per owner).
    pub name: String,
    /// Short description.
    pub description: String,
    /// private | unlisted | public.
    pub visibility: String,
    /// Latest version number (0 only transiently during creation).
    pub latest_version: i64,
    /// Maintained counters.
    pub star_count: i64,
    /// Maintained counters.
    pub fork_count: i64,
    /// Maintained counters.
    pub pull_count: i64,
    /// Fork provenance.
    pub forked_from_snippet_id: Option<i64>,
    /// Fork provenance.
    pub forked_from_version: Option<i64>,
    /// Fork provenance.
    pub forked_from_hash: Option<String>,
    /// Admin hide marker.
    pub hidden_at: Option<String>,
    /// Soft-delete marker.
    pub deleted_at: Option<String>,
    /// Creation time.
    pub created_at: String,
    /// Last metadata/version change.
    pub updated_at: String,
}

/// One immutable snippet version.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct VersionRow {
    /// Primary key.
    pub id: i64,
    /// Owning snippet.
    pub snippet_id: i64,
    /// Version number (from 1, never reused).
    pub version: i64,
    /// script | steps.
    pub mode: String,
    /// Canonical forge-file YAML text.
    pub body: String,
    /// SHA-256 hex per snippet-format §9.
    pub hash: String,
    /// Optional version note.
    pub message: Option<String>,
    /// Creation time.
    pub created_at: String,
}

/// Users repository.
#[async_trait]
pub trait UserRepo: Send + Sync {
    /// Insert a user; `Conflict` on duplicate username/email.
    async fn create(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        now: &str,
    ) -> RepoResult<UserRow>;
    /// By primary key.
    async fn by_id(&self, id: i64) -> RepoResult<Option<UserRow>>;
    /// By username (case-insensitive).
    async fn by_username(&self, username: &str) -> RepoResult<Option<UserRow>>;
    /// By email (case-insensitive).
    async fn by_email(&self, email: &str) -> RepoResult<Option<UserRow>>;
    /// Set display name and bio (already-resolved final values).
    async fn update_profile(
        &self,
        id: i64,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> RepoResult<()>;
    /// Replace the password hash.
    async fn set_password_hash(&self, id: i64, password_hash: &str) -> RepoResult<()>;
    /// Soft-delete the account (sets `deleted_at`).
    async fn soft_delete(&self, id: i64, now: &str) -> RepoResult<()>;
    /// Admin ban/unban (sets or clears `disabled_at`). Banning
    /// also revokes the user's sessions and tokens (done by the
    /// caller in the same operation via the other repos).
    async fn set_disabled(&self, id: i64, disabled: bool, now: &str) -> RepoResult<()>;
}

/// Sessions repository.
#[async_trait]
pub trait SessionRepo: Send + Sync {
    /// Insert a session.
    async fn create(&self, session: &SessionRow) -> RepoResult<()>;
    /// By id hash.
    async fn by_id_hash(&self, id_hash: &str) -> RepoResult<Option<SessionRow>>;
    /// Update `last_used_at`.
    async fn touch(&self, id_hash: &str, now: &str) -> RepoResult<()>;
    /// Delete one session (logout).
    async fn delete(&self, id_hash: &str) -> RepoResult<()>;
    /// Delete all of a user's sessions.
    async fn delete_for_user(&self, user_id: i64) -> RepoResult<u64>;
}

/// API tokens repository.
#[async_trait]
#[allow(clippy::too_many_arguments)] // one row's columns, deliberately explicit
pub trait TokenRepo: Send + Sync {
    /// Insert a token row (the plaintext token is never stored).
    async fn create(
        &self,
        user_id: i64,
        name: &str,
        prefix: &str,
        token_hash: &str,
        scopes: &str,
        expires_at: Option<&str>,
        now: &str,
    ) -> RepoResult<TokenRow>;
    /// By token hash.
    async fn by_hash(&self, token_hash: &str) -> RepoResult<Option<TokenRow>>;
    /// Record a use.
    async fn touch(&self, id: i64, now: &str) -> RepoResult<()>;
    /// Revoke a token.
    async fn revoke(&self, id: i64, now: &str) -> RepoResult<()>;
    /// Revoke all of a user's tokens (account deletion).
    async fn revoke_all_for_user(&self, user_id: i64, now: &str) -> RepoResult<u64>;
    /// A user's tokens, newest first.
    async fn list_for_user(&self, user_id: i64) -> RepoResult<Vec<TokenRow>>;
    /// Count of a user's unrevoked tokens (the per-user limit is
    /// enforced against this).
    async fn count_active_for_user(&self, user_id: i64) -> RepoResult<i64>;
}

/// One filed snippet report.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportRow {
    /// Primary key.
    pub id: i64,
    /// Reported snippet.
    pub snippet_id: i64,
    /// Reporting user.
    pub reporter_id: i64,
    /// Free-text reason (max 500 chars, enforced by the handler).
    pub reason: String,
    /// open | resolved | dismissed.
    pub status: String,
    /// Filing time.
    pub created_at: String,
}

/// Device codes repository.
#[async_trait]
pub trait DeviceCodeRepo: Send + Sync {
    /// Insert a pending device code.
    async fn create(
        &self,
        device_code_hash: &str,
        user_code: &str,
        client_name: &str,
        expires_at: &str,
        now: &str,
    ) -> RepoResult<DeviceCodeRow>;
    /// By user code (uppercase, no hyphen).
    async fn by_user_code(&self, user_code: &str) -> RepoResult<Option<DeviceCodeRow>>;
    /// By device code hash.
    async fn by_hash(&self, device_code_hash: &str) -> RepoResult<Option<DeviceCodeRow>>;
    /// Record a poll (interval enforcement reads `last_polled_at`).
    async fn record_poll(&self, id: i64, now: &str) -> RepoResult<()>;
    /// Set status (and, on approval, the user and minted token).
    async fn set_status(
        &self,
        id: i64,
        status: &str,
        user_id: Option<i64>,
        token_id: Option<i64>,
    ) -> RepoResult<()>;
    /// Increment the failed-attempts counter.
    async fn record_attempt(&self, id: i64) -> RepoResult<()>;
    /// Atomically move an approved code to `consumed`, recording
    /// the minted token. Returns false when the code was not in
    /// `approved` state (a racing poll already delivered it), so
    /// the token is delivered exactly once.
    async fn consume(&self, id: i64, token_id: i64) -> RepoResult<bool>;
}

/// Snippets repository (metadata + immutable versions + tags + stars).
#[async_trait]
#[allow(clippy::too_many_arguments)] // one write's fields, deliberately explicit
pub trait SnippetRepo: Send + Sync {
    /// Create a snippet with its first version and tags, in one
    /// transaction; FTS is synced in the same transaction.
    async fn create_with_version(
        &self,
        owner_id: i64,
        name: &str,
        description: &str,
        visibility: &str,
        mode: &str,
        body: &str,
        hash: &str,
        message: Option<&str>,
        tags: &[String],
        now: &str,
    ) -> RepoResult<(SnippetRow, VersionRow)>;
    /// Append a new immutable version (bumps `latest_version`,
    /// reconciles tags, syncs FTS) in one transaction.
    /// When `expected_latest` is set, the append only happens if
    /// the snippet is still at that version *inside the write
    /// transaction* (optimistic concurrency); the result is then
    /// `None` instead of the new version.
    async fn add_version(
        &self,
        snippet_id: i64,
        mode: &str,
        body: &str,
        hash: &str,
        message: Option<&str>,
        tags: &[String],
        now: &str,
        expected_latest: Option<i64>,
    ) -> RepoResult<Option<VersionRow>>;
    /// By id.
    async fn by_id(&self, id: i64) -> RepoResult<Option<SnippetRow>>;
    /// By owner + name.
    async fn by_owner_name(&self, owner_id: i64, name: &str) -> RepoResult<Option<SnippetRow>>;
    /// One version of a snippet.
    async fn version(&self, snippet_id: i64, version: i64) -> RepoResult<Option<VersionRow>>;
    /// The latest version of a snippet.
    async fn latest_version(&self, snippet_id: i64) -> RepoResult<Option<VersionRow>>;
    /// All versions, newest first.
    async fn versions(&self, snippet_id: i64) -> RepoResult<Vec<VersionRow>>;
    /// Current tags.
    async fn tags(&self, snippet_id: i64) -> RepoResult<Vec<String>>;
    /// Update metadata (description/visibility/tags), syncing FTS.
    async fn update_metadata(
        &self,
        snippet_id: i64,
        description: &str,
        visibility: &str,
        tags: &[String],
        now: &str,
    ) -> RepoResult<()>;
    /// Rename: updates the name and records a redirect from the
    /// old name (expiring at `expires_at`) in one transaction.
    async fn rename(
        &self,
        snippet_id: i64,
        owner_id: i64,
        old_name: &str,
        new_name: &str,
        expires_at: &str,
        now: &str,
    ) -> RepoResult<()>;
    /// Resolve a rename redirect: the snippet id an old name
    /// currently points at, if the redirect is unexpired.
    async fn resolve_rename(
        &self,
        owner_id: i64,
        old_name: &str,
        now: &str,
    ) -> RepoResult<Option<i64>>;
    /// Count of a user's live (not soft-deleted) snippets.
    async fn count_for_owner(&self, owner_id: i64) -> RepoResult<i64>;
    /// A page of a user's live snippets with optional visibility /
    /// tag filters. `sort` is one of `updated|created|name`
    /// (validated by the caller).
    async fn list_for_owner(
        &self,
        owner_id: i64,
        visibility: Option<&str>,
        tag: Option<&str>,
        sort: &str,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>>;
    /// Hard-delete snippets soft-deleted before `deleted_before`
    /// (versions/tags/stars/renames cascade) and drop rename
    /// redirects expired at `now`. Returns rows removed.
    async fn purge_deleted(&self, deleted_before: &str, now: &str) -> RepoResult<u64>;
    /// Public discovery search. When `match_query` is present it
    /// is a *sanitised* FTS5 MATCH string built by the handler
    /// (quoted prefix terms only); `sort` is relevance|stars|
    /// updated. Only public, live, unhidden snippets are ever
    /// returned — the predicates are enforced here as well as by
    /// FTS membership, as defence in depth.
    async fn search_public(
        &self,
        match_query: Option<&str>,
        tag: Option<&str>,
        owner_id: Option<i64>,
        sort: &str,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>>;
    /// Most-used tags on public snippets: `(tag, count)` pairs.
    async fn popular_tags(&self, limit: i64) -> RepoResult<Vec<(String, i64)>>;
    /// Explore feed: most recently updated public snippets.
    async fn explore_recent(&self, limit: i64, offset: i64) -> RepoResult<Vec<SnippetRow>>;
    /// Explore feed: public snippets by stars received since
    /// `since` (the "popular this week" window), then total stars.
    async fn explore_popular(
        &self,
        since: &str,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>>;
    /// Snippets a user starred, most recent star first. Includes
    /// unlisted; excludes deleted/hidden and others' private.
    async fn list_starred(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>>;
    /// A user's public snippets, most recently updated first.
    async fn list_public_for_owner(
        &self,
        owner_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>>;
    /// Count of a user's public live snippets.
    async fn count_public_for_owner(&self, owner_id: i64) -> RepoResult<i64>;
    /// Create a fork: a private copy of a source version in the
    /// caller's account, with provenance recorded and the
    /// source's `fork_count` incremented, in one transaction.
    #[allow(clippy::too_many_arguments)] // one write's fields, deliberately explicit
    async fn create_fork(
        &self,
        owner_id: i64,
        name: &str,
        description: &str,
        mode: &str,
        body: &str,
        hash: &str,
        tags: &[String],
        source_id: i64,
        source_version: i64,
        source_hash: &str,
        now: &str,
    ) -> RepoResult<(SnippetRow, VersionRow)>;
    /// Increment the public pull counter.
    async fn increment_pull(&self, snippet_id: i64) -> RepoResult<()>;
    /// Admin hide/unhide (FTS is synced in the same transaction).
    async fn set_hidden(&self, snippet_id: i64, hidden: bool, now: &str) -> RepoResult<()>;
    /// Soft-delete (removes the FTS row in the same transaction).
    async fn soft_delete(&self, snippet_id: i64, now: &str) -> RepoResult<()>;
    /// Star (idempotent); maintains `star_count` transactionally.
    async fn star(&self, user_id: i64, snippet_id: i64, now: &str) -> RepoResult<()>;
    /// Unstar (idempotent); maintains `star_count` transactionally.
    async fn unstar(&self, user_id: i64, snippet_id: i64) -> RepoResult<()>;
    /// Has this user starred this snippet?
    async fn has_starred(&self, user_id: i64, snippet_id: i64) -> RepoResult<bool>;
}

/// Reports repository (moderation, Step 10 uses it fully).
#[async_trait]
pub trait ReportRepo: Send + Sync {
    /// File a report.
    async fn create(
        &self,
        snippet_id: i64,
        reporter_id: i64,
        reason: &str,
        now: &str,
    ) -> RepoResult<i64>;
    /// Count reports by status.
    async fn count_by_status(&self, status: &str) -> RepoResult<i64>;
    /// List reports, newest first, optionally by status.
    async fn list(&self, status: Option<&str>, limit: i64) -> RepoResult<Vec<ReportRow>>;
    /// Whether this user already has an open report on the
    /// snippet (report dedup).
    async fn exists_open(&self, reporter_id: i64, snippet_id: i64) -> RepoResult<bool>;
}

/// All repositories, backed by one store implementation.
pub struct Repos {
    /// Users.
    pub users: std::sync::Arc<dyn UserRepo>,
    /// Sessions.
    pub sessions: std::sync::Arc<dyn SessionRepo>,
    /// API tokens.
    pub tokens: std::sync::Arc<dyn TokenRepo>,
    /// Device codes.
    pub device_codes: std::sync::Arc<dyn DeviceCodeRepo>,
    /// Snippets.
    pub snippets: std::sync::Arc<dyn SnippetRepo>,
    /// Reports.
    pub reports: std::sync::Arc<dyn ReportRepo>,
}

impl Repos {
    /// Build all repositories over one SQLite pool.
    #[must_use]
    pub fn sqlite(pool: SqlitePool) -> Self {
        let store = std::sync::Arc::new(SqliteStore { pool });
        Self {
            users: store.clone(),
            sessions: store.clone(),
            tokens: store.clone(),
            device_codes: store.clone(),
            snippets: store.clone(),
            reports: store,
        }
    }
}

/// The SQLite implementation of every repository trait.
#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// The underlying pool (health checks).
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.is_unique_violation())
}

/// Map a unique violation to [`RepoError::Conflict`]; pass other
/// errors through as storage errors.
fn map_unique(err: sqlx::Error) -> RepoError {
    if is_unique_violation(&err) {
        RepoError::Conflict("unique constraint violated".to_string())
    } else {
        RepoError::Sqlx(err)
    }
}

// ------------------------------------------------------------------ users

#[async_trait]
impl UserRepo for SqliteStore {
    async fn create(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        now: &str,
    ) -> RepoResult<UserRow> {
        let result = sqlx::query_as::<_, UserRow>(
            "INSERT INTO users (username, email, password_hash, created_at)
             VALUES (?1, ?2, ?3, ?4) RETURNING *",
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(now)
        .fetch_one(&self.pool)
        .await;
        match result {
            Ok(row) => Ok(row),
            Err(e) if is_unique_violation(&e) => Err(RepoError::Conflict(
                "username or email is already taken".to_string(),
            )),
            Err(e) => Err(e.into()),
        }
    }

    async fn by_id(&self, id: i64) -> RepoResult<Option<UserRow>> {
        Ok(
            sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn by_username(&self, username: &str) -> RepoResult<Option<UserRow>> {
        Ok(
            sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE username = ?1 COLLATE NOCASE")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn by_email(&self, email: &str) -> RepoResult<Option<UserRow>> {
        Ok(
            sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE email = ?1 COLLATE NOCASE")
                .bind(email)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn update_profile(
        &self,
        id: i64,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> RepoResult<()> {
        sqlx::query("UPDATE users SET display_name = ?2, bio = ?3 WHERE id = ?1")
            .bind(id)
            .bind(display_name)
            .bind(bio)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_password_hash(&self, id: i64, password_hash: &str) -> RepoResult<()> {
        sqlx::query("UPDATE users SET password_hash = ?2 WHERE id = ?1")
            .bind(id)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_disabled(&self, id: i64, disabled: bool, now: &str) -> RepoResult<()> {
        if disabled {
            sqlx::query("UPDATE users SET disabled_at = ?2 WHERE id = ?1")
                .bind(id)
                .bind(now)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query("UPDATE users SET disabled_at = NULL WHERE id = ?1")
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn soft_delete(&self, id: i64, now: &str) -> RepoResult<()> {
        sqlx::query("UPDATE users SET deleted_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// --------------------------------------------------------------- sessions

#[async_trait]
impl SessionRepo for SqliteStore {
    async fn create(&self, s: &SessionRow) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO sessions (id_hash, user_id, csrf_token, created_at, last_used_at, expires_at, ip_hint, ua_hint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&s.id_hash)
        .bind(s.user_id)
        .bind(&s.csrf_token)
        .bind(&s.created_at)
        .bind(&s.last_used_at)
        .bind(&s.expires_at)
        .bind(&s.ip_hint)
        .bind(&s.ua_hint)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn by_id_hash(&self, id_hash: &str) -> RepoResult<Option<SessionRow>> {
        Ok(
            sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id_hash = ?1")
                .bind(id_hash)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn touch(&self, id_hash: &str, now: &str) -> RepoResult<()> {
        sqlx::query("UPDATE sessions SET last_used_at = ?2 WHERE id_hash = ?1")
            .bind(id_hash)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id_hash: &str) -> RepoResult<()> {
        sqlx::query("DELETE FROM sessions WHERE id_hash = ?1")
            .bind(id_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_for_user(&self, user_id: i64) -> RepoResult<u64> {
        let res = sqlx::query("DELETE FROM sessions WHERE user_id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}

// ----------------------------------------------------------------- tokens

#[async_trait]
impl TokenRepo for SqliteStore {
    async fn create(
        &self,
        user_id: i64,
        name: &str,
        prefix: &str,
        token_hash: &str,
        scopes: &str,
        expires_at: Option<&str>,
        now: &str,
    ) -> RepoResult<TokenRow> {
        Ok(sqlx::query_as::<_, TokenRow>(
            "INSERT INTO api_tokens (user_id, name, prefix, token_hash, scopes, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING *",
        )
        .bind(user_id)
        .bind(name)
        .bind(prefix)
        .bind(token_hash)
        .bind(scopes)
        .bind(now)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?)
    }

    async fn by_hash(&self, token_hash: &str) -> RepoResult<Option<TokenRow>> {
        Ok(
            sqlx::query_as::<_, TokenRow>("SELECT * FROM api_tokens WHERE token_hash = ?1")
                .bind(token_hash)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn touch(&self, id: i64, now: &str) -> RepoResult<()> {
        sqlx::query("UPDATE api_tokens SET last_used_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke(&self, id: i64, now: &str) -> RepoResult<()> {
        sqlx::query("UPDATE api_tokens SET revoked_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke_all_for_user(&self, user_id: i64, now: &str) -> RepoResult<u64> {
        let res = sqlx::query(
            "UPDATE api_tokens SET revoked_at = ?2 WHERE user_id = ?1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    async fn list_for_user(&self, user_id: i64) -> RepoResult<Vec<TokenRow>> {
        Ok(sqlx::query_as::<_, TokenRow>(
            "SELECT * FROM api_tokens WHERE user_id = ?1 ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn count_active_for_user(&self, user_id: i64) -> RepoResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM api_tokens WHERE user_id = ?1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?)
    }
}

// ------------------------------------------------------------ device codes

#[async_trait]
impl DeviceCodeRepo for SqliteStore {
    async fn create(
        &self,
        device_code_hash: &str,
        user_code: &str,
        client_name: &str,
        expires_at: &str,
        now: &str,
    ) -> RepoResult<DeviceCodeRow> {
        Ok(sqlx::query_as::<_, DeviceCodeRow>(
            "INSERT INTO device_codes (device_code_hash, user_code, client_name, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *",
        )
        .bind(device_code_hash)
        .bind(user_code)
        .bind(client_name)
        .bind(now)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?)
    }

    async fn by_user_code(&self, user_code: &str) -> RepoResult<Option<DeviceCodeRow>> {
        Ok(
            sqlx::query_as::<_, DeviceCodeRow>("SELECT * FROM device_codes WHERE user_code = ?1")
                .bind(user_code)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn by_hash(&self, device_code_hash: &str) -> RepoResult<Option<DeviceCodeRow>> {
        Ok(sqlx::query_as::<_, DeviceCodeRow>(
            "SELECT * FROM device_codes WHERE device_code_hash = ?1",
        )
        .bind(device_code_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn record_poll(&self, id: i64, now: &str) -> RepoResult<()> {
        sqlx::query("UPDATE device_codes SET last_polled_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_status(
        &self,
        id: i64,
        status: &str,
        user_id: Option<i64>,
        token_id: Option<i64>,
    ) -> RepoResult<()> {
        sqlx::query(
            "UPDATE device_codes SET status = ?2, user_id = ?3, token_id = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(status)
        .bind(user_id)
        .bind(token_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_attempt(&self, id: i64) -> RepoResult<()> {
        sqlx::query("UPDATE device_codes SET attempts = attempts + 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn consume(&self, id: i64, token_id: i64) -> RepoResult<bool> {
        let res = sqlx::query(
            "UPDATE device_codes SET status = 'consumed', token_id = ?2 \
             WHERE id = ?1 AND status = 'approved'",
        )
        .bind(id)
        .bind(token_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }
}
// --------------------------------------------------------------- snippets

/// The values one snippet contributes to the FTS index, if it is
/// indexable at all: `(name, description, tags, owner, body)`.
type FtsSnapshot = (String, String, String, String, String);

/// Read the snippet's current FTS snapshot inside `tx`, applying
/// exactly the "public, latest version, not hidden, not deleted"
/// rule from spec-data-model §3 — in one place.
async fn fts_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    snippet_id: i64,
) -> RepoResult<Option<FtsSnapshot>> {
    let row = sqlx::query_as(
        "SELECT s.name, s.description,
                COALESCE((SELECT group_concat(tag, ' ') FROM snippet_tags WHERE snippet_id = s.id), ''),
                u.username, v.body
         FROM snippets s
         JOIN users u ON u.id = s.owner_id
         JOIN snippet_versions v ON v.snippet_id = s.id AND v.version = s.latest_version
         WHERE s.id = ?1 AND s.visibility = 'public' AND s.hidden_at IS NULL AND s.deleted_at IS NULL",
    )
    .bind(snippet_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row)
}

/// Reconcile the snippet's FTS row inside `tx` after a mutation.
/// `old` must be the snapshot captured *before* the mutation: the
/// table is contentless (spec-data-model §3), so a row can only be
/// removed with the special `delete` command carrying the values
/// that were originally indexed.
async fn sync_fts(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    snippet_id: i64,
    old: Option<&FtsSnapshot>,
) -> RepoResult<()> {
    if let Some((name, description, tags, owner, body)) = old {
        sqlx::query(
            "INSERT INTO snippets_fts (snippets_fts, rowid, name, description, tags, owner, body)
             VALUES ('delete', ?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(snippet_id)
        .bind(name)
        .bind(description)
        .bind(tags)
        .bind(owner)
        .bind(body)
        .execute(&mut **tx)
        .await?;
    }
    if let Some((name, description, tags, owner, body)) = fts_snapshot(tx, snippet_id).await? {
        sqlx::query(
            "INSERT INTO snippets_fts (rowid, name, description, tags, owner, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(snippet_id)
        .bind(name)
        .bind(description)
        .bind(tags)
        .bind(owner)
        .bind(body)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn replace_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    snippet_id: i64,
    tags: &[String],
) -> RepoResult<()> {
    sqlx::query("DELETE FROM snippet_tags WHERE snippet_id = ?1")
        .bind(snippet_id)
        .execute(&mut **tx)
        .await?;
    for tag in tags {
        sqlx::query("INSERT INTO snippet_tags (snippet_id, tag) VALUES (?1, ?2)")
            .bind(snippet_id)
            .bind(tag)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

#[async_trait]
impl SnippetRepo for SqliteStore {
    #[allow(clippy::too_many_arguments)]
    async fn create_with_version(
        &self,
        owner_id: i64,
        name: &str,
        description: &str,
        visibility: &str,
        mode: &str,
        body: &str,
        hash: &str,
        message: Option<&str>,
        tags: &[String],
        now: &str,
    ) -> RepoResult<(SnippetRow, VersionRow)> {
        let mut tx = self.pool.begin().await?;
        let snippet = sqlx::query_as::<_, SnippetRow>(
            "INSERT INTO snippets (owner_id, name, description, visibility, latest_version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5) RETURNING *",
        )
        .bind(owner_id)
        .bind(name)
        .bind(description)
        .bind(visibility)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                RepoError::Conflict(format!("a snippet named `{name}` already exists"))
            } else {
                e.into()
            }
        })?;
        let version = sqlx::query_as::<_, VersionRow>(
            "INSERT INTO snippet_versions (snippet_id, version, mode, body, hash, message, created_at)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6) RETURNING *",
        )
        .bind(snippet.id)
        .bind(mode)
        .bind(body)
        .bind(hash)
        .bind(message)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        replace_tags(&mut tx, snippet.id, tags).await?;
        sync_fts(&mut tx, snippet.id, None).await?;
        tx.commit().await?;
        Ok((snippet, version))
    }

    async fn add_version(
        &self,
        snippet_id: i64,
        mode: &str,
        body: &str,
        hash: &str,
        message: Option<&str>,
        tags: &[String],
        now: &str,
        expected_latest: Option<i64>,
    ) -> RepoResult<Option<VersionRow>> {
        let mut tx = self.pool.begin().await?;
        let current: Option<i64> =
            sqlx::query_scalar("SELECT latest_version FROM snippets WHERE id = ?1")
                .bind(snippet_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(current) = current else {
            return Err(RepoError::NotFound("snippet".to_string()));
        };
        if expected_latest.is_some_and(|expected| expected != current) {
            // Lost a race against another writer; the transaction
            // is dropped uncommitted and the caller reports 412.
            return Ok(None);
        }
        let old_fts = fts_snapshot(&mut tx, snippet_id).await?;
        let next = current + 1;
        let version = sqlx::query_as::<_, VersionRow>(
            "INSERT INTO snippet_versions (snippet_id, version, mode, body, hash, message, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING *",
        )
        .bind(snippet_id)
        .bind(next)
        .bind(mode)
        .bind(body)
        .bind(hash)
        .bind(message)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE snippets SET latest_version = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(snippet_id)
            .bind(next)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        replace_tags(&mut tx, snippet_id, tags).await?;
        sync_fts(&mut tx, snippet_id, old_fts.as_ref()).await?;
        tx.commit().await?;
        Ok(Some(version))
    }

    async fn by_id(&self, id: i64) -> RepoResult<Option<SnippetRow>> {
        Ok(
            sqlx::query_as::<_, SnippetRow>("SELECT * FROM snippets WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn by_owner_name(&self, owner_id: i64, name: &str) -> RepoResult<Option<SnippetRow>> {
        Ok(sqlx::query_as::<_, SnippetRow>(
            "SELECT * FROM snippets WHERE owner_id = ?1 AND name = ?2",
        )
        .bind(owner_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn version(&self, snippet_id: i64, version: i64) -> RepoResult<Option<VersionRow>> {
        Ok(sqlx::query_as::<_, VersionRow>(
            "SELECT * FROM snippet_versions WHERE snippet_id = ?1 AND version = ?2",
        )
        .bind(snippet_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn latest_version(&self, snippet_id: i64) -> RepoResult<Option<VersionRow>> {
        Ok(sqlx::query_as::<_, VersionRow>(
            "SELECT v.* FROM snippet_versions v JOIN snippets s ON s.id = v.snippet_id
             WHERE v.snippet_id = ?1 AND v.version = s.latest_version",
        )
        .bind(snippet_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn versions(&self, snippet_id: i64) -> RepoResult<Vec<VersionRow>> {
        Ok(sqlx::query_as::<_, VersionRow>(
            "SELECT * FROM snippet_versions WHERE snippet_id = ?1 ORDER BY version DESC",
        )
        .bind(snippet_id)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn tags(&self, snippet_id: i64) -> RepoResult<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT tag FROM snippet_tags WHERE snippet_id = ?1 ORDER BY tag")
                .bind(snippet_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    async fn update_metadata(
        &self,
        snippet_id: i64,
        description: &str,
        visibility: &str,
        tags: &[String],
        now: &str,
    ) -> RepoResult<()> {
        let mut tx = self.pool.begin().await?;
        let old_fts = fts_snapshot(&mut tx, snippet_id).await?;
        let res = sqlx::query(
            "UPDATE snippets SET description = ?2, visibility = ?3, updated_at = ?4 WHERE id = ?1",
        )
        .bind(snippet_id)
        .bind(description)
        .bind(visibility)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() == 0 {
            return Err(RepoError::NotFound("snippet".to_string()));
        }
        sqlx::query("DELETE FROM snippet_tags WHERE snippet_id = ?1")
            .bind(snippet_id)
            .execute(&mut *tx)
            .await?;
        for tag in tags {
            sqlx::query("INSERT INTO snippet_tags (snippet_id, tag) VALUES (?1, ?2)")
                .bind(snippet_id)
                .bind(tag)
                .execute(&mut *tx)
                .await?;
        }
        sync_fts(&mut tx, snippet_id, old_fts.as_ref()).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn rename(
        &self,
        snippet_id: i64,
        owner_id: i64,
        old_name: &str,
        new_name: &str,
        expires_at: &str,
        now: &str,
    ) -> RepoResult<()> {
        let mut tx = self.pool.begin().await?;
        let old_fts = fts_snapshot(&mut tx, snippet_id).await?;
        let res = sqlx::query("UPDATE snippets SET name = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(snippet_id)
            .bind(new_name)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_unique)?;
        if res.rows_affected() == 0 {
            return Err(RepoError::NotFound("snippet".to_string()));
        }
        sqlx::query(
            "INSERT OR REPLACE INTO renames (owner_id, old_name, snippet_id, expires_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(owner_id)
        .bind(old_name)
        .bind(snippet_id)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        sync_fts(&mut tx, snippet_id, old_fts.as_ref()).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn resolve_rename(
        &self,
        owner_id: i64,
        old_name: &str,
        now: &str,
    ) -> RepoResult<Option<i64>> {
        Ok(sqlx::query_scalar(
            "SELECT snippet_id FROM renames WHERE owner_id = ?1 AND old_name = ?2 AND expires_at > ?3",
        )
        .bind(owner_id)
        .bind(old_name)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn count_for_owner(&self, owner_id: i64) -> RepoResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM snippets WHERE owner_id = ?1 AND deleted_at IS NULL",
        )
        .bind(owner_id)
        .fetch_one(&self.pool)
        .await?)
    }

    async fn list_for_owner(
        &self,
        owner_id: i64,
        visibility: Option<&str>,
        tag: Option<&str>,
        sort: &str,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>> {
        let mut qb =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM snippets WHERE owner_id = ");
        qb.push_bind(owner_id);
        qb.push(" AND deleted_at IS NULL");
        if let Some(visibility) = visibility {
            qb.push(" AND visibility = ");
            qb.push_bind(visibility.to_string());
        }
        if let Some(tag) = tag {
            qb.push(
                " AND EXISTS (SELECT 1 FROM snippet_tags \
                 WHERE snippet_id = snippets.id AND tag = ",
            );
            qb.push_bind(tag.to_string());
            qb.push(")");
        }
        match sort {
            "created" => qb.push(" ORDER BY created_at DESC, id DESC"),
            "name" => qb.push(" ORDER BY name ASC, id ASC"),
            _ => qb.push(" ORDER BY updated_at DESC, id DESC"),
        };
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
        Ok(qb
            .build_query_as::<SnippetRow>()
            .fetch_all(&self.pool)
            .await?)
    }

    async fn purge_deleted(&self, deleted_before: &str, now: &str) -> RepoResult<u64> {
        let mut total = 0u64;
        let res =
            sqlx::query("DELETE FROM snippets WHERE deleted_at IS NOT NULL AND deleted_at < ?1")
                .bind(deleted_before)
                .execute(&self.pool)
                .await?;
        total += res.rows_affected();
        let res = sqlx::query("DELETE FROM renames WHERE expires_at <= ?1")
            .bind(now)
            .execute(&self.pool)
            .await?;
        total += res.rows_affected();
        Ok(total)
    }

    async fn search_public(
        &self,
        match_query: Option<&str>,
        tag: Option<&str>,
        owner_id: Option<i64>,
        sort: &str,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("");
        if let Some(match_query) = match_query {
            qb.push(
                "SELECT s.* FROM snippets_fts f JOIN snippets s ON s.id = f.rowid \
                 WHERE f.snippets_fts MATCH ",
            );
            qb.push_bind(match_query.to_string());
            qb.push(" AND ");
        } else {
            qb.push("SELECT s.* FROM snippets s WHERE ");
        }
        qb.push("s.visibility = 'public' AND s.hidden_at IS NULL AND s.deleted_at IS NULL");
        if let Some(owner_id) = owner_id {
            qb.push(" AND s.owner_id = ");
            qb.push_bind(owner_id);
        }
        if let Some(tag) = tag {
            qb.push(
                " AND EXISTS (SELECT 1 FROM snippet_tags \
                 WHERE snippet_id = s.id AND tag = ",
            );
            qb.push_bind(tag.to_string());
            qb.push(")");
        }
        match sort {
            "stars" => qb.push(" ORDER BY s.star_count DESC, s.updated_at DESC, s.id DESC"),
            // Relevance only exists when there is a MATCH query.
            "relevance" if match_query.is_some() => qb.push(" ORDER BY bm25(snippets_fts)"),
            _ => qb.push(" ORDER BY s.updated_at DESC, s.id DESC"),
        };
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
        Ok(qb
            .build_query_as::<SnippetRow>()
            .fetch_all(&self.pool)
            .await?)
    }

    async fn popular_tags(&self, limit: i64) -> RepoResult<Vec<(String, i64)>> {
        Ok(sqlx::query_as::<_, (String, i64)>(
            "SELECT t.tag, COUNT(*) AS c FROM snippet_tags t \
             JOIN snippets s ON s.id = t.snippet_id \
             WHERE s.visibility = 'public' AND s.hidden_at IS NULL AND s.deleted_at IS NULL \
             GROUP BY t.tag ORDER BY c DESC, t.tag ASC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn explore_recent(&self, limit: i64, offset: i64) -> RepoResult<Vec<SnippetRow>> {
        Ok(sqlx::query_as::<_, SnippetRow>(
            "SELECT * FROM snippets WHERE visibility = 'public' AND hidden_at IS NULL \
             AND deleted_at IS NULL ORDER BY updated_at DESC, id DESC LIMIT ?1 OFFSET ?2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn explore_popular(
        &self,
        since: &str,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>> {
        Ok(sqlx::query_as::<_, SnippetRow>(
            "SELECT s.* FROM snippets s \
             LEFT JOIN stars st ON st.snippet_id = s.id AND st.created_at >= ?1 \
             WHERE s.visibility = 'public' AND s.hidden_at IS NULL AND s.deleted_at IS NULL \
             GROUP BY s.id ORDER BY COUNT(st.snippet_id) DESC, s.star_count DESC, s.updated_at DESC \
             LIMIT ?2 OFFSET ?3",
        )
        .bind(since)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn list_starred(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>> {
        Ok(sqlx::query_as::<_, SnippetRow>(
            "SELECT s.* FROM stars st JOIN snippets s ON s.id = st.snippet_id \
             WHERE st.user_id = ?1 AND s.deleted_at IS NULL AND s.hidden_at IS NULL \
             AND (s.visibility IN ('public', 'unlisted') OR s.owner_id = ?1) \
             ORDER BY st.created_at DESC, s.id DESC LIMIT ?2 OFFSET ?3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn list_public_for_owner(
        &self,
        owner_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepoResult<Vec<SnippetRow>> {
        Ok(sqlx::query_as::<_, SnippetRow>(
            "SELECT * FROM snippets WHERE owner_id = ?1 AND visibility = 'public' \
             AND hidden_at IS NULL AND deleted_at IS NULL \
             ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3",
        )
        .bind(owner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn count_public_for_owner(&self, owner_id: i64) -> RepoResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM snippets WHERE owner_id = ?1 AND visibility = 'public' \
             AND hidden_at IS NULL AND deleted_at IS NULL",
        )
        .bind(owner_id)
        .fetch_one(&self.pool)
        .await?)
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_fork(
        &self,
        owner_id: i64,
        name: &str,
        description: &str,
        mode: &str,
        body: &str,
        hash: &str,
        tags: &[String],
        source_id: i64,
        source_version: i64,
        source_hash: &str,
        now: &str,
    ) -> RepoResult<(SnippetRow, VersionRow)> {
        let mut tx = self.pool.begin().await?;
        let snippet = sqlx::query_as::<_, SnippetRow>(
            "INSERT INTO snippets (owner_id, name, description, visibility, latest_version, \
             forked_from_snippet_id, forked_from_version, forked_from_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'private', 1, ?4, ?5, ?6, ?7, ?7) RETURNING *",
        )
        .bind(owner_id)
        .bind(name)
        .bind(description)
        .bind(source_id)
        .bind(source_version)
        .bind(source_hash)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_unique)?;
        let version = sqlx::query_as::<_, VersionRow>(
            "INSERT INTO snippet_versions (snippet_id, version, mode, body, hash, message, created_at) \
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6) RETURNING *",
        )
        .bind(snippet.id)
        .bind(mode)
        .bind(body)
        .bind(hash)
        .bind(format!("Forked from snippet {source_id} v{source_version}"))
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        replace_tags(&mut tx, snippet.id, tags).await?;
        sqlx::query("UPDATE snippets SET fork_count = fork_count + 1 WHERE id = ?1")
            .bind(source_id)
            .execute(&mut *tx)
            .await?;
        // Forks are private: the FTS snapshot rule keeps them out.
        tx.commit().await?;
        Ok((snippet, version))
    }

    async fn increment_pull(&self, snippet_id: i64) -> RepoResult<()> {
        sqlx::query("UPDATE snippets SET pull_count = pull_count + 1 WHERE id = ?1")
            .bind(snippet_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_hidden(&self, snippet_id: i64, hidden: bool, now: &str) -> RepoResult<()> {
        let mut tx = self.pool.begin().await?;
        let old_fts = fts_snapshot(&mut tx, snippet_id).await?;
        if hidden {
            sqlx::query("UPDATE snippets SET hidden_at = ?2, updated_at = ?2 WHERE id = ?1")
                .bind(snippet_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE snippets SET hidden_at = NULL, updated_at = ?2 WHERE id = ?1")
                .bind(snippet_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        }
        sync_fts(&mut tx, snippet_id, old_fts.as_ref()).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn soft_delete(&self, snippet_id: i64, now: &str) -> RepoResult<()> {
        let mut tx = self.pool.begin().await?;
        let old_fts = fts_snapshot(&mut tx, snippet_id).await?;
        sqlx::query("UPDATE snippets SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1")
            .bind(snippet_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sync_fts(&mut tx, snippet_id, old_fts.as_ref()).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn star(&self, user_id: i64, snippet_id: i64, now: &str) -> RepoResult<()> {
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query(
            "INSERT OR IGNORE INTO stars (user_id, snippet_id, created_at) VALUES (?1, ?2, ?3)",
        )
        .bind(user_id)
        .bind(snippet_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() > 0 {
            sqlx::query("UPDATE snippets SET star_count = star_count + 1 WHERE id = ?1")
                .bind(snippet_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn unstar(&self, user_id: i64, snippet_id: i64) -> RepoResult<()> {
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query("DELETE FROM stars WHERE user_id = ?1 AND snippet_id = ?2")
            .bind(user_id)
            .bind(snippet_id)
            .execute(&mut *tx)
            .await?;
        if res.rows_affected() > 0 {
            sqlx::query("UPDATE snippets SET star_count = MAX(star_count - 1, 0) WHERE id = ?1")
                .bind(snippet_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn has_starred(&self, user_id: i64, snippet_id: i64) -> RepoResult<bool> {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM stars WHERE user_id = ?1 AND snippet_id = ?2")
                .bind(user_id)
                .bind(snippet_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(n > 0)
    }
}

// ---------------------------------------------------------------- reports

#[async_trait]
impl ReportRepo for SqliteStore {
    async fn create(
        &self,
        snippet_id: i64,
        reporter_id: i64,
        reason: &str,
        now: &str,
    ) -> RepoResult<i64> {
        let res = sqlx::query(
            "INSERT INTO reports (snippet_id, reporter_id, reason, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(snippet_id)
        .bind(reporter_id)
        .bind(reason)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(res.last_insert_rowid())
    }

    async fn count_by_status(&self, status: &str) -> RepoResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM reports WHERE status = ?1")
                .bind(status)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    async fn list(&self, status: Option<&str>, limit: i64) -> RepoResult<Vec<ReportRow>> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM reports");
        if let Some(status) = status {
            qb.push(" WHERE status = ");
            qb.push_bind(status.to_string());
        }
        qb.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        qb.push_bind(limit);
        Ok(qb
            .build_query_as::<ReportRow>()
            .fetch_all(&self.pool)
            .await?)
    }

    async fn exists_open(&self, reporter_id: i64, snippet_id: i64) -> RepoResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reports WHERE reporter_id = ?1 AND snippet_id = ?2 AND status = 'open'",
        )
        .bind(reporter_id)
        .bind(snippet_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repos() -> (Repos, SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("r.db").display());
        let pool = crate::db::connect(&url).await.unwrap();
        crate::db::migrate(&pool).await.unwrap();
        let repos = Repos::sqlite(pool.clone());
        (repos, pool, dir)
    }

    const NOW: &str = "2026-11-19T00:00:00Z";

    #[tokio::test]
    async fn users_roundtrip_and_conflicts() {
        let (repos, _, _dir) = repos().await;
        let u = repos
            .users
            .create("alice", "alice@example.com", "hash", NOW)
            .await
            .unwrap();
        assert_eq!(u.username, "alice");
        assert_eq!(
            repos.users.by_id(u.id).await.unwrap().unwrap().email,
            "alice@example.com"
        );
        assert!(repos.users.by_username("ALICE").await.unwrap().is_some());
        let dup = repos
            .users
            .create("alice", "other@example.com", "hash", NOW)
            .await;
        assert!(matches!(dup, Err(RepoError::Conflict(_))), "{dup:?}");
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // one long end-to-end repository scenario
    async fn snippets_versions_tags_and_fts() {
        let (repos, pool, _dir) = repos().await;
        let u = repos
            .users
            .create("bob", "bob@example.com", "h", NOW)
            .await
            .unwrap();
        let (snip, v1) = repos
            .snippets
            .create_with_version(
                u.id,
                "ubuntu-setup",
                "Set up Ubuntu",
                "public",
                "steps",
                "name: ubuntu-setup\nsteps: [echo hi]\n",
                "abc123",
                None,
                &["ci".to_string()],
                NOW,
            )
            .await
            .unwrap();
        assert_eq!(v1.version, 1);
        assert_eq!(snip.latest_version, 1);
        // Public + latest => in FTS.
        let hits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM snippets_fts WHERE snippets_fts MATCH '\"ubuntu\"'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 1);
        // New version: FTS follows the latest body.
        let v2 = repos
            .snippets
            .add_version(
                snip.id,
                "steps",
                "name: ubuntu-setup\nsteps: [echo bye]\n",
                "def456",
                Some("v2"),
                &["ci".to_string(), "web".to_string()],
                NOW,
                None,
            )
            .await
            .unwrap()
            .unwrap();
        // A checked append against a stale expectation writes
        // nothing and reports None (optimistic concurrency).
        let raced = repos
            .snippets
            .add_version(
                snip.id,
                "steps",
                "name: ubuntu-setup\nsteps: [echo race]\n",
                "feed99",
                None,
                &[],
                NOW,
                Some(1),
            )
            .await
            .unwrap();
        assert!(raced.is_none(), "stale If-Match must not append");
        assert_eq!(v2.version, 2);
        // The contentless FTS row now indexes v2's body only: the
        // old body's distinctive term must no longer match.
        let hits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM snippets_fts WHERE snippets_fts MATCH '\"bye\"'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 1);
        let hits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM snippets_fts WHERE snippets_fts MATCH '\"hi\"'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 0, "stale v1 body still indexed");
        assert_eq!(
            repos.snippets.tags(snip.id).await.unwrap(),
            vec!["ci", "web"]
        );
        assert_eq!(repos.snippets.versions(snip.id).await.unwrap().len(), 2);
        // Going private removes the FTS row.
        repos
            .snippets
            .update_metadata(snip.id, "Set up Ubuntu", "private", &[], NOW)
            .await
            .unwrap();
        let hits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snippets_fts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hits, 0);
        // Duplicate name for the same owner conflicts.
        let dup = repos
            .snippets
            .create_with_version(
                u.id,
                "ubuntu-setup",
                "",
                "private",
                "steps",
                "x",
                "h",
                None,
                &[],
                NOW,
            )
            .await;
        assert!(matches!(dup, Err(RepoError::Conflict(_))), "{dup:?}");
    }

    #[tokio::test]
    async fn stars_maintain_the_counter() {
        let (repos, _, _dir) = repos().await;
        let owner = repos.users.create("o", "o@e.com", "h", NOW).await.unwrap();
        let fan = repos.users.create("f", "f@e.com", "h", NOW).await.unwrap();
        let (snip, _) = repos
            .snippets
            .create_with_version(
                owner.id,
                "s",
                "",
                "public",
                "steps",
                "b",
                "h",
                None,
                &[],
                NOW,
            )
            .await
            .unwrap();
        repos.snippets.star(fan.id, snip.id, NOW).await.unwrap();
        repos.snippets.star(fan.id, snip.id, NOW).await.unwrap(); // idempotent
        assert_eq!(
            repos
                .snippets
                .by_id(snip.id)
                .await
                .unwrap()
                .unwrap()
                .star_count,
            1
        );
        assert!(repos.snippets.has_starred(fan.id, snip.id).await.unwrap());
        repos.snippets.unstar(fan.id, snip.id).await.unwrap();
        assert_eq!(
            repos
                .snippets
                .by_id(snip.id)
                .await
                .unwrap()
                .unwrap()
                .star_count,
            0
        );
    }

    #[tokio::test]
    async fn sessions_tokens_device_codes_roundtrip() {
        let (repos, _, _dir) = repos().await;
        let u = repos
            .users
            .create("carol", "c@e.com", "h", NOW)
            .await
            .unwrap();
        let session = SessionRow {
            id_hash: "hash1".into(),
            user_id: u.id,
            csrf_token: "csrf".into(),
            created_at: NOW.into(),
            last_used_at: NOW.into(),
            expires_at: "2026-12-19T00:00:00Z".into(),
            ip_hint: None,
            ua_hint: None,
        };
        repos.sessions.create(&session).await.unwrap();
        assert!(repos.sessions.by_id_hash("hash1").await.unwrap().is_some());
        assert_eq!(repos.sessions.delete_for_user(u.id).await.unwrap(), 1);

        let token = repos
            .tokens
            .create(
                u.id,
                "cli",
                "abcd1234",
                "hash-of-token",
                "read write",
                None,
                NOW,
            )
            .await
            .unwrap();
        assert_eq!(
            repos
                .tokens
                .by_hash("hash-of-token")
                .await
                .unwrap()
                .unwrap()
                .id,
            token.id
        );
        repos.tokens.revoke(token.id, NOW).await.unwrap();
        assert!(repos
            .tokens
            .by_hash("hash-of-token")
            .await
            .unwrap()
            .unwrap()
            .revoked_at
            .is_some());

        let dc = repos
            .device_codes
            .create("dhash", "ABCD2345", "cli", "2026-11-19T00:10:00Z", NOW)
            .await
            .unwrap();
        repos.device_codes.record_poll(dc.id, NOW).await.unwrap();
        repos
            .device_codes
            .set_status(dc.id, "approved", Some(u.id), Some(token.id))
            .await
            .unwrap();
        let dc = repos
            .device_codes
            .by_user_code("ABCD2345")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(dc.status, "approved");
        assert_eq!(dc.user_id, Some(u.id));
    }
}
