//! Sessions list/revoke tests (Step 17): the settings
//! security screen's endpoints — list with the current session
//! flagged, revoke another session, cross-user isolation,
//! bearer rejection, CSRF enforcement.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;

struct TestServer {
    base: String,
    client: reqwest::Client,
    _dir: tempfile::TempDir,
}

async fn start() -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sessions.db");
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

const PASSWORD: &str = "a-perfectly-fine-passphrase";

/// Sign up; returns (cookie header value, csrf token).
async fn signup(s: &TestServer, username: &str, email: &str) -> (String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&serde_json::json!({ "username": username, "email": email, "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let me: serde_json::Value = resp.json().await.unwrap();
    (cookie, me["csrf_token"].as_str().unwrap().to_string())
}

/// Log in again (a second session); returns (cookie, csrf).
async fn login(s: &TestServer, username: &str) -> (String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": username, "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let me: serde_json::Value = resp.json().await.unwrap();
    (cookie, me["csrf_token"].as_str().unwrap().to_string())
}

async fn list_sessions(s: &TestServer, cookie: &str) -> Vec<serde_json::Value> {
    let resp = s
        .client
        .get(format!("{}/api/v1/me/sessions", s.base))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    body["sessions"].as_array().unwrap().clone()
}

#[tokio::test]
async fn list_flags_current_and_revoke_kills_the_other_session() {
    let s = start().await;
    let (cookie_a, csrf_a) = signup(&s, "ada", "ada@example.com").await;
    let (cookie_b, _csrf_b) = login(&s, "ada").await;

    let sessions = list_sessions(&s, &cookie_a).await;
    assert_eq!(sessions.len(), 2);
    let current: Vec<_> = sessions.iter().filter(|x| x["current"] == true).collect();
    assert_eq!(current.len(), 1);
    let other = sessions.iter().find(|x| x["current"] == false).unwrap();
    let other_ref = other["ref"].as_str().unwrap().to_string();
    assert!(other_ref.len() >= 8);

    // Revoking without CSRF is refused.
    let resp = s
        .client
        .delete(format!("{}/api/v1/me/sessions/{other_ref}", s.base))
        .header("cookie", &cookie_a)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // With CSRF it works, and session B is really dead.
    let resp = s
        .client
        .delete(format!("{}/api/v1/me/sessions/{other_ref}", s.base))
        .header("cookie", &cookie_a)
        .header("x-csrf-token", &csrf_a)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    let resp = s
        .client
        .get(format!("{}/api/v1/me", s.base))
        .header("cookie", &cookie_b)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    assert_eq!(list_sessions(&s, &cookie_a).await.len(), 1);
}

#[tokio::test]
async fn refs_do_not_cross_users_and_garbage_refs_404() {
    let s = start().await;
    let (cookie_a, csrf_a) = signup(&s, "ada", "ada@example.com").await;
    let (cookie_b, _csrf_b) = signup(&s, "bob", "bob@example.com").await;
    let bob_sessions = list_sessions(&s, &cookie_b).await;
    let bob_ref = bob_sessions[0]["ref"].as_str().unwrap().to_string();

    // Ada cannot revoke Bob's session by its ref.
    let resp = s
        .client
        .delete(format!("{}/api/v1/me/sessions/{bob_ref}", s.base))
        .header("cookie", &cookie_a)
        .header("x-csrf-token", &csrf_a)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    assert_eq!(list_sessions(&s, &cookie_b).await.len(), 1);

    // Garbage refs are 404 too.
    let resp = s
        .client
        .delete(format!("{}/api/v1/me/sessions/zzzz", s.base))
        .header("cookie", &cookie_a)
        .header("x-csrf-token", &csrf_a)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn bearer_tokens_cannot_list_sessions() {
    let s = start().await;
    let (cookie, csrf) = signup(&s, "ada", "ada@example.com").await;
    let resp = s
        .client
        .post(format!("{}/api/v1/tokens", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "name": "cli", "scopes": ["read", "write"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let token = body["token"].as_str().unwrap().to_string();
    let resp = s
        .client
        .get(format!("{}/api/v1/me/sessions", s.base))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    assert_eq!(
        resp.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "session_required"
    );
}
