//! Share-link tests (Peak C6, ADR 0011): minting is
//! owner-only and existence-masked, the token is shown once
//! and stored hashed, the anonymous page shows the
//! collection's current members (private snippets included -
//! that is the link's purpose), and revoked/expired/unknown
//! tokens share one 404.

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
    let db = dir.path().join("shares.db");
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

async fn call(
    s: &TestServer,
    method: &str,
    path: &str,
    auth: Option<&str>,
    csrf: Option<&str>,
    payload: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let mut req = s
        .client
        .request(method.parse().unwrap(), format!("{}/api/v1{path}", s.base));
    if let Some(a) = auth {
        req = if a.starts_with("Bearer ") {
            req.header("authorization", a.to_string())
        } else {
            req.header("cookie", a.to_string())
        };
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

/// anna with a private collection `runbook` holding one
/// private snippet whose body carries a marker.
async fn fixture(s: &TestServer) -> (String, String) {
    let (anna, csrf) = signup(s, "anna").await;
    let (status, _) = call(
        s,
        "POST",
        "/snippets",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({
            "name": "secret-sauce",
            "body": "echo the-marker-body\n",
        })),
    )
    .await;
    assert_eq!(status, 201);
    let (status, _) = call(
        s,
        "POST",
        "/collections",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({ "name": "runbook" })),
    )
    .await;
    assert_eq!(status, 201);
    let (status, _) = call(
        s,
        "PUT",
        "/collections/anna/runbook/items/anna/secret-sauce",
        Some(&anna),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, 201, "add item");
    (anna, csrf)
}

async fn page(s: &TestServer, path: &str) -> (u16, String, Option<String>) {
    let resp = s
        .client
        .get(format!("{}{path}", s.base))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let csp = resp
        .headers()
        .get("content-security-policy")
        .map(|v| v.to_str().unwrap_or_default().to_string());
    (status, resp.text().await.unwrap(), csp)
}

#[tokio::test]
async fn share_link_lifecycle() {
    let s = start().await;
    let (anna, csrf) = fixture(&s).await;

    // Mint (default 7 days): the URL and token come back once.
    let (status, out) = call(
        &s,
        "POST",
        "/collections/anna/runbook/share-links",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, 201, "{out}");
    let url = out["url"].as_str().unwrap().to_string();
    let token = out["token"].as_str().unwrap().to_string();
    assert!(url.ends_with(&token));
    assert!(token.starts_with("fgl_"));
    let link_id = out["id"].as_i64().unwrap();
    assert_eq!(out["active"], true);

    // The list shows the prefix, never the token.
    let (status, out) = call(
        &s,
        "GET",
        "/collections/anna/runbook/share-links",
        Some(&anna),
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(out["links"][0]["prefix"], &token[4..12]);
    assert!(!out.to_string().contains(&token), "token leaked in list");

    // Anonymous viewing: the private snippet's code is shown
    // (that is the link's purpose), read-only, under the CSP.
    let (status, html, csp) = page(&s, &url).await;
    assert_eq!(status, 200);
    assert!(html.contains("secret-sauce"), "{html}");
    assert!(html.contains("the-marker-body"));
    assert!(html.contains("Read-only share"));
    assert!(csp.is_some_and(|c| c.contains("default-src 'self'")));

    // Unknown and malformed tokens share the same 404.
    let (status, _, _) = page(
        &s,
        "/shared/fgl_0000000000000000000000000000000000000000000",
    )
    .await;
    assert_eq!(status, 404);
    let (status, _, _) = page(&s, "/shared/nope").await;
    assert_eq!(status, 404);

    // Live membership: removing the item empties the page.
    let (status, _) = call(
        &s,
        "DELETE",
        "/collections/anna/runbook/items/anna/secret-sauce",
        Some(&anna),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (status, html, _) = page(&s, &url).await;
    assert_eq!(status, 200);
    assert!(!html.contains("the-marker-body"));
    assert!(html.contains("This collection is empty."));

    // Revoke: instant, and the page is gone. A second revoke
    // and revoking a stranger's id both 404.
    let (status, _) = call(
        &s,
        "DELETE",
        &format!("/collections/anna/runbook/share-links/{link_id}"),
        Some(&anna),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (status, _, _) = page(&s, &url).await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &s,
        "DELETE",
        &format!("/collections/anna/runbook/share-links/{link_id}"),
        Some(&anna),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (status, out) = call(
        &s,
        "GET",
        "/collections/anna/runbook/share-links",
        Some(&anna),
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(out["links"][0]["active"], false);
    assert!(out["links"][0]["revoked_at"].is_string());
}

#[tokio::test]
async fn share_link_authorisation() {
    let s = start().await;
    let (anna, csrf) = fixture(&s).await;
    let (bob, bob_csrf) = signup(&s, "bob").await;

    // Another user: masked 404 on mint, list and revoke.
    for (method, path) in [
        ("POST", "/collections/anna/runbook/share-links".to_string()),
        ("GET", "/collections/anna/runbook/share-links".to_string()),
        (
            "DELETE",
            "/collections/anna/runbook/share-links/1".to_string(),
        ),
    ] {
        let (status, _) = call(
            &s,
            method,
            &path,
            Some(&bob),
            Some(&bob_csrf),
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, 404, "{method} {path}");
    }
    // Anonymous mint: 401.
    let (status, _) = call(
        &s,
        "POST",
        "/collections/anna/runbook/share-links",
        None,
        None,
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, 401);
    // A bad expiry is a 400 for the owner.
    let (status, _) = call(
        &s,
        "POST",
        "/collections/anna/runbook/share-links",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({ "expires_in_days": 2 })),
    )
    .await;
    assert_eq!(status, 400);
    // A read-only API token cannot mint (403).
    let (status, tok) = call(
        &s,
        "POST",
        "/tokens",
        Some(&anna),
        Some(&csrf),
        Some(serde_json::json!({ "name": "ro", "scopes": ["read"] })),
    )
    .await;
    assert_eq!(status, 201, "{tok}");
    let bearer = format!("Bearer {}", tok["token"].as_str().unwrap());
    let (status, _) = call(
        &s,
        "POST",
        "/collections/anna/runbook/share-links",
        Some(&bearer),
        None,
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, 403);
}
