//! Per-user activity log (Peak C7): an append-only record of
//! account-relevant events - snippets created, pushed and
//! forked, tokens minted and revoked, sign-ins (flagged when
//! the device has not been seen before), and trust grants the
//! CLI reports after a confirmed run. Visible to the owner
//! only, in Settings - Security and via `fg activity`, and
//! exportable as JSON.
//!
//! Recording is best-effort by design: a failed insert is
//! logged and swallowed, never allowed to fail the operation
//! that caused the event.
//!
//! Endpoints:
//! - `GET  /me/activity`         own events, newest first
//! - `GET  /me/activity/export`  the same as a JSON download
//! - `POST /me/activity`         client-reported events (an
//!   allow-list: only `snippet_trusted`, which only the CLI
//!   can know about)

use crate::auth::{body, RequireUser};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::ActivityRow;
use crate::snippets::check_write_scope_and_csrf;
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

/// Kinds a client may report. Everything else is recorded by
/// the server itself at the point the event happens.
const CLIENT_KINDS: [&str; 1] = ["snippet_trusted"];
/// Cap on a client-reported detail payload.
const MAX_DETAIL_BYTES: usize = 512;
/// Default page size for the list endpoint.
const DEFAULT_LIMIT: i64 = 50;
/// Hard cap for the list endpoint.
const MAX_LIMIT: i64 = 200;
/// Hard cap for the export.
const EXPORT_LIMIT: i64 = 1000;

/// Record an event, swallowing (but logging) any failure.
pub(crate) async fn record(state: &AppState, user_id: i64, kind: &str, detail: serde_json::Value) {
    record_with_ua(state, user_id, kind, detail, "").await;
}

/// Record an event with a user agent (login events).
pub(crate) async fn record_with_ua(
    state: &AppState,
    user_id: i64,
    kind: &str,
    detail: serde_json::Value,
    user_agent: &str,
) {
    let detail_text = detail.to_string();
    if let Err(e) = state
        .repos
        .activity
        .record(user_id, kind, &detail_text, user_agent, &now_iso())
        .await
    {
        tracing::warn!(error = %e, kind, "activity record failed");
    }
}

fn event_json(row: &ActivityRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "kind": row.kind,
        "detail": serde_json::from_str::<serde_json::Value>(&row.detail)
            .unwrap_or(serde_json::Value::Null),
        "created_at": row.created_at,
    })
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<i64>,
}

async fn list_activity(
    State(state): State<AppState>,
    ctx: RequireUser,
    Query(query): Query<ListQuery>,
) -> Result<Response, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let rows = state
        .repos
        .activity
        .list_for_user(ctx.0.user.id, limit)
        .await?;
    let events: Vec<serde_json::Value> = rows.iter().map(event_json).collect();
    Ok(Json(serde_json::json!({ "events": events })).into_response())
}

async fn export_activity(
    State(state): State<AppState>,
    ctx: RequireUser,
) -> Result<Response, ApiError> {
    let rows = state
        .repos
        .activity
        .list_for_user(ctx.0.user.id, EXPORT_LIMIT)
        .await?;
    let events: Vec<serde_json::Value> = rows.iter().map(event_json).collect();
    let doc = serde_json::json!({
        "user": ctx.0.user.username,
        "exported_at": now_iso(),
        "events": events,
    });
    let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"forgehub-activity.json\"",
            ),
        ],
        text,
    )
        .into_response())
}

#[derive(Deserialize)]
struct ReportInput {
    kind: String,
    detail: serde_json::Value,
}

async fn report_activity(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    result: Result<Json<ReportInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    if !CLIENT_KINDS.contains(&req.kind.as_str()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unreportable_kind",
            "Clients may only report these event kinds: snippet_trusted",
        ));
    }
    let refr = req.detail.get("ref").and_then(|v| v.as_str()).unwrap_or("");
    if refr.is_empty() || refr.len() > 200 || !req.detail.is_object() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_detail",
            "A trust report needs a detail object with a snippet ref",
        ));
    }
    if req.detail.to_string().len() > MAX_DETAIL_BYTES {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "detail_too_large",
            "The event detail is too large",
        ));
    }
    record(&state, ctx.0.user.id, &req.kind, req.detail).await;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "recorded": true })),
    )
        .into_response())
}

/// Activity routes (merged into the API router).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/activity", get(list_activity).post(report_activity))
        .route("/me/activity/export", get(export_activity))
}
