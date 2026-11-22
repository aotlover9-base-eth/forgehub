//! Public snippet page tests (Peak C4): the server-rendered
//! page at /s/{owner}/{name} exists for public snippets
//! only, carries canonical + Open Graph metadata and the CLI
//! install line, escapes hostile content, and inherits the
//! app CSP. Everything else is the same 404.

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
    let db = dir.path().join("pages.db");
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
            "FORGEHUB_BASE_URL".to_string(),
            "https://forge.example".to_string(),
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

async fn create_snippet(
    s: &TestServer,
    cookie: &str,
    csrf: &str,
    name: &str,
    visibility: &str,
    description: &str,
    body: &str,
) {
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({
            "name": name,
            "visibility": visibility,
            "description": description,
            "body": body,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create snippet {name}");
}

async fn get_page(s: &TestServer, path: &str) -> (u16, String, String) {
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
        .map_or(String::new(), |v| v.to_str().unwrap().to_string());
    (status, resp.text().await.unwrap(), csp)
}

#[tokio::test]
async fn public_snippet_gets_a_full_page() {
    let s = start().await;
    let (anna, csrf) = signup(&s, "anna").await;
    create_snippet(
        &s,
        &anna,
        &csrf,
        "deploy",
        "public",
        "Deploy the thing",
        "echo deploying\n",
    )
    .await;
    let (status, html, csp) = get_page(&s, "/s/anna/deploy").await;
    assert_eq!(status, 200);
    assert!(
        html.contains("<title>deploy by anna - ForgeHub</title>"),
        "{html}"
    );
    assert!(
        html.contains("<link rel=\"canonical\" href=\"https://forge.example/s/anna/deploy\">"),
        "{html}"
    );
    assert!(html.contains("property=\"og:title\" content=\"deploy by anna - ForgeHub\""));
    assert!(html.contains("property=\"og:url\" content=\"https://forge.example/s/anna/deploy\""));
    assert!(html.contains("Deploy the thing"));
    assert!(html.contains("echo deploying"), "the body is rendered");
    assert!(html.contains("fg pull anna/deploy"), "the install line");
    assert!(html.contains("fg anna/deploy"), "the run line");
    assert!(
        html.contains("data-copy=\"pull-line\""),
        "copy button wiring"
    );
    assert!(
        csp.starts_with("default-src 'self'"),
        "CSP inherited: {csp}"
    );
    // The copy helper is self-hosted.
    let (status, js, _) = get_page(&s, "/s-assets/copy.js").await;
    assert_eq!(status, 200);
    assert!(js.contains("navigator.clipboard"));
}

#[tokio::test]
async fn hostile_content_is_escaped() {
    let s = start().await;
    let (anna, csrf) = signup(&s, "anna").await;
    create_snippet(
        &s,
        &anna,
        &csrf,
        "evil",
        "public",
        "<script>alert(1)</script>",
        "echo \"<img src=x onerror=alert(2)>\"\n",
    )
    .await;
    let (status, html, _) = get_page(&s, "/s/anna/evil").await;
    assert_eq!(status, 200);
    assert!(
        !html.contains("<script>alert(1)</script>"),
        "raw script leaked"
    );
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<img src=x onerror"), "raw img leaked");
    assert!(html.contains("&lt;img src=x onerror=alert(2)&gt;"));
    // The only script on the page is our self-hosted helper.
    assert_eq!(html.matches("<script").count(), 1, "{html}");
    assert!(html.contains("<script src=\"/s-assets/copy.js\" defer></script>"));
}

#[tokio::test]
async fn non_public_and_missing_are_404() {
    let s = start().await;
    let (anna, csrf) = signup(&s, "anna").await;
    create_snippet(&s, &anna, &csrf, "mine", "private", "", "echo hi\n").await;
    create_snippet(&s, &anna, &csrf, "linked", "unlisted", "", "echo hi\n").await;
    for path in [
        "/s/anna/mine",
        "/s/anna/linked",
        "/s/anna/nope",
        "/s/nobody/mine",
    ] {
        let (status, html, _) = get_page(&s, path).await;
        assert_eq!(status, 404, "{path}");
        assert!(html.contains("Not found"), "{path}");
        assert!(!html.contains("echo hi"), "{path} leaked a body");
    }
}
