//! End-to-end HTTP tests: the real router on an ephemeral port,
//! a temp-file SQLite database, and a reqwest client.

use forge_server::config::Config;
use forge_server::http::{apply_api_layers, build_router, CSP};
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct TestServer {
    base: String,
    client: reqwest::Client,
    _dir: tempfile::TempDir,
}

fn test_config(dir: &std::path::Path, extra: &[(&str, &str)]) -> Config {
    let db = dir.join("http.db");
    let mut map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL",
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR",
            dir.join("no-web-here").display().to_string(),
        ),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.clone()))
    .collect();
    for (k, v) in extra {
        map.insert(k.to_string(), v.to_string());
    }
    Config::from_vars(&|k| map.get(k).cloned()).unwrap()
}

async fn start(extra: &[(&str, &str)]) -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path(), extra);
    let pool = forge_server::db::connect(&config.database_url)
        .await
        .unwrap();
    forge_server::db::migrate(&pool).await.unwrap();
    let state = AppState::new(config, Repos::sqlite(pool.clone()), pool);
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    TestServer {
        base: format!("http://{addr}"),
        client: reqwest::Client::new(),
        _dir: dir,
    }
}

#[tokio::test]
async fn health_reports_ok_with_db() {
    let s = start(&[]).await;
    let resp = s
        .client
        .get(format!("{}/api/v1/health", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body, serde_json::json!({ "status": "ok", "db": "ok" }));
}

#[tokio::test]
async fn version_reports_package_version() {
    let s = start(&[]).await;
    let resp = s
        .client
        .get(format!("{}/api/v1/version", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["git_sha"].is_string());
}

#[tokio::test]
async fn unknown_api_route_uses_the_error_envelope() {
    let s = start(&[]).await;
    let resp = s
        .client
        .get(format!("{}/api/v1/nope", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "not_found");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn security_headers_and_request_id_are_present() {
    let s = start(&[]).await;
    let resp = s
        .client
        .get(format!("{}/api/v1/health", s.base))
        .send()
        .await
        .unwrap();
    let headers = resp.headers().clone();
    assert_eq!(headers["content-security-policy"], CSP);
    // The landing page's intro <video> loads media from the
    // app itself; the policy names that explicitly.
    assert!(
        CSP.contains("media-src 'self'"),
        "CSP must carry an explicit media-src 'self': {CSP}"
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(
        headers["referrer-policy"],
        "strict-origin-when-cross-origin"
    );
    assert!(headers["permissions-policy"]
        .to_str()
        .unwrap()
        .contains("camera=()"));
    let id = headers["x-request-id"].to_str().unwrap();
    assert_eq!(id.len(), 36, "uuid request id: {id}");
    // Rate-limit headers ride along on API responses.
    assert_eq!(headers["ratelimit-limit"], "600");
    assert!(headers.get("ratelimit-remaining").is_some());
}

#[tokio::test]
async fn responses_are_gzipped_when_accepted() {
    // Raw HTTP: reqwest would transparently decompress and hide the
    // header, so assert on the wire bytes instead.
    let s = start(&[]).await;
    let addr = s.base.trim_start_matches("http://");
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            b"GET /api/v1/version HTTP/1.1\r\nhost: test\r\naccept-encoding: gzip\r\nconnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).to_lowercase();
    assert!(text.contains("200 ok"), "{text}");
    assert!(text.contains("content-encoding: gzip"), "{text}");
}

#[tokio::test]
async fn rate_limit_returns_429_with_retry_after() {
    let s = start(&[("FORGEHUB_RATE_LIMIT_PER_MINUTE", "3")]).await;
    for i in 0..3 {
        let resp = s
            .client
            .get(format!("{}/api/v1/health", s.base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "request {i}");
    }
    let resp = s
        .client
        .get(format!("{}/api/v1/health", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 429);
    assert!(resp.headers().get("retry-after").is_some());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "rate_limited");
}

#[tokio::test]
async fn body_limit_rejects_oversized_payloads() {
    // A router with the exact production API layer stack around a
    // test echo route (the skeleton has no body-accepting routes).
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path(), &[("FORGEHUB_BODY_LIMIT_KIB", "1")]);
    let pool = forge_server::db::connect(&config.database_url)
        .await
        .unwrap();
    let state = AppState::new(config, Repos::sqlite(pool.clone()), pool);
    let echo = axum::Router::new().route(
        "/echo",
        axum::routing::post(|body: String| async move { body }),
    );
    let app = apply_api_layers(echo, &state).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let client = reqwest::Client::new();
    let small = client
        .post(format!("http://{addr}/echo"))
        .body("hello")
        .send()
        .await
        .unwrap();
    assert_eq!(small.status(), 200);
    let big = client
        .post(format!("http://{addr}/echo"))
        .body("x".repeat(2048))
        .send()
        .await
        .unwrap();
    assert_eq!(big.status(), 413, "2 KiB body against a 1 KiB limit");
}
