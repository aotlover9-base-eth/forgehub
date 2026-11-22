//! `forgehub self-check` (Peak C10): the binary reports
//! its own hash and explains manual verification when the
//! published sums cannot be fetched; with a local sums
//! file it verifies a release archive for real - match
//! passes, mismatch fails.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_forgehub");
/// sha256("abc"), the classic test vector.
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

struct Harness {
    root: std::path::PathBuf,
}

impl Harness {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fg-selfcheck-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn run(&self, args: &[&str], extra_env: &[(&str, &str)]) -> std::process::Output {
        let mut cmd = Command::new(BIN);
        cmd.args(args)
            .env("FORGEHUB_HOME", self.root.join("home"))
            .env("HOME", self.root.join("home"))
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        child.stdin.as_mut().unwrap().write_all(b"").unwrap();
        child.wait_with_output().unwrap()
    }

    fn write(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.root.join(name);
        std::fs::write(&path, bytes).unwrap();
        path.display().to_string()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn offline_run_reports_the_binary_hash_and_explains() {
    let h = Harness::new("offline");
    // Port 1 on loopback refuses instantly: a forced
    // offline path with no real network involved.
    let out = h.run(
        &["self-check"],
        &[("FORGEHUB_SUMS_URL", "http://127.0.0.1:1/SHA256SUMS")],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(text.contains("binary sha256:"), "{text}");
    assert!(text.contains("offline:"), "{text}");
    assert!(text.contains("gh release download"), "{text}");
    assert!(text.contains("sha256sum -c SHA256SUMS"), "{text}");
}

#[test]
fn archive_verifies_against_a_local_sums_file() {
    let h = Harness::new("match");
    let archive = h.write("forgehub-test.tar.gz", b"abc");
    let sums = h.write(
        "SHA256SUMS",
        format!("{ABC_SHA256}  forgehub-test.tar.gz\n").as_bytes(),
    );
    let out = h.run(&["self-check", "--archive", &archive, "--sums", &sums], &[]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout(&out).contains("matches the published checksum"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn archive_mismatch_fails_loudly() {
    let h = Harness::new("mismatch");
    let archive = h.write("forgehub-test.tar.gz", b"abc");
    let sums = h.write(
        "SHA256SUMS",
        b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  forgehub-test.tar.gz\n",
    );
    let out = h.run(&["self-check", "--archive", &archive, "--sums", &sums], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("MISMATCH"), "{}", stdout(&out));
}

#[test]
fn json_shape_is_stable() {
    let h = Harness::new("json");
    let out = h.run(
        &["--json", "self-check"],
        &[("FORGEHUB_SUMS_URL", "http://127.0.0.1:1/SHA256SUMS")],
    );
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json output");
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    let hash = value["binary_sha256"].as_str().expect("hash");
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(value["sums_note"].as_str().unwrap().contains("offline"));
}
