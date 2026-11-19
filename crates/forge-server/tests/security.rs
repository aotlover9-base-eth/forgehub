//! Step 19 adversarial tests: session fixation, prod cookie
//! flags, cross-session CSRF, Origin mismatch, X-Forwarded-For
//! spoofing against the login throttle, hostile search input,
//! control characters in content, and oversized bodies.
//! Harness copied from tests/auth.rs.

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

async fn start(extra: &[(&str, &str)]) -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sec.db");
    let mut map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL",
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR",
            dir.path().join("no-web").display().to_string(),
        ),
        ("FORGEHUB_LOGIN_BACKOFF_BASE_MS", "60000".to_string()),
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
        client: reqwest::Client::new(),
        _dir: dir,
    }
}

const PASSWORD: &str = "a-perfectly-fine-passphrase";

fn set_cookie_value(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("forgehub_session="))
        .map(ToString::to_string)
}

async fn signup(s: &TestServer, username: &str) -> (String, String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&serde_json::json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "password": PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "signup {username}");
    let header = set_cookie_value(&resp).expect("session cookie set");
    let cookie = header
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("forgehub_session=")
        .to_string();
    let body: serde_json::Value = resp.json().await.unwrap();
    let csrf = body["csrf_token"].as_str().unwrap().to_string();
    (header, cookie, csrf)
}

#[tokio::test]
async fn prod_cookies_are_secure_httponly_lax() {
    let s = start(&[
        ("FORGEHUB_ENV", "prod"),
        (
            "FORGEHUB_SECRET_KEY",
            "step19-test-secret-key-that-is-long-enough",
        ),
        ("FORGEHUB_BASE_URL", "https://forgehub.example.com"),
    ])
    .await;
    let (header, _, _) = signup(&s, "produser").await;
    assert!(header.contains("Secure"), "cookie lacks Secure: {header}");
    assert!(
        header.contains("HttpOnly"),
        "cookie lacks HttpOnly: {header}"
    );
    assert!(
        header.contains("SameSite=Lax"),
        "cookie lacks SameSite=Lax: {header}"
    );
    assert!(header.contains("Path=/"), "cookie lacks Path=/: {header}");
}

#[tokio::test]
async fn login_does_not_honour_a_planted_session_cookie() {
    // Session fixation: an attacker plants a known session id
    // in the victim's browser; login must issue a fresh id.
    let s = start(&[]).await;
    signup(&s, "victim").await;
    let planted = "a".repeat(64);
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .header("cookie", format!("forgehub_session={planted}"))
        .json(&serde_json::json!({ "identifier": "victim", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let header = set_cookie_value(&resp).expect("login sets a cookie");
    assert!(
        !header.contains(&planted),
        "session id was fixated by the planted cookie"
    );
}

#[tokio::test]
async fn csrf_token_from_another_session_is_rejected() {
    let s = start(&[]).await;
    let (_, cookie_a, _csrf_a) = signup(&s, "alice19").await;
    let (_, _cookie_b, csrf_b) = signup(&s, "bob19").await;
    // Alice's cookie + Bob's CSRF token: must not validate.
    let resp = s
        .client
        .patch(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={cookie_a}"))
        .header("x-csrf-token", csrf_b)
        .json(&serde_json::json!({ "bio": "pwned" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "cross-session CSRF token accepted");
}

#[tokio::test]
async fn csrf_rejects_mismatched_origin_even_with_valid_token() {
    let s = start(&[]).await;
    let (_, cookie, csrf) = signup(&s, "carol19").await;
    let resp = s
        .client
        .patch(format!("{}/api/v1/me", s.base))
        .header("cookie", format!("forgehub_session={cookie}"))
        .header("x-csrf-token", csrf)
        .header("origin", "http://evil.example")
        .json(&serde_json::json!({ "bio": "pwned" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "mismatched Origin accepted");
}

#[tokio::test]
async fn forwarded_for_spoofing_does_not_reset_login_throttle() {
    // The throttle keys on the socket address (ConnectInfo),
    // never on X-Forwarded-For: rotating the header must not
    // buy the attacker fresh attempts.
    let s = start(&[]).await;
    signup(&s, "target19").await;
    for i in 0..5 {
        let resp = s
            .client
            .post(format!("{}/api/v1/auth/login", s.base))
            .header("x-forwarded-for", format!("203.0.113.{i}"))
            .json(&serde_json::json!({ "identifier": "target19", "password": "wrong-password" }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401, "failure {i}");
    }
    // Locked now — even the correct password is refused with
    // 429 before any password check.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .header("x-forwarded-for", "198.51.100.99")
        .json(&serde_json::json!({ "identifier": "target19", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 429, "XFF spoofing bypassed the throttle");
}

#[tokio::test]
async fn hostile_search_queries_never_500() {
    let s = start(&[]).await;
    signup(&s, "searcher").await;
    let queries = [
        "\"(unclosed",
        "a OR b NOT c",
        "NEAR(x y)",
        "name:*",
        "\"\"\"\"",
        "'; DROP TABLE snippets; --",
        &"x".repeat(10_000),
        "\u{202e}\u{1f600}",
    ];
    for q in queries {
        let resp = s
            .client
            .get(format!("{}/api/v1/search", s.base))
            .query(&[("q", q)])
            .send()
            .await
            .unwrap();
        assert!(
            resp.status().as_u16() < 500,
            "query {q:?} -> {}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn control_characters_in_content_never_500() {
    let s = start(&[]).await;
    let (_, cookie, csrf) = signup(&s, "mallory").await;
    // NUL, ESC and a bidi override in the description; a NUL in
    // the body. The API may accept (stored as inert text) or
    // reject with a 4xx — never a 500, and a stored value must
    // round-trip as the same JSON string.
    let description = "evil \u{0} desc \u{1b}[31m \u{202e}flip";
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", format!("forgehub_session={cookie}"))
        .header("x-csrf-token", csrf.clone())
        .json(&serde_json::json!({
            "name": "evil-one",
            "body": "echo hi\u{0}there",
            "description": description,
            "visibility": "private",
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(status < 500, "hostile content -> {status}");
    if status == 201 {
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["description"].as_str().unwrap(), description);
    }
}

#[tokio::test]
async fn oversized_body_is_413_not_500() {
    let s = start(&[]).await;
    let (_, cookie, csrf) = signup(&s, "bigbody").await;
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", format!("forgehub_session={cookie}"))
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({
            "name": "too-big",
            "body": "x".repeat(300 * 1024),
            "visibility": "private",
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let text = resp.text().await.unwrap();
    assert_eq!(status, 413, "oversized body -> {status}: {text}");
}

#[tokio::test]
async fn forged_bearer_tokens_get_a_generic_401() {
    let s = start(&[]).await;
    for token in [
        format!("fgh_{}", "A".repeat(43)),
        "fgh_short".to_string(),
        "not-a-token".to_string(),
    ] {
        let resp = s
            .client
            .get(format!("{}/api/v1/me", s.base))
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401, "token {token} -> {}", resp.status());
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"]["code"], "unauthorized");
    }
}
