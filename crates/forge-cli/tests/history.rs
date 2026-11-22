//! Peak C3 integration tests: `fg history` and `fg rollback`
//! against the real server - version listing, rollback as a
//! new version (history never rewritten), the no-op case,
//! and the local-only explanation.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_forgehub");

struct Rig {
    base: String,
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
    fn write_local(&self, name: &str, script: &str) {
        let dir = self.p("fg").join("snippets");
        std::fs::create_dir_all(&dir).unwrap();
        let yaml = format!(
            "name: {name}\ndescription: Test snippet {name}\nmode: script\nscript: |\n  {script}\n"
        );
        std::fs::write(dir.join(format!("{name}.yaml")), yaml).unwrap();
    }
}

fn start_server() -> String {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("server.db");
    let web_dir = dir.path().join("no-web");
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
            map.insert("FORGEHUB_DEVICE_POLL_INTERVAL_SECS".into(), "1".into());
            map.insert("FORGEHUB_LOGIN_BACKOFF_BASE_MS".into(), "60".into());
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

fn rig(base: &str) -> Rig {
    Rig {
        base: base.to_string(),
        home: tempfile::tempdir().unwrap(),
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

fn cli_login(rig: &Rig, token: &str) {
    let (code, _, stderr) = rig.run(
        &["login", "--registry", &rig.base, "--token"],
        Some(token.as_bytes()),
    );
    assert_eq!(code, 0, "cli login: {stderr}");
}

fn api_get(base: &str, token: &str, path: &str) -> (u16, serde_json::Value) {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("{base}{path}"))
        .bearer_auth(token)
        .send()
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().unwrap_or(serde_json::Value::Null))
}

/// Anna with `hello` at v2 (v1: echo ONE, v2: echo TWO).
fn anna_with_two_versions(base: &str) -> (Rig, String) {
    let anna = rig(base);
    let token = signup_and_token(base, "anna");
    cli_login(&anna, &token);
    anna.write_local("hello", "echo ONE");
    let (code, _, err) = anna.run(&["push", "hello", "--public"], None);
    assert_eq!(code, 0, "{err}");
    anna.write_local("hello", "echo TWO");
    let (code, _, err) = anna.run(&["push", "hello"], None);
    assert_eq!(code, 0, "{err}");
    (anna, token)
}

#[test]
fn history_lists_versions_newest_first() {
    let base = start_server();
    let (anna, _token) = anna_with_two_versions(&base);
    let (code, out, err) = anna.run(&["history", "hello"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("history of anna/hello"), "{out}");
    let v2 = out.find("v2").unwrap();
    let v1 = out.find("v1").unwrap();
    assert!(v2 < v1, "newest first: {out}");
    assert!(out.contains("(current)"), "{out}");
    // Someone else's public history is readable too.
    let bob = rig(&base);
    cli_login(&bob, &signup_and_token(&base, "bob"));
    let (code, out, _) = bob.run(&["history", "anna/hello"], None);
    assert_eq!(code, 0);
    assert!(out.contains("v1"), "{out}");
}

#[test]
fn rollback_publishes_a_new_version_with_the_old_content() {
    let base = start_server();
    let (anna, token) = anna_with_two_versions(&base);
    let (code, out, err) = anna.run(&["-y", "rollback", "hello", "1"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("v2 -> v3 (content of v1)"), "{out}");
    // The server agrees: latest is v3, body is v1's, and the
    // version list kept v2 (nothing was rewritten).
    let (status, latest) = api_get(&base, &token, "/api/v1/snippets/anna/hello");
    assert_eq!(status, 200);
    assert_eq!(latest["version"], 3);
    assert!(latest["body"].as_str().unwrap().contains("echo ONE"));
    let (_, versions) = api_get(&base, &token, "/api/v1/snippets/anna/hello/versions");
    let list = versions["versions"].as_array().unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0]["message"], "Rollback to v1");

    // Rolling back to the same content again is a no-op.
    let (code, out, _) = anna.run(&["-y", "rollback", "hello", "1"], None);
    assert_eq!(code, 0);
    assert!(out.contains("already at v1's content"), "{out}");
    let (_, versions) = api_get(&base, &token, "/api/v1/snippets/anna/hello/versions");
    assert_eq!(versions["versions"].as_array().unwrap().len(), 3);

    // A missing version is a clean not-found.
    let (code, _, _) = anna.run(&["-y", "rollback", "hello", "9"], None);
    assert_eq!(code, 3);
}

#[test]
fn rollback_of_someone_elses_snippet_is_refused() {
    let base = start_server();
    let (_anna, _token) = anna_with_two_versions(&base);
    let bob = rig(&base);
    cli_login(&bob, &signup_and_token(&base, "bob"));
    let (code, _, err) = bob.run(&["-y", "rollback", "anna/hello", "1"], None);
    assert_ne!(code, 0);
    assert!(err.contains("only the owner can publish"), "{err}");
}

#[test]
fn history_of_a_local_only_snippet_explains_itself() {
    let base = start_server();
    let (anna, _token) = anna_with_two_versions(&base);
    anna.write_local("local-only", "echo hi");
    let (code, out, err) = anna.run(&["history", "local-only"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("local files have no version history"), "{out}");
    assert!(out.contains("fg push local-only"), "{out}");
}
