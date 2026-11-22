//! Peak C1 integration tests: the real CLI against the real
//! server - collection lifecycle commands, the run-a-collection
//! fallback (order + stop-on-failure), item filtering for other
//! users, and the trust model holding through collection runs.

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
    fn cmd(&self) -> Command {
        let mut c = Command::new(BIN);
        c.env("HOME", self.p("home"))
            .env("FORGEHUB_HOME", self.p("fg"))
            .env("XDG_CONFIG_HOME", self.p("xdg"))
            .env_remove("FORGEHUB_TOKEN")
            .env_remove("FORGEHUB_REGISTRY")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_TTY");
        c
    }
    fn run(&self, args: &[&str], stdin: Option<&[u8]>) -> (i32, String, String) {
        let mut child = self
            .cmd()
            .args(args)
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

fn push(rig: &Rig, name: &str, script: &str, visibility_flag: &str) {
    rig.write_local(name, script);
    let (code, _, stderr) = rig.run(&["push", name, visibility_flag], None);
    assert_eq!(code, 0, "push {name}: {stderr}");
}

#[test]
fn collection_lifecycle_via_cli() {
    let base = start_server();
    let anna = rig(&base);
    cli_login(&anna, &signup_and_token(&base, "anna"));
    push(&anna, "one", "echo one", "--public");
    push(&anna, "two", "echo two", "--public");

    let (code, out, err) = anna.run(&["collection", "create", "runbook"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("created collection anna/runbook"), "{out}");

    // Duplicate create -> server 409 mapped to an error.
    let (code, _, _) = anna.run(&["collection", "create", "runbook"], None);
    assert_ne!(code, 0);

    for name in ["anna/one", "anna/two"] {
        let (code, out, err) = anna.run(&["collection", "add", "runbook", name], None);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains(&format!("added {name}")), "{out}");
    }
    // Adding again is a friendly no-op.
    let (code, out, _) = anna.run(&["collection", "add", "runbook", "anna/one"], None);
    assert_eq!(code, 0);
    assert!(out.contains("already in"), "{out}");

    let (code, out, _) = anna.run(&["collection", "list"], None);
    assert_eq!(code, 0);
    assert!(out.contains("runbook (private, 2 item(s))"), "{out}");

    let (code, out, _) = anna.run(&["collection", "show", "runbook"], None);
    assert_eq!(code, 0);
    let one_at = out.find("anna/one").unwrap();
    let two_at = out.find("anna/two").unwrap();
    assert!(one_at < two_at, "insertion order: {out}");

    // Reorder sticks.
    let (code, _, err) = anna.run(
        &["collection", "order", "runbook", "anna/two", "anna/one"],
        None,
    );
    assert_eq!(code, 0, "{err}");
    let (_, out, _) = anna.run(&["collection", "show", "runbook"], None);
    assert!(
        out.find("anna/two").unwrap() < out.find("anna/one").unwrap(),
        "{out}"
    );

    // A partial reorder is refused by the server.
    let (code, _, _) = anna.run(&["collection", "order", "runbook", "anna/two"], None);
    assert_ne!(code, 0);

    // Visibility + remove + delete.
    let (code, out, _) = anna.run(&["collection", "visibility", "runbook", "public"], None);
    assert_eq!(code, 0);
    assert!(out.contains("is now public"), "{out}");
    let (code, out, _) = anna.run(&["collection", "remove", "runbook", "anna/one"], None);
    assert_eq!(code, 0);
    assert!(out.contains("removed anna/one"), "{out}");
    let (code, _, err) = anna.run(&["-y", "collection", "delete", "runbook"], None);
    assert_eq!(code, 0, "{err}");
    let (code, out, _) = anna.run(&["collection", "list"], None);
    assert_eq!(code, 0);
    assert!(out.contains("no collections yet"), "{out}");
}

#[test]
fn collection_run_executes_in_order_and_stops_on_failure() {
    let base = start_server();
    let anna = rig(&base);
    cli_login(&anna, &signup_and_token(&base, "anna"));
    push(&anna, "first", "echo FIRST-MARKER", "--public");
    push(&anna, "bad", "exit 3", "--public");
    push(&anna, "never", "echo NEVER-MARKER", "--public");

    anna.run(&["collection", "create", "pipeline"], None);
    for name in ["anna/first", "anna/bad", "anna/never"] {
        let (code, _, err) = anna.run(&["collection", "add", "pipeline", name], None);
        assert_eq!(code, 0, "{err}");
    }
    let (code, out, err) = anna.run(&["-y", "run", "pipeline"], None);
    assert_eq!(code, 3, "the failing member's code surfaces: {out} {err}");
    assert!(out.contains("FIRST-MARKER"), "{out}");
    assert!(!out.contains("NEVER-MARKER"), "run must stop: {out}");
    assert!(err.contains("stopped at anna/bad"), "{err}");

    // Without the failing member, the whole collection runs.
    let (code, _, _) = anna.run(&["collection", "remove", "pipeline", "anna/bad"], None);
    assert_eq!(code, 0);
    let (code, out, _) = anna.run(&["-y", "run", "pipeline"], None);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.find("FIRST-MARKER").unwrap() < out.find("NEVER-MARKER").unwrap(),
        "{out}"
    );
}

#[test]
fn collection_show_filters_and_foreign_runs_stay_gated() {
    let base = start_server();
    let anna = rig(&base);
    cli_login(&anna, &signup_and_token(&base, "anna"));
    push(&anna, "open", "echo OPEN-MARKER", "--public");
    push(&anna, "secret", "echo SECRET-MARKER", "--private");

    anna.run(
        &["collection", "create", "mixed", "--visibility", "public"],
        None,
    );
    for name in ["anna/open", "anna/secret"] {
        let (code, _, err) = anna.run(&["collection", "add", "mixed", name], None);
        assert_eq!(code, 0, "{err}");
    }

    let bob = rig(&base);
    cli_login(&bob, &signup_and_token(&base, "bob"));
    // Bob sees the public item only; the private item's name
    // must not appear anywhere in his view.
    let (code, out, err) = bob.run(&["collection", "show", "anna/mixed"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("anna/open"), "{out}");
    assert!(!out.contains("secret"), "private item leaked: {out}");

    // Bob running anna's collection: the first member is
    // someone else's snippet, so the typed-yes trust prompt
    // applies - with no TTY it refuses, and -y does not
    // bypass it. Nothing executes.
    for args in [vec!["run", "anna/mixed"], vec!["-y", "run", "anna/mixed"]] {
        let (code, out, _) = bob.run(&args, None);
        assert_ne!(code, 0, "foreign collection run must not proceed: {args:?}");
        assert!(!out.contains("OPEN-MARKER"), "{out}");
    }
}

#[test]
fn share_link_commands_round_trip() {
    let base = start_server();
    let anna = rig(&base);
    cli_login(&anna, &signup_and_token(&base, "anna"));
    push(&anna, "sauce", "echo SAUCE-MARKER", "--private");
    anna.run(&["collection", "create", "runbook"], None);
    let (code, _, err) = anna.run(&["collection", "add", "runbook", "anna/sauce"], None);
    assert_eq!(code, 0, "{err}");

    // Mint: the full URL prints once.
    let (code, out, err) = anna.run(&["collection", "share", "runbook"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("/shared/fgl_"), "{out}");
    let url = out
        .lines()
        .find(|l| l.contains("/shared/fgl_"))
        .unwrap()
        .trim()
        .to_string();

    // The anonymous page shows the private snippet's code.
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send().unwrap();
    assert_eq!(resp.status(), 200);
    let html = resp.text().unwrap();
    assert!(html.contains("SAUCE-MARKER"), "{html}");

    // List shows the prefix and an active state; bad expiry
    // is refused by the server.
    let (code, out, err) = anna.run(&["collection", "shares", "runbook"], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("active"), "{out}");
    let (code, _, err) = anna.run(&["collection", "share", "runbook", "--days", "2"], None);
    assert_ne!(code, 0);
    assert!(err.contains("1, 7 or 30"), "{err}");

    // Revoke by the listed id; the page dies with it.
    let (code, out, _) = anna.run(&["--json", "collection", "shares", "runbook"], None);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let id = parsed["links"][0]["id"].as_i64().unwrap();
    let (code, out, err) = anna.run(&["collection", "unshare", "runbook", &id.to_string()], None);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("revoked"), "{out}");
    let resp = client.get(&url).send().unwrap();
    assert_eq!(resp.status(), 404);
}
