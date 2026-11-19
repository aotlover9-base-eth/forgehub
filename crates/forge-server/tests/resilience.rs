//! Resilience checks (Step 20): a locked database must surface
//! as a graceful, generic 5xx — never a hang, a panic, or a
//! response that leaks storage internals.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use sqlx::Connection;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Start the real server on an ephemeral port; returns
/// `(base_url, db_path, tempdir guard)`.
async fn start() -> (String, std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("res.db");
    let map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL".to_string(),
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR".to_string(),
            dir.path().join("no-web").display().to_string(),
        ),
        ("FORGEHUB_LOGIN_BACKOFF_BASE_MS".to_string(), "60".into()),
    ]
    .into_iter()
    .collect();
    let config = Config::from_vars(&|k| map.get(k).cloned()).unwrap();
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
    (format!("http://{addr}"), db, dir)
}

#[tokio::test]
async fn locked_database_yields_generic_500_not_leaked_internals() {
    let (base, db, _dir) = start().await;
    // Hold an exclusive write lock from a second connection,
    // the way a stuck writer or a second process would.
    let mut lock_conn = sqlx::SqliteConnection::connect(&format!("sqlite://{}", db.display()))
        .await
        .unwrap();
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut lock_conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users (username, email, password_hash, created_at) \
         VALUES ('locker', 'locker@example.com', 'x', '2026-11-19T00:00:00Z')",
    )
    .execute(&mut lock_conn)
    .await
    .unwrap();

    // A write request now blocks on the busy timeout (~5 s) and
    // must then fail gracefully: 500 + the standard envelope,
    // with no storage details in the body.
    let client = reqwest::Client::new();
    let t = Instant::now();
    let resp = client
        .post(format!("{base}/api/v1/auth/signup"))
        .json(&serde_json::json!({
            "username": "victim",
            "email": "victim@example.com",
            "password": "a long passphrase",
        }))
        .send()
        .await
        .unwrap();
    let elapsed = t.elapsed();
    assert!(
        elapsed < Duration::from_secs(20),
        "request must not hang: {elapsed:?}"
    );
    assert_eq!(resp.status(), 500, "locked DB write should be a 500");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("\"code\":\"internal_error\""),
        "expected the generic error envelope, got: {body}"
    );
    assert!(
        !body.to_lowercase().contains("locked") && !body.contains("sqlx"),
        "response must not leak storage internals, got: {body}"
    );

    // Reads still work under WAL while the writer is locked out.
    let health = client
        .get(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200);

    sqlx::query("ROLLBACK")
        .execute(&mut lock_conn)
        .await
        .unwrap();
    lock_conn.close().await.unwrap();

    // And once the lock clears, the same write succeeds.
    let resp = client
        .post(format!("{base}/api/v1/auth/signup"))
        .json(&serde_json::json!({
            "username": "victim",
            "email": "victim@example.com",
            "password": "a long passphrase",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "write recovers after the lock clears");
}
