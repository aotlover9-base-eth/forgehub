//! End-to-end auth tests (Step 7): signup/login/logout, sessions,
//! CSRF, throttling, password change, account deletion — against
//! the real router on an ephemeral port.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;
use std::time::Duration;

struct TestServer {
    base: String,
    client: reqwest::Client,
    _dir: tempfile::TempDir,
}

async fn start(extra: &[(&str, &str)]) -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("auth.db");
    let mut map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL",
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR",
            dir.path().join("no-web").display().to_string(),
        ),
        // Fast backoff so the throttling test stays quick.
        ("FORGEHUB_LOGIN_BACKOFF_BASE_MS", "60".to_string()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.clone()))
    .collect();
    for (k, v) in extra {
        map.insert(k.to_string(), v.to_string());
    }
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
        // No cookie feature on the client: tests pass cookies
        // manually so they control exactly what is sent.
        client: reqwest::Client::new(),
        _dir: dir,
    }
}

const PASSWORD: &str = "a-perfectly-fine-passphrase";

fn signup_body(username: &str, email: &str, password: &str) -> serde_json::Value {
    serde_json::json!({ "username": username, "email": email, "password": password })
}

/// Sign up and return (Me JSON, session cookie value, csrf token).
async fn signup_ok(
    s: &TestServer,
    username: &str,
    email: &str,
) -> (serde_json::Value, String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&signup_body(username, email, PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "signup {username}");
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let me: serde_json::Value = resp.json().await.unwrap();
    let csrf = me["csrf_token"].as_str().unwrap().to_string();
    (me, cookie, csrf)
}

fn session_id(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("forgehub_session=")
        .to_string()
}

async fn me(s: &TestServer, cookie: &str) -> reqwest::Response {
    s.client
        .get(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={}", session_id(cookie)))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn signup_login_logout_happy_path() {
    let s = start(&[]).await;
    let (me_json, cookie, _csrf) = signup_ok(&s, "alice", "alice@example.com").await;
    assert_eq!(me_json["username"], "alice");
    assert_eq!(me_json["email"], "alice@example.com");
    assert!(
        me_json.get("password").is_none(),
        "password must never be returned"
    );
    assert!(me_json.get("password_hash").is_none());
    // Cookie flags.
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");
    assert!(cookie.contains("Path=/"), "{cookie}");
    assert!(
        !cookie.contains("Secure"),
        "dev must not set Secure: {cookie}"
    );
    // /me works with the cookie.
    let resp = me(&s, &cookie).await;
    assert_eq!(resp.status(), 200);
    // Login with username and with email both work.
    for identifier in ["alice", "alice@example.com"] {
        let resp = s
            .client
            .post(format!("{}/api/v1/auth/login", s.base))
            .json(&serde_json::json!({ "identifier": identifier, "password": PASSWORD }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "login as {identifier}");
    }
    // Logout needs CSRF; then the session is dead.
    let csrf = me_json["csrf_token"].as_str().unwrap();
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/logout", s.base))
        .header(
            "cookie",
            format!("forgehub_session={}", session_id(&cookie)),
        )
        .header("x-csrf-token", csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    let resp = me(&s, &cookie).await;
    assert_eq!(resp.status(), 401, "session must be dead after logout");
}

#[tokio::test]
async fn signup_validation_and_duplicates() {
    let s = start(&[]).await;
    let cases: Vec<(serde_json::Value, u16, &str)> = vec![
        (
            signup_body("a", "a@b.co", PASSWORD),
            400,
            "username too short",
        ),
        (
            signup_body("Admin", "a@b.co", PASSWORD),
            400,
            "reserved/uppercase",
        ),
        (
            signup_body("bad name", "a@b.co", PASSWORD),
            400,
            "username charset",
        ),
        (
            signup_body("alice", "not-an-email", PASSWORD),
            400,
            "email format",
        ),
        (
            signup_body("alice", "a@b.co", "short"),
            400,
            "password length",
        ),
        (
            signup_body("alice", "a@b.co", "password123"),
            400,
            "denylisted password",
        ),
        (
            signup_body("alice", "a@b.co", &"x".repeat(300)),
            400,
            "password too long",
        ),
    ];
    for (body, status, why) in cases {
        let resp = s
            .client
            .post(format!("{}/api/v1/auth/signup", s.base))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), status, "{why}");
        let v: serde_json::Value = resp.json().await.unwrap();
        assert!(v["error"]["message"].is_string(), "{why}: envelope");
    }
    let (_, _c, _t) = signup_ok(&s, "alice", "alice@example.com").await;
    // Duplicate username -> explicit 409 (usernames are public).
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&signup_body("alice", "other@example.com", PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "username_taken");
    // Duplicate email -> generic 409 that does not confirm the email.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&signup_body("alice2", "alice@example.com", PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "signup_conflict");
    assert!(
        !v["error"]["message"].as_str().unwrap().contains("email"),
        "message must not reveal the email exists: {v}"
    );
}

#[tokio::test]
async fn signup_disabled_returns_403() {
    let s = start(&[("FORGEHUB_ALLOW_SIGNUP", "false")]).await;
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&signup_body("alice", "alice@example.com", PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn login_failures_are_generic_and_throttled() {
    let s = start(&[]).await;
    let (_, _c, _t) = signup_ok(&s, "bob", "bob@example.com").await;
    let login = |password: &str| {
        let client = s.client.clone();
        let base = s.base.clone();
        let password = password.to_string();
        async move {
            client
                .post(format!("{base}/api/v1/auth/login"))
                .json(&serde_json::json!({ "identifier": "bob", "password": password }))
                .send()
                .await
                .unwrap()
        }
    };
    // Wrong password and unknown user produce the identical response.
    let wrong = login("wrong-password-1").await;
    assert_eq!(wrong.status(), 401);
    let wrong_body: serde_json::Value = wrong.json().await.unwrap();
    let ghost = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "nosuchuser", "password": "whatever-12345" }))
        .send()
        .await
        .unwrap();
    assert_eq!(ghost.status(), 401);
    let ghost_body: serde_json::Value = ghost.json().await.unwrap();
    assert_eq!(wrong_body, ghost_body, "no user-enumeration oracle");
    // Three more failures: bob's account reaches 4 and the shared
    // IP key reaches 5 (ghost attempt included), which locks the
    // IP. The next attempt is 429 before any password check.
    for i in 0..3 {
        let resp = login("wrong-password-1").await;
        assert_eq!(resp.status(), 401, "failure {i}");
    }
    let resp = login(PASSWORD).await;
    assert_eq!(resp.status(), 429, "locked after 5 failures");
    assert!(resp.headers().get("retry-after").is_some());
    // After the (test-shortened) backoff, the right password works.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let resp = login(PASSWORD).await;
    assert_eq!(resp.status(), 200, "lock expires");
}

#[tokio::test]
async fn csrf_is_required_for_cookie_mutations() {
    let s = start(&[]).await;
    let (_, cookie, csrf) = signup_ok(&s, "carol", "carol@example.com").await;
    let id = session_id(&cookie);
    // No token -> 403 csrf_failed.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/logout", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "csrf_failed");
    // Wrong token -> 403.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/logout", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", "deadbeef")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    // Foreign Origin -> 403 even with the right token.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/logout", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", &csrf)
        .header("origin", "https://evil.example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    // Right token -> works.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/logout", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn session_expiry_and_logout_all() {
    // Idle expiry of 1 second.
    let s = start(&[
        ("FORGEHUB_SESSION_IDLE_SECS", "1"),
        ("FORGEHUB_SESSION_ABSOLUTE_SECS", "3600"),
    ])
    .await;
    let (_, cookie, _t) = signup_ok(&s, "dave", "dave@example.com").await;
    assert_eq!(me(&s, &cookie).await.status(), 200);
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        me(&s, &cookie).await.status(),
        401,
        "idle session must expire"
    );

    // logout-all kills every session.
    let s = start(&[]).await;
    let (_, cookie1, csrf) = signup_ok(&s, "erin", "erin@example.com").await;
    let login = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "erin", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    let cookie2 = login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(me(&s, &cookie1).await.status(), 200);
    assert_eq!(me(&s, &cookie2).await.status(), 200);
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/logout-all", s.base))
        .header(
            "cookie",
            format!("forgehub_session={}", session_id(&cookie1)),
        )
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(me(&s, &cookie1).await.status(), 401);
    assert_eq!(me(&s, &cookie2).await.status(), 401);
}

#[tokio::test]
async fn password_change_revokes_other_sessions_and_rotates() {
    let s = start(&[]).await;
    let (_, cookie1, csrf1) = signup_ok(&s, "frank", "frank@example.com").await;
    let login = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "frank", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    let cookie2 = login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    // Wrong current password -> 401.
    let resp = s
        .client
        .patch(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={}", session_id(&cookie1)))
        .header("x-csrf-token", &csrf1)
        .json(&serde_json::json!({ "current_password": "nope-nope-nope", "new_password": "a-brand-new-passphrase" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    // Correct change: current session rotates, the other dies.
    let resp = s
        .client
        .patch(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={}", session_id(&cookie1)))
        .header("x-csrf-token", &csrf1)
        .json(&serde_json::json!({ "current_password": PASSWORD, "new_password": "a-brand-new-passphrase" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let new_cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(
        session_id(&new_cookie),
        session_id(&cookie1),
        "session rotated"
    );
    assert_eq!(me(&s, &cookie1).await.status(), 401, "old session revoked");
    assert_eq!(
        me(&s, &cookie2).await.status(),
        401,
        "other session revoked"
    );
    assert_eq!(
        me(&s, &new_cookie).await.status(),
        200,
        "rotated session works"
    );
    // Old password no longer logs in; the new one does.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "frank", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "frank", "password": "a-brand-new-passphrase" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn profile_patch_and_account_deletion() {
    let s = start(&[]).await;
    let (_, cookie, csrf) = signup_ok(&s, "grace", "grace@example.com").await;
    let id = session_id(&cookie);
    let resp = s
        .client
        .patch(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "display_name": "Grace H", "bio": "compilers" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["display_name"], "Grace H");
    assert_eq!(v["bio"], "compilers");
    // Over-long bio -> 400.
    let resp = s
        .client
        .patch(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "bio": "x".repeat(281) }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    // Delete with the wrong typed username -> 400; right one -> 204
    // and the account is gone for login and for the session.
    let resp = s
        .client
        .delete(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "username": "notgrace", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let resp = s
        .client
        .delete(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={id}"))
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "username": "grace", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(me(&s, &cookie).await.status(), 401);
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "grace", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "deleted account cannot log in");
}

#[tokio::test]
async fn injection_and_garbage_inputs_fail_safely() {
    let s = start(&[]).await;
    let (_, _c, _t) = signup_ok(&s, "heidi", "heidi@example.com").await;
    for identifier in ["' OR '1'='1", "heidi'--", "\"; DROP TABLE users;--"] {
        let resp = s
            .client
            .post(format!("{}/api/v1/auth/login", s.base))
            .json(&serde_json::json!({ "identifier": identifier, "password": "whatever-12345" }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401, "injection attempt: {identifier}");
    }
    // The users table obviously still works.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "heidi", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    // Malformed JSON -> 400 envelope, not a panic or plain text.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "validation_failed");
    // Unauthenticated /me -> 401 envelope.
    let resp = s
        .client
        .get(format!("{}/api/v1/me", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "unauthorized");
}
