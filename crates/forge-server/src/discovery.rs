//! Discovery and social basics (Step 10): FTS5 search over
//! public snippets, popular tags, explore feeds, stars, forks,
//! public profiles, and abuse reports.
//!
//! Safety properties (all tested in `tests/discovery.rs`):
//! - search/explore/profiles only ever expose **public** snippets
//!   (the repository enforces the predicates, FTS membership is
//!   the second layer, not the only one);
//! - user search text can never become FTS5 syntax: queries are
//!   reduced to alphanumeric terms and re-emitted as quoted
//!   prefix phrases, so operators (`OR`, `NEAR`, column filters,
//!   quotes) are inert data;
//! - profiles never expose email or any private field;
//! - reports are deduplicated per (reporter, snippet) and never
//!   reveal whether a report already exists.

use crate::auth::{body, CurrentUser, RequireUser};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::{SnippetRow, UserRow};
use crate::snippets::{
    check_write_scope_and_csrf, decode_cursor, encode_cursor, not_found, page_limit,
    resolve_readable, snippet_json,
};
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Router;
use serde::Deserialize;

// ---------------------------------------------------------------- search

/// Reduce raw user text to search terms: whitespace-split, keep
/// only alphanumeric/underscore characters, cap the term count.
/// FTS5 syntax characters never survive this.
pub(crate) fn search_terms(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .take(10)
        .collect()
}

/// Build the FTS5 MATCH string from sanitised terms: each term
/// becomes a quoted prefix phrase (`"apt"*`), terms AND together.
pub(crate) fn match_query(terms: &[String]) -> Option<String> {
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .iter()
            .map(|t| format!("\"{t}\"*"))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Plain-text highlight ranges (byte offsets) for the matched
/// terms in each searchable field. One range per term per field,
/// extended to the end of the matched word (prefix matches).
fn highlights(
    snippet: &SnippetRow,
    owner: &str,
    tags: &[String],
    body: &str,
    terms: &[String],
) -> Vec<serde_json::Value> {
    let fields: [(&str, String); 5] = [
        ("name", snippet.name.clone()),
        ("description", snippet.description.clone()),
        ("tags", tags.join(" ")),
        ("owner", owner.to_string()),
        ("body", body.to_string()),
    ];
    let mut out = Vec::new();
    for (field, text) in &fields {
        let lower = text.to_lowercase();
        for term in terms {
            let needle = term.to_lowercase();
            if let Some(start) = lower.find(&needle) {
                let mut end = start + needle.len();
                let bytes = text.as_bytes();
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
                {
                    end += 1;
                }
                out.push(serde_json::json!({ "field": field, "start": start, "end": end }));
            }
        }
    }
    out.truncate(20);
    out
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
    tag: Option<String>,
    owner: Option<String>,
    sort: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
}

/// Page through rows, rendering each with its latest version.
async fn render_page(
    state: &AppState,
    rows: &[SnippetRow],
    viewer: Option<&UserRow>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let mut items = Vec::new();
    for snippet in rows {
        let latest = state
            .repos
            .snippets
            .latest_version(snippet.id)
            .await?
            .ok_or_else(ApiError::internal)?;
        items.push(snippet_json(state, snippet, &latest, viewer).await?);
    }
    Ok(items)
}

async fn search(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Query(query): Query<SearchQuery>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let raw_q = query.q.as_deref().unwrap_or("");
    if raw_q.chars().count() > 200 {
        return Err(ApiError::validation("q must be at most 200 characters"));
    }
    let terms = search_terms(raw_q);
    let match_q = match_query(&terms);
    let sort = query.sort.as_deref().unwrap_or("relevance");
    if !["relevance", "stars", "updated"].contains(&sort) {
        return Err(ApiError::validation(
            "sort must be relevance, stars or updated",
        ));
    }
    let limit = page_limit(query.limit)?;
    let offset = query
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    let owner_id = match &query.owner {
        Some(username) => state
            .repos
            .users
            .by_username(username)
            .await?
            .filter(|u| u.deleted_at.is_none() && u.disabled_at.is_none())
            .map(|u| u.id),
        None => None,
    };
    if query.owner.is_some() && owner_id.is_none() {
        // Unknown owner: an empty page, not an error.
        return Ok(Json(serde_json::json!({ "results": [], "next_cursor": null })).into_response());
    }
    if match_q.is_none() && query.tag.is_none() && query.owner.is_none() {
        // No query and no filters: nothing to search (explore is
        // the browsing surface).
        return Ok(Json(serde_json::json!({ "results": [], "next_cursor": null })).into_response());
    }
    let rows = state
        .repos
        .snippets
        .search_public(
            match_q.as_deref(),
            query.tag.as_deref(),
            owner_id,
            sort,
            limit + 1,
            offset,
        )
        .await?;
    let has_more = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
    let mut results = Vec::new();
    for snippet in rows.iter().take(usize::try_from(limit).unwrap_or(0)) {
        let latest = state
            .repos
            .snippets
            .latest_version(snippet.id)
            .await?
            .ok_or_else(ApiError::internal)?;
        let owner = state
            .repos
            .users
            .by_id(snippet.owner_id)
            .await?
            .ok_or_else(ApiError::internal)?;
        let tags = state.repos.snippets.tags(snippet.id).await?;
        let item = snippet_json(&state, snippet, &latest, viewer_user.as_ref()).await?;
        results.push(serde_json::json!({
            "snippet": item,
            "highlights": highlights(snippet, &owner.username, &tags, &latest.body, &terms),
        }));
    }
    Ok(Json(serde_json::json!({
        "results": results,
        "next_cursor": if has_more { Some(encode_cursor(offset + limit)) } else { None },
    }))
    .into_response())
}

// ------------------------------------------------------------ tags/explore

async fn popular_tags(State(state): State<AppState>) -> Result<Response, ApiError> {
    let tags = state.repos.snippets.popular_tags(50).await?;
    let list: Vec<serde_json::Value> = tags
        .iter()
        .map(|(tag, count)| serde_json::json!({ "tag": tag, "count": count }))
        .collect();
    Ok(Json(serde_json::json!({ "tags": list })).into_response())
}

#[derive(Deserialize)]
pub struct ExploreQuery {
    feed: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
}

async fn explore(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Query(query): Query<ExploreQuery>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let feed = query.feed.as_deref().unwrap_or("recent");
    if !["recent", "popular"].contains(&feed) {
        return Err(ApiError::validation("feed must be recent or popular"));
    }
    let limit = page_limit(query.limit)?;
    let offset = query
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    let rows: Vec<SnippetRow> = if offset == 0 {
        // First pages come from the 60-second cache.
        let cached = {
            let cache = state
                .explore_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.get(feed).and_then(|(at, rows)| {
                if at.elapsed() < std::time::Duration::from_secs(60) {
                    Some(rows.clone())
                } else {
                    None
                }
            })
        };
        if let Some(rows) = cached {
            rows
        } else {
            let fresh = fetch_explore(&state, feed, 100, 0).await?;
            let mut cache = state
                .explore_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.insert(feed.to_string(), (std::time::Instant::now(), fresh.clone()));
            fresh
        }
    } else {
        fetch_explore(&state, feed, limit + 1, offset).await?
    };
    let page: Vec<SnippetRow> = rows
        .iter()
        .skip(usize::try_from(offset).unwrap_or(0))
        .take(usize::try_from(limit + 1).unwrap_or(0))
        .cloned()
        .collect();
    let has_more = page.len() > usize::try_from(limit).unwrap_or(0);
    let items = render_page(
        &state,
        &page[..page.len().min(usize::try_from(limit).unwrap_or(0))],
        viewer_user.as_ref(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snippets": items,
        "next_cursor": if has_more { Some(encode_cursor(offset + limit)) } else { None },
    }))
    .into_response())
}

async fn fetch_explore(
    state: &AppState,
    feed: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<SnippetRow>, ApiError> {
    if feed == "popular" {
        let since = crate::auth::fmt_ts(chrono::Utc::now() - chrono::Duration::days(7));
        Ok(state
            .repos
            .snippets
            .explore_popular(&since, limit, offset)
            .await?)
    } else {
        Ok(state.repos.snippets.explore_recent(limit, offset).await?)
    }
}

// --------------------------------------------------------------- profiles

async fn profile(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Response, ApiError> {
    let user = state
        .repos
        .users
        .by_username(&username)
        .await?
        .filter(|u| u.deleted_at.is_none() && u.disabled_at.is_none())
        .ok_or_else(|| ApiError::not_found("user_not_found", "User not found"))?;
    let count = state.repos.snippets.count_public_for_owner(user.id).await?;
    Ok(Json(serde_json::json!({
        "username": user.username,
        "display_name": user.display_name,
        "bio": user.bio,
        "created_at": user.created_at,
        "public_snippet_count": count,
    }))
    .into_response())
}

#[derive(Deserialize)]
pub struct PageQ {
    limit: Option<i64>,
    cursor: Option<String>,
}

async fn user_snippets(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Path(username): Path<String>,
    Query(query): Query<PageQ>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let user = state
        .repos
        .users
        .by_username(&username)
        .await?
        .filter(|u| u.deleted_at.is_none() && u.disabled_at.is_none())
        .ok_or_else(|| ApiError::not_found("user_not_found", "User not found"))?;
    let limit = page_limit(query.limit)?;
    let offset = query
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    let rows = state
        .repos
        .snippets
        .list_public_for_owner(user.id, limit + 1, offset)
        .await?;
    let has_more = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
    let items = render_page(
        &state,
        &rows[..rows.len().min(usize::try_from(limit).unwrap_or(0))],
        viewer_user.as_ref(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snippets": items,
        "next_cursor": if has_more { Some(encode_cursor(offset + limit)) } else { None },
    }))
    .into_response())
}

// ------------------------------------------------------------------ stars

async fn star(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let resolved = resolve_readable(&state, &owner, &name, Some(&ctx.0.user)).await?;
    if resolved.snippet.visibility == "private" {
        return Err(not_found());
    }
    state
        .repos
        .snippets
        .star(ctx.0.user.id, resolved.snippet.id, &now_iso())
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn unstar(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let resolved = resolve_readable(&state, &owner, &name, Some(&ctx.0.user)).await?;
    if resolved.snippet.visibility == "private" {
        return Err(not_found());
    }
    state
        .repos
        .snippets
        .unstar(ctx.0.user.id, resolved.snippet.id)
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn my_stars(
    State(state): State<AppState>,
    ctx: RequireUser,
    Query(query): Query<PageQ>,
) -> Result<Response, ApiError> {
    if !ctx.0.kind.has_scope("read") {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_scope",
            "This token cannot read",
        ));
    }
    let limit = page_limit(query.limit)?;
    let offset = query
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    let rows = state
        .repos
        .snippets
        .list_starred(ctx.0.user.id, limit + 1, offset)
        .await?;
    let has_more = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
    let items = render_page(
        &state,
        &rows[..rows.len().min(usize::try_from(limit).unwrap_or(0))],
        Some(&ctx.0.user),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snippets": items,
        "next_cursor": if has_more { Some(encode_cursor(offset + limit)) } else { None },
    }))
    .into_response())
}

// ------------------------------------------------------------------- fork

#[derive(Deserialize)]
struct ForkBody {
    name: Option<String>,
}

async fn fork(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    payload: Option<Json<ForkBody>>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let resolved = resolve_readable(&state, &owner, &name, Some(&ctx.0.user)).await?;
    let source = resolved.snippet;
    let new_name = payload
        .and_then(|Json(b)| b.name)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| source.name.clone());
    forge_core::names::validate_snippet_name(&new_name)
        .map_err(|e| ApiError::validation(e.to_string()))?;
    let count = state.repos.snippets.count_for_owner(ctx.0.user.id).await?;
    if count >= state.config.max_snippets_per_user {
        return Err(ApiError::validation(format!(
            "Snippet limit reached ({} per user)",
            state.config.max_snippets_per_user
        )));
    }
    let latest = state
        .repos
        .snippets
        .latest_version(source.id)
        .await?
        .ok_or_else(ApiError::internal)?;
    let tags = state.repos.snippets.tags(source.id).await?;
    let (snippet, version) = match state
        .repos
        .snippets
        .create_fork(
            ctx.0.user.id,
            &new_name,
            &source.description,
            &latest.mode,
            &latest.body,
            &latest.hash,
            &tags,
            source.id,
            latest.version,
            &latest.hash,
            &now_iso(),
        )
        .await
    {
        Ok(pair) => pair,
        Err(crate::repo::RepoError::Conflict(_)) => {
            return Err(ApiError::conflict(
                "name_taken",
                "You already have a snippet with that name",
            ));
        }
        Err(e) => return Err(e.into()),
    };
    let json = snippet_json(&state, &snippet, &version, Some(&ctx.0.user)).await?;
    Ok((StatusCode::CREATED, Json(json)).into_response())
}

// ----------------------------------------------------------------- report

#[derive(Deserialize)]
struct ReportBody {
    reason: String,
}

async fn report(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    result: Result<Json<ReportBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let reason = req.reason.trim();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(ApiError::validation(
            "A reason of 1 to 500 characters is required",
        ));
    }
    let resolved = resolve_readable(&state, &owner, &name, Some(&ctx.0.user)).await?;
    // One open report per (reporter, snippet); a duplicate is a
    // silent success, so report state is never an oracle.
    if !state
        .repos
        .reports
        .exists_open(ctx.0.user.id, resolved.snippet.id)
        .await?
    {
        state
            .repos
            .reports
            .create(resolved.snippet.id, ctx.0.user.id, reason, &now_iso())
            .await?;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Discovery routes (merged under `/api/v1`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/tags", get(popular_tags))
        .route("/explore", get(explore))
        .route("/users/{username}", get(profile))
        .route("/users/{username}/snippets", get(user_snippets))
        .route("/me/stars", get(my_stars))
        .route("/snippets/{owner}/{name}/star", put(star).delete(unstar))
        .route("/snippets/{owner}/{name}/fork", post(fork))
        .route("/snippets/{owner}/{name}/report", post(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_sanitised() {
        assert_eq!(search_terms("apt install"), ["apt", "install"]);
        assert_eq!(search_terms("\" OR 1=1 --"), ["OR", "11"]);
        assert_eq!(
            search_terms("name:docker NEAR(a b)"),
            ["namedocker", "NEARa", "b"]
        );
        assert_eq!(search_terms("*** !!!").len(), 0);
        assert_eq!(search_terms(&"x ".repeat(30)).len(), 10);
    }

    #[test]
    fn match_strings_are_quoted_prefixes() {
        let m = match_query(&search_terms("docker comp")).unwrap();
        assert_eq!(m, "\"docker\"* \"comp\"*");
        assert!(match_query(&[]).is_none());
    }
}
