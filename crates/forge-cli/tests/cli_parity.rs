//! End-to-end tests for the Step 5 parity commands (roadmap Step 5:
//! variables/parameterize, clone/import/export, analyze history
//! mining, stats from history.db, backup/restore, doctor, the
//! rc-file alias installer, dynamic completions, spec-cli §3 JSON).

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
            "forgehub-parity-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&base);
        let home = base.join("home");
        let work = base.join("work");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        let h = Self { home, work };
        h.fg(&["init"]);
        h
    }

    fn fg(&self, args: &[&str]) -> std::process::Output {
        self.fg_env(args, &[])
    }

    fn fg_env(&self, args: &[&str], extra_env: &[(&str, &str)]) -> std::process::Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_forgehub"));
        cmd.args(args)
            .env("FORGEHUB_HOME", &self.home)
            .env("HOME", &self.home)
            .current_dir(&self.work)
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

    fn write_snippet(&self, name: &str, yaml: &str) {
        let dir = self.home.join("snippets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.yaml")), yaml).unwrap();
    }

    fn stdout(out: &std::process::Output) -> String {
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn stderr(out: &std::process::Output) -> String {
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.home.parent().unwrap_or(&self.home));
    }
}

#[test]
fn parameterize_extracts_repeated_url() {
    let h = Harness::new("param");
    h.write_snippet(
        "deploy",
        "name: deploy\nsteps:\n  - curl https://api.example.com/v1/deploy -X POST\n  - \"curl https://api.example.com/v1/deploy -H 'Accept: json'\"\n",
    );
    let out = h.fg(&["parameterize", "deploy", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(
        Harness::stdout(&out).contains("{{URL}}"),
        "{}",
        Harness::stdout(&out)
    );
    let out = h.fg(&["show", "deploy"]);
    let shown = Harness::stdout(&out);
    assert!(shown.contains("{{URL}}"), "{shown}");
    assert!(shown.contains("https://api.example.com/v1"), "{shown}");
    // And the parameterized snippet still runs (default value used).
    let out = h.fg(&["run", "deploy", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    // --dry is accepted as an alias of --dry-run.
    let out = h.fg(&["run", "deploy", "--dry"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(Harness::stdout(&out).contains("Dry run"));
}

#[test]
fn parameterize_pairs_and_refusal_without_confirmation() {
    let h = Harness::new("param2");
    h.write_snippet(
        "deploy",
        "name: deploy\nsteps:\n  - curl https://api.example.com/v1/deploy -X POST\n  - curl https://api.example.com/v1/deploy -X GET\n",
    );
    // Non-TTY without -y: preview + refusal, file unchanged.
    let out = h.fg(&["parameterize", "deploy"]);
    assert_eq!(out.status.code(), Some(2), "{}", Harness::stderr(&out));
    let raw = std::fs::read_to_string(h.home.join("snippets/deploy.yaml")).unwrap();
    assert!(!raw.contains("{{API}}"), "file must be unchanged: {raw}");
    // Explicit --literal/--as pairs apply directly.
    let out = h.fg(&[
        "parameterize",
        "deploy",
        "--literal",
        "https://api.example.com/v1/deploy",
        "--as",
        "API",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let raw = std::fs::read_to_string(h.home.join("snippets/deploy.yaml")).unwrap();
    assert!(raw.contains("{{API}}"), "{raw}");
    // Unpaired flags are an error.
    let out = h.fg(&["parameterize", "deploy", "--literal", "x"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn clone_import_export_roundtrip() {
    let h = Harness::new("clone");
    h.write_snippet("orig", "name: orig\nsteps: [echo one]\n");
    let out = h.fg(&["clone", "orig", "copy"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(h.home.join("snippets/copy.yaml").exists());
    let out = h.fg(&["clone", "orig", "copy"]);
    assert_eq!(out.status.code(), Some(1));

    // Export to stdout and to a file.
    let out = h.fg(&["export", "orig"]);
    assert!(Harness::stdout(&out).contains("echo one"));
    let file = h.work.join("out.yaml");
    let out = h.fg(&["export", "orig", "--out", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    assert!(file.exists());

    // Import a Forgenesis-flavoured file: notes for dropped fields and
    // the lowercased name.
    let legacy = h.work.join("legacy.yaml");
    std::fs::write(
        &legacy,
        "name: LegacyThing\npacks: [web]\nsteps: [echo legacy]\n",
    )
    .unwrap();
    let out = h.fg(&["import", legacy.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("packs"), "{stdout}");
    assert!(stdout.contains("lowercased"), "{stdout}");
    assert!(h.home.join("snippets/legacything.yaml").exists());
    // Second import without --force refuses.
    let out = h.fg(&["import", legacy.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let out = h.fg(&["import", legacy.to_str().unwrap(), "--name", "other-name"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
}

#[test]
fn import_directory_and_forgenesis() {
    let h = Harness::new("impdir");
    let dir = h.work.join("bundle");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("one.yaml"), "name: one\nsteps: [echo 1]\n").unwrap();
    std::fs::write(dir.join("two.yml"), "name: two\nsteps: [echo 2]\n").unwrap();
    std::fs::write(dir.join("ignored.txt"), "name: nope\n").unwrap();
    let out = h.fg(&["import", dir.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(h.home.join("snippets/one.yaml").exists());
    assert!(h.home.join("snippets/two.yaml").exists());
    // Directory re-import: collisions are skipped, not fatal, not silent.
    let out = h.fg(&["--json", "import", dir.to_str().unwrap()]);
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("\"skipped\""), "{stdout}");
    assert!(stdout.contains("already exists"), "{stdout}");

    // --from-forgenesis reads $FORGENESIS_HOME/workflows.
    let fg_home = h.work.join("fghome");
    std::fs::create_dir_all(fg_home.join("workflows")).unwrap();
    std::fs::write(
        fg_home.join("workflows/old.yaml"),
        "name: OldOne\nsteps: [echo old]\n",
    )
    .unwrap();
    let out = h.fg_env(
        &["import", "--from-forgenesis"],
        &[("FORGENESIS_HOME", fg_home.to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(h.home.join("snippets/oldone.yaml").exists());
}

#[test]
fn analyze_mines_history_and_scrubs_secrets() {
    let h = Harness::new("analyze");
    let hist = h.work.join("history.txt");
    let mut text = String::new();
    for _ in 0..4 {
        text.push_str("git pull\npnpm install\ndocker compose up -d\nls -la\n");
    }
    // A secret-shaped command repeated just as often must never appear.
    for _ in 0..4 {
        text.push_str("export API_KEY=supersecretvalue\ngit pull\n");
    }
    std::fs::write(&hist, &text).unwrap();

    let out = h.fg(&["analyze", "--file", hist.to_str().unwrap(), "--dry"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("git pull"), "{stdout}");
    assert!(stdout.contains("pnpm install"), "{stdout}");
    assert!(
        !stdout.contains("supersecretvalue"),
        "secret leaked: {stdout}"
    );
    assert!(!stdout.contains("API_KEY"), "secret leaked: {stdout}");
    // --dry created nothing.
    assert!(!h.home.join("snippets/git-pull.yaml").exists());

    // -y creates the top candidates as snippets.
    let out = h.fg(&["analyze", "--file", hist.to_str().unwrap(), "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let names: Vec<String> = std::fs::read_dir(h.home.join("snippets"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!names.is_empty(), "analyze -y created nothing");
    for n in &names {
        let body = std::fs::read_to_string(h.home.join("snippets").join(n)).unwrap();
        assert!(!body.contains("supersecretvalue"), "secret saved in {n}");
        assert!(!body.contains("API_KEY"), "secret saved in {n}");
    }
}

#[test]
fn stats_come_from_run_history() {
    let h = Harness::new("stats");
    h.write_snippet("a", "name: a\nsteps: [echo 1, echo 2]\n");
    // No runs yet.
    let out = h.fg(&["--json", "stats"]);
    let stdout = Harness::stdout(&out);
    assert!(
        stdout.contains("\"runs\": 0") || stdout.contains("\"runs\":0"),
        "{stdout}"
    );
    assert!(stdout.contains("\"estimated_saved_ms\""), "{stdout}");

    // Two runs of a 2-command snippet.
    for _ in 0..2 {
        let out = h.fg(&["run", "a", "-y"]);
        assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    }
    let out = h.fg(&["--json", "stats"]);
    let stdout = Harness::stdout(&out);
    assert!(
        stdout.contains("\"runs\": 2") || stdout.contains("\"runs\":2"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"commands\": 4") || stdout.contains("\"commands\":4"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"failures\": 0") || stdout.contains("\"failures\":0"),
        "{stdout}"
    );
    assert!(stdout.contains("\"workflows\""), "{stdout}");
    assert!(stdout.contains("\"last_run\""), "{stdout}");
    // Saved = 4 commands x 12 s - runtime: well over 40 s.
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        v["estimated_saved_ms"].as_u64().unwrap() > 40_000,
        "{stdout}"
    );
    // --explain prints the formula.
    let out = h.fg(&["stats", "--explain"]);
    assert!(Harness::stdout(&out).contains("human_overhead_seconds"));
    // A dry run is not recorded.
    let out = h.fg(&["run", "a", "--dry"]);
    assert_eq!(out.status.code(), Some(0));
    let out = h.fg(&["--json", "stats"]);
    let v: serde_json::Value = serde_json::from_str(&Harness::stdout(&out)).unwrap();
    assert_eq!(v["runs"].as_u64(), Some(2));
}

#[test]
fn list_and_show_json_have_spec_keys() {
    let h = Harness::new("jsonkeys");
    h.write_snippet(
        "demo",
        "name: demo\ndescription: Demo snippet\ntags: [ci]\nvariables:\n  WHO: { default: world }\nsteps: [\"echo hi {{WHO}}\"]\n",
    );
    let out = h.fg(&["--json", "list"]);
    let v: serde_json::Value = serde_json::from_str(&Harness::stdout(&out)).unwrap();
    let w = &v["workflows"][0];
    assert_eq!(w["name"], "demo");
    assert_eq!(w["description"], "Demo snippet");
    assert_eq!(w["tags"][0], "ci");
    assert_eq!(w["mode"], "steps");
    assert_eq!(w["origin"], "local");
    // --tag filter
    let out = h.fg(&["list", "--tag", "nope"]);
    assert!(Harness::stdout(&out).contains("No snippets"));

    let out = h.fg(&["--json", "show", "demo"]);
    let v: serde_json::Value = serde_json::from_str(&Harness::stdout(&out)).unwrap();
    assert_eq!(v["name"], "demo");
    assert_eq!(v["shell"], "bash");
    assert_eq!(v["variables"]["WHO"]["default"], "world");
    assert_eq!(v["commands"][0], "echo hi {{WHO}}");
}

#[test]
fn backup_restore_roundtrip() {
    let h = Harness::new("backup");
    h.write_snippet(
        "keep",
        "name: keep\ndescription: precious\nsteps: [echo keepme]\n",
    );
    let archive = h.work.join("backup.tar");
    let out = h.fg(&["backup", "--out", archive.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(archive.exists());

    // Delete the snippet, restore, and it is back.
    let out = h.fg(&["delete", "keep", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    let out = h.fg(&["restore", archive.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "restore without -y and no TTY refuses"
    );
    let out = h.fg(&["restore", archive.to_str().unwrap(), "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let out = h.fg(&["show", "keep"]);
    assert!(Harness::stdout(&out).contains("precious"));

    // A corrupted archive is refused (hash mismatch).
    let mut bytes = std::fs::read(&archive).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    let corrupt = h.work.join("corrupt.tar");
    std::fs::write(&corrupt, bytes).unwrap();
    let out = h.fg(&["restore", corrupt.to_str().unwrap(), "-y"]);
    assert_eq!(out.status.code(), Some(1), "{}", Harness::stderr(&out));
}

#[test]
fn backup_defaults_to_backups_dir_and_supports_single_snippet() {
    let h = Harness::new("backup2");
    h.write_snippet("one", "name: one\nsteps: [echo 1]\n");
    h.write_snippet("two", "name: two\nsteps: [echo 2]\n");
    let out = h.fg(&["backup"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let backups: Vec<PathBuf> = std::fs::read_dir(h.home.join("backups"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
    assert!(backups[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("forgehub-backup-"));
    // Restore with no argument picks the newest backup.
    let out = h.fg(&["delete", "one", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    let out = h.fg(&["restore", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(h.home.join("snippets/one.yaml").exists());
    // Single-snippet backup contains only that snippet.
    let single = h.work.join("single.tar");
    let out = h.fg(&["backup", "one", "-o", single.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let out = h.fg(&["delete", "two", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    let out = h.fg(&["restore", single.to_str().unwrap(), "-y"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        !h.home.join("snippets/two.yaml").exists(),
        "single backup must not restore two"
    );
}

#[test]
fn doctor_passes_on_healthy_home() {
    let h = Harness::new("doctor");
    h.write_snippet("fine", "name: fine\nsteps: [echo ok]\n");
    let out = h.fg(&["doctor"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stdout(&out));
    // Spec-cli section 3 JSON keys: checks[{name, level, detail}], ok.
    let out = h.fg(&["--json", "doctor"]);
    let v: serde_json::Value = serde_json::from_str(&Harness::stdout(&out)).unwrap();
    assert_eq!(v["ok"], true);
    let first = &v["checks"][0];
    assert!(
        first.get("name").is_some()
            && first.get("level").is_some()
            && first.get("detail").is_some()
    );
    let names: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"alias"), "{names:?}");
    assert!(names.contains(&"database"), "{names:?}");
    assert!(names.contains(&"credentials"), "{names:?}");

    // A topic filters the checks.
    let out = h.fg(&["--json", "doctor", "alias"]);
    let v: serde_json::Value = serde_json::from_str(&Harness::stdout(&out)).unwrap();
    assert_eq!(v["checks"].as_array().unwrap().len(), 1);
    assert_eq!(v["checks"][0]["name"], "alias");
    // An unknown topic is an error naming the valid ones.
    let out = h.fg(&["doctor", "nonsense"]);
    assert_eq!(out.status.code(), Some(1));

    // A broken snippet file makes doctor fail and name the file.
    std::fs::write(h.home.join("snippets/broken.yaml"), "name: [unclosed\n").unwrap();
    let out = h.fg(&["doctor"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(Harness::stdout(&out).contains("broken.yaml"));
}

#[test]
fn completions_emit_dynamic_registration() {
    let h = Harness::new("completions");
    for shell in ["bash", "zsh", "fish"] {
        let out = h.fg(&["completions", shell]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{shell}: {}",
            Harness::stderr(&out)
        );
        let stdout = Harness::stdout(&out);
        assert!(stdout.contains("forgehub"), "{shell}");
        assert!(
            stdout.contains("COMPLETE"),
            "{shell}: dynamic registration expected: {stdout}"
        );
    }
}

#[test]
fn alias_install_uninstall_rc_block() {
    let h = Harness::new("alias");
    // A pre-existing rc file with user content.
    let rc = h.home.join(".bashrc");
    std::fs::write(&rc, "# my bashrc\nexport EDITOR=vi\n").unwrap();

    // Non-TTY without -y refuses to edit the rc file.
    let out = h.fg(&["alias", "install", "--shell", "bash"]);
    assert_eq!(out.status.code(), Some(2), "{}", Harness::stderr(&out));
    assert!(!std::fs::read_to_string(&rc).unwrap().contains("forgehub"));

    let out = h.fg(&["alias", "install", "--shell", "bash", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let text = std::fs::read_to_string(&rc).unwrap();
    assert!(text.contains("alias fg='forgehub'"), "{text}");
    assert!(text.contains("alias jobs-fg='builtin fg'"), "{text}");
    assert!(
        text.contains("# my bashrc"),
        "user content preserved: {text}"
    );
    // A backup of the original rc file exists.
    let backups: Vec<String> = std::fs::read_dir(&h.home)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("forgehub-backup"))
        .collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
    let backup_text = std::fs::read_to_string(h.home.join(&backups[0])).unwrap();
    assert_eq!(backup_text, "# my bashrc\nexport EDITOR=vi\n");

    // Idempotent: a second install changes nothing and makes no new backup.
    let out = h.fg(&["alias", "install", "--shell", "bash", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    let text2 = std::fs::read_to_string(&rc).unwrap();
    assert_eq!(text2.matches("alias fg='forgehub'").count(), 1);
    let backups2: Vec<String> = std::fs::read_dir(&h.home)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("forgehub-backup"))
        .collect();
    assert_eq!(backups2.len(), 1, "{backups2:?}");

    // Show reports the state.
    let out = h.fg(&["--json", "alias", "show", "--shell", "bash"]);
    assert!(
        Harness::stdout(&out).contains("\"installed\": true")
            || Harness::stdout(&out).contains("\"installed\":true")
    );

    // Uninstall removes only the block; user content survives.
    let out = h.fg(&["alias", "uninstall", "--shell", "bash", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let text = std::fs::read_to_string(&rc).unwrap();
    assert!(!text.contains("forgehub"), "{text}");
    assert!(text.contains("# my bashrc"), "{text}");
    assert!(text.contains("export EDITOR=vi"), "{text}");
}

#[test]
fn alias_fish_writes_function_files() {
    let h = Harness::new("aliasfish");
    let out = h.fg(&["alias", "install", "--shell", "fish", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let fg = h.home.join(".config/fish/functions/fg.fish");
    let jobs = h.home.join(".config/fish/functions/jobs-fg.fish");
    assert!(std::fs::read_to_string(&fg)
        .unwrap()
        .contains("function fg"));
    assert!(std::fs::read_to_string(&jobs)
        .unwrap()
        .contains("builtin fg"));
    let out = h.fg(&["alias", "uninstall", "--shell", "fish", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!fg.exists() && !jobs.exists());
}
