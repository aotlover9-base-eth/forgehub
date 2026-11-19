//! Snippet API tests (Step 9): the authorisation matrix is the
//! priority — every endpoint × {anonymous, other user, owner,
//! read token, write token} × {private, unlisted, public} — plus
//! versions, rename redirects, optimistic concurrency, secret
//! scanning, and pagination.

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
    let db = dir.path().join("snippets.db");
    let mut map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL",
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR",
            dir.path().join("no-web").display().to_string(),
        ),
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

/// Create a snippet via session; returns (status, json).
async fn create(
    s: &TestServer,
    cookie: &str,
    csrf: &str,
    payload: serde_json::Value,
) -> (u16, serde_json::Value) {
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&payload)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

fn script_payload(name: &str, visibility: &str, body: &str) -> serde_json::Value {
    serde_json::json!({ "name": name, "visibility": visibility, "body": body })
}

async fn get_as(
    s: &TestServer,
    path: &str,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> (u16, serde_json::Value) {
    let mut req = s.client.get(format!("{}{path}", s.base));
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    if let Some(t) = bearer {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn read_authorisation_matrix() {
    let s = start(&[]).await;
    let (owner, oc) = signup(&s, "owner1").await;
    let (other, _oc2) = signup(&s, "other1").await;
    let owner_read = make_token(&s, &owner, &oc, &["read"]).await;
    let owner_write = make_token(&s, &owner, &oc, &["write"]).await;
    let other_read = make_token(&s, &other, &_oc2, &["read"]).await;
    for visibility in ["private", "unlisted", "public"] {
        let name = format!("snip-{visibility}");
        let (status, _) = create(
            &s,
            &owner,
            &oc,
            script_payload(&name, visibility, "echo hi\n"),
        )
        .await;
        assert_eq!(status, 201, "create {visibility}");
        let path = format!("/api/v1/snippets/owner1/{name}");
        // anonymous
        let (anon, _) = get_as(&s, &path, None, None).await;
        // other user (session), owner (session), owner's tokens, other's token
        let (oth, _) = get_as(&s, &path, Some(&other), None).await;
        let (own, _) = get_as(&s, &path, Some(&owner), None).await;
        let (ort, _) = get_as(&s, &path, None, Some(&owner_read)).await;
        let (owt, _) = get_as(&s, &path, None, Some(&owner_write)).await;
        let (xrt, _) = get_as(&s, &path, None, Some(&other_read)).await;
        let expected_anon = if visibility == "private" { 404 } else { 200 };
        assert_eq!(anon, expected_anon, "anon x {visibility}");
        assert_eq!(
            oth, expected_anon,
            "other user x {visibility} (unlisted/public readable)"
        );
        assert_eq!(xrt, expected_anon, "other's token x {visibility}");
        assert_eq!(own, 200, "owner x {visibility}");
        assert_eq!(ort, 200, "owner read token x {visibility}");
        assert_eq!(owt, 200, "owner write token x {visibility}");
        // The same rule holds on the sub-resources.
        for suffix in ["/versions", "/raw", "/versions/1"] {
            let (st, _) = get_as(&s, &format!("{path}{suffix}"), None, None).await;
            assert_eq!(st, expected_anon, "anon x {visibility} x {suffix}");
        }
    }
}

#[tokio::test]
async fn create_normalises_and_hashes_like_the_cli() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "carol").await;
    // Plain script text is wrapped; hash equals forge-core's.
    let (status, v) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("hello", "public", "echo hello\n"),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(v["version"], 1);
    assert_eq!(v["summary"]["mode"], "script");
    let parsed = forge_core::Snippet::from_yaml(v["body"].as_str().unwrap()).unwrap();
    assert_eq!(
        v["hash"],
        forge_core::hash::content_hash(&parsed),
        "server hash == CLI hash"
    );
    assert_eq!(v["short_hash"], &v["hash"].as_str().unwrap()[..8]);
    // A forge file (steps mode) parses, summarises, lints.
    let yaml = "name: ubuntu-setup\ndescription: Set up Ubuntu\nsteps:\n  - run: sudo apt update\n  - run: rm -rf /tmp/build\n";
    let (status, v) = create(
        &s,
        &cookie,
        &csrf,
        serde_json::json!({ "name": "ubuntu-setup", "visibility": "private", "body": yaml }),
    )
    .await;
    assert_eq!(status, 201, "{v}");
    assert_eq!(v["summary"]["mode"], "steps");
    assert_eq!(v["summary"]["steps_count"], 2);
    assert_eq!(v["summary"]["uses_sudo"], true);
    // Duplicate name -> 409; reserved name -> 400; case -> 400.
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("hello", "public", "echo x\n"),
    )
    .await;
    assert_eq!(status, 409);
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("login", "public", "echo x\n"),
    )
    .await;
    assert_eq!(status, 400, "reserved word");
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("Hello", "public", "echo x\n"),
    )
    .await;
    assert_eq!(status, 400, "uppercase rejected");
}

#[tokio::test]
async fn create_rejects_secrets_and_bad_input() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "dave").await;
    let secret = "q".repeat(24);
    let body = format!("export TOKEN={secret}\n");
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&script_payload("leaky", "private", &body))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422);
    let text = resp.text().await.unwrap();
    assert!(text.contains("secret_detected"), "{text}");
    assert!(
        !text.contains(&secret),
        "the secret value must never be echoed"
    );
    // Name mismatch between file and request -> 400.
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        serde_json::json!({ "name": "a", "body": "name: b\nsteps:\n  - echo hi\n" }),
    )
    .await;
    assert_eq!(status, 400);
    // Broken forge-looking YAML -> 400 (not silently wrapped).
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        serde_json::json!({ "name": "broken", "body": "name: broken\nsteps: [unclosed\n" }),
    )
    .await;
    assert_eq!(status, 400);
    // Oversized body -> 413.
    let big = "echo ".to_string() + &"x".repeat(70_000);
    let (status, _) = create(&s, &cookie, &csrf, script_payload("big", "private", &big)).await;
    assert_eq!(status, 413);
    // Unauthenticated create -> 401; read-only token -> 403.
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .json(&script_payload("nope", "private", "echo hi\n"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let read_token = make_token(&s, &cookie, &csrf, &["read"]).await;
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("authorization", format!("Bearer {read_token}"))
        .json(&script_payload("nope", "private", "echo hi\n"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "insufficient_scope");
    // Session create without CSRF -> 403.
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", &cookie)
        .json(&script_payload("nope", "private", "echo hi\n"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

async fn put(
    s: &TestServer,
    cookie: Option<&str>,
    csrf: Option<&str>,
    bearer: Option<&str>,
    path: &str,
    if_match: Option<i64>,
    payload: serde_json::Value,
) -> (u16, serde_json::Value) {
    let mut req = s.client.put(format!("{}{path}", s.base));
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    if let Some(c) = csrf {
        req = req.header("x-csrf-token", c);
    }
    if let Some(t) = bearer {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(v) = if_match {
        req = req.header("if-match", format!("\"{v}\""));
    }
    let resp = req.json(&payload).send().await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn put_versions_immutable_with_optimistic_concurrency() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "erin").await;
    let (other, oc2) = signup(&s, "frank").await;
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("tool", "public", "echo v1\n"),
    )
    .await;
    assert_eq!(status, 201);
    let path = "/api/v1/snippets/erin/tool";
    let payload = script_payload("tool", "public", "echo v2\n");
    // Missing If-Match -> 400; stale -> 412; non-owner -> 404.
    let (status, _) = put(
        &s,
        Some(&cookie),
        Some(&csrf),
        None,
        path,
        None,
        payload.clone(),
    )
    .await;
    assert_eq!(status, 400);
    let (status, v) = put(
        &s,
        Some(&cookie),
        Some(&csrf),
        None,
        path,
        Some(7),
        payload.clone(),
    )
    .await;
    assert_eq!(status, 412);
    assert_eq!(v["error"]["details"]["latest_version"], 1);
    let (status, _) = put(
        &s,
        Some(&other),
        Some(&oc2),
        None,
        path,
        Some(1),
        payload.clone(),
    )
    .await;
    assert_eq!(
        status, 404,
        "non-owner write is a 404 even on public snippets"
    );
    // Identical body -> unchanged, no new version.
    let (status, v) = put(
        &s,
        Some(&cookie),
        Some(&csrf),
        None,
        path,
        Some(1),
        script_payload("tool", "public", "echo v1\n"),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(v["unchanged"], true);
    assert_eq!(v["version"], 1);
    // Real change -> version 2; version 1 stays intact.
    let (status, v) = put(&s, Some(&cookie), Some(&csrf), None, path, Some(1), payload).await;
    assert_eq!(status, 200);
    assert_eq!(v["unchanged"], false);
    assert_eq!(v["version"], 2);
    let (_, v1) = get_as(&s, &format!("{path}?version=1"), None, None).await;
    assert!(
        v1["body"].as_str().unwrap().contains("echo v1"),
        "v1 immutable"
    );
    let (_, list) = get_as(&s, &format!("{path}/versions"), None, None).await;
    let versions = list["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["version"], 2, "newest first");
    // A write token can PUT; a read token cannot.
    let write_token = make_token(&s, &cookie, &csrf, &["write"]).await;
    let read_token = make_token(&s, &cookie, &csrf, &["read"]).await;
    let (status, _) = put(
        &s,
        None,
        None,
        Some(&read_token),
        path,
        Some(2),
        script_payload("tool", "public", "echo v3\n"),
    )
    .await;
    assert_eq!(status, 403);
    let (status, v) = put(
        &s,
        None,
        None,
        Some(&write_token),
        path,
        Some(2),
        script_payload("tool", "public", "echo v3\n"),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(v["version"], 3);
}

#[tokio::test]
async fn patch_metadata_and_rename_redirect() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "gwen").await;
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        serde_json::json!({ "name": "old-name", "visibility": "private", "body": "echo hi\n", "tags": ["a"] }),
    )
    .await;
    assert_eq!(status, 201);
    let patch = |payload: serde_json::Value| {
        let (s, cookie, csrf) = (&s, &cookie, &csrf);
        async move {
            let resp = s
                .client
                .patch(format!("{}/api/v1/snippets/gwen/old-name", s.base))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .json(&payload)
                .send()
                .await
                .unwrap();
            let status = resp.status().as_u16();
            (status, resp.json().await.unwrap_or(serde_json::Value::Null))
        }
    };
    // Metadata patch creates no version.
    let (status, v) = patch(serde_json::json!({ "description": "New desc", "visibility": "public", "tags": ["b", "c"] })).await;
    assert_eq!(status, 200);
    assert_eq!(v["description"], "New desc");
    assert_eq!(v["visibility"], "public");
    assert_eq!(v["tags"], serde_json::json!(["b", "c"]));
    assert_eq!(v["version"], 1, "PATCH must not create a version");
    // Rename: old name resolves with renamed_from; writes don't follow.
    let (status, v) = patch(serde_json::json!({ "name": "new-name" })).await;
    assert_eq!(status, 200);
    assert_eq!(v["name"], "new-name");
    let (status, v) = get_as(&s, "/api/v1/snippets/gwen/old-name", None, None).await;
    assert_eq!(status, 200, "redirect resolves for reads");
    assert_eq!(v["name"], "new-name");
    assert_eq!(v["renamed_from"], "old-name");
    let (status, _) = put(
        &s,
        Some(&cookie),
        Some(&csrf),
        None,
        "/api/v1/snippets/gwen/old-name",
        Some(1),
        script_payload("old-name", "public", "echo x\n"),
    )
    .await;
    assert_eq!(status, 404, "writes do not follow redirects");
    // Renaming onto an existing name -> 409.
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("taken", "private", "echo t\n"),
    )
    .await;
    assert_eq!(status, 201);
    let resp = s
        .client
        .patch(format!("{}/api/v1/snippets/gwen/new-name", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "name": "taken" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    // A new snippet with the old name shadows the redirect.
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("old-name", "private", "echo fresh\n"),
    )
    .await;
    assert_eq!(status, 201);
    let (_, v) = get_as(&s, "/api/v1/snippets/gwen/old-name", Some(&cookie), None).await;
    assert!(v["body"].as_str().unwrap().contains("echo fresh"));
    assert!(v.get("renamed_from").is_none());
}

#[tokio::test]
async fn delete_soft_and_name_reserved() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "henry").await;
    let (other, oc2) = signup(&s, "iris").await;
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("gone", "public", "echo hi\n"),
    )
    .await;
    assert_eq!(status, 201);
    let resp = s
        .client
        .delete(format!("{}/api/v1/snippets/henry/gone", s.base))
        .header("cookie", &other)
        .header("x-csrf-token", &oc2)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "non-owner delete");
    let resp = s
        .client
        .delete(format!("{}/api/v1/snippets/henry/gone", s.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    let (status, _) = get_as(&s, "/api/v1/snippets/henry/gone", Some(&cookie), None).await;
    assert_eq!(status, 404, "deleted is gone for everyone, owner included");
    // The name stays reserved during the 30-day retention.
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("gone", "public", "echo back\n"),
    )
    .await;
    assert_eq!(status, 409);
}

#[tokio::test]
async fn diff_raw_and_etag() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "judy").await;
    let (status, _) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("d", "public", "echo one\necho two\n"),
    )
    .await;
    assert_eq!(status, 201);
    let (status, _) = put(
        &s,
        Some(&cookie),
        Some(&csrf),
        None,
        "/api/v1/snippets/judy/d",
        Some(1),
        script_payload("d", "public", "echo one\necho 2\n"),
    )
    .await;
    assert_eq!(status, 200);
    let (_, v) = get_as(&s, "/api/v1/snippets/judy/d/diff?from=1&to=2", None, None).await;
    let diff = v["diff"].as_str().unwrap();
    assert!(
        diff.contains("-  echo two") || diff.contains("-echo two") || diff.contains("echo two"),
        "{diff}"
    );
    assert!(diff.contains("echo 2"), "{diff}");
    let (status, _) = get_as(&s, "/api/v1/snippets/judy/d/diff?from=1&to=9", None, None).await;
    assert_eq!(status, 404);
    // Raw: text/plain, exact canonical body.
    let resp = s
        .client
        .get(format!("{}/api/v1/snippets/judy/d/raw", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let raw = resp.text().await.unwrap();
    let (_, detail) = get_as(&s, "/api/v1/snippets/judy/d", None, None).await;
    assert_eq!(raw, detail["body"].as_str().unwrap());
    // ETag / If-None-Match.
    let resp = s
        .client
        .get(format!("{}/api/v1/snippets/judy/d", s.base))
        .send()
        .await
        .unwrap();
    let etag = resp
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let resp = s
        .client
        .get(format!("{}/api/v1/snippets/judy/d", s.base))
        .header("if-none-match", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 304);
}

#[tokio::test]
async fn my_snippets_filters_and_pagination() {
    let s = start(&[]).await;
    let (cookie, csrf) = signup(&s, "kyle").await;
    for (name, vis, tags) in [
        ("alpha", "public", vec!["web"]),
        ("beta", "private", vec!["web", "db"]),
        ("gamma", "private", vec!["db"]),
    ] {
        let (status, _) = create(
            &s,
            &cookie,
            &csrf,
            serde_json::json!({ "name": name, "visibility": vis, "body": "echo x\n", "tags": tags }),
        )
        .await;
        assert_eq!(status, 201);
    }
    let (_, v) = get_as(&s, "/api/v1/me/snippets", Some(&cookie), None).await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 3);
    assert!(v["next_cursor"].is_null());
    let (_, v) = get_as(
        &s,
        "/api/v1/me/snippets?visibility=private",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 2);
    let (_, v) = get_as(&s, "/api/v1/me/snippets?tag=db", Some(&cookie), None).await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 2);
    let (_, v) = get_as(&s, "/api/v1/me/snippets?sort=name", Some(&cookie), None).await;
    let names: Vec<&str> = v["snippets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["alpha", "beta", "gamma"]);
    // Pagination: limit 2 -> cursor -> last page.
    let (_, v) = get_as(&s, "/api/v1/me/snippets?limit=2", Some(&cookie), None).await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 2);
    let cursor = v["next_cursor"].as_str().unwrap().to_string();
    let (_, v) = get_as(
        &s,
        &format!("/api/v1/me/snippets?limit=2&cursor={cursor}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 1);
    assert!(v["next_cursor"].is_null());
    // Anonymous -> 401; garbage cursor -> 400.
    let (status, _) = get_as(&s, "/api/v1/me/snippets", None, None).await;
    assert_eq!(status, 401);
    let (status, _) = get_as(&s, "/api/v1/me/snippets?cursor=!!!", Some(&cookie), None).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn snippet_count_limit() {
    let s = start(&[("FORGEHUB_MAX_SNIPPETS_PER_USER", "2")]).await;
    let (cookie, csrf) = signup(&s, "liam").await;
    for name in ["one", "two"] {
        let (status, _) = create(
            &s,
            &cookie,
            &csrf,
            script_payload(name, "private", "echo x\n"),
        )
        .await;
        assert_eq!(status, 201);
    }
    let (status, v) = create(
        &s,
        &cookie,
        &csrf,
        script_payload("three", "private", "echo x\n"),
    )
    .await;
    assert_eq!(status, 400);
    assert!(
        v["error"]["message"].as_str().unwrap().contains("limit"),
        "{v}"
    );
}

#[tokio::test]
async fn validate_endpoint_reports_without_persisting() {
    let s = start(&[]).await;
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets/validate", s.base))
        .json(&script_payload("ok-name", "private", "echo fine\n"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["valid"], true);
    assert!(v["hash"].as_str().unwrap().len() == 64);
    let secret = "q".repeat(24);
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets/validate", s.base))
        .json(&script_payload(
            "ok-name",
            "private",
            &format!("export TOKEN={secret}\n"),
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["valid"], false);
    assert_eq!(v["secret_findings"].as_array().unwrap().len(), 1);
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets/validate", s.base))
        .json(&serde_json::json!({ "name": "ok-name", "body": "name: other\nsteps: [broken\n" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["valid"], false);
    assert!(!v["errors"].as_array().unwrap().is_empty());
}
