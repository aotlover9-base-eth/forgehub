//! Step 12 integration tests: the real CLI against the real
//! server - push/pull round trips with hash equality, optimistic
//! concurrency conflicts, status states, secret refusal, remote
//! management and forks.

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
    fn write_local(&self, name: &str, script: &str) -> String {
        let dir = self.p("fg").join("snippets");
        std::fs::create_dir_all(&dir).unwrap();
        let yaml = format!(
            "name: {name}\ndescription: Test snippet {name}\nmode: script\nscript: |\n  {script}\n"
        );
        std::fs::write(dir.join(format!("{name}.yaml")), &yaml).unwrap();
        yaml
    }
    fn read_local(&self, name: &str) -> String {
        std::fs::read_to_string(self.p("fg").join("snippets").join(format!("{name}.yaml"))).unwrap()
    }
    fn local_hash(&self, name: &str) -> String {
        let snippet = forge_core::Snippet::from_yaml(&self.read_local(name)).unwrap();
        forge_core::hash::content_hash(&snippet)
    }
}

fn start_server() -> String {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("server.db");
    let web_dir = dir.path().join("no-web");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let _dir = dir; // keep the temp dir alive with the server
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

/// Sign up via the API and mint a token; returns (token, cookie,
/// csrf) so tests can also act as the web session.
fn signup_and_token(base: &str, username: &str) -> (String, String, String) {
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
    (created["token"].as_str().unwrap().to_string(), cookie, csrf)
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

/// Push a new version directly through the API (the "other
/// machine" in conflict tests).
fn api_put_new_version(base: &str, token: &str, owner: &str, name: &str, script: &str) {
    let (status, detail) = api_get(base, token, &format!("/api/v1/snippets/{owner}/{name}"));
    assert_eq!(status, 200);
    let version = detail["version"].as_i64().unwrap();
    let yaml = format!(
        "name: {name}\ndescription: Changed elsewhere\nmode: script\nscript: |\n  {script}\n"
    );
    let client = reqwest::blocking::Client::new();
    let resp = client
        .put(format!("{base}/api/v1/snippets/{owner}/{name}"))
        .bearer_auth(token)
        .header("if-match", format!("\"{version}\""))
        .json(&serde_json::json!({ "name": name, "body": yaml }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200, "api put");
}

#[test]
fn push_pull_round_trip_with_hash_equality() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("hello", "echo hello-from-a");
    let expected_hash = a.local_hash("hello");

    let (code, stdout, stderr) = a.run(&["push", "hello"], None);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("anna/hello v1"), "{stdout}");
    assert!(
        stdout.contains(&format!("sha256:{}", &expected_hash[..8])),
        "{stdout}"
    );

    // THE critical acceptance: server hash == CLI hash.
    let (status, detail) = api_get(&base, &token, "/api/v1/snippets/anna/hello");
    assert_eq!(status, 200);
    assert_eq!(detail["hash"], expected_hash, "hash parity CLI vs server");
    assert_eq!(
        detail["visibility"], "private",
        "new pushes default to private"
    );

    // Up-to-date second push.
    let (code, stdout, _) = a.run(&["push", "hello"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("up to date"), "{stdout}");

    // Pull into a second home (same account), under the same name.
    let b = rig(&base);
    cli_login(&b, &token);
    let (code, stdout, stderr) = b.run(&["pull", "hello"], None);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("saved as `hello`"), "{stdout}");
    assert_eq!(
        b.local_hash("hello"),
        expected_hash,
        "pulled content identical"
    );
    assert!(
        b.p("fg").join("snippets").join("hello.meta.json").exists(),
        "provenance sidecar written"
    );
    let (code, stdout, _) = b.run(&["status", "hello"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("unchanged"), "{stdout}");
}

#[test]
fn conflict_is_reported_and_recoverable() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "ben");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("tool", "echo v1");
    assert_eq!(a.run(&["push", "tool"], None).0, 0);
    a.write_local("tool", "echo v2-local");
    assert_eq!(a.run(&["push", "tool"], None).0, 0, "v2 pushes");

    // Another machine pushes v3 first.
    api_put_new_version(&base, &token, "ben", "tool", "echo v3-remote");
    // Our stale push now conflicts (If-Match on v2, remote is v3).
    let (code, _, stderr) = a.run(&["push", "tool"], None);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("version conflict"), "{stderr}");
    assert!(stderr.contains("pull"), "{stderr}");

    // diff shows the remote change; pull --force recovers.
    let (code, stdout, _) = a.run(&["diff", "tool"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("echo v3-remote"), "{stdout}");
    let (code, _, stderr) = a.run(&["pull", "tool", "--force"], None);
    assert_eq!(code, 0, "{stderr}");
    assert!(a.read_local("tool").contains("echo v3-remote"));
    let (_, stdout, _) = a.run(&["status", "tool"], None);
    assert!(stdout.contains("unchanged"), "{stdout}");
}

#[test]
fn status_states_ahead_behind_diverged() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "cleo");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("s", "echo base");
    assert_eq!(a.run(&["push", "s"], None).0, 0);

    a.write_local("s", "echo local-edit");
    let (_, stdout, _) = a.run(&["status", "s"], None);
    assert!(stdout.contains("ahead"), "after local edit: {stdout}");

    assert_eq!(a.run(&["push", "s"], None).0, 0);
    api_put_new_version(&base, &token, "cleo", "s", "echo remote-edit");
    let (_, stdout, _) = a.run(&["status", "s"], None);
    assert!(stdout.contains("behind"), "after remote edit: {stdout}");

    a.write_local("s", "echo both-moved");
    let (_, stdout, _) = a.run(&["status", "s"], None);
    assert!(stdout.contains("diverged"), "after both moved: {stdout}");
}

#[test]
fn push_refuses_secrets_and_nothing_reaches_the_server() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "diya");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("leaky", "export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE");
    let (code, _, stderr) = a.run(&["push", "leaky"], None);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("secret"), "{stderr}");
    let (status, _) = api_get(&base, &token, "/api/v1/snippets/diya/leaky");
    assert_eq!(status, 404, "nothing was uploaded");
}

#[test]
fn cloud_commands_require_login() {
    let base = start_server();
    let _ = signup_and_token(&base, "eli");
    let a = rig(&base);
    a.write_local("x", "echo x");
    let (code, _, stderr) = a.run(&["push", "x"], None);
    assert_eq!(code, 4, "{stderr}");
    assert!(stderr.contains("forgehub login"), "{stderr}");
    let (code, _, stderr) = a.run(&["pull", "x"], None);
    assert_eq!(code, 4, "{stderr}");
}

#[test]
fn pull_never_overwrites_without_force() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "faye");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("mine", "echo original");
    assert_eq!(a.run(&["push", "mine"], None).0, 0);
    // Local diverges; a plain pull must refuse (non-TTY here).
    a.write_local("mine", "echo local-changes");
    let (code, _, stderr) = a.run(&["pull", "mine"], None);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("--force"), "{stderr}");
    assert!(a.read_local("mine").contains("local-changes"), "untouched");
    let (code, _, _) = a.run(&["pull", "mine", "--force"], None);
    assert_eq!(code, 0);
    assert!(a.read_local("mine").contains("original"));
}

#[test]
fn remote_management_and_fork() {
    let base = start_server();
    let (token_a, _, _) = signup_and_token(&base, "gus");
    let (token_b, _, _) = signup_and_token(&base, "hana");
    let a = rig(&base);
    cli_login(&a, &token_a);
    a.write_local("shared", "echo shared-thing");
    let (code, _, stderr) = a.run(&["push", "shared", "--public"], None);
    assert_eq!(code, 0, "{stderr}");

    let b = rig(&base);
    cli_login(&b, &token_b);
    let (code, stdout, _) = b.run(&["remote", "search", "shared"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("gus/shared"), "{stdout}");
    let (code, stdout, _) = b.run(&["remote", "show", "gus/shared"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("shared-thing"), "{stdout}");

    // Fork into B's account (private), then it is in B's list.
    let (code, stdout, stderr) = b.run(&["fork", "gus/shared"], None);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("hana/shared"), "{stdout}");
    let (code, stdout, _) = b.run(&["remote", "list"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("shared"), "{stdout}");
    let (status, detail) = api_get(&base, &token_b, "/api/v1/snippets/hana/shared");
    assert_eq!(status, 200);
    assert_eq!(detail["visibility"], "private");
    assert_eq!(detail["forked_from"]["owner"], "gus");

    // A changes visibility, then deletes; B's fork survives.
    let (code, _, _) = a.run(&["remote", "visibility", "shared", "private", "-y"], None);
    assert_eq!(code, 0);
    let (code, _, stderr) = a.run(&["remote", "delete", "shared", "-y"], None);
    assert_eq!(code, 0, "{stderr}");
    let (status, _) = api_get(&base, &token_a, "/api/v1/snippets/gus/shared");
    assert_eq!(status, 404);
    let (status, _) = api_get(&base, &token_b, "/api/v1/snippets/hana/shared");
    assert_eq!(status, 200, "the fork is independent");
}

#[test]
fn push_all_summary() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "ivan");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("one", "echo 1");
    a.write_local("two", "echo 2");
    let (code, stdout, stderr) = a.run(&["push", "--all"], None);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("2 pushed, 0 up to date, 0 failed"),
        "{stdout}"
    );
    let (code, stdout, _) = a.run(&["push", "--all"], None);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("0 pushed, 2 up to date, 0 failed"),
        "{stdout}"
    );
}

/// Peak B9: the `default_visibility` config key applies when a
/// push CREATES a snippet; an existing snippet keeps its
/// visibility when no flag is given, even if the default has
/// changed since.
#[test]
fn push_uses_config_default_visibility_for_creates_only() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    let (code, _, stderr) = a.run(&["config", "set", "default_visibility", "public"], None);
    assert_eq!(code, 0, "config set: {stderr}");
    a.write_local("fresh", "echo one");
    let (code, _, stderr) = a.run(&["push", "fresh"], None);
    assert_eq!(code, 0, "push: {stderr}");
    let (status, detail) = api_get(&base, &token, "/api/v1/snippets/anna/fresh");
    assert_eq!(status, 200);
    assert_eq!(detail["version"], 1);
    assert_eq!(
        detail["visibility"], "public",
        "create uses the config default"
    );

    let (code, _, stderr) = a.run(&["config", "set", "default_visibility", "private"], None);
    assert_eq!(code, 0, "config set 2: {stderr}");
    a.write_local("fresh", "echo two");
    let (code, _, stderr) = a.run(&["push", "fresh"], None);
    assert_eq!(code, 0, "push 2: {stderr}");
    let (_, detail) = api_get(&base, &token, "/api/v1/snippets/anna/fresh");
    assert_eq!(detail["version"], 2);
    assert_eq!(
        detail["visibility"], "public",
        "existing snippets keep their visibility without a flag"
    );
}

/// Peak B5's owed live verification, closed in B9: `fg edit
/// <owner/name>` against the real server downloads the snippet,
/// runs $EDITOR over it and pushes the result as a new version.
#[test]
fn cloud_edit_pushes_a_new_version_live() {
    let base = start_server();
    let (token, _, _) = signup_and_token(&base, "anna");
    let a = rig(&base);
    cli_login(&a, &token);
    a.write_local("hello", "echo MARKER");
    let (code, _, stderr) = a.run(&["push", "hello"], None);
    assert_eq!(code, 0, "push: {stderr}");

    // A scripted $EDITOR that rewrites the marker in place.
    let editor = a.p("editor.sh");
    std::fs::write(&editor, "#!/bin/sh\nsed -i 's/MARKER/edited/' \"$1\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = a
        .cmd()
        .args(["-y", "edit", "anna/hello"])
        .env("EDITOR", &editor)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "cloud edit: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (status, detail) = api_get(&base, &token, "/api/v1/snippets/anna/hello");
    assert_eq!(status, 200);
    assert_eq!(detail["version"], 2, "the edit became a new version");
    assert!(
        detail["body"].as_str().unwrap().contains("echo edited"),
        "the edited body was pushed: {detail}"
    );
}
