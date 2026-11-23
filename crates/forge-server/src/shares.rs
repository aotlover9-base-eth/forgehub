//! Collection share links (Peak C6, ADR 0011): an owner can
//! mint an unguessable, expiring, revocable link that lets
//! anyone READ a collection - including its private snippets -
//! on a server-rendered page at `/shared/{token}`. The link
//! grants viewing only: no run, no pull, no account data.
//! Tokens are stored hashed (SHA-256) with a display prefix,
//! exactly like API tokens; the plaintext is shown once.
//!
//! API (owner-only, existence-masked like collection writes):
//! - `POST   /collections/{owner}/{name}/share-links`       mint
//! - `GET    /collections/{owner}/{name}/share-links`       list
//! - `DELETE /collections/{owner}/{name}/share-links/{id}`  revoke
//!
//! Page (anonymous):
//! - `GET    /shared/{token}`

use crate::auth::{body, fmt_ts, random_from_alphabet, sha256_hex, RequireUser};
use crate::collections::{collection_not_found, resolve_own_collection};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::pages::{esc, PAGE_CSS};
use crate::repo::ShareLinkRow;
use crate::snippets::check_write_scope_and_csrf;
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Router;
use serde::Deserialize;
use std::fmt::Write as _;

/// ADR 0011 rule 3: at most this many unrevoked links per
/// collection.
pub(crate) const MAX_ACTIVE_LINKS_PER_COLLECTION: i64 = 10;
/// The expiry choices offered at mint time, in days.
const ALLOWED_EXPIRY_DAYS: [i64; 3] = [1, 7, 30];

const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Generate a share token: `(plaintext, prefix, sha256 hash)`.
/// The plaintext must be shown once and never stored.
fn generate_share_token() -> (String, String, String) {
    let random = random_from_alphabet(BASE62, 43);
    let plaintext = format!("fgl_{random}");
    let prefix = random[..8].to_string();
    let hash = sha256_hex(&plaintext);
    (plaintext, prefix, hash)
}

/// A link is usable only while unrevoked and unexpired. ISO
/// timestamps compare lexicographically.
fn link_active(link: &ShareLinkRow, now: &str) -> bool {
    link.revoked_at.is_none() && link.expires_at.as_str() > now
}

fn link_json(link: &ShareLinkRow, now: &str) -> serde_json::Value {
    serde_json::json!({
        "id": link.id,
        "prefix": link.token_prefix,
        "created_at": link.created_at,
        "expires_at": link.expires_at,
        "revoked_at": link.revoked_at,
        "active": link_active(link, now),
    })
}

// ------------------------------------------------------------ API

#[derive(Deserialize)]
struct ShareLinkInput {
    expires_in_days: Option<i64>,
}

async fn create_share_link(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    result: Result<Json<ShareLinkInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let days = req.expires_in_days.unwrap_or(7);
    if !ALLOWED_EXPIRY_DAYS.contains(&days) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_expiry",
            "Share links expire after 1, 7 or 30 days - pick one",
        ));
    }
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    let active = state
        .repos
        .collections
        .active_share_link_count(row.id)
        .await?;
    if active >= MAX_ACTIVE_LINKS_PER_COLLECTION {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "too_many_links",
            "This collection already has 10 active share links; revoke one first",
        ));
    }
    let (plaintext, prefix, hash) = generate_share_token();
    let now = now_iso();
    let expires = fmt_ts(chrono::Utc::now() + chrono::Duration::days(days));
    let link = state
        .repos
        .collections
        .create_share_link(row.id, &hash, &prefix, &now, &expires)
        .await?;
    let mut out = link_json(&link, &now);
    // The one and only time the full URL is available.
    out["url"] = serde_json::json!(format!("/shared/{plaintext}"));
    out["token"] = serde_json::json!(plaintext);
    Ok((StatusCode::CREATED, Json(out)).into_response())
}

async fn list_share_links(
    State(state): State<AppState>,
    ctx: RequireUser,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    let now = now_iso();
    let links = state.repos.collections.share_links(row.id).await?;
    let out: Vec<serde_json::Value> = links.iter().map(|l| link_json(l, &now)).collect();
    Ok(Json(serde_json::json!({ "links": out })).into_response())
}

async fn revoke_share_link(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name, id)): Path<(String, String, i64)>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    let revoked = state
        .repos
        .collections
        .revoke_share_link(id, row.id, &now_iso())
        .await?;
    if !revoked {
        return Err(collection_not_found());
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ------------------------------------------------------------ page

fn link_not_found_page() -> Response {
    let html = format!("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Link not available - ForgeHub</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<style>{PAGE_CSS}</style>\n</head>\n<body>\n<main>\n<h1>This link is not available</h1>\n<p>It may have expired or been revoked by its owner, or the address may be mistyped. Ask the person who shared it for a fresh link.</p>\n<p><a href=\"/\">ForgeHub</a></p>\n</main>\n</body>\n</html>\n");
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

async fn shared_collection_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    // Cheap shape check before the hash lookup: real tokens
    // are `fgl_` + 43 base62 characters.
    if token.len() != 47 || !token.starts_with("fgl_") {
        return Ok(link_not_found_page());
    }
    let hash = sha256_hex(&token);
    let Some((link, collection, owner)) = state.repos.collections.share_link_by_hash(&hash).await?
    else {
        return Ok(link_not_found_page());
    };
    if !link_active(&link, &now_iso()) {
        return Ok(link_not_found_page());
    }
    let items = state.repos.collections.items(collection.id).await?;
    let mut sections = String::new();
    for item in &items {
        // An admin action outranks an owner's share (ADR 0011
        // rule 4); deleted snippets are gone for everyone.
        if item.hidden_at.is_some() || item.deleted_at.is_some() {
            continue;
        }
        let Some(snippet) = state
            .repos
            .snippets
            .by_owner_name(item.snippet_owner_id, &item.name)
            .await?
        else {
            continue;
        };
        let Some(version) = state
            .repos
            .snippets
            .version(item.snippet_id, snippet.latest_version)
            .await?
        else {
            continue;
        };
        write!(
            sections,
            "<section>\n<h2>{} <small>by {} - v{}</small></h2>\n<p>{}</p>\n<pre><code>{}</code></pre>\n</section>\n",
            esc(&item.name),
            esc(&item.owner_username),
            item.latest_version,
            esc(&item.description),
            esc(&version.body),
        )
        .expect("writing to a String cannot fail");
    }
    if sections.is_empty() {
        sections.push_str("<p>This collection is empty.</p>\n");
    }
    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>{} - a shared ForgeHub collection</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"robots\" content=\"noindex\">\n<style>{}</style>\n</head>\n<body>\n<header>\n<p>Shared by {} with a read-only link. The link expires on {}.</p>\n</header>\n<main>\n<h1>{}</h1>\n<p>{}</p>\n</main>\n{}\n<footer>\n<p>Read-only share: this page shows code for reading. Running snippets needs a ForgeHub account and access to each snippet - and, as always, read code before you run it.</p>\n</footer>\n</body>\n</html>\n",
        esc(&collection.name),
        PAGE_CSS,
        esc(&owner.username),
        esc(&link.expires_at),
        esc(&collection.name),
        esc(&collection.description),
        sections,
    );
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}

/// API routes (merged into the API router).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/collections/{owner}/{name}/share-links",
            post(create_share_link).get(list_share_links),
        )
        .route(
            "/collections/{owner}/{name}/share-links/{id}",
            delete(revoke_share_link),
        )
}

/// The anonymous page route (merged outside the API router).
pub fn page_routes() -> Router<AppState> {
    Router::new().route("/shared/{token}", get(shared_collection_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(revoked: Option<&str>, expires: &str) -> ShareLinkRow {
        ShareLinkRow {
            id: 1,
            collection_id: 1,
            token_hash: "h".into(),
            token_prefix: "abcdefgh".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            expires_at: expires.into(),
            revoked_at: revoked.map(str::to_string),
        }
    }

    #[test]
    fn activity_rules() {
        let now = "2026-06-01T00:00:00Z";
        assert!(link_active(&link(None, "2026-06-02T00:00:00Z"), now));
        assert!(!link_active(&link(None, "2026-05-31T00:00:00Z"), now));
        assert!(!link_active(
            &link(Some("2026-05-01T00:00:00Z"), "2027-01-01T00:00:00Z"),
            now
        ));
    }

    #[test]
    fn tokens_are_unique_and_hashed() {
        let (a, prefix_a, hash_a) = generate_share_token();
        let (b, _, _) = generate_share_token();
        assert_ne!(a, b);
        assert!(a.starts_with("fgl_"));
        assert_eq!(a.len(), 47);
        assert_eq!(prefix_a.len(), 8);
        assert_eq!(hash_a, sha256_hex(&a));
        assert!(!a.contains(&hash_a));
    }
}
