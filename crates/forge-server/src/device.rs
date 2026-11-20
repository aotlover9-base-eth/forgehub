//! Device-code login flow (Step 8; RFC 8628 in spirit,
//! T-ACCT-4). A CLI asks for codes, the user approves in the
//! browser (cookie session + CSRF, explicit action only), and the
//! CLI polls for the result.
//!
//! Storage rules: the high-entropy `device_code` is stored only as
//! a SHA-256 hash; the short `user_code` is stored in plaintext
//! (it is a lookup key the user types, protected by attempt
//! lockouts per code and per IP, a 10-minute expiry, and single
//! use). The API token is minted at *delivery* time - approval
//! only records who approved - so its plaintext never needs to be
//! stored anywhere.

use crate::auth::{
    body, fmt_ts, parse_iso, random_from_alphabet, sha256_hex, ClientIp, RequireSession,
};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::DeviceCodeRow;
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Device codes live 10 minutes (openapi: `expires_in` 600).
pub const CODE_TTL_SECS: i64 = 600;
/// Wrong user-code lookups tolerated per IP before a lockout.
const MAX_LOOKUP_FAILURES: u32 = 10;
/// How long a locked-out IP stays locked out.
const LOOKUP_LOCKOUT: Duration = Duration::from_secs(300);
/// Failed actions tolerated per code before the code is burned.
const MAX_CODE_ATTEMPTS: i64 = 10;

/// Unambiguous user-code alphabet: no 0/O, 1/I/L confusion.
const USER_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Per-IP failed-lookup lockout for user codes (T-ACCT-4). A
/// guessed code that does not exist has no row to count against,
/// so the counting lives here, keyed by client IP.
#[derive(Debug, Default)]
pub struct DeviceLimiter {
    inner: Mutex<HashMap<String, (u32, Option<Instant>)>>,
}

impl DeviceLimiter {
    /// New, empty limiter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn key(ip: Option<SocketAddr>) -> String {
        ip.map_or_else(|| "unknown".to_string(), |a| a.ip().to_string())
    }

    /// Is this IP currently locked out of code lookups?
    #[must_use]
    pub fn is_locked(&self, ip: Option<SocketAddr>) -> bool {
        let map = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match map.get(&Self::key(ip)) {
            Some((_, Some(until))) => *until > Instant::now(),
            _ => false,
        }
    }

    /// Record a failed lookup; locks the IP at the budget.
    pub fn record_failure(&self, ip: Option<SocketAddr>) {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() > 10_000 {
            map.retain(|_, (_, until)| until.is_some_and(|u| u > Instant::now()));
        }
        let entry = map.entry(Self::key(ip)).or_insert((0, None));
        entry.0 = entry.0.saturating_add(1);
        if entry.0 >= MAX_LOOKUP_FAILURES {
            entry.1 = Some(Instant::now() + LOOKUP_LOCKOUT);
        }
    }
}

/// Normalise a user code the way users type it: case-insensitive,
/// hyphen optional, surrounding whitespace ignored.
fn normalize_user_code(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_uppercase()
}

/// `ABCDEFGH` → `ABCD-EFGH` for display.
fn display_user_code(stored: &str) -> String {
    if stored.len() == 8 {
        format!("{}-{}", &stored[..4], &stored[4..])
    } else {
        stored.to_string()
    }
}

fn code_expired(row: &DeviceCodeRow) -> bool {
    parse_iso(&row.expires_at).is_none_or(|e| e <= chrono::Utc::now())
}

#[derive(Deserialize)]
struct CodeRequest {
    client_name: String,
}

async fn request_code(
    State(state): State<AppState>,
    result: Result<Json<CodeRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    let client_name = req.client_name.trim().to_string();
    if client_name.is_empty()
        || client_name.chars().count() > 80
        || client_name.chars().any(char::is_control)
    {
        return Err(ApiError::validation(
            "client_name must be 1-80 characters without control characters",
        ));
    }
    let device_code = crate::auth::random_hex(32);
    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::seconds(CODE_TTL_SECS);
    // User codes must be unique among live codes; retry a few
    // times on the (astronomically unlikely) collision.
    let mut stored_row: Option<DeviceCodeRow> = None;
    for _ in 0..5 {
        let user_code = random_from_alphabet(USER_CODE_ALPHABET, 8);
        let attempt = state
            .repos
            .device_codes
            .create(
                &sha256_hex(&device_code),
                &user_code,
                &client_name,
                &fmt_ts(expires),
                &fmt_ts(now),
            )
            .await;
        match attempt {
            Ok(created) => {
                stored_row = Some(created);
                break;
            }
            Err(crate::repo::RepoError::Conflict(_)) => {}
            Err(e) => return Err(e.into()),
        }
    }
    let row = stored_row.ok_or_else(ApiError::internal)?;
    let shown = display_user_code(&row.user_code);
    let base = state.config.base_url.trim_end_matches('/');
    Ok(Json(serde_json::json!({
        "device_code": device_code,
        "user_code": shown,
        "verification_uri": format!("{base}/device"),
        "verification_uri_complete": format!("{base}/device?code={shown}"),
        "expires_in": CODE_TTL_SECS,
        "interval": state.config.device_poll_interval_secs,
    }))
    .into_response())
}

#[derive(Deserialize)]
struct TokenRequest {
    device_code: String,
}

async fn poll_token(
    State(state): State<AppState>,
    result: Result<Json<TokenRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    let hash = sha256_hex(req.device_code.trim());
    let Some(row) = state.repos.device_codes.by_hash(&hash).await? else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_device_code",
            "Unknown device code",
        ));
    };
    let pending = |status: &str, error: &str| {
        Json(serde_json::json!({ "status": status, "error": error })).into_response()
    };
    if row.status == "consumed" || code_expired(&row) {
        if row.status != "consumed" && row.status != "expired" {
            let _ = state
                .repos
                .device_codes
                .set_status(row.id, "expired", row.user_id, None)
                .await;
        }
        return Ok(pending("expired", "expired_token"));
    }
    match row.status.as_str() {
        "denied" => Ok(pending("denied", "access_denied")),
        "approved" => {
            // Mint the token now - the only moment its plaintext
            // exists - then burn the code atomically (single use:
            // a racing poll that loses the consume revokes its
            // own minted token and reports the code as spent).
            let user_id = row.user_id.ok_or_else(ApiError::internal)?;
            let today = chrono::Utc::now().format("%Y-%m-%d");
            let name = format!("CLI on {} ({today})", row.client_name);
            let (token, plaintext) =
                crate::tokens::create_token(&state, user_id, &name, "read write", None).await?;
            let consumed = state.repos.device_codes.consume(row.id, token.id).await?;
            if !consumed {
                state.repos.tokens.revoke(token.id, &now_iso()).await?;
                return Ok(pending("expired", "expired_token"));
            }
            Ok(Json(serde_json::json!({
                "status": "approved",
                "access_token": plaintext,
                "token_type": "Bearer",
                "scopes": ["read", "write"],
            }))
            .into_response())
        }
        _ => {
            // Pending: enforce the poll interval (RFC 8628 slow_down).
            let now = chrono::Utc::now();
            let interval = chrono::Duration::seconds(
                i64::try_from(state.config.device_poll_interval_secs).unwrap_or(5),
            );
            let too_fast = row
                .last_polled_at
                .as_deref()
                .and_then(parse_iso)
                .is_some_and(|last| now - last < interval);
            state
                .repos
                .device_codes
                .record_poll(row.id, &now_iso())
                .await?;
            if too_fast {
                Ok(pending("pending", "slow_down"))
            } else {
                Ok(pending("pending", "authorization_pending"))
            }
        }
    }
}

/// Shared lookup for lookup/approve/deny: resolves the code,
/// applies the IP lockout, and burns codes that collect too many
/// failed actions. `Ok(None)` means "answer 404".
async fn find_actionable_code(
    state: &AppState,
    ip: Option<SocketAddr>,
    raw_code: &str,
) -> Result<Option<DeviceCodeRow>, ApiError> {
    if state.device_limiter.is_locked(ip) {
        return Err(ApiError::rate_limited());
    }
    let code = normalize_user_code(raw_code);
    let row = state.repos.device_codes.by_user_code(&code).await?;
    let Some(row) = row else {
        state.device_limiter.record_failure(ip);
        return Ok(None);
    };
    if row.attempts >= MAX_CODE_ATTEMPTS {
        return Ok(None);
    }
    Ok(Some(row))
}

#[derive(Deserialize)]
struct LookupQuery {
    user_code: String,
}

async fn lookup(
    State(state): State<AppState>,
    _ctx: RequireSession,
    ClientIp(ip): ClientIp,
    Query(query): Query<LookupQuery>,
) -> Result<Response, ApiError> {
    let Some(row) = find_actionable_code(&state, ip, &query.user_code).await? else {
        return Err(ApiError::not_found(
            "device_code_not_found",
            "No such device code",
        ));
    };
    if row.status != "pending" || code_expired(&row) {
        state.device_limiter.record_failure(ip);
        return Err(ApiError::not_found(
            "device_code_not_found",
            "No such device code",
        ));
    }
    Ok(Json(serde_json::json!({
        "client_name": row.client_name,
        "scopes": ["read", "write"],
        "expires_at": row.expires_at,
    }))
    .into_response())
}

#[derive(Deserialize)]
struct DecideRequest {
    user_code: String,
}

/// Approve or deny. Returns the HTTP status to send.
async fn decide(
    state: &AppState,
    ctx: &RequireSession,
    ip: Option<SocketAddr>,
    headers: &HeaderMap,
    raw_code: &str,
    approve: bool,
) -> Result<Response, ApiError> {
    crate::auth::verify_csrf(headers, &ctx.session)?;
    let Some(row) = find_actionable_code(state, ip, raw_code).await? else {
        return Err(ApiError::not_found(
            "device_code_not_found",
            "No such device code",
        ));
    };
    if code_expired(&row) {
        return Err(ApiError::new(
            StatusCode::GONE,
            "device_code_expired",
            "That device code has expired; start again from the CLI",
        ));
    }
    if row.status != "pending" {
        state.repos.device_codes.record_attempt(row.id).await?;
        return Err(ApiError::not_found(
            "device_code_not_found",
            "No such device code",
        ));
    }
    if approve {
        state
            .repos
            .device_codes
            .set_status(row.id, "approved", Some(ctx.user.id), None)
            .await?;
    } else {
        state
            .repos
            .device_codes
            .set_status(row.id, "denied", Some(ctx.user.id), None)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn approve(
    State(state): State<AppState>,
    ctx: RequireSession,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    result: Result<Json<DecideRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    decide(&state, &ctx, ip, &headers, &req.user_code, true).await
}

async fn deny(
    State(state): State<AppState>,
    ctx: RequireSession,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    result: Result<Json<DecideRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    decide(&state, &ctx, ip, &headers, &req.user_code, false).await
}

/// Device-flow routes (merged under `/api/v1`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/device/code", post(request_code))
        .route("/auth/device/token", post(poll_token))
        .route("/auth/device/lookup", get(lookup))
        .route("/auth/device/approve", post(approve))
        .route("/auth/device/deny", post(deny))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_code_normalisation_accepts_typing_variants() {
        assert_eq!(normalize_user_code("abcd-efgh"), "ABCDEFGH");
        assert_eq!(normalize_user_code(" ABCD EFGH "), "ABCDEFGH");
        assert_eq!(normalize_user_code("abcdefgh"), "ABCDEFGH");
    }

    #[test]
    fn user_code_display_groups_four_by_four() {
        assert_eq!(display_user_code("ABCDEFGH"), "ABCD-EFGH");
    }

    #[test]
    fn limiter_locks_after_budget() {
        let limiter = DeviceLimiter::new();
        let ip: Option<SocketAddr> = Some("10.0.0.9:5555".parse().unwrap());
        for _ in 0..MAX_LOOKUP_FAILURES - 1 {
            limiter.record_failure(ip);
            assert!(!limiter.is_locked(ip));
        }
        limiter.record_failure(ip);
        assert!(limiter.is_locked(ip));
        // A different IP is unaffected.
        let other: Option<SocketAddr> = Some("10.0.0.10:5555".parse().unwrap());
        assert!(!limiter.is_locked(other));
    }
}
