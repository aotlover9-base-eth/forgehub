//! Peak C5 CLI test: `fg import --from-gist` against the real
//! server, with the server's GitHub API base pointed at a
//! local mock (ADR 0010: tests never touch the real GitHub).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_forgehub");

fn mock_github() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = r#"{"id":"0123abcd","files":{"deploy.sh":{"filename":"deploy.sh","content":"echo from-gist\n","truncated":false}}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}")
}

fn start_server(github_base: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("server.db");
    let web_dir = dir.path().join("no-web");
    let github = github_base.to_string();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let _dir = dir;
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut map: HashMap<String, String> = HashMap::new();
            map.insert(
                "FORGEHUB_DATABASE_URL".into(),
                format!("sqlite://{}", db_path.display()),
            );
            map.insert("FORGEHUB_WEB_DIR".into(), web_dir.display().to_string());
            map.insert("FORGEHUB_GIST_API_BASE".into(), github);
            let config = forge_server::config::Config::from_vars(&|k| map.get(k).cloned()).unwrap();
            let pool = forge_server::db::connect(&config.database_url)
                .await
                .unwrap();
            forge_server::db::migrate(&pool).await.unwrap();
            let state = forge_server::state::AppState::new(
                config,
                forge_server::repo::Repos::sqlite(pool.clone()),
                pool,
            );
            let app = forge_server::http::build_router(state);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tx.send(format!("http://{addr}")).unwrap();
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });
    });
    rx.recv_timeout(Duration::from_secs(30)).unwrap()
}

struct Rig {
    home: tempfile::TempDir,
}

impl Rig {
    fn p(&self, name: &str) -> PathBuf {
        self.home.path().join(name)
    }
    fn run(&self, args: &[&str], stdin: Option<&[u8]>) -> (i32, String, String) {
        let mut child = Command::new(BIN)
            .args(args)
            .env("HOME", self.p("home"))
            .env("FORGEHUB_HOME", self.p("fg"))
            .env("XDG_CONFIG_HOME", self.p("xdg"))
            .env_remove("FORGEHUB_TOKEN")
            .env_remove("FORGEHUB_REGISTRY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(bytes) = stdin {
            child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
        }
        drop(child.stdin.take());
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }
}

fn signup_and_token(base: &str, username: &str) -> String {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/auth/signup"))
        .json(&serde_json::json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "password": "test-password-11",
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201, "signup {username}");
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let body: serde_json::Value = resp.json().unwrap();
    let csrf = body["csrf_token"].as_str().unwrap().to_string();
    let resp = client
        .post(format!("{base}/api/v1/tokens"))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "name": "test", "scopes": ["read", "write"] }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201, "mint token");
    let created: serde_json::Value = resp.json().unwrap();
    created["token"].as_str().unwrap().to_string()
}

#[test]
fn import_from_gist_via_cli() {
    let github = mock_github();
    let base = start_server(&github);
    let rig = Rig {
        home: tempfile::tempdir().unwrap(),
    };
    let token = signup_and_token(&base, "anna");
    let (code, _, err) = rig.run(
        &["login", "--registry", &base, "--token"],
        Some(token.as_bytes()),
    );
    assert_eq!(code, 0, "{err}");

    let (code, out, err) = rig.run(
        &[
            "import",
            "--from-gist",
            "https://gist.github.com/someone/0123abcd",
        ],
        None,
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("imported deploy (from deploy.sh)"), "{out}");

    // The snippet is really there (in the cloud), private,
    // with the gist's content.
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/snippets/anna/deploy"))
        .bearer_auth(&token)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().unwrap();
    assert_eq!(body["visibility"], "private");
    assert!(body["body"].as_str().unwrap().contains("echo from-gist"));

    // A bad URL is refused by the server with a clear error.
    let (code, _, err) = rig.run(
        &["import", "--from-gist", "https://example.com/anna/0123abcd"],
        None,
    );
    assert_ne!(code, 0);
    assert!(err.contains("gist"), "{err}");
}
