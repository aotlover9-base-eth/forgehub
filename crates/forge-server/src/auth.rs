//! Account authentication: signup, login, logout, sessions, CSRF,
//! and the `/me` endpoints (Step 7; openapi auth group, ADR 0005,
//! threat model T-ACCT-1/2).
//!
//! Passwords use argon2id with parameters m = 19456 KiB, t = 3,
//! p = 1, measured at ~50 ms per hash on the dev machine (release
//! build; debug builds are several times slower); the parameters
//! travel inside the PHC string, so they can be raised later
//! without invalidating stored hashes.
//! Passwords are never logged and never returned.
//!
//! Sessions: a 256-bit random id in an `HttpOnly`, `SameSite=Lax`
//! cookie (`Secure` in prod); only its SHA-256 is stored. Idle and
//! absolute expiry come from config. Mutating cookie-authenticated
//! requests need the per-session `X-CSRF-Token` header (returned by
//! `GET /me`) plus a matching `Origin` when the browser sends one.

use crate::config::Env;
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::{SessionRow, UserRow};
use crate::state::AppState;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequestParts, Json, State};
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use rand_core::{OsRng, RngCore};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ------------------------------------------------------------ passwords

/// Argon2id parameters (see module docs).
fn argon2() -> Argon2<'static> {
    let params = Params::new(19_456, 3, 1, None).unwrap_or_default();
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a password into a PHC string (blocking; ~50–100 ms).
fn hash_password_blocking(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(OsRng);
    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| ApiError::internal())
}

/// Verify a password against a stored PHC string (blocking).
fn verify_password_blocking(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    argon2()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

async fn hash_password(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || hash_password_blocking(&password))
        .await
        .map_err(|_| ApiError::internal())?
}

async fn verify_password(password: String, phc: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password_blocking(&password, &phc))
        .await
        .unwrap_or(false)
}

/// A real argon2id hash of a throwaway password, computed once, so
/// login attempts for nonexistent users cost the same verification
/// time as real ones (no timing oracle for account existence).
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password_blocking("dummy-password-for-timing-equalisation")
            .unwrap_or_else(|_| String::new())
    })
}

// -------------------------------------------------------------- random

/// `n` CSPRNG bytes, hex-encoded (2n chars).
pub(crate) fn random_hex(n: usize) -> String {
    use std::fmt::Write;
    let mut buf = vec![0u8; n];
    OsRng.fill_bytes(&mut buf);
    buf.iter().fold(String::with_capacity(n * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// `n` characters drawn uniformly from `alphabet` with a CSPRNG
/// (rejection sampling, so no modulo bias).
pub(crate) fn random_from_alphabet(alphabet: &[u8], n: usize) -> String {
    let limit = 256 - (256 % alphabet.len());
    let mut out = String::with_capacity(n);
    let mut buf = [0u8; 64];
    while out.len() < n {
        OsRng.fill_bytes(&mut buf);
        for b in buf {
            if (b as usize) < limit {
                out.push(alphabet[(b as usize) % alphabet.len()] as char);
                if out.len() == n {
                    break;
                }
            }
        }
    }
    out
}

/// SHA-256 hex of a string (session ids and, later, tokens).
#[must_use]
pub fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Constant-time byte comparison for secrets (CSRF tokens).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ---------------------------------------------------------- validation

/// The small common-password denylist (checked case-insensitively,
/// exact match). Deliberately short: it blocks the passwords that
/// credential-stuffing lists try first; length and throttling do
/// the rest.
const COMMON_PASSWORDS: &[&str] = &[
    "password",
    "password1",
    "password123",
    "123456",
    "12345678",
    "123456789",
    "1234567890",
    "12345",
    "123123",
    "111111",
    "000000",
    "qwerty",
    "qwerty123",
    "qwertyuiop",
    "abc123",
    "letmein",
    "letmein123",
    "admin",
    "admin123",
    "administrator",
    "welcome",
    "welcome1",
    "welcome123",
    "login",
    "passw0rd",
    "p@ssw0rd",
    "p@ssword",
    "changeme",
    "secret",
    "secret123",
    "sunshine",
    "princess",
    "football",
    "baseball",
    "basketball",
    "monkey",
    "dragon",
    "shadow",
    "master",
    "superman",
    "batman",
    "spiderman",
    "iloveyou",
    "trustno1",
    "starwars",
    "pokemon",
    "naruto",
    "computer",
    "internet",
    "server",
    "linux",
    "windows",
    "ubuntu",
    "debian",
    "root123",
    "toor",
    "default",
    "guest",
    "user123",
    "test123",
    "demo123",
    "forgehub",
    "forgehub123",
    "forgenesis",
    "hunter2",
    "hunter22",
    "correct",
    "staple",
    "battery",
    "orange",
    "banana",
    "purple",
    "summer",
    "winter",
    "spring",
    "autumn",
    "january",
    "february",
    "december",
    "charlie",
    "thomas",
    "jordan",
    "michael",
    "jennifer",
    "michelle",
    "samantha",
    "andrew",
    "joshua",
    "daniel",
    "matthew",
    "anthony",
    "kevin",
    "jason",
    "jeffrey",
    "ryan",
    "jacob",
    "nicholas",
    "eric",
    "jonathan",
    "stephen",
    "larry",
    "justin",
    "scott",
    "frank",
    "brandon",
    "steven",
    "benjamin",
    "samuel",
    "gregory",
    "alexander",
    "patrick",
    "jack",
    "dennis",
    "jerry",
    "tyler",
    "aaron",
    "jose",
    "nathan",
    "henry",
    "douglas",
    "zachary",
    "kyle",
    "noah",
    "ethan",
    "jeremy",
    "walter",
    "christian",
    "keith",
    "roger",
    "terry",
    "austin",
    "sean",
    "gerald",
    "carl",
    "harold",
    "jordan23",
    "password1234",
    "123qwe",
    "qwe123",
    "qweasd",
    "zxcvbn",
    "zxcvbnm",
    "asdfgh",
    "asdf1234",
    "1q2w3e",
    "1q2w3e4r",
    "q1w2e3r4",
];

/// Validate a password for signup / password change.
///
/// # Errors
/// A user-safe message naming the failed rule.
pub fn validate_password(password: &str, username: &str) -> Result<(), String> {
    if password.chars().count() < 10 {
        return Err("Password must be at least 10 characters".to_string());
    }
    if password.len() > 256 {
        return Err("Password must be at most 256 characters".to_string());
    }
    if password.eq_ignore_ascii_case(username) {
        return Err("Password must not be the same as your username".to_string());
    }
    if COMMON_PASSWORDS
        .iter()
        .any(|p| p.eq_ignore_ascii_case(password))
    {
        return Err("That password is too common; choose a less predictable one".to_string());
    }
    Ok(())
}

/// Validate an email address's format (not deliverability): one
/// `@`, sane local part, dotted domain with valid labels, ≤ 254
/// chars overall.
///
/// # Errors
/// A user-safe message.
pub fn validate_email(email: &str) -> Result<(), String> {
    let err = || "Enter a valid email address".to_string();
    if email.is_empty() || email.len() > 254 || email.chars().any(char::is_control) {
        return Err(err());
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(err());
    };
    if domain.contains('@') || local.is_empty() || local.len() > 64 {
        return Err(err());
    }
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return Err(err());
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c))
    {
        return Err(err());
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return Err(err());
    }
    for label in &labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(err());
        }
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(err());
    }
    Ok(())
}

// -------------------------------------------------------- login throttle

/// Failures tolerated before lockout starts.
const MAX_FAILURES: u32 = 5;
/// Backoff cap.
const MAX_BACKOFF: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct FailState {
    failures: u32,
    locked_until: Option<Instant>,
}

/// Per-key login throttling with exponential backoff (T-ACCT-1).
/// Keys are `ip:<addr>` and `acct:<lowercased identifier>`; a lock
/// on either blocks the attempt.
#[derive(Debug)]
pub struct LoginThrottle {
    inner: Mutex<HashMap<String, FailState>>,
    base: Duration,
}

impl LoginThrottle {
    /// New throttle with the given backoff base (config).
    #[must_use]
    pub fn new(base: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            base,
        }
    }

    /// If `key` is currently locked, how long remains.
    #[must_use]
    pub fn locked_for(&self, key: &str) -> Option<Duration> {
        let map = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = map.get(key)?;
        state
            .locked_until
            .and_then(|until| until.checked_duration_since(Instant::now()))
    }

    /// Record a failed attempt; starts/extends the lock once the
    /// failure budget is exhausted. Backoff doubles per extra
    /// failure: base, 2·base, 4·base, … capped at 300 s.
    pub fn record_failure(&self, key: &str) {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() > 10_000 {
            map.retain(|_, s| s.locked_until.is_some_and(|u| u > Instant::now()) || s.failures > 0);
        }
        let state = map.entry(key.to_string()).or_insert(FailState {
            failures: 0,
            locked_until: None,
        });
        state.failures = state.failures.saturating_add(1);
        if state.failures >= MAX_FAILURES {
            let over = state.failures - MAX_FAILURES;
            let factor = 2u32.saturating_pow(over.min(10));
            let delay = self.base.saturating_mul(factor).min(MAX_BACKOFF);
            state.locked_until = Some(Instant::now() + delay);
        }
    }

    /// Clear a key after a successful login.
    pub fn record_success(&self, key: &str) {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.remove(key);
    }
}

// -------------------------------------------------------------- sessions

/// The session cookie name (openapi `cookieSession`).
pub const SESSION_COOKIE: &str = "forgehub_session";

/// Format a timestamp in the storage format (UTC ISO-8601).
/// Public because the server binary's retention job uses it.
#[must_use]
pub fn fmt_ts(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub(crate) fn parse_iso(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&chrono::Utc))
}

/// Extract one cookie's value from the `Cookie` header.
fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            if k == name {
                return Some(v);
            }
        }
    }
    None
}

fn session_cookie(state: &AppState, id: &str) -> String {
    let secure = if state.config.env == Env::Prod {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{SESSION_COOKIE}={id}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}{secure}",
        state.config.session_absolute_secs
    )
}

fn clear_cookie(state: &AppState) -> String {
    let secure = if state.config.env == Env::Prod {
        "; Secure"
    } else {
        ""
    };
    format!("{SESSION_COOKIE}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0{secure}")
}

fn attach_cookie(resp: &mut Response, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        resp.headers_mut().append(header::SET_COOKIE, value);
    }
}

/// Create a session row for `user_id` and return it with the
/// `Set-Cookie` value. The plaintext id never touches the database.
async fn start_session(
    state: &AppState,
    user_id: i64,
    ip: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Result<(SessionRow, String), ApiError> {
    let id = random_hex(32);
    let now = chrono::Utc::now();
    let row = SessionRow {
        id_hash: sha256_hex(&id),
        user_id,
        csrf_token: random_hex(32),
        created_at: fmt_ts(now),
        last_used_at: fmt_ts(now),
        expires_at: fmt_ts(
            now + chrono::Duration::seconds(
                i64::try_from(state.config.session_absolute_secs).unwrap_or(2_592_000),
            ),
        ),
        ip_hint: ip.map(|a| a.ip().to_string()),
        ua_hint: headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|ua| ua.chars().take(120).collect()),
    };
    state.repos.sessions.create(&row).await?;
    Ok((row, session_cookie(state, &id)))
}

/// How the caller authenticated (ADR 0005): a browser cookie
/// session, or a bearer API token (Step 8).
#[derive(Debug, Clone)]
pub enum AuthKind {
    /// Cookie session.
    Session(SessionRow),
    /// Bearer API token.
    Token(crate::repo::TokenRow),
}

impl AuthKind {
    /// The session row, when cookie-authenticated.
    #[must_use]
    pub fn session(&self) -> Option<&SessionRow> {
        match self {
            Self::Session(row) => Some(row),
            Self::Token(_) => None,
        }
    }

    /// Scope check: sessions act with the user's full authority;
    /// tokens are limited to their scopes (`write` implies `read`,
    /// normalised at creation).
    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        match self {
            Self::Session(_) => true,
            Self::Token(row) => row.scopes.split_whitespace().any(|s| s == scope),
        }
    }
}

/// The authenticated principal.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// The authenticated user.
    pub user: UserRow,
    /// Session or token.
    pub kind: AuthKind,
}

/// Resolve the session cookie, enforcing absolute + idle expiry
/// and account state. Expired sessions are deleted on sight;
/// `last_used_at` is touched at most once a minute.
async fn resolve_session(state: &AppState, headers: &HeaderMap) -> Option<AuthContext> {
    let raw = cookie_value(headers, SESSION_COOKIE)?;
    let id_hash = sha256_hex(raw);
    let session = state.repos.sessions.by_id_hash(&id_hash).await.ok()??;
    let now = chrono::Utc::now();
    let expired = parse_iso(&session.expires_at).is_none_or(|e| e <= now);
    let idle_limit =
        chrono::Duration::seconds(i64::try_from(state.config.session_idle_secs).unwrap_or(604_800));
    let idle = parse_iso(&session.last_used_at).is_none_or(|last| last + idle_limit <= now);
    if expired || idle {
        let _ = state.repos.sessions.delete(&id_hash).await;
        return None;
    }
    let user = state.repos.users.by_id(session.user_id).await.ok()??;
    if user.deleted_at.is_some() || user.disabled_at.is_some() {
        return None;
    }
    if parse_iso(&session.last_used_at)
        .is_some_and(|last| now - last > chrono::Duration::seconds(60))
    {
        let _ = state.repos.sessions.touch(&id_hash, &now_iso()).await;
    }
    Some(AuthContext {
        user,
        kind: AuthKind::Session(session),
    })
}

/// Resolve an `Authorization: Bearer fgh_…` header: hash the
/// presented token, look it up, enforce revocation/expiry and
/// account state, and touch `last_used_at` at most once a minute.
/// A revoked token is rejected on the very next request — there is
/// no cache.
async fn resolve_token(state: &AppState, headers: &HeaderMap) -> Option<AuthContext> {
    let header = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let presented = header.strip_prefix("Bearer ")?;
    if !presented.starts_with("fgh_") {
        return None;
    }
    let token = state
        .repos
        .tokens
        .by_hash(&sha256_hex(presented))
        .await
        .ok()??;
    if token.revoked_at.is_some() {
        return None;
    }
    let now = chrono::Utc::now();
    if token
        .expires_at
        .as_deref()
        .and_then(parse_iso)
        .is_some_and(|e| e <= now)
    {
        return None;
    }
    let user = state.repos.users.by_id(token.user_id).await.ok()??;
    if user.deleted_at.is_some() || user.disabled_at.is_some() {
        return None;
    }
    let use_is_stale = token
        .last_used_at
        .as_deref()
        .and_then(parse_iso)
        .is_none_or(|last| now - last > chrono::Duration::seconds(60));
    if use_is_stale {
        let _ = state.repos.tokens.touch(token.id, &now_iso()).await;
    }
    Some(AuthContext {
        user,
        kind: AuthKind::Token(token),
    })
}

/// Resolve whichever credential the request carries. A bearer
/// header, when present, wins over the cookie (explicit beats
/// ambient).
async fn resolve_auth(state: &AppState, headers: &HeaderMap) -> Option<AuthContext> {
    if headers.contains_key(header::AUTHORIZATION) {
        return resolve_token(state, headers).await;
    }
    resolve_session(state, headers).await
}

/// Verify the CSRF defence for a cookie-authenticated mutation:
/// `Origin` (when sent) must match `Host`, and `X-CSRF-Token` must
/// equal the session's token (constant-time compare).
///
/// # Errors
/// 403 `csrf_failed`.
pub fn verify_csrf(headers: &HeaderMap, session: &SessionRow) -> Result<(), ApiError> {
    let fail = || ApiError::new(StatusCode::FORBIDDEN, "csrf_failed", "CSRF check failed");
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let authority = origin
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("");
        if authority.is_empty() || authority != host {
            return Err(fail());
        }
    }
    let presented = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !ct_eq(presented.as_bytes(), session.csrf_token.as_bytes()) {
        return Err(fail());
    }
    Ok(())
}

// ------------------------------------------------------------ extractors

/// Optional authentication: never rejects. Handlers that render
/// differently for signed-in users use this.
pub struct CurrentUser(pub Option<AuthContext>);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(resolve_auth(state, &parts.headers).await))
    }
}

/// Required authentication (cookie session or bearer token).
/// Rejects 401 `unauthorized`.
pub struct RequireUser(pub AuthContext);

impl FromRequestParts<AppState> for RequireUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        resolve_auth(state, &parts.headers)
            .await
            .map(RequireUser)
            .ok_or_else(|| ApiError::unauthorized("Authentication required"))
    }
}

/// Required *cookie-session* authentication: account mutations
/// (logout, password change, account deletion, token management)
/// are never available to bearer tokens (ADR 0005, T-ACCT-3).
/// Bearer-authenticated requests get 403 `session_required`;
/// unauthenticated ones 401.
pub struct RequireSession {
    /// The authenticated user.
    pub user: UserRow,
    /// The session behind the cookie.
    pub session: SessionRow,
}

impl FromRequestParts<AppState> for RequireSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve_auth(state, &parts.headers).await {
            Some(AuthContext {
                user,
                kind: AuthKind::Session(session),
            }) => Ok(Self { user, session }),
            Some(_) => Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "session_required",
                "This operation requires a web session, not an API token",
            )),
            None => Err(ApiError::unauthorized("Authentication required")),
        }
    }
}

// -------------------------------------------------------------- handlers

pub(crate) fn body<T>(result: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    result.map(|Json(v)| v).map_err(|e| {
        // Step 19: a body that exceeds the request size limit
        // is 413 Payload Too Large (code body_too_large, the
        // same shape the snippet endpoints use), not a 400
        // validation error.
        if matches!(e, JsonRejection::BytesRejection(_)) {
            ApiError::new(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
                "Request body exceeds the size limit",
            )
        } else {
            ApiError::validation(format!("Invalid request body: {}", e.body_text()))
        }
    })
}

fn me_json(user: &UserRow, session: Option<&SessionRow>) -> serde_json::Value {
    let mut me = serde_json::json!({
        "username": user.username,
        "email": user.email,
        "display_name": user.display_name,
        "bio": user.bio,
        "created_at": user.created_at,
    });
    if let Some(session) = session {
        me["csrf_token"] = serde_json::Value::String(session.csrf_token.clone());
    }
    me
}

/// The client's socket address, read from the `ConnectInfo`
/// extension that `axum::serve … with_connect_info` installs
/// (`ConnectInfo` is not a handler extractor in axum 0.8, so this
/// wraps the extension lookup).
pub struct ClientIp(pub Option<SocketAddr>);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    // The trait method is async by contract; this impl only reads
    // request extensions and never awaits.
    #[allow(clippy::unused_async_trait_impl)]
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<SocketAddr>>()
                .map(|c| c.0),
        ))
    }
}

#[derive(Deserialize)]
struct SignupRequest {
    username: String,
    email: String,
    password: String,
}

async fn signup(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    result: Result<Json<SignupRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    if !state.config.allow_signup {
        return Err(ApiError::forbidden("Signups are disabled on this server"));
    }
    let req = body(result)?;
    // Usernames are names, not tags: mixed case is rejected, never
    // folded (Step 2 consistency rule; uniqueness is still
    // case-insensitive in the repository).
    let username = req.username.trim().to_string();
    forge_core::names::validate_username(&username)
        .map_err(|e| ApiError::validation(e.to_string()))?;
    let email = req.email.trim().to_lowercase();
    validate_email(&email).map_err(ApiError::validation)?;
    validate_password(&req.password, &username).map_err(ApiError::validation)?;
    if state.repos.users.by_username(&username).await?.is_some() {
        return Err(ApiError::conflict(
            "username_taken",
            "That username is taken",
        ));
    }
    if state.repos.users.by_email(&email).await?.is_some() {
        // Deliberately generic: usernames are public, emails are
        // not, so an email conflict must not confirm the address
        // is registered (documented in the report).
        return Err(ApiError::conflict(
            "signup_conflict",
            "An account with these details cannot be created",
        ));
    }
    let hash = hash_password(req.password.clone()).await?;
    let user = match state
        .repos
        .users
        .create(&username, &email, &hash, &now_iso())
        .await
    {
        Ok(user) => user,
        Err(crate::repo::RepoError::Conflict(_)) => {
            return Err(ApiError::conflict(
                "signup_conflict",
                "An account with these details cannot be created",
            ));
        }
        Err(e) => return Err(e.into()),
    };
    let (session, cookie) = start_session(&state, user.id, ip, &headers).await?;
    let mut resp = (StatusCode::CREATED, Json(me_json(&user, Some(&session)))).into_response();
    attach_cookie(&mut resp, &cookie);
    Ok(resp)
}

#[derive(Deserialize)]
struct LoginRequest {
    identifier: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    result: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    let identifier = req.identifier.trim().to_lowercase();
    let ip_key = format!(
        "ip:{}",
        ip.map_or_else(|| "unknown".into(), |a| a.ip().to_string())
    );
    let acct_key = format!("acct:{identifier}");
    for key in [&ip_key, &acct_key] {
        if let Some(remaining) = state.login_throttle.locked_for(key) {
            let mut resp = ApiError::rate_limited().into_response();
            let secs = remaining.as_secs().max(1);
            if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
                resp.headers_mut().insert("retry-after", value);
            }
            return Ok(resp);
        }
    }
    let invalid = || ApiError::unauthorized("Invalid credentials");
    if identifier.len() > 254 || req.password.len() > 256 || identifier.is_empty() {
        return Err(invalid());
    }
    let user = if identifier.contains('@') {
        state.repos.users.by_email(&identifier).await?
    } else {
        state.repos.users.by_username(&identifier).await?
    };
    let active_user = user.filter(|u| u.deleted_at.is_none() && u.disabled_at.is_none());
    let verified = if let Some(candidate) = &active_user {
        verify_password(req.password.clone(), candidate.password_hash.clone()).await
    } else {
        // Equalise timing: verify against the dummy hash.
        verify_password(req.password.clone(), dummy_hash().to_string()).await
    };
    if !verified {
        state.login_throttle.record_failure(&ip_key);
        state.login_throttle.record_failure(&acct_key);
        return Err(invalid());
    }
    let user = active_user.ok_or_else(invalid)?;
    state.login_throttle.record_success(&ip_key);
    state.login_throttle.record_success(&acct_key);
    // Session rotation: any session the request carried dies here.
    if let Some(raw) = cookie_value(&headers, SESSION_COOKIE) {
        let _ = state.repos.sessions.delete(&sha256_hex(raw)).await;
    }
    let (session, cookie) = start_session(&state, user.id, ip, &headers).await?;
    let mut resp = (StatusCode::OK, Json(me_json(&user, Some(&session)))).into_response();
    attach_cookie(&mut resp, &cookie);
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Password reset (2026-11-20 increment): emailed single-use
// tokens. Only hashes are stored; the request endpoint's
// answer never reveals whether an account exists.
// ---------------------------------------------------------------------------

/// Reset links live for one hour.
const RESET_TOKEN_TTL_SECS: i64 = 3600;
/// URL-safe alphabet for reset tokens (they travel in a query
/// string): 48 chars ≈ 285 bits of entropy.
const RESET_ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_";

#[derive(Deserialize)]
struct ResetRequestBody {
    email: String,
}

async fn password_reset_request(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    result: Result<Json<ResetRequestBody>, JsonRejection>,
) -> Response {
    // Always 204: malformed body, unknown address, throttled,
    // mail down — the response is identical either way, so it
    // cannot confirm that an account exists (the same rule the
    // signup handler applies to email conflicts).
    let Ok(Json(req)) = result else {
        return StatusCode::NO_CONTENT.into_response();
    };
    let email = req.email.trim().to_lowercase();
    // Throttled with the login throttle's budget (5 strikes,
    // exponential backoff), keyed by source IP and by target
    // address so one inbox cannot be bombed from many IPs.
    // A locked key skips the work silently.
    let ip_key = format!(
        "reset-ip:{}",
        ip.map_or_else(|| "unknown".into(), |a| a.ip().to_string())
    );
    let mail_key = format!("reset-mail:{email}");
    if state.login_throttle.locked_for(&ip_key).is_some()
        || state.login_throttle.locked_for(&mail_key).is_some()
    {
        return StatusCode::NO_CONTENT.into_response();
    }
    state.login_throttle.record_failure(&ip_key);
    state.login_throttle.record_failure(&mail_key);
    if validate_email(&email).is_err() {
        return StatusCode::NO_CONTENT.into_response();
    }
    let user = match state.repos.users.by_email(&email).await {
        Ok(found) => found.filter(|u| u.deleted_at.is_none() && u.disabled_at.is_none()),
        Err(err) => {
            tracing::error!(error = %err, "password reset: user lookup failed");
            return StatusCode::NO_CONTENT.into_response();
        }
    };
    let Some(user) = user else {
        return StatusCode::NO_CONTENT.into_response();
    };
    let token = random_from_alphabet(RESET_ALPHABET, 48);
    let now = chrono::Utc::now();
    let created = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let expires = (now + chrono::Duration::seconds(RESET_TOKEN_TTL_SECS))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    if let Err(err) = state
        .repos
        .password_resets
        .issue(user.id, &sha256_hex(&token), &created, &expires)
        .await
    {
        tracing::error!(error = %err, "password reset: issuing a token failed");
        return StatusCode::NO_CONTENT.into_response();
    }
    let reset_url = format!("{}/reset-password?token={token}", state.config.base_url);
    let mailer = state.mailer.clone();
    let recipient = user.email.clone();
    // lettre blocks; keep the send off the async runtime. A
    // delivery failure is logged, never answered — see above.
    tokio::task::spawn_blocking(move || {
        if let Err(err) = mailer.send_password_reset(&recipient, &reset_url) {
            tracing::error!(error = %err, "password reset: mail delivery failed");
        }
    });
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
struct ResetConfirmBody {
    token: String,
    new_password: String,
}

async fn password_reset_confirm(
    State(state): State<AppState>,
    result: Result<Json<ResetConfirmBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    // One generic error for every failure mode — unknown,
    // used or expired token — so the endpoint is not an
    // oracle for which links are live.
    let invalid = || {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_token",
            "This reset link is invalid or has expired. Request a new one.",
        )
    };
    if req.token.is_empty() || req.token.len() > 128 {
        return Err(invalid());
    }
    let row = state
        .repos
        .password_resets
        .by_hash(&sha256_hex(&req.token))
        .await?
        .ok_or_else(invalid)?;
    if row.used_at.is_some() || row.expires_at <= now_iso() {
        return Err(invalid());
    }
    let user = state
        .repos
        .users
        .by_id(row.user_id)
        .await?
        .filter(|u| u.deleted_at.is_none() && u.disabled_at.is_none())
        .ok_or_else(invalid)?;
    validate_password(&req.new_password, &user.username).map_err(ApiError::validation)?;
    // Consume first: if anything below fails, the link is
    // dead and the user requests a fresh one — failing safe.
    state
        .repos
        .password_resets
        .mark_used(row.id, &now_iso())
        .await?;
    let hash = hash_password(req.new_password.clone()).await?;
    state.repos.users.set_password_hash(user.id, &hash).await?;
    // A reset can mean the account was compromised: every
    // session and API token dies with the old password.
    state.repos.sessions.delete_for_user(user.id).await?;
    state
        .repos
        .tokens
        .revoke_all_for_user(user.id, &now_iso())
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn logout(
    State(state): State<AppState>,
    ctx: RequireSession,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    verify_csrf(&headers, &ctx.session)?;
    state.repos.sessions.delete(&ctx.session.id_hash).await?;
    let mut resp = StatusCode::NO_CONTENT.into_response();
    attach_cookie(&mut resp, &clear_cookie(&state));
    Ok(resp)
}

async fn logout_all(
    State(state): State<AppState>,
    ctx: RequireSession,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    verify_csrf(&headers, &ctx.session)?;
    state.repos.sessions.delete_for_user(ctx.user.id).await?;
    let mut resp = StatusCode::NO_CONTENT.into_response();
    attach_cookie(&mut resp, &clear_cookie(&state));
    Ok(resp)
}

async fn get_me(RequireUser(ctx): RequireUser) -> Response {
    Json(me_json(&ctx.user, ctx.kind.session())).into_response()
}

// ---------------------------------------------------------- sessions list

/// The public reference for a session: a prefix of its stored
/// id hash. The hash is one-way, so showing a prefix is safe,
/// and it is unique enough to address one of the user's own
/// sessions for revocation.
fn session_ref(id_hash: &str) -> &str {
    &id_hash[..10.min(id_hash.len())]
}

fn session_json(row: &crate::repo::SessionRow, current: &str) -> serde_json::Value {
    serde_json::json!({
        "ref": session_ref(&row.id_hash),
        "created_at": row.created_at,
        "last_used_at": row.last_used_at,
        "expires_at": row.expires_at,
        "current": row.id_hash == current,
    })
}

/// GET /me/sessions — the signed-in user's browser sessions
/// (cookie sessions only; a bearer token is not a session).
async fn list_sessions(
    State(state): State<AppState>,
    ctx: RequireSession,
) -> Result<Response, ApiError> {
    let rows = state.repos.sessions.list_for_user(ctx.user.id).await?;
    let sessions: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| session_json(row, &ctx.session.id_hash))
        .collect();
    Ok(Json(serde_json::json!({ "sessions": sessions })).into_response())
}

/// DELETE /me/sessions/{ref} — revoke one of the user's
/// sessions by its public ref. Revoking the current session
/// is a logout. Unknown or ambiguous refs are a 404 (the ref
/// is only ever shown to the owner, so this is not an oracle).
async fn revoke_session(
    State(state): State<AppState>,
    ctx: RequireSession,
    headers: HeaderMap,
    axum::extract::Path(reference): axum::extract::Path<String>,
) -> Result<Response, ApiError> {
    verify_csrf(&headers, &ctx.session)?;
    if reference.len() < 8 || !reference.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::not_found("session_not_found", "No such session"));
    }
    let rows = state.repos.sessions.list_for_user(ctx.user.id).await?;
    let matches: Vec<&crate::repo::SessionRow> = rows
        .iter()
        .filter(|row| row.id_hash.starts_with(&reference))
        .collect();
    if matches.len() != 1 {
        return Err(ApiError::not_found("session_not_found", "No such session"));
    }
    state.repos.sessions.delete(&matches[0].id_hash).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize)]
#[allow(clippy::option_option)] // tri-state JSON: absent vs null vs value
struct PatchMeRequest {
    display_name: Option<Option<String>>,
    bio: Option<Option<String>>,
    current_password: Option<String>,
    new_password: Option<String>,
}

async fn patch_me(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ctx: RequireSession,
    headers: HeaderMap,
    result: Result<Json<PatchMeRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    verify_csrf(&headers, &ctx.session)?;
    let req = body(result)?;
    let mut user = ctx.user.clone();
    let mut session = ctx.session.clone();
    let mut new_cookie: Option<String> = None;

    // Validate a requested password change BEFORE writing anything,
    // so a rejected change cannot leave a half-applied PATCH.
    let pending_hash = match (&req.new_password, &req.current_password) {
        (Some(new_password), Some(current)) => {
            if !verify_password(current.clone(), user.password_hash.clone()).await {
                return Err(ApiError::unauthorized("Invalid credentials"));
            }
            validate_password(new_password, &user.username).map_err(ApiError::validation)?;
            Some(hash_password(new_password.clone()).await?)
        }
        (Some(_), None) => {
            return Err(ApiError::validation(
                "current_password is required to change the password",
            ));
        }
        (None, Some(_)) => {
            return Err(ApiError::validation(
                "new_password is required when current_password is given",
            ));
        }
        (None, None) => None,
    };

    if req.display_name.is_some() || req.bio.is_some() {
        let display = match &req.display_name {
            Some(Some(v)) => {
                let v = v.trim().to_string();
                if v.chars().count() > 80 || v.chars().any(char::is_control) {
                    return Err(ApiError::validation(
                        "Display name must be at most 80 characters without control characters",
                    ));
                }
                Some(v).filter(|s| !s.is_empty())
            }
            Some(None) => None,
            None => user.display_name.clone(),
        };
        let bio = match &req.bio {
            Some(Some(v)) => {
                let v = v.trim().to_string();
                if v.chars().count() > 280 || v.chars().any(|c| c.is_control() && c != '\n') {
                    return Err(ApiError::validation(
                        "Bio must be at most 280 characters without control characters",
                    ));
                }
                Some(v).filter(|s| !s.is_empty())
            }
            Some(None) => None,
            None => user.bio.clone(),
        };
        state
            .repos
            .users
            .update_profile(user.id, display.as_deref(), bio.as_deref())
            .await?;
        user.display_name = display;
        user.bio = bio;
    }

    if let Some(hash) = pending_hash {
        state.repos.users.set_password_hash(user.id, &hash).await?;
        user.password_hash = hash;
        // Revoke every session (including this one) and rotate:
        // the caller gets a fresh session + cookie (T-ACCT-2).
        state.repos.sessions.delete_for_user(user.id).await?;
        let (fresh, cookie) = start_session(&state, user.id, ip, &headers).await?;
        session = fresh;
        new_cookie = Some(cookie);
    }

    let mut resp = Json(me_json(&user, Some(&session))).into_response();
    if let Some(cookie) = new_cookie {
        attach_cookie(&mut resp, &cookie);
    }
    Ok(resp)
}

#[derive(Deserialize)]
struct DeleteMeRequest {
    username: String,
    password: String,
}

async fn delete_me(
    State(state): State<AppState>,
    ctx: RequireSession,
    headers: HeaderMap,
    result: Result<Json<DeleteMeRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    verify_csrf(&headers, &ctx.session)?;
    let req = body(result)?;
    if req.username.trim().to_lowercase() != ctx.user.username {
        return Err(ApiError::validation(
            "Type your exact username to confirm account deletion",
        ));
    }
    if !verify_password(req.password.clone(), ctx.user.password_hash.clone()).await {
        return Err(ApiError::unauthorized("Invalid credentials"));
    }
    let now = now_iso();
    state.repos.users.soft_delete(ctx.user.id, &now).await?;
    state.repos.sessions.delete_for_user(ctx.user.id).await?;
    state
        .repos
        .tokens
        .revoke_all_for_user(ctx.user.id, &now)
        .await?;
    let mut resp = StatusCode::NO_CONTENT.into_response();
    attach_cookie(&mut resp, &clear_cookie(&state));
    Ok(resp)
}

/// The auth route group (merged under `/api/v1`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/password-reset/request", post(password_reset_request))
        .route("/auth/password-reset/confirm", post(password_reset_confirm))
        .route("/auth/logout", post(logout))
        .route("/auth/logout-all", post(logout_all))
        .route("/me", get(get_me).patch(patch_me).delete(delete_me))
        .route("/me/sessions", get(list_sessions))
        .route("/me/sessions/{ref}", axum::routing::delete(revoke_session))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_rules() {
        assert!(validate_password("short", "alice").is_err());
        assert!(
            validate_password("password123", "alice").is_err(),
            "denylist"
        );
        assert!(
            validate_password("PASSWORD123", "alice").is_err(),
            "denylist is case-insensitive"
        );
        assert!(
            validate_password("AliceAlice", "alicealice").is_err(),
            "password equal to the username (any case) is rejected"
        );
        assert!(validate_password("a-perfectly-fine-passphrase", "alice").is_ok());
        assert!(validate_password(&"x".repeat(257), "alice").is_err());
    }

    #[test]
    fn email_rules() {
        for ok in ["a@b.co", "first.last+tag@sub.example.com", "x@y.io"] {
            assert!(validate_email(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "no-at-sign",
            "a@@b.com",
            "a@b",
            "a@.com",
            "a@b.c",
            ".a@b.com",
            "a.@b.com",
            "a..b@c.com",
            "a@-b.com",
            "a@b-.com",
            "a b@c.com",
            "a@c m.com",
        ] {
            assert!(validate_email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn hashing_roundtrips_and_rejects_wrong_passwords() {
        let hash = hash_password_blocking("a-perfectly-fine-passphrase").unwrap();
        assert!(hash.starts_with("$argon2id$"), "{hash}");
        assert!(verify_password_blocking(
            "a-perfectly-fine-passphrase",
            &hash
        ));
        assert!(!verify_password_blocking("wrong-password-here", &hash));
        assert!(!verify_password_blocking(
            "a-perfectly-fine-passphrase",
            "not-a-phc-string"
        ));
        // Unique salts: two hashes of one password differ.
        let other = hash_password_blocking("a-perfectly-fine-passphrase").unwrap();
        assert_ne!(hash, other);
    }

    #[test]
    fn throttle_locks_after_budget_and_success_clears() {
        let throttle = LoginThrottle::new(Duration::from_millis(40));
        for _ in 0..MAX_FAILURES - 1 {
            throttle.record_failure("k");
            assert!(throttle.locked_for("k").is_none());
        }
        throttle.record_failure("k");
        assert!(throttle.locked_for("k").is_some(), "locked at the budget");
        throttle.record_success("k");
        assert!(throttle.locked_for("k").is_none());
    }

    #[test]
    fn ct_eq_compares_fully() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
    }

    #[test]
    fn cookie_parsing_finds_the_session() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; forgehub_session=abc123; other=1"),
        );
        assert_eq!(cookie_value(&headers, SESSION_COOKIE), Some("abc123"));
        assert_eq!(cookie_value(&headers, "missing"), None);
    }
}
