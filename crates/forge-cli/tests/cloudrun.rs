//! Step 13 integration tests: run-from-cloud against the real
//! server - own snippets, the third-party trust model, changed
//! hashes, version pins, offline replay, display defences,
//! shadowing and suggestions.

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
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.cmd().args(args).stdin(Stdio::null()).output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }
    fn write_local_raw(&self, name: &str, yaml: &str) {
        let dir = self.p("fg").join("snippets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.yaml")), yaml).unwrap();
    }
    /// A script-mode snippet whose execution writes a marker
    /// file (proof it ran, independent of stdio inheritance).
    fn write_local(&self, name: &str, marker: &str) {
        let marker_path = self.p("marker.txt");
        self.write_local_raw(
            name,
            &format!(
                "name: {name}\ndescription: Run test {name}\nmode: script\nscript: |\n  echo ran-{marker} > {}\n",
                marker_path.display()
            ),
        );
    }
    fn marker(&self) -> Option<String> {
        std::fs::read_to_string(self.p("marker.txt")).ok()
    }
    fn delete_local(&self, name: &str) {
        std::fs::remove_file(self.p("fg").join("snippets").join(format!("{name}.yaml"))).unwrap();
    }
    /// Seed a trust record directly (the interactive typed-yes
    /// path is covered by unit tests; a subprocess here has no
    /// TTY, where the CLI must fail safe instead).
    fn seed_trust(&self, owner: &str, name: &str, hash: &str, version: i64) {
        let db_path = self.p("fg").join("trust.db");
        let conn = rusqlite::Connection::open(db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trust (
                owner TEXT NOT NULL, name TEXT NOT NULL, hash TEXT NOT NULL,
                version INTEGER NOT NULL, first_trusted_at TEXT NOT NULL,
                last_run_at TEXT NOT NULL, runs INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (owner, name));",
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO trust VALUES (?1, ?2, ?3, ?4, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
            rusqlite::params![owner, name, hash, version],
        )
        .unwrap();
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
    resp.json::<serde_json::Value>().unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

fn cli_login(rig: &Rig, token: &str) {
    let mut child = rig
        .cmd()
        .args(["login", "--registry", &rig.base, "--token"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(token.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "login: {}",
        String::from_utf8_lossy(&out.stderr)
    );
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

#[test]
fn own_cloud_snippet_runs_with_flag_yes_and_fails_safe_without() {
    let base = start_server();
    let token = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("cloudy", "cloudy");
    assert_eq!(a.run(&["push", "cloudy"]).0, 0);
    a.delete_local("cloudy");

    // No -y, no TTY: fail safe, nothing runs.
    let (code, _, stderr) = a.run(&["cloudy"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(a.marker().is_none(), "must not have run");

    // -y is allowed for my own snippet.
    let (code, stdout, stderr) = a.run(&["cloudy", "-y"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("exit code 0"), "{stdout}");
    assert_eq!(a.marker().as_deref(), Some("ran-cloudy\n"));

    // Trust was recorded by the run.
    let (code, stdout, _) = a.run(&["trust", "list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("anna/cloudy"), "{stdout}");
}

#[test]
fn third_party_needs_trust_then_flag_yes_works() {
    let base = start_server();
    let token_a = signup_and_token(&base, "anna");
    let token_b = signup_and_token(&base, "bob");
    let a = rig(&base);
    cli_login(&a, &token_a);
    a.write_local("shared", "shared");
    assert_eq!(a.run(&["push", "shared", "--public"]).0, 0);
    let (_, detail) = api_get(&base, &token_a, "/api/v1/snippets/anna/shared");
    let hash = detail["hash"].as_str().unwrap().to_string();

    let b = rig(&base);
    cli_login(&b, &token_b);
    // Untrusted: even -y must refuse (typed yes required). The
    // snippet's marker path points at A's home (it was authored
    // there) - execution anywhere writes that file.
    let (code, _, stderr) = b.run(&["run", "anna/shared", "-y"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("not yours"), "{stderr}");
    assert!(a.marker().is_none());

    // With the exact hash trusted, -y runs it.
    b.seed_trust("anna", "shared", &hash, 1);
    let (code, stdout, stderr) = b.run(&["run", "anna/shared", "-y"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("exit code 0"), "{stdout}");
    assert_eq!(a.marker().as_deref(), Some("ran-shared\n"));

    // Revoking trust restores the typed-yes requirement.
    let (code, _, _) = b.run(&["trust", "revoke", "anna/shared"]);
    assert_eq!(code, 0);
    std::fs::remove_file(a.p("marker.txt")).unwrap();
    let (code, _, _) = b.run(&["run", "anna/shared", "-y"]);
    assert_eq!(code, 2);
    assert!(a.marker().is_none());
}

#[test]
fn changed_hash_shows_diff_and_reconfirms() {
    let base = start_server();
    let token_a = signup_and_token(&base, "anna");
    let token_b = signup_and_token(&base, "bob");
    let a = rig(&base);
    cli_login(&a, &token_a);
    a.write_local("evolving", "v1");
    assert_eq!(a.run(&["push", "evolving", "--public"]).0, 0);
    let (_, detail) = api_get(&base, &token_a, "/api/v1/snippets/anna/evolving");
    let hash_v1 = detail["hash"].as_str().unwrap().to_string();

    let b = rig(&base);
    cli_login(&b, &token_b);
    b.seed_trust("anna", "evolving", &hash_v1, 1);
    // Bob fetches v1 once (populating his cache for the diff).
    let (code, _, _) = b.run(&["run", "anna/evolving", "--dry"]);
    assert_eq!(code, 0);

    // Anna changes the snippet.
    a.write_local("evolving", "v2");
    assert_eq!(a.run(&["push", "evolving"]).0, 0);

    // Bob's -y no longer suffices: changed content re-confirms,
    // and the diff is shown.
    let (code, _, stderr) = b.run(&["run", "anna/evolving", "-y"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("CHANGED"), "{stderr}");
    assert!(
        stderr.contains("ran-v2"),
        "diff shows the new line: {stderr}"
    );
    assert!(b.marker().is_none());
}

#[test]
fn version_pin_and_offline_replay() {
    let base = start_server();
    let token = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("pinned", "one");
    assert_eq!(a.run(&["push", "pinned"]).0, 0);
    a.write_local("pinned", "two");
    assert_eq!(a.run(&["push", "pinned"]).0, 0);
    a.delete_local("pinned");

    // Pin to v1: the card shows the v1 content.
    let (code, stdout, stderr) = a.run(&["run", "pinned@1", "--dry"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("version 1"), "{stdout}");
    assert!(stdout.contains("ran-one"), "{stdout}");

    // Fetch latest once (dry), then replay offline.
    let (code, _, _) = a.run(&["run", "anna/pinned", "--dry"]);
    assert_eq!(code, 0);
    let (code, _, stderr) = a.run(&["run", "anna/pinned", "--offline", "-y"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(a.marker().as_deref(), Some("ran-two\n"));

    // Never-fetched + offline = a clear error.
    let (code, _, stderr) = a.run(&["run", "anna/never-seen", "--offline", "-y"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("nothing cached"), "{stderr}");
}

#[test]
fn malicious_display_is_escaped() {
    let base = start_server();
    let token_a = signup_and_token(&base, "anna");
    let token_b = signup_and_token(&base, "bob");
    let a = rig(&base);
    cli_login(&a, &token_a);
    // The attack arrives as a raw script body through the API
    // (a forge file with raw control characters cannot even be
    // written locally - the parser rejects them). ANSI
    // clear-line, CR overwrite and a bidi override try to
    // disguise the real command in the description and body.
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/snippets"))
        .bearer_auth(&token_a)
        .json(&serde_json::json!({
            "name": "evil",
            "description": "nice\u{1b}[2K\rHIDDEN\u{202e}txt",
            "visibility": "public",
            "body": "echo innocent\necho pwned\u{1b}[2K\rSAFE\n",
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201, "api create evil snippet");

    let b = rig(&base);
    cli_login(&b, &token_b);
    let (code, stdout, stderr) = b.run(&["run", "anna/evil", "--dry"]);
    assert_eq!(code, 0, "{stderr}");
    let shown = format!("{stdout}{stderr}");
    assert!(!shown.contains('\u{1b}'), "raw ESC leaked into the card");
    assert!(!shown.contains('\u{202e}'), "raw bidi override leaked");
    assert!(!shown.contains('\r'), "raw CR leaked");
    assert!(shown.contains("\\e"), "escapes are shown visibly: {shown}");
    assert!(shown.contains("not yours"), "{shown}");
}

#[test]
fn local_shadowing_and_typo_suggestions() {
    let base = start_server();
    let token = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    // A cloud snippet named hello…
    a.write_local("hello", "cloud");
    assert_eq!(a.run(&["push", "hello"]).0, 0);
    // …and a DIFFERENT local one with the same name: local wins.
    a.write_local("hello", "local");
    let (code, _, stderr) = a.run(&["hello", "-y"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(a.marker().as_deref(), Some("ran-local\n"));

    // Typo: exit 3 with a suggestion, never an auto-run.
    std::fs::remove_file(a.p("marker.txt")).unwrap();
    let (code, _, stderr) = a.run(&["helo"]);
    assert_eq!(code, 3, "{stderr}");
    assert!(stderr.contains("hello"), "{stderr}");
    assert!(a.marker().is_none());
}

#[test]
fn activity_log_via_cli() {
    let base = start_server();
    let token = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("cloudy", "cloudy");
    assert_eq!(a.run(&["push", "cloudy"]).0, 0);
    a.delete_local("cloudy");

    // First confirmed run: a fresh trust grant, reported to
    // the activity log (Peak C7).
    let (code, _, stderr) = a.run(&["cloudy", "-y"]);
    assert_eq!(code, 0, "{stderr}");

    let (code, stdout, stderr) = a.run(&["activity"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("snippet_created"), "{stdout}");
    assert!(stdout.contains("snippet_trusted"), "{stdout}");
    assert!(stdout.contains("ref=anna/cloudy"), "{stdout}");
    assert!(stdout.contains("login"), "{stdout}");

    // A second run of already-trusted content is not a new
    // trust event.
    std::fs::remove_file(a.p("marker.txt")).unwrap();
    let (code, _, _) = a.run(&["cloudy", "-y"]);
    assert_eq!(code, 0);
    let (_, out) = api_get(&base, &token, "/api/v1/me/activity?limit=50");
    let trusted = out["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "snippet_trusted")
        .count();
    assert_eq!(trusted, 1, "{out}");
}

#[test]
fn cloud_runs_land_in_library_stats() {
    let base = start_server();
    let token = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("cloudy", "cloudy");
    assert_eq!(a.run(&["push", "cloudy"]).0, 0);
    a.delete_local("cloudy");

    // Before any run: empty stats.
    let (_, out) = api_get(&base, &token, "/api/v1/me/stats");
    assert_eq!(out["runs"], 0, "{out}");

    let (code, _, stderr) = a.run(&["cloudy", "-y"]);
    assert_eq!(code, 0, "{stderr}");

    let (_, out) = api_get(&base, &token, "/api/v1/me/stats");
    assert_eq!(out["runs"], 1, "{out}");
    assert_eq!(out["workflows"][0]["ref"], "anna/cloudy");
    assert_eq!(out["workflows"][0]["runs"], 1);
    assert_eq!(out["failures"], 0);
    // The snippet has one command; the estimate uses the
    // fg stats formula (commands x overhead - duration).
    assert!(out["estimated_saved_ms"].as_i64().unwrap() > 0, "{out}");
    assert!(out["workflows"][0]["last_run"].is_string());
}
