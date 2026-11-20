//! Password-reset end-to-end tests (2026-11-20 increment):
//! the emailed-token flow against the real router, with a
//! capture mailer in place of SMTP. Covers the happy path,
//! the security consequences (sessions and API tokens die),
//! enumeration safety, single use, superseded links, expiry
//! and the weak-password rule.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::mail::Mailer;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct CaptureMailer {
    sent: Mutex<Vec<(String, String)>>,
}

impl Mailer for CaptureMailer {
    fn send_password_reset(&self, to: &str, reset_url: &str) -> anyhow::Result<()> {
        self.sent
            .lock()
            .unwrap()
            .push((to.to_string(), reset_url.to_string()));
        Ok(())
    }

    fn describe(&self) -> &'static str {
        "capture"
    }
}

struct TestServer {
    base: String,
    client: reqwest::Client,
    mail: Arc<CaptureMailer>,
    pool: sqlx::SqlitePool,
    _dir: tempfile::TempDir,
}

async fn start() -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("reset.db");
    let map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL".to_string(),
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR".to_string(),
            dir.path().join("no-web").display().to_string(),
        ),
        (
            "FORGEHUB_LOGIN_BACKOFF_BASE_MS".to_string(),
            "60".to_string(),
        ),
    ]
    .into_iter()
    .collect();
    let config = Config::from_vars(&|k| map.get(k).cloned()).unwrap();
    let pool = forge_server::db::connect(&config.database_url)
        .await
        .unwrap();
    forge_server::db::migrate(&pool).await.unwrap();
    let mail = Arc::new(CaptureMailer::default());
    let state = AppState::with_mailer(
        config,
        Repos::sqlite(pool.clone()),
        pool.clone(),
        mail.clone(),
    );
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
        mail,
        pool,
        _dir: dir,
    }
}

const OLD_PASSWORD: &str = "a-perfectly-fine-passphrase";
const NEW_PASSWORD: &str = "a-brand-new-passphrase-42";

/// Sign up; returns (session cookie value, csrf token).
async fn signup_ok(s: &TestServer, username: &str, email: &str) -> (String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&serde_json::json!({
            "username": username, "email": email, "password": OLD_PASSWORD
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "signup must succeed");
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .trim()
        .to_string();
    let me: serde_json::Value = resp.json().await.unwrap();
    let csrf = me["csrf_token"].as_str().unwrap().to_string();
    (cookie, csrf)
}

async fn request_reset(s: &TestServer, email: &str) -> u16 {
    s.client
        .post(format!("{}/api/v1/auth/password-reset/request", s.base))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn confirm_reset(
    s: &TestServer,
    token: &str,
    new_password: &str,
) -> (u16, serde_json::Value) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/password-reset/confirm", s.base))
        .json(&serde_json::json!({ "token": token, "new_password": new_password }))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = resp.json().await.unwrap_or(serde_json::Value::Null);
    (status, body)
}

async fn login_status(s: &TestServer, identifier: &str, password: &str) -> u16 {
    s.client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": identifier, "password": password }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// The token from the most recent captured mail.
fn captured_token(s: &TestServer) -> String {
    let sent = s.mail.sent.lock().unwrap();
    let (_, url) = sent.last().expect("a reset mail must have been captured");
    url.split("token=")
        .nth(1)
        .expect("the reset URL carries a token")
        .to_string()
}

#[tokio::test]
async fn the_full_flow_and_its_security_consequences() {
    let s = start().await;
    let (cookie, csrf) = signup_ok(&s, "ada", "ada@example.com").await;

    // An API token minted before the reset, to prove it dies.
    let created = s
        .client
        .post(format!("{}/api/v1/tokens", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "name": "before-reset", "scopes": ["read"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let created: serde_json::Value = created.json().await.unwrap();
    let api_token = created["token"].as_str().unwrap().to_string();

    assert_eq!(request_reset(&s, "ada@example.com").await, 204);
    // Mail delivery is spawned off the request path; poll briefly.
    let mut token = String::new();
    for _ in 0..50 {
        if !s.mail.sent.lock().unwrap().is_empty() {
            token = captured_token(&s);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(!token.is_empty(), "the reset mail must arrive");
    assert_eq!(s.mail.sent.lock().unwrap()[0].0, "ada@example.com");

    let (status, _) = confirm_reset(&s, &token, NEW_PASSWORD).await;
    assert_eq!(status, 204, "confirm must succeed");

    // The old session is dead.
    let me = s
        .client
        .get(format!("{}/api/v1/me", s.base))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 401, "the pre-reset session must be revoked");
    // The old API token is dead.
    let me = s
        .client
        .get(format!("{}/api/v1/me", s.base))
        .header("authorization", format!("Bearer {api_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 401, "the pre-reset API token must be revoked");
    // Old password out, new password in.
    assert_eq!(login_status(&s, "ada", OLD_PASSWORD).await, 401);
    assert_eq!(login_status(&s, "ada", NEW_PASSWORD).await, 200);
    // The link is single-use.
    let (status, body) = confirm_reset(&s, &token, "another-fine-passphrase-7").await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_token");
}

#[tokio::test]
async fn requests_never_reveal_whether_an_account_exists() {
    let s = start().await;
    signup_ok(&s, "ada", "ada@example.com").await;
    assert_eq!(request_reset(&s, "ghost@example.com").await, 204);
    assert_eq!(request_reset(&s, "not-an-email").await, 204);
    assert_eq!(request_reset(&s, "").await, 204);
    assert!(
        s.mail.sent.lock().unwrap().is_empty(),
        "no mail may be captured for unknown or invalid addresses"
    );
}

#[tokio::test]
async fn unknown_expired_and_superseded_tokens_all_fail_the_same_way() {
    let s = start().await;
    signup_ok(&s, "ada", "ada@example.com").await;

    // Unknown token.
    let (status, body) = confirm_reset(&s, "no-such-token", NEW_PASSWORD).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_token");

    // A second request supersedes the first link.
    assert_eq!(request_reset(&s, "ada@example.com").await, 204);
    assert_eq!(request_reset(&s, "ada@example.com").await, 204);
    let mut first = String::new();
    let mut second = String::new();
    for _ in 0..50 {
        if s.mail.sent.lock().unwrap().len() >= 2 {
            let sent = s.mail.sent.lock().unwrap();
            first = sent[0].1.split("token=").nth(1).unwrap().to_string();
            second = sent[1].1.split("token=").nth(1).unwrap().to_string();
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(!first.is_empty() && first != second);
    let (status, _) = confirm_reset(&s, &first, NEW_PASSWORD).await;
    assert_eq!(status, 400, "the superseded link must be dead");

    // Expire the live one in the database: same generic failure.
    sqlx::query("UPDATE password_reset_tokens SET expires_at = '2000-01-01T00:00:00Z'")
        .execute(&s.pool)
        .await
        .unwrap();
    let (status, body) = confirm_reset(&s, &second, NEW_PASSWORD).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_token");
}

#[tokio::test]
async fn a_weak_new_password_is_rejected_without_consuming_the_link() {
    let s = start().await;
    signup_ok(&s, "ada", "ada@example.com").await;
    assert_eq!(request_reset(&s, "ada@example.com").await, 204);
    let mut token = String::new();
    for _ in 0..50 {
        if !s.mail.sent.lock().unwrap().is_empty() {
            token = captured_token(&s);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_ne!(token, "");
    let (status, body) = confirm_reset(&s, &token, "short").await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "validation_failed");
    // The same link still works with a proper password.
    let (status, _) = confirm_reset(&s, &token, NEW_PASSWORD).await;
    assert_eq!(status, 204);
    assert_eq!(login_status(&s, "ada", NEW_PASSWORD).await, 200);
}
