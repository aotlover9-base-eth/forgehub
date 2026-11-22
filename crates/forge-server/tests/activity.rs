//! Activity log tests (Peak C7): server-side events are
//! recorded at the point they happen (signup/login with the
//! new-device flag, snippet create/fork, token create/revoke),
//! the log is strictly per-user, client reports are limited to
//! the `snippet_trusted` allow-list, and the export is a JSON
//! download.

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
    let db = dir.path().join("activity.db");
    let map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL".to_string(),
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR".to_string(),
            dir.path().join("no-web").display().to_string(),
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

async fn signup_ua(s: &TestServer, username: &str, ua: &str) -> (String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .header("user-agent", ua)
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

async fn login_ua(s: &TestServer, username: &str, ua: &str) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .header("user-agent", ua)
        .json(&serde_json::json!({ "identifier": username, "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "login {username}");
}

async fn call(
    s: &TestServer,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    payload: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let mut req = s
        .client
        .request(method.parse().unwrap(), format!("{}/api/v1{path}", s.base));
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    if let Some(c) = csrf {
        req = req.header("x-csrf-token", c);
    }
    if let Some(p) = payload {
        req = req.json(&p);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

async fn events(s: &TestServer, cookie: &str) -> Vec<serde_json::Value> {
    let (status, out) = call(s, "GET", "/me/activity", Some(cookie), None, None).await;
    assert_eq!(status, 200);
    out["events"].as_array().unwrap().clone()
}

fn kinds(events: &[serde_json::Value]) -> Vec<&str> {
    events.iter().map(|e| e["kind"].as_str().unwrap()).collect()
}

#[tokio::test]
async fn activity_records_account_events() {
    let s = start().await;
    let (anna, csrf) = signup_ua(&s, "anna", "BrowserOne/1.0").await;

    // Signup itself is the first login, from a new device.
    let ev = events(&s, &anna).await;
    assert_eq!(kinds(&ev), ["login"]);
    assert_eq!(ev[0]["detail"]["via"], "signup");
    assert_eq!(ev[0]["detail"]["new_device"], true);

    // Same browser again: not new. A different one: new.
    login_ua(&s, "anna", "BrowserOne/1.0").await;
    login_ua(&s, "anna", "BrowserTwo/2.0").await;
    let ev = events(&s, &anna).await;
    let logins: Vec<&serde_json::Value> = ev.iter().filter(|e| e["kind"] == "login").collect();
    assert_eq!(logins.len(), 3);
    assert_eq!(
        logins[0]["detail"]["new_device"], true,
        "newest first: BrowserTwo"
    );
    assert_eq!(logins[1]["detail"]["new_device"], false, "BrowserOne again");
    assert_eq!(logins[0]["detail"]["via"], "login");

    // Snippet create + fork are recorded with their details.
    let (status, _) = call(
        &s,
        "POST",
        "/snippets",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({ "name": "deploy", "body": "echo hi\n" })),
    )
    .await;
    assert_eq!(status, 201);
    let (status, _) = call(
        &s,
        "POST",
        "/snippets/anna/deploy/fork",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({ "name": "deploy-copy" })),
    )
    .await;
    assert_eq!(status, 201);
    let ev = events(&s, &anna).await;
    let fork = ev.iter().find(|e| e["kind"] == "snippet_forked").unwrap();
    assert_eq!(fork["detail"]["from"], "anna/deploy");
    assert_eq!(fork["detail"]["name"], "deploy-copy");
    assert!(ev
        .iter()
        .any(|e| e["kind"] == "snippet_created" && e["detail"]["name"] == "deploy"));

    // Token create + revoke.
    let (status, tok) = call(
        &s,
        "POST",
        "/tokens",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({ "name": "laptop", "scopes": ["read", "write"] })),
    )
    .await;
    assert_eq!(status, 201, "{tok}");
    let token_id = tok["id"].as_i64().unwrap();
    let (status, _) = call(
        &s,
        "DELETE",
        &format!("/tokens/{token_id}"),
        Some(&anna),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, 204);
    let ev = events(&s, &anna).await;
    assert!(ev
        .iter()
        .any(|e| e["kind"] == "token_created" && e["detail"]["name"] == "laptop"));
    assert!(ev.iter().any(|e| e["kind"] == "token_revoked"));

    // The limit parameter is honoured.
    let (status, out) = call(&s, "GET", "/me/activity?limit=2", Some(&anna), None, None).await;
    assert_eq!(status, 200);
    assert_eq!(out["events"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn activity_is_private_and_reports_are_allowlisted() {
    let s = start().await;
    let (anna, csrf) = signup_ua(&s, "anna", "BrowserOne/1.0").await;
    let (bob, bob_csrf) = signup_ua(&s, "bob", "BrowserOne/1.0").await;

    // Bob's log has only his own signup login.
    let ev = events(&s, &bob).await;
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0]["kind"], "login");

    // Anonymous: 401.
    let resp = s
        .client
        .get(format!("{}/api/v1/me/activity", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // Client report: snippet_trusted is accepted...
    let (status, _) = call(
        &s,
        "POST",
        "/me/activity",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({
            "kind": "snippet_trusted",
            "detail": { "ref": "bob/setup", "version": 3 },
        })),
    )
    .await;
    assert_eq!(status, 201);
    // ...anything else is refused, as is a missing ref.
    for payload in [
        serde_json::json!({ "kind": "login", "detail": { "ref": "x/y" } }),
        serde_json::json!({ "kind": "snippet_trusted", "detail": {} }),
        serde_json::json!({ "kind": "snippet_trusted", "detail": { "ref": "" } }),
    ] {
        let (status, _) = call(
            &s,
            "POST",
            "/me/activity",
            Some(&anna),
            Some(&csrf),
            Some(payload),
        )
        .await;
        assert_eq!(status, 400);
    }
    let ev = events(&s, &anna).await;
    let trust = ev.iter().find(|e| e["kind"] == "snippet_trusted").unwrap();
    assert_eq!(trust["detail"]["ref"], "bob/setup");
    // Bob never sees anna's trust event.
    let ev = events(&s, &bob).await;
    assert!(ev.iter().all(|e| e["kind"] != "snippet_trusted"));
    let _ = bob_csrf;

    // Export: a JSON download naming the user.
    let resp = s
        .client
        .get(format!("{}/api/v1/me/activity/export", s.base))
        .header("cookie", &anna)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let disp = resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(disp.contains("forgehub-activity.json"), "{disp}");
    let doc: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(doc["user"], "anna");
    assert!(doc["events"].as_array().unwrap().len() >= 2);
}
