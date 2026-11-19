//! Personal API tokens (Step 8; ADR 0005, T-ACCT-3).
//!
//! A token is `fgh_` + 43 base62 characters drawn from a CSPRNG
//! (≈256 bits). Only its SHA-256 is stored, plus the first 8
//! characters of the random part as a display prefix. The
//! plaintext is returned exactly once, at creation. Management
//! endpoints require a cookie session ([`RequireSession`]) — a
//! token can never mint or revoke tokens.

use crate::auth::{body, random_from_alphabet, sha256_hex, RequireSession};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::TokenRow;
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::Router;
use serde::Deserialize;

/// Maximum unrevoked tokens per user.
pub const MAX_TOKENS_PER_USER: i64 = 25;

const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Generate a fresh token: `(plaintext, prefix, sha256 hash)`.
/// The plaintext must be shown once and never stored.
#[must_use]
pub fn generate_token() -> (String, String, String) {
    let random = random_from_alphabet(BASE62, 43);
    let plaintext = format!("fgh_{random}");
    let prefix = random[..8].to_string();
    let hash = sha256_hex(&plaintext);
    (plaintext, prefix, hash)
}

/// Normalise requested scopes: a subset of {read, write}, `write`
/// implies `read`, stored space-joined in canonical order.
///
/// # Errors
/// A user-safe message for empty or unknown scopes.
pub fn normalize_scopes(scopes: &[String]) -> Result<String, ApiError> {
    if scopes.is_empty() {
        return Err(ApiError::validation("At least one scope is required"));
    }
    let mut read = false;
    let mut write = false;
    for scope in scopes {
        match scope.as_str() {
            "read" => read = true,
            "write" => write = true,
            other => {
                return Err(ApiError::validation(format!(
                    "Unknown scope `{other}` (expected read or write)"
                )));
            }
        }
    }
    if write {
        read = true;
    }
    let mut out = String::new();
    if read {
        out.push_str("read");
    }
    if write {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("write");
    }
    Ok(out)
}

/// Split stored scopes back into the API's array form.
#[must_use]
pub fn scopes_array(stored: &str) -> Vec<String> {
    stored.split_whitespace().map(str::to_string).collect()
}

/// The public (hash-free) JSON shape of a token (openapi `ApiToken`).
#[must_use]
pub fn token_json(row: &TokenRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "name": row.name,
        "prefix": row.prefix,
        "scopes": scopes_array(&row.scopes),
        "created_at": row.created_at,
        "last_used_at": row.last_used_at,
        "expires_at": row.expires_at,
        "revoked_at": row.revoked_at,
    })
}

/// Create a token row for `user_id` and return `(row, plaintext)`.
/// Shared by the management endpoint and the device flow.
///
/// # Errors
/// When the per-user limit is reached or storage fails.
pub async fn create_token(
    state: &AppState,
    user_id: i64,
    name: &str,
    scopes: &str,
    expires_at: Option<String>,
) -> Result<(TokenRow, String), ApiError> {
    let active = state.repos.tokens.count_active_for_user(user_id).await?;
    if active >= MAX_TOKENS_PER_USER {
        return Err(ApiError::validation(format!(
            "Token limit reached ({MAX_TOKENS_PER_USER} active tokens); revoke one first"
        )));
    }
    let (plaintext, prefix, hash) = generate_token();
    let row = state
        .repos
        .tokens
        .create(
            user_id,
            name,
            &prefix,
            &hash,
            scopes,
            expires_at.as_deref(),
            &now_iso(),
        )
        .await?;
    Ok((row, plaintext))
}

async fn list_tokens(
    State(state): State<AppState>,
    ctx: RequireSession,
) -> Result<Response, ApiError> {
    let rows = state.repos.tokens.list_for_user(ctx.user.id).await?;
    let tokens: Vec<serde_json::Value> = rows.iter().map(token_json).collect();
    Ok(Json(serde_json::json!({ "tokens": tokens })).into_response())
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    name: String,
    scopes: Vec<String>,
    expires_in_days: Option<i64>,
}

async fn create_token_handler(
    State(state): State<AppState>,
    ctx: RequireSession,
    headers: axum::http::HeaderMap,
    result: Result<Json<CreateTokenRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    crate::auth::verify_csrf(&headers, &ctx.session)?;
    let req = body(result)?;
    let name = req.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(ApiError::validation(
            "Token name must be 1-80 characters without control characters",
        ));
    }
    let scopes = normalize_scopes(&req.scopes)?;
    let expires_at = match req.expires_in_days {
        Some(days) if (1..=365).contains(&days) => Some(crate::auth::fmt_ts(
            chrono::Utc::now() + chrono::Duration::days(days),
        )),
        Some(_) => {
            return Err(ApiError::validation(
                "expires_in_days must be between 1 and 365",
            ));
        }
        None => None,
    };
    let (row, plaintext) = create_token(&state, ctx.user.id, &name, &scopes, expires_at).await?;
    let mut json = token_json(&row);
    json["token"] = serde_json::Value::String(plaintext);
    Ok((StatusCode::CREATED, Json(json)).into_response())
}

async fn revoke_token(
    State(state): State<AppState>,
    ctx: RequireSession,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    crate::auth::verify_csrf(&headers, &ctx.session)?;
    let rows = state.repos.tokens.list_for_user(ctx.user.id).await?;
    if !rows.iter().any(|t| t.id == id) {
        return Err(ApiError::not_found("token_not_found", "Token not found"));
    }
    state.repos.tokens.revoke(id, &now_iso()).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Token management routes (merged under `/api/v1`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tokens", get(list_tokens).post(create_token_handler))
        .route("/tokens/{id}", delete(revoke_token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_the_right_shape() {
        let (plaintext, prefix, hash) = generate_token();
        assert!(plaintext.starts_with("fgh_"), "{plaintext}");
        assert_eq!(plaintext.len(), 4 + 43);
        assert_eq!(prefix.len(), 8);
        assert!(plaintext[4..].starts_with(&prefix));
        assert_eq!(hash, sha256_hex(&plaintext));
        assert!(!plaintext.contains(&hash));
        let (other, _, _) = generate_token();
        assert_ne!(plaintext, other, "tokens are random");
    }

    #[test]
    fn scope_normalisation() {
        assert_eq!(normalize_scopes(&["read".into()]).unwrap(), "read");
        assert_eq!(
            normalize_scopes(&["write".into()]).unwrap(),
            "read write",
            "write implies read"
        );
        assert_eq!(
            normalize_scopes(&["write".into(), "read".into()]).unwrap(),
            "read write"
        );
        assert!(normalize_scopes(&[]).is_err());
        assert!(normalize_scopes(&["admin".into()]).is_err());
    }
}
