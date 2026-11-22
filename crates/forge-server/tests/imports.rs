//! Import endpoint tests (Peak C5): history analysis runs
//! the shared mining pipeline (secrets never come back), and
//! gist import follows ADR 0010 - the fetch target is built
//! from the parsed id against a local mock GitHub API, files
//! go through the creation pipeline, collisions and secrets
//! skip with reasons.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;
use std::io::{Read, Write};

struct TestServer {
    base: String,
    client: reqwest::Client,
    _dir: tempfile::TempDir,
}

/// A one-shot mock GitHub API: serves `GIST_DOC` for any
/// /gists/<id> path except /gists/missing (404).
fn mock_github() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let (status, body) = if request.contains("/gists/missing") {
                (404, "{\"message\":\"Not Found\"}".to_string())
            } else {
                (200, gist_doc())
            };
            let resp = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}")
}

/// The mock gist document. The AWS example key is assembled
/// from pieces on purpose: written out whole, it is exactly
/// the shape secret scanners (and redaction layers) eat.
fn gist_doc() -> String {
    let aws_key = format!("AKIA{}", "IOSFODNN7EXAMPLE");
    GIST_DOC.replace("__AWS_KEY__", &aws_key)
}

const GIST_DOC: &str = r#"{
  "id": "0123abcd",
  "files": {
    "deploy.sh": {
      "filename": "deploy.sh",
      "content": "echo deploying\n",
      "truncated": false
    },
    "setup": {
      "filename": "setup",
      "content": "name: my-setup\ndescription: From a gist\nsteps:\n  - run: echo setup\n",
      "truncated": false
    },
    "leaky.sh": {
      "filename": "leaky.sh",
      "content": "export AWS_SECRET_ACCESS_KEY=__AWS_KEY__\n",
      "truncated": false
    },
    "dupe.sh": {
      "filename": "dupe.sh",
      "content": "echo dupe\n",
      "truncated": false
    }
  }
}"#;

async fn start(github_base: &str) -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("imports.db");
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
            "FORGEHUB_GIST_API_BASE".to_string(),
            github_base.to_string(),
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

async fn post(
    s: &TestServer,
    path: &str,
    cookie: &str,
    csrf: &str,
    payload: serde_json::Value,
) -> (u16, serde_json::Value) {
    let resp = s
        .client
        .post(format!("{}/api/v1{path}", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&payload)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn history_analyze_mines_and_never_returns_secrets() {
    let s = start("https://api.github.com").await;
    let (anna, csrf) = signup(&s, "anna").await;
    let mut text = String::new();
    for _ in 0..3 {
        text.push_str("docker compose build\ndocker compose up -d\nkubectl get pods\n");
    }
    let aws_key = format!("AKIA{}", "IOSFODNN7EXAMPLE");
    text.push_str("export AWS_SECRET_ACCESS_KEY=");
    text.push_str(&aws_key);
    text.push('\n');
    text.push_str("curl -H 'Authorization: Bearer <redacted>' https://x\n");
    let (status, out) = post(
        &s,
        "/imports/history/analyze",
        &anna,
        &csrf,
        serde_json::json!({ "text": text }),
    )
    .await;
    assert_eq!(status, 200);
    let candidates = out["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty(), "{out}");
    assert_eq!(candidates[0]["name"], "docker-compose");
    let rendered = out.to_string();
    assert!(
        !rendered.contains("AKIAIOSFODNN7EXAMPLE"),
        "secret leaked: {rendered}"
    );
    assert!(
        !rendered.contains("some-token-value"),
        "bearer token leaked"
    );

    // Over the 64 KiB cap -> 413.
    let big = "x".repeat(65 * 1024);
    let (status, _) = post(
        &s,
        "/imports/history/analyze",
        &anna,
        &csrf,
        serde_json::json!({ "text": big }),
    )
    .await;
    assert_eq!(status, 413);
}

#[tokio::test]
async fn gist_import_creates_private_snippets_and_skips_with_reasons() {
    let github = mock_github();
    let s = start(&github).await;
    let (anna, csrf) = signup(&s, "anna").await;
    // Pre-create `dupe` so the gist's dupe.sh collides.
    let (status, _) = post(
        &s,
        "/snippets",
        &anna,
        &csrf,
        serde_json::json!({ "name": "dupe", "body": "echo mine\n" }),
    )
    .await;
    assert_eq!(status, 201);

    let (status, out) = post(
        &s,
        "/imports/gist",
        &anna,
        &csrf,
        serde_json::json!({ "url": "https://gist.github.com/someone/0123abcd" }),
    )
    .await;
    assert_eq!(status, 200, "{out}");
    let created: Vec<&str> = out["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(created, ["deploy", "my-setup"], "{out}");
    let skipped: Vec<(&str, &str)> = out["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["file"].as_str().unwrap(), c["reason"].as_str().unwrap()))
        .collect();
    assert_eq!(skipped.len(), 2, "{out}");
    assert!(skipped
        .iter()
        .any(|(f, r)| *f == "dupe.sh" && r.contains("already have")));
    assert!(
        skipped
            .iter()
            .any(|(f, r)| *f == "leaky.sh" && r.to_lowercase().contains("secret")),
        "{skipped:?}"
    );

    // The imports are real, private snippets.
    let resp = s
        .client
        .get(format!("{}/api/v1/snippets/anna/deploy", s.base))
        .header("cookie", &anna)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["visibility"], "private");
    assert!(body["body"].as_str().unwrap().contains("echo deploying"));
}

#[tokio::test]
async fn gist_import_rejects_bad_urls_and_missing_gists() {
    let github = mock_github();
    let s = start(&github).await;
    let (anna, csrf) = signup(&s, "anna").await;
    for url in [
        "https://evil.example/anna/0123abcd",
        "http://gist.github.com/anna/0123abcd",
        "https://gist.github.com/anna/0123abcd/../../etc",
        "not a url",
    ] {
        let (status, _) = post(
            &s,
            "/imports/gist",
            &anna,
            &csrf,
            serde_json::json!({ "url": url }),
        )
        .await;
        assert_eq!(status, 400, "{url}");
    }
    let (status, _) = post(
        &s,
        "/imports/gist",
        &anna,
        &csrf,
        serde_json::json!({ "url": "https://gist.github.com/anna/missing" }),
    )
    .await;
    assert_eq!(status, 404);
    // Anonymous import -> 401.
    let resp = s
        .client
        .post(format!("{}/api/v1/imports/gist", s.base))
        .json(&serde_json::json!({ "url": "https://gist.github.com/anna/0123abcd" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}
