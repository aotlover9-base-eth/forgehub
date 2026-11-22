//! Usage stats tests (Peak C8): run reports are validated,
//! aggregates count only the reporter's own runs, and other
//! users' runs of your snippets never enter your numbers.

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
    let db = dir.path().join("stats.db");
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

fn report(
    refr: &str,
    commands: i64,
    duration_ms: i64,
    success: bool,
    saved: i64,
) -> serde_json::Value {
    serde_json::json!({
        "ref": refr,
        "version": 1,
        "commands": commands,
        "duration_ms": duration_ms,
        "success": success,
        "estimated_saved_ms": saved,
    })
}

async fn post_run(s: &TestServer, cookie: &str, csrf: &str, payload: serde_json::Value) -> u16 {
    let resp = s
        .client
        .post(format!("{}/api/v1/me/runs", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&payload)
        .send()
        .await
        .unwrap();
    resp.status().as_u16()
}

async fn stats(s: &TestServer, cookie: &str) -> serde_json::Value {
    let resp = s
        .client
        .get(format!("{}/api/v1/me/stats", s.base))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.json().await.unwrap()
}

#[tokio::test]
async fn stats_aggregate_own_runs_only() {
    let s = start().await;
    let (anna, anna_csrf) = signup(&s, "anna").await;
    let (bob, bob_csrf) = signup(&s, "bob").await;

    // Anna runs her own snippet twice (one failure) and bob's once.
    assert_eq!(
        post_run(
            &s,
            &anna,
            &anna_csrf,
            report("anna/deploy", 3, 900, true, 35_100)
        )
        .await,
        201
    );
    assert_eq!(
        post_run(
            &s,
            &anna,
            &anna_csrf,
            report("anna/deploy", 3, 1_500, false, 34_500)
        )
        .await,
        201
    );
    assert_eq!(
        post_run(
            &s,
            &anna,
            &anna_csrf,
            report("bob/setup", 2, 400, true, 23_600)
        )
        .await,
        201
    );
    // Bob runs anna's snippet: this must NOT enter anna's stats.
    assert_eq!(
        post_run(
            &s,
            &bob,
            &bob_csrf,
            report("anna/deploy", 3, 800, true, 35_200)
        )
        .await,
        201
    );

    let out = stats(&s, &anna).await;
    assert_eq!(out["runs"], 3);
    assert_eq!(out["commands"], 8);
    assert_eq!(out["total_ms"], 2_800);
    assert_eq!(out["failures"], 1);
    assert_eq!(out["estimated_saved_ms"], 93_200);
    let workflows = out["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 2);
    assert_eq!(workflows[0]["ref"], "anna/deploy");
    assert_eq!(workflows[0]["runs"], 2);
    assert_eq!(workflows[0]["failures"], 1);
    assert!(workflows[0]["last_run"].is_string());

    let bob_out = stats(&s, &bob).await;
    assert_eq!(bob_out["runs"], 1);
    assert_eq!(bob_out["workflows"][0]["ref"], "anna/deploy");
}

#[tokio::test]
async fn run_reports_are_validated() {
    let s = start().await;
    let (anna, csrf) = signup(&s, "anna").await;
    for bad in [
        serde_json::json!({ "ref": "noslash", "commands": 1, "duration_ms": 1, "success": true, "estimated_saved_ms": 0 }),
        serde_json::json!({ "ref": "a/b/c", "commands": 1, "duration_ms": 1, "success": true, "estimated_saved_ms": 0 }),
        serde_json::json!({ "ref": "anna/deploy", "commands": -1, "duration_ms": 1, "success": true, "estimated_saved_ms": 0 }),
        serde_json::json!({ "ref": "anna/deploy", "commands": 1, "duration_ms": 86_400_001, "success": true, "estimated_saved_ms": 0 }),
        serde_json::json!({ "ref": "anna/deploy", "commands": 1, "duration_ms": 1, "success": true, "estimated_saved_ms": -5 }),
    ] {
        assert_eq!(post_run(&s, &anna, &csrf, bad).await, 400);
    }
    // Anonymous report: 401. Anonymous stats: 401.
    let resp = s
        .client
        .post(format!("{}/api/v1/me/runs", s.base))
        .json(&report("anna/deploy", 1, 1, true, 0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let resp = s
        .client
        .get(format!("{}/api/v1/me/stats", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    // Nothing was recorded by the invalid attempts.
    let out = stats(&s, &anna).await;
    assert_eq!(out["runs"], 0);
}
