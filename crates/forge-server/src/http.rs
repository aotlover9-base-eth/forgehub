//! HTTP routing and cross-cutting middleware: health/version,
//! request ids, security headers, body limit, timeout, compression,
//! rate limiting, and static hosting of the web app with an SPA
//! fallback.
//!
//! The CSP lives in exactly one place — [`CSP`] — and is asserted by
//! the HTTP tests.

use crate::error::ApiError;
use crate::ratelimit::{rate_limit_layer, RateLimiter};
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

/// The Content-Security-Policy for every response. The web app is
/// self-hosted (no CDNs, no third-party scripts or fonts — agent.md
/// §2), so `'self'` suffices; `'unsafe-inline'` for styles is needed
/// by Material Web's injected styles.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; \
base-uri 'self'; form-action 'self'";

async fn health(State(state): State<AppState>) -> Response {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ok", "db": "ok" }))).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "health check: database unreachable");
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "Database unavailable",
            )
            .into_response()
        }
    }
}

async fn version() -> Response {
    let git_sha = std::env::var("FORGEHUB_GIT_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "dev".to_string());
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "git_sha": git_sha })).into_response()
}

/// API fallback: unknown `/api/v1/*` paths get the error envelope,
/// never an HTML 404.
async fn api_not_found() -> Response {
    ApiError::not_found("not_found", "No such API route").into_response()
}

/// Assign a request id (uuid v4) and echo it as `x-request-id`;
/// the id is in scope for the request's tracing span.
async fn request_id_layer(req: Request, next: Next) -> Response {
    use tracing::Instrument;
    let id = uuid::Uuid::new_v4().to_string();
    let span = tracing::info_span!("request", request_id = %id);
    let mut resp = next.run(req).instrument(span).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        resp.headers_mut().insert("x-request-id", value);
    }
    resp
}

/// Security headers on every response (design-spec / threat model).
async fn security_headers_layer(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    resp
}

/// Cache headers for static assets: content-hashed files are
/// immutable; everything else (notably index.html) is no-cache.
async fn static_cache_layer(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut resp = next.run(req).await;
    let hashed = path.starts_with("/assets/") && {
        let file = path.rsplit('/').next().unwrap_or_default();
        // Vite emits name-<8+ hash chars>.ext
        file.rsplit('-').next().is_some_and(|tail| {
            tail.split('.')
                .next()
                .is_some_and(|h| h.len() >= 8 && h.chars().all(|c| c.is_ascii_alphanumeric()))
        })
    };
    let value = if hashed {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    resp
}

/// The cross-cutting layer stack for API routes, shared by the
/// production router and the test harness so tests exercise the
/// exact production behaviour (body limit, timeout, rate limit).
/// Compression lives one level up in [`build_router`] so it also
/// covers the static web app.
pub fn apply_api_layers<S>(router: Router<S>, state: &AppState) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let limiter = Arc::new(RateLimiter::new(crate::ratelimit::RateLimit::per_minute(
        state.config.rate_limit_per_minute,
    )));
    // TimeoutLayer fails with a boxed error; pair it with a
    // HandleErrorLayer (408 on elapsed, 500 otherwise) inside a
    // ServiceBuilder so the composed layer has an Infallible error,
    // as Router::layer requires at every step.
    let timeout_stack = tower::ServiceBuilder::new()
        .layer(axum::error_handling::HandleErrorLayer::new(
            |err: tower::BoxError| async move {
                if err.is::<tower::timeout::error::Elapsed>() {
                    ApiError::new(
                        StatusCode::REQUEST_TIMEOUT,
                        "request_timeout",
                        "Request timed out",
                    )
                } else {
                    tracing::error!(error = %err, "request layer error");
                    ApiError::internal()
                }
            },
        ))
        .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(
            state.config.request_timeout_secs,
        )));
    router
        .layer(middleware::from_fn_with_state(limiter, rate_limit_layer))
        .layer(timeout_stack)
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.body_limit_bytes,
        ))
}

/// Build the full application router.
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .merge(crate::auth::routes())
        .fallback(api_not_found);
    // Static web app (skipped with a warning when not built yet).
    let web_dir = state.config.web_dir.clone();
    let api = apply_api_layers(api, &state).with_state(state);

    let mut app = Router::new().nest("/api/v1", api);
    if web_dir.join("index.html").is_file() {
        let index = web_dir.join("index.html");
        let serve = ServeDir::new(&web_dir).fallback(ServeFile::new(index));
        app = app.fallback_service(serve);
    } else {
        tracing::warn!(
            web_dir = %web_dir.display(),
            "web build not found; serving API only (build web/ or set FORGEHUB_WEB_DIR)"
        );
        app = app.fallback(api_not_found);
    }

    app.layer(CompressionLayer::new())
        .layer(middleware::from_fn(security_headers_layer))
        .layer(middleware::from_fn(static_cache_layer))
        .layer(middleware::from_fn(request_id_layer))
}
