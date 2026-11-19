//! Snippet and version API (Step 9; openapi `snippets` group).
//!
//! All validation, parsing and hashing go through `forge-core`, so
//! a hash computed here is byte-identical to the one the CLI
//! computes locally (spec-snippet-format §9). Bodies are stored as
//! canonical forge-file YAML (`Snippet::to_yaml`).
//!
//! Authorisation rules (tested exhaustively in
//! `tests/snippets.rs`):
//! - public/unlisted snippets are readable by anyone, including
//!   anonymous callers; private ones return **404, not 403**, to
//!   everyone but the owner, on every endpoint, so existence never
//!   leaks;
//! - writes are owner-only (non-owners get the same 404) and need
//!   the `write` scope for bearer tokens; cookie mutations need
//!   CSRF, bearer requests are exempt (ADR 0005);
//! - every create/update is secret-scanned: findings reject the
//!   write with 422 (kinds + line numbers, never values). Danger
//!   lints are returned as warnings only.
//!
//! Rename policy: renaming records a redirect from the old name
//! for 30 days; reads (GET detail/versions/diff/raw) resolve it,
//! writes to the old name do not. Creating a new snippet with the
//! old name shadows the redirect. A soft-deleted snippet's name
//! stays reserved until the purge (30 days), so a deleted name
//! cannot be instantly re-registered by anyone, including its
//! former owner.

use crate::auth::{body, verify_csrf, AuthKind, ClientIp, CurrentUser, RequireUser};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::{SnippetRow, UserRow, VersionRow};
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

// ------------------------------------------------------------ submission

/// A parsed, validated, scan-clean submission ready to store.
#[derive(Debug)]
struct ParsedInput {
    /// Canonical forge-file YAML (what gets stored).
    canonical_body: String,
    /// Content hash (spec §9) of the parsed snippet.
    hash: String,
    /// Final description (request field wins over the file's).
    description: String,
    /// Final tags (request field wins over the file's).
    tags: Vec<String>,
    /// `script` or `steps`.
    mode: &'static str,
}

/// Does this text look like an (attempted) forge file rather than
/// a plain shell script? Used to decide between "wrap as script"
/// and "report the YAML error".
fn looks_like_forge_file(body: &str) -> bool {
    body.lines().take(40).any(|line| {
        let line = line.trim_end();
        [
            "name:",
            "description:",
            "tags:",
            "mode:",
            "steps:",
            "script:",
            "variables:",
        ]
        .iter()
        .any(|key| line.starts_with(key))
    })
}

/// Parse + validate + secret-scan a submitted body. `name` is the
/// authoritative snippet name (path/field); a forge file naming a
/// different snippet is rejected.
///
/// # Errors
/// 400 validation, 413 body too large, 422 secret findings.
fn parse_submission(
    name: &str,
    description: Option<&str>,
    tags: Option<&[String]>,
    mode_hint: Option<&str>,
    submitted: &str,
) -> Result<ParsedInput, ApiError> {
    if submitted.len() > 65_536 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "body_too_large",
            "Snippet body must be at most 64 KiB",
        ));
    }
    let mut snippet = match forge_core::Snippet::from_yaml(submitted) {
        Ok(parsed) => {
            if parsed.name != name {
                return Err(ApiError::validation(format!(
                    "The forge file names itself `{}` but the request names `{name}`",
                    parsed.name
                )));
            }
            parsed
        }
        Err(err) => {
            if mode_hint == Some("script") || !looks_like_forge_file(submitted) {
                // Plain script text: wrap it (spec: server wraps).
                let mut wrapped = forge_core::Snippet::new(name);
                wrapped.mode = Some(forge_core::Mode::Script);
                wrapped.script = Some(submitted.to_string());
                wrapped
            } else {
                return Err(ApiError::validation(format!("Invalid forge file: {err}")));
            }
        }
    };
    if let Some(hint) = mode_hint {
        let actual = match snippet.effective_mode() {
            forge_core::Mode::Script => "script",
            forge_core::Mode::Steps => "steps",
        };
        if hint != actual {
            return Err(ApiError::validation(format!(
                "mode `{hint}` does not match the body (which is {actual})"
            )));
        }
    }
    if let Some(description) = description {
        snippet.description = description.to_string();
    }
    if let Some(tags) = tags {
        snippet.tags = forge_core::names::normalize_tags(tags)
            .map_err(|e| ApiError::validation(e.to_string()))?;
    }
    snippet
        .validate()
        .map_err(|e| ApiError::validation(e.to_string()))?;
    // Secret scan over the *submitted* text, so line numbers match
    // the file the user sent. Findings carry kind + line only.
    let findings = forge_core::scan::scan(submitted);
    if !findings.is_empty() {
        let list: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| serde_json::json!({ "kind": f.kind.as_str(), "line": f.line }))
            .collect();
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret_detected",
            "The snippet appears to contain secrets; remove them before saving",
        )
        .with_details(serde_json::json!({ "findings": list })));
    }
    let canonical_body = snippet.to_yaml().map_err(|_| ApiError::internal())?;
    Ok(ParsedInput {
        canonical_body,
        hash: forge_core::hash::content_hash(&snippet),
        description: snippet.description.clone(),
        tags: snippet.tags.clone(),
        mode: match snippet.effective_mode() {
            forge_core::Mode::Script => "script",
            forge_core::Mode::Steps => "steps",
        },
    })
}

/// Parse a stored canonical body back into a summary. Stored
/// bodies were validated on write; a failure here means data
/// corruption, hence 500.
fn summary_json(body: &str) -> Result<serde_json::Value, ApiError> {
    let snippet = forge_core::Snippet::from_yaml(body).map_err(|_| ApiError::internal())?;
    let warnings = forge_core::lint::lint_snippet(&snippet);
    let uses_sudo = warnings
        .iter()
        .any(|w| w.kind == forge_core::lint::LintKind::Sudo);
    Ok(serde_json::json!({
        "mode": match snippet.effective_mode() {
            forge_core::Mode::Script => "script",
            forge_core::Mode::Steps => "steps",
        },
        "steps_count": snippet.command_count(),
        "variables": snippet.referenced_variables(),
        "uses_sudo": uses_sudo,
        "lint_warnings": warnings.iter().map(|w| serde_json::json!({
            "kind": w.kind.as_str(),
            "line": w.line,
            "message": w.kind.message(),
        })).collect::<Vec<_>>(),
    }))
}

// ------------------------------------------------------------ responses

/// The openapi `Snippet` shape for one version of a snippet.
/// Render a snippet from pre-fetched parts (Step 20: page
/// handlers hydrate a whole page with a handful of bulk
/// queries instead of ~5 queries per snippet).
pub(crate) fn snippet_json_parts(
    snippet: &SnippetRow,
    version: &VersionRow,
    owner_username: &str,
    tags: &[String],
    starred_by_me: bool,
    forked_from: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    Ok(serde_json::json!({
        "owner": owner_username,
        "name": snippet.name,
        "description": snippet.description,
        "visibility": snippet.visibility,
        "tags": tags,
        "version": version.version,
        "hash": version.hash,
        "short_hash": version.hash[..8.min(version.hash.len())],
        "body": version.body,
        "message": version.message,
        "summary": summary_json(&version.body)?,
        "star_count": snippet.star_count,
        "fork_count": snippet.fork_count,
        "pull_count": snippet.pull_count,
        "forked_from": forked_from,
        "starred_by_me": starred_by_me,
        "created_at": snippet.created_at,
        "updated_at": snippet.updated_at,
    }))
}

/// The `forked_from` provenance object for one snippet, given
/// its (already fetched) source snippet and source owner name.
pub(crate) fn forked_from_json(
    snippet: &SnippetRow,
    source: &SnippetRow,
    source_owner_username: &str,
) -> serde_json::Value {
    serde_json::json!({
        "owner": source_owner_username,
        "name": source.name,
        "version": snippet.forked_from_version,
        "hash": snippet.forked_from_hash,
    })
}

pub(crate) async fn snippet_json(
    state: &AppState,
    snippet: &SnippetRow,
    version: &VersionRow,
    viewer: Option<&UserRow>,
) -> Result<serde_json::Value, ApiError> {
    let owner = state
        .repos
        .users
        .by_id(snippet.owner_id)
        .await?
        .ok_or_else(ApiError::internal)?;
    let tags = state.repos.snippets.tags(snippet.id).await?;
    let starred_by_me = match viewer {
        Some(user) => {
            state
                .repos
                .snippets
                .has_starred(user.id, snippet.id)
                .await?
        }
        None => false,
    };
    let forked_from = match snippet.forked_from_snippet_id {
        Some(source_id) => match state.repos.snippets.by_id(source_id).await? {
            Some(source) => {
                let source_owner = state
                    .repos
                    .users
                    .by_id(source.owner_id)
                    .await?
                    .ok_or_else(ApiError::internal)?;
                Some(forked_from_json(snippet, &source, &source_owner.username))
            }
            None => None,
        },
        None => None,
    };
    snippet_json_parts(
        snippet,
        version,
        &owner.username,
        &tags,
        starred_by_me,
        forked_from.as_ref(),
    )
}

fn etag_matches(headers: &HeaderMap, hash: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == "*" || v.trim().trim_matches('"') == hash)
}

// --------------------------------------------------------- authorisation

/// The outcome of resolving `/{owner}/{name}`.
pub(crate) struct Resolved {
    /// The resolved live snippet.
    pub(crate) snippet: SnippetRow,
    /// The old name the caller used, when a rename redirect was
    /// followed (reads only).
    pub(crate) renamed_from: Option<String>,
}

pub(crate) fn not_found() -> ApiError {
    ApiError::not_found("snippet_not_found", "Snippet not found")
}

/// Resolve owner + name to a live snippet. With `follow_rename`,
/// an unexpired rename redirect is honoured when no live snippet
/// carries the name.
pub(crate) async fn resolve(
    state: &AppState,
    owner_name: &str,
    name: &str,
    follow_rename: bool,
) -> Result<Resolved, ApiError> {
    let owner = state
        .repos
        .users
        .by_username(owner_name)
        .await?
        .filter(|u| u.deleted_at.is_none())
        .ok_or_else(not_found)?;
    if let Some(snippet) = state
        .repos
        .snippets
        .by_owner_name(owner.id, name)
        .await?
        .filter(|s| s.deleted_at.is_none())
    {
        return Ok(Resolved {
            snippet,
            renamed_from: None,
        });
    }
    if follow_rename {
        if let Some(snippet_id) = state
            .repos
            .snippets
            .resolve_rename(owner.id, name, &now_iso())
            .await?
        {
            if let Some(snippet) = state
                .repos
                .snippets
                .by_id(snippet_id)
                .await?
                .filter(|s| s.deleted_at.is_none())
            {
                return Ok(Resolved {
                    snippet,
                    renamed_from: Some(name.to_string()),
                });
            }
        }
    }
    Err(not_found())
}

/// Read access: public/unlisted for everyone; private (and
/// admin-hidden) for the owner only.
fn can_read(snippet: &SnippetRow, viewer: Option<&UserRow>) -> bool {
    let is_owner = viewer.is_some_and(|u| u.id == snippet.owner_id);
    if is_owner {
        return true;
    }
    if snippet.hidden_at.is_some() {
        return false;
    }
    snippet.visibility == "public" || snippet.visibility == "unlisted"
}

/// Resolve + authorise a read, mapping every failure to the same
/// 404 so private existence never leaks.
pub(crate) async fn resolve_readable(
    state: &AppState,
    owner: &str,
    name: &str,
    viewer: Option<&UserRow>,
) -> Result<Resolved, ApiError> {
    let resolved = resolve(state, owner, name, true).await?;
    if !can_read(&resolved.snippet, viewer) {
        return Err(not_found());
    }
    Ok(resolved)
}

/// Gate a write: caller must be the owner (404 otherwise),
/// bearer tokens need the `write` scope, cookie sessions need
/// CSRF. Sessions act with full authority.
fn authorise_write(
    ctx: &RequireUser,
    headers: &HeaderMap,
    snippet: &SnippetRow,
) -> Result<(), ApiError> {
    if ctx.0.user.id != snippet.owner_id {
        return Err(not_found());
    }
    check_write_scope_and_csrf(ctx, headers)
}

/// Scope + CSRF gate shared by writes that have no snippet yet
/// (create) and those that do.
pub(crate) fn check_write_scope_and_csrf(
    ctx: &RequireUser,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    match &ctx.0.kind {
        AuthKind::Session(session) => verify_csrf(headers, session),
        AuthKind::Token(_) => {
            if ctx.0.kind.has_scope("write") {
                Ok(())
            } else {
                Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "insufficient_scope",
                    "This token has the read scope only",
                ))
            }
        }
    }
}

fn validate_visibility(value: &str) -> Result<(), ApiError> {
    match value {
        "private" | "unlisted" | "public" => Ok(()),
        other => Err(ApiError::validation(format!(
            "Unknown visibility `{other}` (expected private, unlisted or public)"
        ))),
    }
}

// -------------------------------------------------------------- handlers

#[derive(Deserialize)]
struct SnippetInput {
    name: String,
    description: Option<String>,
    visibility: Option<String>,
    tags: Option<Vec<String>>,
    mode: Option<String>,
    body: String,
    message: Option<String>,
}

async fn create_snippet(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    result: Result<Json<SnippetInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let name = req.name.trim().to_string();
    forge_core::names::validate_snippet_name(&name)
        .map_err(|e| ApiError::validation(e.to_string()))?;
    let visibility = req.visibility.as_deref().unwrap_or("private");
    validate_visibility(visibility)?;
    if let Some(description) = &req.description {
        if description.chars().count() > 280 {
            return Err(ApiError::validation(
                "Description must be at most 280 characters",
            ));
        }
    }
    let count = state.repos.snippets.count_for_owner(ctx.0.user.id).await?;
    if count >= state.config.max_snippets_per_user {
        return Err(ApiError::validation(format!(
            "Snippet limit reached ({} per user)",
            state.config.max_snippets_per_user
        )));
    }
    let parsed = parse_submission(
        &name,
        req.description.as_deref(),
        req.tags.as_deref(),
        req.mode.as_deref(),
        &req.body,
    )?;
    let now = now_iso();
    let (snippet, version) = match state
        .repos
        .snippets
        .create_with_version(
            ctx.0.user.id,
            &name,
            &parsed.description,
            visibility,
            parsed.mode,
            &parsed.canonical_body,
            &parsed.hash,
            req.message.as_deref(),
            &parsed.tags,
            &now,
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
    let mut resp = (StatusCode::CREATED, Json(json)).into_response();
    resp.headers_mut().insert(
        header::ETAG,
        format!("\"{}\"", version.hash)
            .parse()
            .unwrap_or(header::HeaderValue::from_static("")),
    );
    Ok(resp)
}

#[derive(Deserialize)]
struct GetQuery {
    version: Option<i64>,
    intent: Option<String>,
}

async fn get_snippet(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    client_ip: ClientIp,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    Query(query): Query<GetQuery>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let mut resolved = resolve_readable(&state, &owner, &name, viewer_user.as_ref()).await?;
    // Pull counting (Step 10): a public snippet fetched with
    // intent=run by a non-owner counts once per IP per hour. It
    // is a popularity number only — nothing per-user is stored.
    if query.intent.as_deref() == Some("run")
        && resolved.snippet.visibility == "public"
        && viewer_user.as_ref().map(|u| u.id) != Some(resolved.snippet.owner_id)
    {
        let ip_text = client_ip.0.map_or(String::new(), |addr| addr.to_string());
        let ip_hash = forge_core::hash::sha256_hex(ip_text.as_bytes());
        if state.should_count_pull(&ip_hash, resolved.snippet.id) {
            state
                .repos
                .snippets
                .increment_pull(resolved.snippet.id)
                .await?;
            resolved.snippet.pull_count += 1;
        }
    }
    let version_no = query.version.unwrap_or(resolved.snippet.latest_version);
    let version = state
        .repos
        .snippets
        .version(resolved.snippet.id, version_no)
        .await?
        .ok_or_else(not_found)?;
    if etag_matches(&headers, &version.hash) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let mut json = snippet_json(&state, &resolved.snippet, &version, viewer_user.as_ref()).await?;
    if let Some(old) = &resolved.renamed_from {
        json["renamed_from"] = serde_json::Value::String(old.clone());
    }
    let mut resp = Json(json).into_response();
    resp.headers_mut().insert(
        header::ETAG,
        format!("\"{}\"", version.hash)
            .parse()
            .unwrap_or(header::HeaderValue::from_static("")),
    );
    Ok(resp)
}

/// Optimistic concurrency for PUT (openapi: `If-Match` is a
/// required quoted version number). The value is re-checked
/// inside the append transaction as well; this early check just
/// fails fast and reports the current latest version.
fn require_if_match(headers: &HeaderMap, latest: i64) -> Result<i64, ApiError> {
    let if_match = headers
        .get("if-match")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().trim_matches('"').parse::<i64>().ok())
        .ok_or_else(|| {
            ApiError::validation("An If-Match header with the latest version number is required")
        })?;
    if if_match != latest {
        return Err(ApiError::new(
            StatusCode::PRECONDITION_FAILED,
            "version_conflict",
            "The snippet changed since you read it; re-read and retry",
        )
        .with_details(serde_json::json!({ "latest_version": latest })));
    }
    Ok(if_match)
}

async fn put_snippet(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    result: Result<Json<SnippetInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    let resolved = resolve(&state, &owner, &name, false).await?;
    authorise_write(&ctx, &headers, &resolved.snippet)?;
    let if_match = require_if_match(&headers, resolved.snippet.latest_version)?;
    let req = body(result)?;
    if req.name.trim() != name {
        return Err(ApiError::validation(
            "The body's name must match the path (rename via PATCH)",
        ));
    }
    let parsed = parse_submission(
        &name,
        req.description.as_deref(),
        req.tags.as_deref(),
        req.mode.as_deref(),
        &req.body,
    )?;
    let latest = state
        .repos
        .snippets
        .latest_version(resolved.snippet.id)
        .await?
        .ok_or_else(ApiError::internal)?;
    let snippet_row = resolved.snippet.clone();
    if parsed.hash == latest.hash {
        let mut json = snippet_json(&state, &snippet_row, &latest, Some(&ctx.0.user)).await?;
        json["unchanged"] = serde_json::Value::Bool(true);
        return Ok(Json(json).into_response());
    }
    let versions = state.repos.snippets.versions(snippet_row.id).await?;
    if i64::try_from(versions.len()).unwrap_or(i64::MAX) >= state.config.max_versions_per_snippet {
        return Err(ApiError::validation(format!(
            "Version limit reached ({} per snippet)",
            state.config.max_versions_per_snippet
        )));
    }
    if let Some(message) = &req.message {
        if message.chars().count() > 200 {
            return Err(ApiError::validation(
                "Version message must be at most 200 characters",
            ));
        }
    }
    let version = state
        .repos
        .snippets
        .add_version(
            snippet_row.id,
            parsed.mode,
            &parsed.canonical_body,
            &parsed.hash,
            req.message.as_deref(),
            &parsed.tags,
            &now_iso(),
            Some(if_match),
        )
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::PRECONDITION_FAILED,
                "version_conflict",
                "The snippet changed since you read it; re-read and retry",
            )
            .with_details(serde_json::json!({
                "latest_version": resolved.snippet.latest_version,
            }))
        })?;
    // add_version may also change description-bearing metadata? No:
    // description lives on the snippet row and PUT bodies carry a
    // full file; apply the parsed description/tags via metadata
    // only when they differ, without creating another version.
    let current_tags = state.repos.snippets.tags(snippet_row.id).await?;
    if parsed.description != snippet_row.description || parsed.tags != current_tags {
        state
            .repos
            .snippets
            .update_metadata(
                snippet_row.id,
                &parsed.description,
                &snippet_row.visibility,
                &parsed.tags,
                &now_iso(),
            )
            .await?;
    }
    let fresh = state
        .repos
        .snippets
        .by_id(snippet_row.id)
        .await?
        .ok_or_else(ApiError::internal)?;
    let mut json = snippet_json(&state, &fresh, &version, Some(&ctx.0.user)).await?;
    json["unchanged"] = serde_json::Value::Bool(false);
    Ok(Json(json).into_response())
}

#[derive(Deserialize)]
struct PatchSnippet {
    description: Option<String>,
    visibility: Option<String>,
    tags: Option<Vec<String>>,
    name: Option<String>,
}

async fn patch_snippet(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    result: Result<Json<PatchSnippet>, JsonRejection>,
) -> Result<Response, ApiError> {
    let resolved = resolve(&state, &owner, &name, false).await?;
    authorise_write(&ctx, &headers, &resolved.snippet)?;
    let req = body(result)?;
    let snippet = resolved.snippet;
    let description = req.description.as_deref().unwrap_or(&snippet.description);
    if description.chars().count() > 280 {
        return Err(ApiError::validation(
            "Description must be at most 280 characters",
        ));
    }
    let visibility = req.visibility.as_deref().unwrap_or(&snippet.visibility);
    validate_visibility(visibility)?;
    let tags = match &req.tags {
        Some(tags) => forge_core::names::normalize_tags(tags)
            .map_err(|e| ApiError::validation(e.to_string()))?,
        None => state.repos.snippets.tags(snippet.id).await?,
    };
    state
        .repos
        .snippets
        .update_metadata(snippet.id, description, visibility, &tags, &now_iso())
        .await?;
    if let Some(new_name) = &req.name {
        let new_name = new_name.trim().to_string();
        if new_name != snippet.name {
            forge_core::names::validate_snippet_name(&new_name)
                .map_err(|e| ApiError::validation(e.to_string()))?;
            if state
                .repos
                .snippets
                .by_owner_name(ctx.0.user.id, &new_name)
                .await?
                .is_some_and(|s| s.deleted_at.is_none())
            {
                return Err(ApiError::conflict(
                    "name_taken",
                    "You already have a snippet with that name",
                ));
            }
            let expires = crate::auth::fmt_ts(chrono::Utc::now() + chrono::Duration::days(30));
            match state
                .repos
                .snippets
                .rename(
                    snippet.id,
                    ctx.0.user.id,
                    &snippet.name,
                    &new_name,
                    &expires,
                    &now_iso(),
                )
                .await
            {
                Ok(()) => {}
                Err(crate::repo::RepoError::Conflict(_)) => {
                    return Err(ApiError::conflict(
                        "name_taken",
                        "You already have a snippet with that name",
                    ));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    let fresh = state
        .repos
        .snippets
        .by_id(snippet.id)
        .await?
        .ok_or_else(ApiError::internal)?;
    let latest = state
        .repos
        .snippets
        .latest_version(snippet.id)
        .await?
        .ok_or_else(ApiError::internal)?;
    Ok(Json(snippet_json(&state, &fresh, &latest, Some(&ctx.0.user)).await?).into_response())
}

async fn delete_snippet(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let resolved = resolve(&state, &owner, &name, false).await?;
    authorise_write(&ctx, &headers, &resolved.snippet)?;
    state
        .repos
        .snippets
        .soft_delete(resolved.snippet.id, &now_iso())
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ------------------------------------------------------- versions & diff

#[derive(Deserialize)]
struct PageQuery {
    limit: Option<i64>,
    cursor: Option<String>,
    visibility: Option<String>,
    tag: Option<String>,
    sort: Option<String>,
}

/// Cursors are opaque to clients: base64url of the decimal offset.
pub(crate) fn encode_cursor(offset: i64) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    use std::fmt::Write as _;
    let raw = offset.to_string();
    let mut out = String::new();
    let bytes = raw.as_bytes();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let _ = write!(out, "{}", ALPHABET[(n >> 18) as usize & 63] as char);
        let _ = write!(out, "{}", ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            let _ = write!(out, "{}", ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            let _ = write!(out, "{}", ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

pub(crate) fn decode_cursor(cursor: &str) -> Result<i64, ApiError> {
    let mut values = Vec::new();
    for c in cursor.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(ApiError::validation("Invalid cursor")),
        };
        values.push(v);
    }
    let mut bytes = Vec::new();
    for chunk in values.chunks(4) {
        let mut n: u32 = 0;
        for (i, v) in chunk.iter().enumerate() {
            n |= u32::from(*v) << (18 - 6 * i);
        }
        // n holds 24 bits; each masked byte fits a u8 by
        // construction.
        bytes.push(u8::try_from((n >> 16) & 0xFF).unwrap_or(0));
        if chunk.len() > 2 {
            bytes.push(u8::try_from((n >> 8) & 0xFF).unwrap_or(0));
        }
        if chunk.len() > 3 {
            bytes.push(u8::try_from(n & 0xFF).unwrap_or(0));
        }
    }
    String::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|n| *n >= 0)
        .ok_or_else(|| ApiError::validation("Invalid cursor"))
}

pub(crate) fn page_limit(limit: Option<i64>) -> Result<i64, ApiError> {
    match limit {
        Some(n) if (1..=100).contains(&n) => Ok(n),
        Some(_) => Err(ApiError::validation("limit must be between 1 and 100")),
        None => Ok(20),
    }
}

async fn list_versions(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Path((owner, name)): Path<(String, String)>,
    Query(query): Query<PageQuery>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let resolved = resolve_readable(&state, &owner, &name, viewer_user.as_ref()).await?;
    let limit = page_limit(query.limit)?;
    let offset = query
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    let all = state.repos.snippets.versions(resolved.snippet.id).await?;
    let total = i64::try_from(all.len()).unwrap_or(i64::MAX);
    let page: Vec<serde_json::Value> = all
        .iter()
        .skip(usize::try_from(offset).unwrap_or(0))
        .take(usize::try_from(limit).unwrap_or(0))
        .map(|v| {
            serde_json::json!({
                "version": v.version,
                "hash": v.hash,
                "short_hash": v.hash[..8.min(v.hash.len())],
                "message": v.message,
                "created_at": v.created_at,
            })
        })
        .collect();
    let next = offset + limit;
    Ok(Json(serde_json::json!({
        "versions": page,
        "next_cursor": if next < total { Some(encode_cursor(next)) } else { None },
    }))
    .into_response())
}

async fn get_version(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Path((owner, name, version_no)): Path<(String, String, i64)>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let resolved = resolve_readable(&state, &owner, &name, viewer_user.as_ref()).await?;
    let version = state
        .repos
        .snippets
        .version(resolved.snippet.id, version_no)
        .await?
        .ok_or_else(not_found)?;
    Ok(
        Json(snippet_json(&state, &resolved.snippet, &version, viewer_user.as_ref()).await?)
            .into_response(),
    )
}

#[derive(Deserialize)]
struct DiffQuery {
    from: i64,
    to: i64,
}

async fn diff_versions(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Path((owner, name)): Path<(String, String)>,
    Query(query): Query<DiffQuery>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let resolved = resolve_readable(&state, &owner, &name, viewer_user.as_ref()).await?;
    let from = state
        .repos
        .snippets
        .version(resolved.snippet.id, query.from)
        .await?
        .ok_or_else(not_found)?;
    let to = state
        .repos
        .snippets
        .version(resolved.snippet.id, query.to)
        .await?
        .ok_or_else(not_found)?;
    let diff = similar::TextDiff::from_lines(&from.body, &to.body);
    let text = diff
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("{owner}/{name} v{}", query.from),
            &format!("{owner}/{name} v{}", query.to),
        )
        .to_string();
    if text.len() > 256 * 1024 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "diff_too_large",
            "The diff is larger than 256 KiB; fetch the versions and diff locally",
        ));
    }
    Ok(
        Json(serde_json::json!({ "from": query.from, "to": query.to, "diff": text }))
            .into_response(),
    )
}

async fn get_raw(
    State(state): State<AppState>,
    CurrentUser(viewer): CurrentUser,
    Path((owner, name)): Path<(String, String)>,
    Query(query): Query<GetQuery>,
) -> Result<Response, ApiError> {
    let viewer_user = viewer.as_ref().map(|c| c.user.clone());
    let resolved = resolve_readable(&state, &owner, &name, viewer_user.as_ref()).await?;
    let version_no = query.version.unwrap_or(resolved.snippet.latest_version);
    let version = state
        .repos
        .snippets
        .version(resolved.snippet.id, version_no)
        .await?
        .ok_or_else(not_found)?;
    Ok((
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        version.body,
    )
        .into_response())
}

// ------------------------------------------------------------ me/snippets

async fn my_snippets(
    State(state): State<AppState>,
    ctx: RequireUser,
    Query(query): Query<PageQuery>,
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
    if let Some(visibility) = &query.visibility {
        validate_visibility(visibility)?;
    }
    let sort = query.sort.as_deref().unwrap_or("updated");
    if !["updated", "created", "name"].contains(&sort) {
        return Err(ApiError::validation(
            "sort must be updated, created or name",
        ));
    }
    let rows = state
        .repos
        .snippets
        .list_for_owner(
            ctx.0.user.id,
            query.visibility.as_deref(),
            query.tag.as_deref(),
            sort,
            limit + 1,
            offset,
        )
        .await?;
    let has_more = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
    let mut items = Vec::new();
    for snippet in rows.iter().take(usize::try_from(limit).unwrap_or(0)) {
        let latest = state
            .repos
            .snippets
            .latest_version(snippet.id)
            .await?
            .ok_or_else(ApiError::internal)?;
        items.push(snippet_json(&state, snippet, &latest, Some(&ctx.0.user)).await?);
    }
    Ok(Json(serde_json::json!({
        "snippets": items,
        "next_cursor": if has_more { Some(encode_cursor(offset + limit)) } else { None },
    }))
    .into_response())
}

// --------------------------------------------------------------- validate

async fn validate_snippet(
    result: Result<Json<SnippetInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    let req = body(result)?;
    let name = req.name.trim().to_string();
    if let Err(e) = forge_core::names::validate_snippet_name(&name) {
        return Ok(Json(serde_json::json!({
            "valid": false,
            "errors": [{ "message": e.to_string() }],
        }))
        .into_response());
    }
    match parse_submission(
        &name,
        req.description.as_deref(),
        req.tags.as_deref(),
        req.mode.as_deref(),
        &req.body,
    ) {
        Ok(parsed) => {
            let summary = summary_json(&parsed.canonical_body)?;
            Ok(Json(serde_json::json!({
                "valid": true,
                "errors": [],
                "hash": parsed.hash,
                "summary": summary,
            }))
            .into_response())
        }
        Err(err) if err.status == StatusCode::UNPROCESSABLE_ENTITY => {
            let findings = err
                .details
                .as_ref()
                .and_then(|d| d.get("findings").cloned())
                .unwrap_or(serde_json::Value::Array(vec![]));
            let mut resp = Json(serde_json::json!({
                "valid": false,
                "errors": [{ "message": err.message }],
                "secret_findings": findings,
            }))
            .into_response();
            *resp.status_mut() = StatusCode::UNPROCESSABLE_ENTITY;
            Ok(resp)
        }
        Err(err) => Ok(Json(serde_json::json!({
            "valid": false,
            "errors": [{ "message": err.message }],
        }))
        .into_response()),
    }
}

/// Snippet routes (merged under `/api/v1`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/snippets", post(create_snippet))
        .route("/snippets/validate", post(validate_snippet))
        .route(
            "/snippets/{owner}/{name}",
            get(get_snippet)
                .put(put_snippet)
                .patch(patch_snippet)
                .delete(delete_snippet),
        )
        .route("/snippets/{owner}/{name}/versions", get(list_versions))
        .route(
            "/snippets/{owner}/{name}/versions/{version}",
            get(get_version),
        )
        .route("/snippets/{owner}/{name}/diff", get(diff_versions))
        .route("/snippets/{owner}/{name}/raw", get(get_raw))
        .route("/me/snippets", get(my_snippets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_roundtrip() {
        for n in [0, 1, 20, 999, 1_000_000] {
            assert_eq!(decode_cursor(&encode_cursor(n)).unwrap(), n);
        }
        assert!(decode_cursor("!!!").is_err());
        assert!(decode_cursor("").is_err());
    }

    #[test]
    fn forge_file_detection() {
        assert!(looks_like_forge_file("name: x\nsteps:\n  - echo hi\n"));
        assert!(looks_like_forge_file("script: |\n  echo hi\n"));
        assert!(!looks_like_forge_file("#!/bin/bash\napt update\n"));
        assert!(!looks_like_forge_file("echo names: stuff\n"));
    }

    #[test]
    fn submission_wraps_plain_scripts_and_hashes_stably() {
        let parsed = parse_submission("demo", None, None, None, "echo hello\n").unwrap();
        assert_eq!(parsed.mode, "script");
        assert!(parsed.canonical_body.contains("echo hello"));
        // The hash equals forge-core's own hash of the same file —
        // the CLI will compute the identical value.
        let snippet = forge_core::Snippet::from_yaml(&parsed.canonical_body).unwrap();
        assert_eq!(parsed.hash, forge_core::hash::content_hash(&snippet));
    }

    #[test]
    fn submission_rejects_secrets_with_findings() {
        let body = format!("export TOKEN={}\n", "q".repeat(24));
        let err = parse_submission("demo", None, None, None, &body).unwrap_err();
        assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.code, "secret_detected");
    }
}
