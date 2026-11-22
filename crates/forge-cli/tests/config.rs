//! Peak B9 integration tests: `fg config get|set|list` with
//! the real binary against a temp `FORGEHUB_HOME`. No server:
//! config is a local-only surface.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Harness {
    home: PathBuf,
    work: PathBuf,
}

impl Harness {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "forgehub-config-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&base);
        let home = base.join("home");
        let work = base.join("work");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        let h = Self { home, work };
        let out = h.fg(&["init"]);
        assert_eq!(out.status.code(), Some(0), "init");
        h
    }

    fn fg(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_forgehub"));
        cmd.args(args)
            .env("FORGEHUB_HOME", &self.home)
            .env("HOME", &self.home)
            .env_remove("FORGEHUB_REGISTRY")
            .current_dir(&self.work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        child.stdin.as_mut().unwrap().write_all(b"").unwrap();
        child.wait_with_output().unwrap()
    }

    fn stdout(&self, args: &[&str]) -> String {
        let out = self.fg(args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn config_yaml(&self) -> String {
        std::fs::read_to_string(self.home.join("config.yaml")).unwrap()
    }
}

#[test]
fn list_shows_defaults_on_a_fresh_home() {
    let h = Harness::new("list");
    let out = h.stdout(&["config", "list"]);
    assert!(out.contains("registry_url"), "{out}");
    assert!(out.contains("(not set)"), "{out}");
    assert!(out.contains("color"), "{out}");
    assert!(out.contains("auto (default)"), "{out}");
    assert!(out.contains("default_visibility"), "{out}");
    assert!(out.contains("private (default)"), "{out}");
    assert!(out.contains("cache_size_mb"), "{out}");
    assert!(out.contains("100"), "{out}");
}

#[test]
fn set_get_round_trip_for_every_key() {
    let h = Harness::new("roundtrip");
    for (key, value) in [
        ("color", "never"),
        ("shell", "fish"),
        ("default_visibility", "unlisted"),
        ("cache_size_mb", "250"),
        ("registry_url", "http://127.0.0.1:8080"),
    ] {
        let out = h.stdout(&["config", "set", key, value]);
        assert!(out.contains(&format!("{key} = {value}")), "{out}");
        let got = h.stdout(&["config", "get", key]);
        assert!(got.contains(value), "{key}: {got}");
        // cache_size_mb always has an effective value in the
        // struct, so its source reads "effective"; the other
        // keys report "config" once set.
        let marker = if key == "cache_size_mb" {
            "(effective)"
        } else {
            "(config)"
        };
        assert!(got.contains(marker), "{key}: {got}");
    }
    // The British spelling is an accepted alias for `color`.
    let got = h.stdout(&["config", "get", "colour"]);
    assert!(got.contains("never"), "{got}");
    // Everything landed in config.yaml under the struct's keys.
    let yaml = h.config_yaml();
    assert!(yaml.contains("color: never"), "{yaml}");
    assert!(yaml.contains("default_shell: fish"), "{yaml}");
    assert!(yaml.contains("default_visibility: unlisted"), "{yaml}");
    assert!(yaml.contains("cache_max_mb: 250"), "{yaml}");
    assert!(yaml.contains("server: http://127.0.0.1:8080"), "{yaml}");
}

#[test]
fn registry_url_is_normalised_on_set() {
    let h = Harness::new("normalise");
    let out = h.stdout(&["config", "set", "registry_url", "http://127.0.0.1:8080/"]);
    assert!(
        out.contains("registry_url = http://127.0.0.1:8080"),
        "{out}"
    );
    let got = h.stdout(&["config", "get", "registry_url"]);
    assert!(got.contains("http://127.0.0.1:8080"), "{got}");
    assert!(!got.contains("8080/"), "{got}");
}

#[test]
fn invalid_values_and_unknown_keys_are_refused() {
    let h = Harness::new("invalid");
    for args in [
        vec!["config", "set", "default_visibility", "friends"],
        vec!["config", "set", "color", "sometimes"],
        vec!["config", "set", "cache_size_mb", "0"],
        vec!["config", "set", "cache_size_mb", "1.5"],
        vec!["config", "set", "shell", "powershell"],
        vec!["config", "set", "registry_url", "not a url"],
        vec!["config", "set", "token", "fgh_secret"],
        vec!["config", "get", "api_token"],
    ] {
        let out = h.fg(&args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("fix:"), "{args:?}: {stderr}");
    }
    // Nothing invalid reached the file: no token key, ever.
    let yaml = h.config_yaml();
    assert!(!yaml.contains("token"), "{yaml}");
    assert!(!yaml.contains("fgh_"), "{yaml}");
}

#[test]
fn json_list_and_get_have_stable_keys() {
    let h = Harness::new("json");
    let out = h.stdout(&["--json", "config", "list"]);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    let keys: Vec<&str> = doc["config"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "registry_url",
            "color",
            "shell",
            "default_visibility",
            "cache_size_mb"
        ]
    );
    let out = h.stdout(&["--json", "config", "get", "color"]);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["key"], "color");
    assert_eq!(doc["value"], "auto");
    assert_eq!(doc["source"], "default");
}
