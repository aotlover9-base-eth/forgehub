//! Step 11 integration tests: the real `forgehub` binary against
//! the real `forge-server` (in-process, ephemeral port), with a
//! scripted "browser" (plain HTTP calls with a session cookie)
//! approving or denying device codes.
//!
//! The CLI is driven as a subprocess with an isolated `HOME` /
//! `FORGEHUB_HOME` / `XDG_CONFIG_HOME` per test.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_forgehub");

struct Rig {
    base: String,
    db_path: PathBuf,
    home: tempfile::TempDir,
}

impl Rig {
    fn env_path(&self, name: &str) -> PathBuf {
        self.home.path().join(name)
    }
    fn credentials_path(&self) -> PathBuf {
        self.env_path("xdg")
            .join("forgehub")
            .join("credentials.toml")
    }
    /// Base command with an isolated environment.
    fn cmd(&self) -> Command {
        let mut c = Command::new(BIN);
        c.env("HOME", self.env_path("home"))
            .env("FORGEHUB_HOME", self.env_path("fg"))
            .env("XDG_CONFIG_HOME", self.env_path("xdg"))
            .env_remove("FORGEHUB_TOKEN")
            .env_remove("FORGEHUB_REGISTRY")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_TTY");
        c
    }
    /// Run the CLI to completion with optional stdin bytes.
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
    /// Spawn a long-running CLI command (device login) to drive
    /// from the test while it polls.
    fn spawn(&self, args: &[&str]) -> Child {
        self.cmd()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }
    /// The newest pending device `user_code`, straight from the DB.
    fn pending_user_code(&self) -> Option<String> {
        let conn = rusqlite::Connection::open(&self.db_path).ok()?;
        conn.query_row(
            "SELECT user_code FROM device_codes ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok()
    }
    fn wait_for_code(&self) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(code) = self.pending_user_code() {
                return code;
            }
            assert!(Instant::now() < deadline, "device code never appeared");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn expire_codes(&self) {
        let conn = rusqlite::Connection::open(&self.db_path).unwrap();
        conn.execute(
            "UPDATE device_codes SET expires_at = '2000-01-01T00:00:00Z'",
            [],
        )
        .unwrap();
    }
}

/// Start a real server on an ephemeral port in a background thread.
fn start_server() -> Rig {
    let home = tempfile::tempdir().unwrap();
    let db_path = home.path().join("server.db");
    let web_dir = home.path().join("no-web");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let db_for_thread = db_path.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut map: HashMap<String, String> = HashMap::new();
            map.insert(
                "FORGEHUB_DATABASE_URL".into(),
                format!("sqlite://{}", db_for_thread.display()),
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
    let base = rx.recv_timeout(Duration::from_secs(30)).unwrap();
    Rig {
        base,
        db_path,
        home,
    }
}

/// A scripted browser: signup returns (cookie, csrf).
fn browser_signup(base: &str, username: &str) -> (String, String) {
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
    (cookie, csrf)
}

fn browser_decide(base: &str, cookie: &str, csrf: &str, user_code: &str, action: &str) {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!(
            "{base}/api/v1/auth/device/lookup?user_code={user_code}"
        ))
        .header("cookie", cookie)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200, "lookup before {action}");
    let resp = client
        .post(format!("{base}/api/v1/auth/device/{action}"))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({ "user_code": user_code }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 204, "{action} the code");
}

fn finish(child: Child) -> (i32, String, String) {
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn stored_token(rig: &Rig) -> String {
    let text = std::fs::read_to_string(rig.credentials_path()).unwrap();
    let parsed: toml::Value = text.parse().unwrap();
    parsed["registries"][&rig.base]["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn device_login_whoami_logout_revoke_end_to_end() {
    let rig = start_server();
    let (cookie, csrf) = browser_signup(&rig.base, "anna");

    let child = rig.spawn(&["login", "--registry", &rig.base]);
    let user_code = rig.wait_for_code();
    browser_decide(&rig.base, &cookie, &csrf, &user_code, "approve");
    let (code, stdout, _) = finish(child);
    assert_eq!(code, 0, "login succeeds: {stdout}");
    assert!(stdout.contains("Logged in as anna"), "{stdout}");

    // Credentials: right place, right permissions, and the full
    // token never appeared in the CLI's own output.
    let creds = rig.credentials_path();
    assert!(creds.exists(), "credentials file written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&creds).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "credentials file mode");
        let dir_mode = std::fs::metadata(creds.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "credentials dir mode");
    }
    let token = stored_token(&rig);
    assert!(token.starts_with("fgh_"));
    assert!(!stdout.contains(&token), "the token must never be printed");

    // whoami, human and JSON.
    let (code, stdout, _) = rig.run(&["whoami"], None);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("anna"), "{stdout}");
    assert!(stdout.contains(&rig.base), "{stdout}");
    let (code, stdout, _) = rig.run(&["--json", "whoami"], None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["username"], "anna");
    assert_eq!(v["registry"], rig.base);
    assert_eq!(v["scopes"], serde_json::json!(["read", "write"]));
    assert_eq!(v["token_prefix"], &token[4..12]);
    assert!(v.get("token").is_none(), "JSON never carries the token");

    // logout --revoke kills the token server-side too.
    let (code, stdout, _) = rig.run(&["logout", "--revoke"], None);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("revoked"), "{stdout}");
    assert!(!rig.credentials_path().exists(), "credentials removed");
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("{}/api/v1/me", rig.base))
        .bearer_auth(&token)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 401, "token revoked server-side");
    let (code, _, stderr) = rig.run(&["whoami"], None);
    assert_eq!(code, 4, "whoami after logout: {stderr}");
}

#[test]
fn device_login_denied_is_a_refusal() {
    let rig = start_server();
    let (cookie, csrf) = browser_signup(&rig.base, "ben");
    let child = rig.spawn(&["login", "--registry", &rig.base]);
    let user_code = rig.wait_for_code();
    browser_decide(&rig.base, &cookie, &csrf, &user_code, "deny");
    let (code, _, stderr) = finish(child);
    assert_eq!(code, 2, "denied is a refusal: {stderr}");
    assert!(stderr.contains("denied"), "{stderr}");
    assert!(!rig.credentials_path().exists());
}

#[test]
fn device_login_expired_code() {
    let rig = start_server();
    let _ = browser_signup(&rig.base, "cleo");
    let child = rig.spawn(&["login", "--registry", &rig.base]);
    let _ = rig.wait_for_code();
    rig.expire_codes();
    let (code, _, stderr) = finish(child);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("expired"), "{stderr}");
}

#[test]
fn token_stdin_login_and_env_override() {
    let rig = start_server();
    let (cookie, csrf) = browser_signup(&rig.base, "diya");
    // Mint a token through the session API, as a user would in the
    // web UI, then hand it to the CLI on stdin.
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{}/api/v1/tokens", rig.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "name": "ci token", "scopes": ["read"] }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().unwrap();
    let token = created["token"].as_str().unwrap().to_string();

    let (code, _, stderr) = rig.run(
        &["login", "--registry", &rig.base, "--token"],
        Some(token.as_bytes()),
    );
    assert_eq!(code, 0, "{stderr}");
    let (code, stdout, _) = rig.run(&["whoami"], None);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("diya"), "{stdout}");

    // FORGEHUB_TOKEN alone (no credentials file) also works.
    std::fs::remove_file(rig.credentials_path()).unwrap();
    let out = rig
        .cmd()
        .args(["whoami"])
        .env("FORGEHUB_TOKEN", &token)
        .env("FORGEHUB_REGISTRY", &rig.base)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("diya"));
}

#[test]
fn bad_and_unreachable_registries_fail_clearly() {
    let rig = start_server();
    let (code, _, stderr) = rig.run(&["login", "--registry", "notaurl"], None);
    assert_eq!(code, 1);
    assert!(stderr.contains("invalid registry URL"), "{stderr}");

    // A port that is guaranteed closed: bind, take the number, drop.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let (code, _, stderr) = rig.run(
        &["login", "--registry", &format!("http://127.0.0.1:{port}")],
        None,
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("cannot connect"), "{stderr}");
}

#[test]
fn credentials_permissions_are_enforced() {
    let rig = start_server();
    let (cookie, csrf) = browser_signup(&rig.base, "eli");
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{}/api/v1/tokens", rig.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({ "name": "t", "scopes": ["read"] }))
        .send()
        .unwrap();
    let created: serde_json::Value = resp.json().unwrap();
    let token = created["token"].as_str().unwrap().to_string();
    let (code, _, _) = rig.run(
        &["login", "--registry", &rig.base, "--token"],
        Some(token.as_bytes()),
    );
    assert_eq!(code, 0);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            rig.credentials_path(),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        // Non-TTY stdin (piped/null): refused with the fix named.
        let (code, _, stderr) = rig.run(&["whoami"], None);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("chmod 600"), "{stderr}");
        std::fs::set_permissions(
            rig.credentials_path(),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let (code, _, _) = rig.run(&["whoami"], None);
        assert_eq!(code, 0, "fixed permissions work again");
    }
}

#[test]
fn whoami_without_login_is_exit_4() {
    let rig = start_server();
    let (code, _, stderr) = rig.run(&["whoami"], None);
    assert_eq!(code, 4, "{stderr}");
    assert!(stderr.contains("fg login"), "{stderr}");
    let (code, stdout, _) = rig.run(&["logout"], None);
    assert_eq!(code, 0, "logout while logged out is a no-op: {stdout}");
    assert!(stdout.contains("Not logged in"), "{stdout}");
}

/// Mint a token through the session API (helper for Step 20 tests).
fn mint_token(base: &str, cookie: &str, csrf: &str, scopes: &[&str]) -> String {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/tokens"))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({ "name": "step20", "scopes": scopes }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().unwrap();
    created["token"].as_str().unwrap().to_string()
}

/// Step 20: `remote list` (and `status`) follow `next_cursor`
/// past the first 100-item page instead of silently truncating.
#[test]
fn remote_list_follows_pagination_past_100() {
    let rig = start_server();
    let (cookie, csrf) = browser_signup(&rig.base, "pager");
    let token = mint_token(&rig.base, &cookie, &csrf, &["read", "write"]);
    let client = reqwest::blocking::Client::new();
    for i in 0..105 {
        let resp = client
            .post(format!("{}/api/v1/snippets", rig.base))
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "name": format!("bulk-{i:03}"),
                "body": "echo bulk",
                "visibility": "private",
            }))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 201, "create bulk-{i:03}");
    }
    let (code, _, stderr) = rig.run(
        &["login", "--registry", &rig.base, "--token"],
        Some(token.as_bytes()),
    );
    assert_eq!(code, 0, "{stderr}");
    let (code, stdout, stderr) = rig.run(&["remote", "list", "--json"], None);
    assert_eq!(code, 0, "{stderr}");
    let body: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        body["snippets"].as_array().unwrap().len(),
        105,
        "remote list must follow pagination past 100"
    );
}

/// Step 20: a cloud run fetches the snippet with exactly ONE
/// HTTP request (no metadata pre-flight, no second fetch).
#[test]
fn cloud_run_uses_exactly_one_snippet_request() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // A counting variant of the usual test server: wrap the
    // real router with middleware that counts snippet GETs.
    let home = tempfile::tempdir().unwrap();
    let db_path = home.path().join("server.db");
    let web_dir = home.path().join("no-web");
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_thread = counter.clone();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut map: HashMap<String, String> = HashMap::new();
            map.insert(
                "FORGEHUB_DATABASE_URL".into(),
                format!("sqlite://{}", db_path.display()),
            );
            map.insert("FORGEHUB_WEB_DIR".into(), web_dir.display().to_string());
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
            let app = forge_server::http::build_router(state).layer(axum::middleware::from_fn(
                move |req: axum::extract::Request, next: axum::middleware::Next| {
                    let c = counter_thread.clone();
                    async move {
                        if req.method() == axum::http::Method::GET
                            && req.uri().path().starts_with("/api/v1/snippets/")
                        {
                            c.fetch_add(1, Ordering::SeqCst);
                        }
                        next.run(req).await
                    }
                },
            ));
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
    let base = rx.recv_timeout(Duration::from_secs(30)).unwrap();
    let rig = Rig {
        base,
        db_path: home.path().join("server.db"),
        home,
    };

    let (cookie, csrf) = browser_signup(&rig.base, "runner");
    let token = mint_token(&rig.base, &cookie, &csrf, &["read", "write"]);
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{}/api/v1/snippets", rig.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": "one-shot",
            "body": "echo counted-run",
            "visibility": "private",
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);
    let (code, _, stderr) = rig.run(
        &["login", "--registry", &rig.base, "--token"],
        Some(token.as_bytes()),
    );
    assert_eq!(code, 0, "{stderr}");

    counter.store(0, Ordering::SeqCst);
    // Own snippet + -y: the [y/N] prompt is skippable, so this
    // runs non-interactively through the exact CLI path.
    let (code, stdout, stderr) = rig.run(&["run", "one-shot", "-y"], None);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("counted-run"), "{stdout}");
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "a cloud run must make exactly one snippet GET"
    );
}
