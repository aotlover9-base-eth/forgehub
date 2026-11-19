//! Step 18 integration tests: help snapshot (examples in
//! every subcommand's help), `cache list|clear` against a
//! seeded cache, and `update --check` against fake servers
//! (a local GitHub-API stand-in and a real registry).

use std::io::Write;
use std::process::{Command, Output};

fn forgehub(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forgehub"));
    cmd.env("FORGEHUB_HOME", home.join(".forgehub"))
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env_remove("FORGEHUB_REGISTRY")
        .env_remove("FORGEHUB_TOKEN");
    cmd
}

fn run(cmd: &mut Command, stdin: &str) -> Output {
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

const SUBCOMMANDS: &[&str] = &[
    "init",
    "record",
    "run",
    "list",
    "show",
    "search",
    "tag",
    "rename",
    "delete",
    "edit",
    "parameterize",
    "clone",
    "import",
    "export",
    "analyze",
    "stats",
    "backup",
    "restore",
    "login",
    "logout",
    "whoami",
    "push",
    "pull",
    "status",
    "diff",
    "fork",
    "remote",
    "trust",
    "doctor",
    "alias",
    "completions",
    "ui",
    "cache",
    "update",
];

#[test]
fn top_level_help_lists_everything_and_subcommand_help_has_examples() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(forgehub(dir.path()).arg("--help"), "");
    let help = stdout(&out);
    for name in SUBCOMMANDS {
        assert!(help.contains(name), "top-level help missing {name}");
    }
    assert!(help.contains("ForgeHub"));
    assert!(help.contains("Save shell command snippets once"));
    for name in SUBCOMMANDS {
        let out = run(forgehub(dir.path()).args([name, "--help"]), "");
        let text = stdout(&out);
        assert!(
            text.contains("Examples:"),
            "{name} --help has no Examples section:\n{text}"
        );
        assert!(text.contains("fg "), "{name} --help examples should use fg");
    }
}

#[test]
fn ui_refuses_without_a_terminal() {
    let dir = tempfile::tempdir().unwrap();
    // The test harness pipes stdout, so the TUI must refuse
    // cleanly (and the run path it would have used is the
    // plain `run` command — dispatch shares cmd_run).
    let out = run(forgehub(dir.path()).arg("ui"), "");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(stderr.contains("needs a terminal"), "stderr: {stderr}");
}

#[test]
fn cache_list_and_clear_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join(".forgehub/cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(
        cache_dir.join("ada--ubuntu-setup--v2.yaml"),
        "name: ubuntu-setup\n",
    )
    .unwrap();
    std::fs::write(
        cache_dir.join("ada--ubuntu-setup.json"),
        r#"{"version":2,"hash":"x","visibility":"public"}"#,
    )
    .unwrap();

    let out = run(forgehub(dir.path()).args(["--json", "cache", "list"]), "");
    assert!(out.status.success(), "{}", stdout(&out));
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(parsed["entries"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["entries"][0]["name"], "ubuntu-setup");
    assert_eq!(parsed["entries"][0]["version"], 2);
    assert_eq!(parsed["cap_bytes"], 100 * 1024 * 1024);

    let out = run(forgehub(dir.path()).args(["--json", "cache", "clear"]), "");
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(parsed["cleared_entries"], 1);
    assert!(!cache_dir.join("ada--ubuntu-setup--v2.yaml").exists());
    assert!(!cache_dir.join("ada--ubuntu-setup.json").exists());

    let out = run(forgehub(dir.path()).args(["cache", "list"]), "");
    assert!(stdout(&out).contains("Cache is empty"));
}

/// A one-shot HTTP server answering a fixed JSON body.
fn fake_server(body: &'static str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}")
}

#[test]
fn update_check_reports_newer_from_fake_github() {
    let dir = tempfile::tempdir().unwrap();
    let base = fake_server(r#"{"tag_name":"v9.9.9"}"#);
    let mut cmd = forgehub(dir.path());
    cmd.env("FORGEHUB_GITHUB_API", &base)
        .args(["--json", "update", "--check"]);
    let out = run(&mut cmd, "");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(parsed["update_available"], true);
    assert_eq!(parsed["latest"], "v9.9.9");
    assert!(parsed["source"]
        .as_str()
        .unwrap()
        .contains("releases/tag/v9.9.9"));
}

#[test]
fn update_check_latest_when_fake_github_matches() {
    let dir = tempfile::tempdir().unwrap();
    let base = fake_server(r#"{"tag_name":"v0.1.0"}"#);
    let mut cmd = forgehub(dir.path());
    cmd.env("FORGEHUB_GITHUB_API", &base)
        .args(["update", "--check"]);
    let out = run(&mut cmd, "");
    assert!(out.status.success());
    assert!(stdout(&out).contains("latest known version"));
}

#[test]
fn offline_flag_fails_cloud_commands_fast_with_a_clear_message() {
    let dir = tempfile::tempdir().unwrap();
    // `status` needs a cloud session; with --offline it must
    // refuse before any network or credential work.
    let out = run(forgehub(dir.path()).args(["--offline", "status"]), "");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("offline mode"),
        "unexpected stderr: {stderr}"
    );
}
