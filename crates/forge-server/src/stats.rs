//! Usage stats the user owns (Peak C8): the CLI reports each
//! completed cloud run here (best-effort, online only), and
//! the owner reads the aggregates back - in the web app's
//! "Your library" summary and via the API. Rows belong to the
//! runner: a user's stats count ONLY their own runs, never
//! other people's runs of their snippets. Local runs are
//! never reported; they live in the CLI's local history
//! database and power `fg stats`, and never leave the machine.
//!
//! Endpoints:
//! - `POST /me/runs`   report one completed cloud run
//! - `GET  /me/stats`  own aggregates (totals + per-snippet)

use crate::auth::{body, RequireUser};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::snippets::check_write_scope_and_csrf;
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

/// How many per-snippet rows the summary returns.
const STATS_LIMIT: i64 = 50;
/// Validation caps for a run report (a day-long run of a
/// huge snippet is beyond plausible; anything past this is
/// a bug or an attack, not data).
const MAX_DURATION_MS: i64 = 86_400_000;
const MAX_COMMANDS: i64 = 100_000;
const MAX_SAVED_MS: i64 = 1_000_000_000_000;

#[derive(Deserialize)]
struct RunReport {
    #[serde(rename = "ref")]
    refr: String,
    version: Option<i64>,
    commands: i64,
    duration_ms: i64,
    success: bool,
    estimated_saved_ms: i64,
}

async fn report_run(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    result: Result<Json<RunReport>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let valid_ref = {
        let mut parts = req.refr.split('/');
        matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(o), Some(n), None) if !o.is_empty() && !n.is_empty()
        )
    };
    if !valid_ref || req.refr.len() > 200 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_ref",
            "A run report needs a snippet ref as owner/name",
        ));
    }
    if !(0..=MAX_COMMANDS).contains(&req.commands)
        || !(0..=MAX_DURATION_MS).contains(&req.duration_ms)
        || !(0..=MAX_SAVED_MS).contains(&req.estimated_saved_ms)
        || req.version.is_some_and(|v| !(0..=1_000_000).contains(&v))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_report",
            "The run report's numbers are out of range",
        ));
    }
    state
        .repos
        .run_stats
        .record_run_report(
            ctx.0.user.id,
            &req.refr,
            req.version.unwrap_or(0),
            req.commands,
            req.duration_ms,
            req.success,
            req.estimated_saved_ms,
            &now_iso(),
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "recorded": true })),
    )
        .into_response())
}

async fn my_stats(State(state): State<AppState>, ctx: RequireUser) -> Result<Response, ApiError> {
    // Fetch well past the display limit so the totals cover
    // every workflow, then show only the top rows.
    let rows = state
        .repos
        .run_stats
        .stats_for_user(ctx.0.user.id, 10_000)
        .await?;
    let workflows: Vec<serde_json::Value> = rows
        .iter()
        .take(usize::try_from(STATS_LIMIT).unwrap_or(50))
        .map(|r| {
            serde_json::json!({
                "ref": r.ref_,
                "runs": r.runs,
                "commands": r.commands,
                "total_ms": r.total_ms,
                "failures": r.failures,
                "estimated_saved_ms": r.saved_ms,
                "last_run": r.last_run,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "runs": rows.iter().map(|r| r.runs).sum::<i64>(),
        "commands": rows.iter().map(|r| r.commands).sum::<i64>(),
        "total_ms": rows.iter().map(|r| r.total_ms).sum::<i64>(),
        "failures": rows.iter().map(|r| r.failures).sum::<i64>(),
        "estimated_saved_ms": rows.iter().map(|r| r.saved_ms).sum::<i64>(),
        "workflows": workflows,
    }))
    .into_response())
}

/// Stats routes (merged into the API router).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/runs", axum::routing::post(report_run))
        .route("/me/stats", get(my_stats))
}
