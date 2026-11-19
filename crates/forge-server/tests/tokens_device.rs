//! End-to-end token + device-flow tests (Step 8): token CRUD and
//! bearer auth, session-only management, and the full RFC 8628
//! style device flow including every failure branch.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;
use std::time::Duration;

struct TestServer {
    base: String,
    client: reqwest::Client,
    db_url: String,
    _dir: tempfile::TempDir,
}

async fn start(extra: &[(&str, &str)]) -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("tokens.db");
    let db_url = format!("sqlite://{}", db.display());
    let mut map: HashMap<String, String> = [
        ("FORGEHUB_DATABASE_URL", db_url.clone()),
        (
            "FORGEHUB_WEB_DIR",
            dir.path().join("no-web").display().to_string(),
        ),
        ("FORGEHUB_DEVICE_POLL_INTERVAL_SECS", "0".to_string()),
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
        db_url,
        _dir: dir,
    }
}

const PASSWORD: &str = "a-perfectly-fine-passphrase";

/// Sign up; return (session cookie header value, csrf token).
async fn signup(s: &TestServer, username: &str) -> (String, String) {
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
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let id = cookie
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("forgehub_session=")
        .to_string();
    let me: serde_json::Value = resp.json().await.unwrap();
    (
        format!("forgehub_session={id}"),
        me["csrf_token"].as_str().unwrap().to_string(),
    )
}

async fn create_token(
    s: &TestServer,
    cookie: &str,
    csrf: &str,
    scopes: &[&str],
) -> serde_json::Value {
    let resp = s
        .client
        .post(format!("{}/api/v1/tokens", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({ "name": "test token", "scopes": scopes }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create token");
    resp.json().await.unwrap()
}

async fn me_with_bearer(s: &TestServer, token: &str) -> reqwest::Response {
    s.client
        .get(format!("{}/api/v1/me", s.base))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn token_crud_and_bearer_auth() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "ivy").await;
    let created = create_token(&s, &cookie, &csrf, &["write"]).await;
    let plaintext = created["token"].as_str().unwrap().to_string();
    assert!(plaintext.starts_with("fgh_"), "{plaintext}");
    assert_eq!(created["prefix"], &plaintext[4..12]);
    assert_eq!(
        created["scopes"],
        serde_json::json!(["read", "write"]),
        "write implies read"
    );
    assert!(
        created.get("hash").is_none() && created.get("token_hash").is_none(),
        "the hash must never be returned: {created}"
    );
    let id = created["id"].as_i64().unwrap();

    // Bearer auth works and carries no CSRF token.
    let resp = me_with_bearer(&s, &plaintext).await;
    assert_eq!(resp.status(), 200);
    let me: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(me["username"], "ivy");
    assert!(me.get("csrf_token").is_none(), "tokens get no csrf_token");
    // Garbage and truncated tokens are rejected.
    assert_eq!(me_with_bearer(&s, "fgh_nonsense").await.status(), 401);
    assert_eq!(me_with_bearer(&s, &plaintext[..20]).await.status(), 401);

    // Listing shows metadata only.
    let resp = s
        .client
        .get(format!("{}/api/v1/tokens", s.base))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(list["tokens"].as_array().unwrap().len(), 1);
    assert!(
        list["tokens"][0].get("token").is_none(),
        "plaintext is shown once, at creation only"
    );

    // Revoke: the token dies immediately.
    let resp = s
        .client
        .delete(format!("{}/api/v1/tokens/{id}", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(
        me_with_bearer(&s, &plaintext).await.status(),
        401,
        "revoked token rejected at once"
    );
    // Revoking someone else's / a missing token is a 404.
    let resp = s
        .client
        .delete(format!("{}/api/v1/tokens/99999", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn tokens_cannot_manage_tokens_or_account() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "jack").await;
    let created = create_token(&s, &cookie, &csrf, &["read", "write"]).await;
    let plaintext = created["token"].as_str().unwrap().to_string();
    let id = created["id"].as_i64().unwrap();
    let bearer = format!("Bearer {plaintext}");
    for (method, url, what) in [
        ("GET", format!("{}/api/v1/tokens", s.base), "list tokens"),
        ("POST", format!("{}/api/v1/tokens", s.base), "create token"),
        (
            "DELETE",
            format!("{}/api/v1/tokens/{id}", s.base),
            "revoke token",
        ),
        (
            "PATCH",
            format!("{}/api/v1/me", s.base),
            "change profile/password",
        ),
        ("POST", format!("{}/api/v1/auth/logout", s.base), "logout"),
    ] {
        let builder = match method {
            "GET" => s.client.get(&url),
            "POST" => s.client.post(&url),
            "DELETE" => s.client.delete(&url),
            _ => s.client.patch(&url),
        };
        let resp = builder
            .header("authorization", &bearer)
            .json(&serde_json::json!({ "name": "x", "scopes": ["read"] }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 403, "{what} with a bearer token must be 403");
        let v: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(v["error"]["code"], "session_required", "{what}");
    }
    // But the token still works for what it is for.
    assert_eq!(me_with_bearer(&s, &plaintext).await.status(), 200);
}

#[tokio::test]
async fn expired_token_is_rejected() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "kate").await;
    let created = create_token(&s, &cookie, &csrf, &["read"]).await;
    let plaintext = created["token"].as_str().unwrap().to_string();
    assert_eq!(me_with_bearer(&s, &plaintext).await.status(), 200);
    // Force-expire the token directly in the database.
    let pool = forge_server::db::connect(&s.db_url).await.unwrap();
    sqlx::query("UPDATE api_tokens SET expires_at = '2000-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(me_with_bearer(&s, &plaintext).await.status(), 401);
}

async fn request_device_code(s: &TestServer) -> serde_json::Value {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/code", s.base))
        .json(&serde_json::json!({ "client_name": "forgehub CLI on testbox" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.json().await.unwrap()
}

async fn poll(s: &TestServer, device_code: &str) -> serde_json::Value {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/token", s.base))
        .json(&serde_json::json!({ "device_code": device_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "poll");
    resp.json().await.unwrap()
}

#[tokio::test]
async fn device_flow_end_to_end() {
    let s = start(&[]).await;
    let codes = request_device_code(&s).await;
    let device_code = codes["device_code"].as_str().unwrap().to_string();
    let user_code = codes["user_code"].as_str().unwrap().to_string();
    assert_eq!(user_code.len(), 9, "ABCD-EFGH shape: {user_code}");
    assert_eq!(&user_code[4..5], "-");
    assert_eq!(codes["expires_in"], 600);
    assert!(codes["verification_uri_complete"]
        .as_str()
        .unwrap()
        .contains(&user_code));
    // Poll before approval: pending.
    let p = poll(&s, &device_code).await;
    assert_eq!(p["status"], "pending");
    assert_eq!(p["error"], "authorization_pending");
    // The user signs up, looks the code up, tries to approve
    // without CSRF (403), then approves properly.
    let (cookie, csrf) = signup(&s, "liam").await;
    let resp = s
        .client
        .get(format!(
            "{}/api/v1/auth/device/lookup?user_code={user_code}",
            s.base
        ))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let lookup: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(lookup["client_name"], "forgehub CLI on testbox");
    assert_eq!(lookup["scopes"], serde_json::json!(["read", "write"]));
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/approve", s.base))
        .header("cookie", &cookie)
        .json(&serde_json::json!({ "user_code": user_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "approve without CSRF");
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/approve", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        // Typed lowercase without the hyphen: normalisation works.
        .json(&serde_json::json!({ "user_code": user_code.replace('-', "").to_lowercase() }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    // Poll delivers the token, exactly once.
    let p = poll(&s, &device_code).await;
    assert_eq!(p["status"], "approved");
    let access = p["access_token"].as_str().unwrap().to_string();
    assert!(access.starts_with("fgh_"));
    assert_eq!(p["token_type"], "Bearer");
    let replay = poll(&s, &device_code).await;
    assert_eq!(
        replay["status"], "expired",
        "single use: replay gets no token"
    );
    assert_eq!(replay["error"], "expired_token");
    assert!(replay.get("access_token").is_none());
    // The minted token is a normal, listed, working token.
    assert_eq!(me_with_bearer(&s, &access).await.status(), 200);
    let resp = s
        .client
        .get(format!("{}/api/v1/tokens", s.base))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = resp.json().await.unwrap();
    let names: Vec<&str> = list["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("CLI on forgehub CLI on testbox (")),
        "{names:?}"
    );
    // The consumed code no longer resolves in lookup.
    let resp = s
        .client
        .get(format!(
            "{}/api/v1/auth/device/lookup?user_code={user_code}",
            s.base
        ))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn device_denied_and_unknown_codes() {
    let s = start(&[]).await;
    let codes = request_device_code(&s).await;
    let device_code = codes["device_code"].as_str().unwrap().to_string();
    let user_code = codes["user_code"].as_str().unwrap().to_string();
    let (cookie, csrf) = signup(&s, "mia").await;
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/deny", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "user_code": user_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    let p = poll(&s, &device_code).await;
    assert_eq!(p["status"], "denied");
    assert_eq!(p["error"], "access_denied");
    // Unknown device code -> 400; unknown user code -> 404.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/token", s.base))
        .json(&serde_json::json!({ "device_code": "deadbeef".repeat(8) }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let resp = s
        .client
        .get(format!(
            "{}/api/v1/auth/device/lookup?user_code=ZZZZ-ZZZZ",
            s.base
        ))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn device_poll_slow_down() {
    let s = start(&[("FORGEHUB_DEVICE_POLL_INTERVAL_SECS", "2")]).await;
    let codes = request_device_code(&s).await;
    assert_eq!(codes["interval"], 2);
    let device_code = codes["device_code"].as_str().unwrap().to_string();
    let p = poll(&s, &device_code).await;
    assert_eq!(p["error"], "authorization_pending");
    let p = poll(&s, &device_code).await;
    assert_eq!(p["error"], "slow_down", "polling inside the interval");
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let p = poll(&s, &device_code).await;
    assert_eq!(
        p["error"], "authorization_pending",
        "interval respected again"
    );
}

#[tokio::test]
async fn device_expired_code() {
    let s = start(&[]).await;
    let codes = request_device_code(&s).await;
    let device_code = codes["device_code"].as_str().unwrap().to_string();
    let user_code = codes["user_code"].as_str().unwrap().to_string();
    let (cookie, csrf) = signup(&s, "noah").await;
    let pool = forge_server::db::connect(&s.db_url).await.unwrap();
    sqlx::query("UPDATE device_codes SET expires_at = '2000-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    // Approving an expired code is a 410; polling says expired.
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/device/approve", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "user_code": user_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 410);
    let p = poll(&s, &device_code).await;
    assert_eq!(p["status"], "expired");
    assert_eq!(p["error"], "expired_token");
}

#[tokio::test]
async fn device_lookup_bruteforce_lockout() {
    let s = start(&[]).await;
    let (cookie, _csrf) = signup(&s, "olga").await;
    for i in 0..10 {
        let resp = s
            .client
            .get(format!(
                "{}/api/v1/auth/device/lookup?user_code=AAAA-AA{i:02}",
                s.base
            ))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404, "wrong guess {i}");
    }
    // Eleventh attempt from the same IP is locked out — even
    // before the code is checked.
    let resp = s
        .client
        .get(format!(
            "{}/api/v1/auth/device/lookup?user_code=AAAA-AAAA",
            s.base
        ))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 429, "locked out after 10 failed lookups");
}

#[tokio::test]
async fn token_self_revoke_via_current() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "rene").await;
    let created = create_token(&s, &cookie, &csrf, &["read"]).await;
    let plaintext = created["token"].as_str().unwrap().to_string();
    let other = create_token(&s, &cookie, &csrf, &["read"]).await;
    let other_plaintext = other["token"].as_str().unwrap().to_string();

    // A session calling /tokens/current has no current token.
    let resp = s
        .client
        .delete(format!("{}/api/v1/tokens/current", s.base))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "no_current_token");

    // The bearer token revokes itself; the other token is untouched.
    let resp = s
        .client
        .delete(format!("{}/api/v1/tokens/current", s.base))
        .header("authorization", format!("Bearer {plaintext}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "self-revoke");
    assert_eq!(
        me_with_bearer(&s, &plaintext).await.status(),
        401,
        "revoked immediately"
    );
    assert_eq!(
        me_with_bearer(&s, &other_plaintext).await.status(),
        200,
        "other token unaffected"
    );

    // Anonymous gets 401.
    let resp = s
        .client
        .delete(format!("{}/api/v1/tokens/current", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}
