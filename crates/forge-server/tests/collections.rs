//! Collection API tests (Peak C1). The invariant under test:
//! a collection NEVER widens access - private collections are
//! 404 to everyone but the owner, and items are filtered
//! through the snippet read rules for every viewer, including
//! when a snippet's visibility changes after it was added.

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
    let db = dir.path().join("collections.db");
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

async fn make_token(s: &TestServer, cookie: &str, csrf: &str, scopes: &[&str]) -> String {
    let resp = s
        .client
        .post(format!("{}/api/v1/tokens", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({ "name": "t", "scopes": scopes }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let v: serde_json::Value = resp.json().await.unwrap();
    v["token"].as_str().unwrap().to_string()
}

/// Create a snippet via session; asserts 201.
async fn create_snippet(s: &TestServer, cookie: &str, csrf: &str, name: &str, visibility: &str) {
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({
            "name": name,
            "visibility": visibility,
            "body": "echo hi\n",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create snippet {name}");
}

/// One call against the collections API. `auth` is a full
/// header value pair list built by the helpers below.
async fn call(
    s: &TestServer,
    method: reqwest::Method,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    token: Option<&str>,
    payload: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let mut req = s.client.request(method, format!("{}/api/v1{path}", s.base));
    if let Some(cookie) = cookie {
        req = req.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        req = req.header("x-csrf-token", csrf);
    }
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    if let Some(payload) = payload {
        req = req.json(&payload);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

fn item_refs(detail: &serde_json::Value) -> Vec<String> {
    detail["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            format!(
                "{}/{}",
                i["owner"].as_str().unwrap(),
                i["name"].as_str().unwrap()
            )
        })
        .collect()
}

/// Arrange: anna owns collection `mixed` (public) containing
/// anna/secret-ops (private), anna/open-ops (public) and
/// bob/bob-tool (public); bob also owns bob-diary (private).
/// Returns (anna cookie, anna csrf, bob cookie, bob csrf).
async fn arrange_mixed(s: &TestServer) -> (String, String, String, String) {
    let (anna, anna_csrf) = signup(s, "anna").await;
    let (bob, bob_csrf) = signup(s, "bob").await;
    create_snippet(s, &anna, &anna_csrf, "secret-ops", "private").await;
    create_snippet(s, &anna, &anna_csrf, "open-ops", "public").await;
    create_snippet(s, &bob, &bob_csrf, "bob-tool", "public").await;
    create_snippet(s, &bob, &bob_csrf, "bob-diary", "private").await;
    let (status, _) = call(
        s,
        reqwest::Method::POST,
        "/collections",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "name": "mixed", "visibility": "public" })),
    )
    .await;
    assert_eq!(status, 201);
    (anna, anna_csrf, bob, bob_csrf)
}

/// Add one item; returns the status.
async fn add(s: &TestServer, cookie: &str, csrf: &str, collection: &str, refr: &str) -> u16 {
    let (status, _) = call(
        s,
        reqwest::Method::PUT,
        &format!("/collections/anna/{collection}/items/{refr}"),
        Some(cookie),
        Some(csrf),
        None,
        None,
    )
    .await;
    status
}

/// Arrange: anna owns `ordered` (public) with one, two, three.
async fn arrange_ordered(s: &TestServer) -> (String, String, String, String) {
    let (anna, anna_csrf) = signup(s, "anna").await;
    let (bob, bob_csrf) = signup(s, "bob").await;
    for name in ["one", "two", "three"] {
        create_snippet(s, &anna, &anna_csrf, name, "public").await;
    }
    let (status, _) = call(
        s,
        reqwest::Method::POST,
        "/collections",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "name": "ordered", "visibility": "public" })),
    )
    .await;
    assert_eq!(status, 201);
    for name in ["one", "two", "three"] {
        assert_eq!(
            add(s, &anna, &anna_csrf, "ordered", &format!("anna/{name}")).await,
            201
        );
    }
    (anna, anna_csrf, bob, bob_csrf)
}

#[tokio::test]
async fn create_rules() {
    let s = start().await;
    let (anna, anna_csrf) = signup(&s, "anna").await;
    let (status, created) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "name": "runbook", "description": "ops" })),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(created["visibility"], "private");
    assert_eq!(created["item_count"], 0);
    // Duplicate name -> 409; bad name -> 400.
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "name": "runbook" })),
    )
    .await;
    assert_eq!(status, 409);
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "name": "Bad Name" })),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn private_collection_visibility_matrix() {
    let s = start().await;
    let (anna, anna_csrf) = signup(&s, "anna").await;
    let (bob, _bob_csrf) = signup(&s, "bob").await;
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "name": "runbook" })),
    )
    .await;
    assert_eq!(status, 201);

    // Owner reads it; bob and anonymous get the same 404.
    let (status, detail) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/runbook",
        Some(&anna),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(detail["total_items"], 0);
    for cookie in [Some(bob.as_str()), None] {
        let (status, _) = call(
            &s,
            reqwest::Method::GET,
            "/collections/anna/runbook",
            cookie,
            None,
            None,
            None,
        )
        .await;
        assert_eq!(status, 404, "private collection must 404 for non-owners");
    }

    // Public: bob and anonymous can read it now.
    let (status, _) = call(
        &s,
        reqwest::Method::PATCH,
        "/collections/anna/runbook",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "visibility": "public" })),
    )
    .await;
    assert_eq!(status, 200);
    let (status, _) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/runbook",
        None,
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);

    // /me/collections lists it for anna, not for bob.
    let (status, mine) = call(
        &s,
        reqwest::Method::GET,
        "/me/collections",
        Some(&anna),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(mine["collections"].as_array().unwrap().len(), 1);
    let (_, mine) = call(
        &s,
        reqwest::Method::GET,
        "/me/collections",
        Some(&bob),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(mine["collections"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn adding_items_rules() {
    let s = start().await;
    let (anna, anna_csrf, bob, bob_csrf) = arrange_mixed(&s).await;
    // Anna adds her private snippet, her public one, bob's public one.
    for refr in ["anna/secret-ops", "anna/open-ops", "bob/bob-tool"] {
        assert_eq!(
            add(&s, &anna, &anna_csrf, "mixed", refr).await,
            201,
            "add {refr}"
        );
    }
    // Duplicate add -> 409.
    assert_eq!(
        add(&s, &anna, &anna_csrf, "mixed", "anna/open-ops").await,
        409
    );
    // Anna cannot add bob's private snippet (same 404 as missing).
    assert_eq!(
        add(&s, &anna, &anna_csrf, "mixed", "bob/bob-diary").await,
        404
    );
    // Bob cannot add to anna's collection at all.
    assert_eq!(add(&s, &bob, &bob_csrf, "mixed", "bob/bob-tool").await, 404);
}

#[tokio::test]
async fn item_filtering_and_visibility_flip() {
    let s = start().await;
    let (anna, anna_csrf, bob, bob_csrf) = arrange_mixed(&s).await;
    for refr in ["anna/secret-ops", "anna/open-ops", "bob/bob-tool"] {
        assert_eq!(add(&s, &anna, &anna_csrf, "mixed", refr).await, 201);
    }
    // The owner sees all three items, in insertion order.
    let (_, detail) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/mixed",
        Some(&anna),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        item_refs(&detail),
        ["anna/secret-ops", "anna/open-ops", "bob/bob-tool"]
    );
    assert_eq!(detail["item_count"], 3);
    assert_eq!(detail["total_items"], 3);

    // Bob (and anonymous) see only the two public items; the
    // private one is invisible and the count does not leak it.
    for cookie in [Some(bob.as_str()), None] {
        let (_, detail) = call(
            &s,
            reqwest::Method::GET,
            "/collections/anna/mixed",
            cookie,
            None,
            None,
            None,
        )
        .await;
        assert_eq!(item_refs(&detail), ["anna/open-ops", "bob/bob-tool"]);
        assert_eq!(detail["item_count"], 2);
        assert!(detail.get("total_items").is_none());
    }

    // Bob's snippet goes private: it vanishes from the public
    // collection for everyone except bob himself.
    let resp = s
        .client
        .patch(format!("{}/api/v1/snippets/bob/bob-tool", s.base))
        .header("cookie", &bob)
        .header("x-csrf-token", &bob_csrf)
        .json(&serde_json::json!({ "visibility": "private" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let (_, detail) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/mixed",
        Some(&anna),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(item_refs(&detail), ["anna/secret-ops", "anna/open-ops"]);
    let (_, detail) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/mixed",
        Some(&bob),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        item_refs(&detail),
        ["anna/open-ops", "bob/bob-tool"],
        "bob still sees his own snippet in the list"
    );
}

#[tokio::test]
async fn reorder_rules() {
    let s = start().await;
    let (anna, anna_csrf, bob, bob_csrf) = arrange_ordered(&s).await;
    // Reorder by a non-owner -> 404; with a wrong set -> 400.
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections/anna/ordered/reorder",
        Some(&bob),
        Some(&bob_csrf),
        None,
        Some(serde_json::json!({ "items": ["anna/three", "anna/two", "anna/one"] })),
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections/anna/ordered/reorder",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "items": ["anna/three", "anna/one"] })),
    )
    .await;
    assert_eq!(status, 400);
    // A real reorder sticks.
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections/anna/ordered/reorder",
        Some(&anna),
        Some(&anna_csrf),
        None,
        Some(serde_json::json!({ "items": ["anna/three", "anna/one", "anna/two"] })),
    )
    .await;
    assert_eq!(status, 200);
    let (_, detail) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/ordered",
        None,
        None,
        None,
        None,
    )
    .await;
    assert_eq!(item_refs(&detail), ["anna/three", "anna/one", "anna/two"]);
}

#[tokio::test]
async fn remove_and_delete_rules() {
    let s = start().await;
    let (anna, anna_csrf, bob, bob_csrf) = arrange_ordered(&s).await;
    // Remove: non-owner 404, owner 204, again 404.
    let (status, _) = call(
        &s,
        reqwest::Method::DELETE,
        "/collections/anna/ordered/items/anna/one",
        Some(&bob),
        Some(&bob_csrf),
        None,
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &s,
        reqwest::Method::DELETE,
        "/collections/anna/ordered/items/anna/one",
        Some(&anna),
        Some(&anna_csrf),
        None,
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (status, _) = call(
        &s,
        reqwest::Method::DELETE,
        "/collections/anna/ordered/items/anna/one",
        Some(&anna),
        Some(&anna_csrf),
        None,
        None,
    )
    .await;
    assert_eq!(status, 404);
    // PATCH + DELETE by a non-owner are 404; owner's delete works.
    let (status, _) = call(
        &s,
        reqwest::Method::PATCH,
        "/collections/anna/ordered",
        Some(&bob),
        Some(&bob_csrf),
        None,
        Some(serde_json::json!({ "visibility": "private" })),
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &s,
        reqwest::Method::DELETE,
        "/collections/anna/ordered",
        Some(&anna),
        Some(&anna_csrf),
        None,
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (status, _) = call(
        &s,
        reqwest::Method::GET,
        "/collections/anna/ordered",
        Some(&anna),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn token_scopes_are_enforced() {
    let s = start().await;
    let (anna, anna_csrf) = signup(&s, "anna").await;
    let read_token = make_token(&s, &anna, &anna_csrf, &["read"]).await;
    let write_token = make_token(&s, &anna, &anna_csrf, &["read", "write"]).await;
    // Anonymous create -> 401.
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        None,
        None,
        None,
        Some(serde_json::json!({ "name": "nope" })),
    )
    .await;
    assert_eq!(status, 401);
    // Read-only token create -> 403.
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        None,
        None,
        Some(&read_token),
        Some(serde_json::json!({ "name": "nope" })),
    )
    .await;
    assert_eq!(status, 403);
    // Write token create -> 201; read token can list.
    let (status, _) = call(
        &s,
        reqwest::Method::POST,
        "/collections",
        None,
        None,
        Some(&write_token),
        Some(serde_json::json!({ "name": "via-token" })),
    )
    .await;
    assert_eq!(status, 201);
    let (status, mine) = call(
        &s,
        reqwest::Method::GET,
        "/me/collections",
        None,
        None,
        Some(&read_token),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(mine["collections"].as_array().unwrap().len(), 1);
}
