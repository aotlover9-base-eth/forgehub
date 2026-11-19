//! End-to-end tests for the local CLI (Step 4): a temp `FORGEHUB_HOME`,
//! the real binary, real subprocess execution.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Harness {
    home: PathBuf,
    work: PathBuf,
}

impl Harness {
    fn new(tag: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("forgehub-cli-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&base);
        let home = base.join("home");
        let work = base.join("work");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        Self { home, work }
    }

    fn fg(&self, args: &[&str]) -> std::process::Output {
        self.fg_env(args, &[], "")
    }

    fn fg_env(
        &self,
        args: &[&str],
        extra_env: &[(&str, &str)],
        stdin_text: &str,
    ) -> std::process::Output {
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
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(stdin_text.as_bytes())
            .unwrap();
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
fn init_creates_home_and_is_idempotent() {
    let h = Harness::new("init");
    let out = h.fg(&["init"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(h.home.join("config.yaml").exists());
    assert!(h.home.join("snippets").is_dir());
    let out = h.fg(&["init"]);
    assert_eq!(out.status.code(), Some(0));
    let out = h.fg(&["--json", "init"]);
    assert!(Harness::stdout(&out).contains("\"home\""));
}

#[test]
fn record_with_fake_shell_then_run() {
    let h = Harness::new("record");
    h.fg(&["init"]);
    // A fake shell that ignores -i and writes history lines.
    let fake = h.work.join("fake-shell.sh");
    std::fs::write(
        &fake,
        "#!/bin/sh\nprintf 'echo one\\necho two\\n' >> \"$HISTFILE\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = h.fg_env(
        &[
            "record",
            "demo",
            "--description",
            "demo snippet",
            "--tags",
            "ci,fast",
        ],
        &[("SHELL", fake.to_str().unwrap())],
        "",
    );
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(Harness::stdout(&out).contains("Saved `demo` (2 commands)"));

    let out = h.fg(&["list"]);
    assert!(Harness::stdout(&out).contains("demo (2 commands)"));

    let out = h.fg(&["--json", "list"]);
    assert!(
        Harness::stdout(&out).contains("\"name\": \"demo\"")
            || Harness::stdout(&out).contains("\"name\":\"demo\"")
    );

    let out = h.fg(&["show", "demo"]);
    assert!(Harness::stdout(&out).contains("echo one"));

    let out = h.fg(&["run", "demo", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(Harness::stdout(&out).contains("echo one"));
    assert!(Harness::stdout(&out).contains("Dry run"));

    // Non-TTY without --yes: confirmation fails safe with exit 2.
    let out = h.fg(&["run", "demo"]);
    assert_eq!(out.status.code(), Some(2), "{}", Harness::stderr(&out));

    // With --yes it really runs.
    let out = h.fg(&["run", "demo", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("one") && stdout.contains("two"), "{stdout}");

    // Shorthand: fg demo -y
    let out = h.fg(&["demo", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
}

#[test]
fn record_refuses_secret_without_tty() {
    let h = Harness::new("record-secret");
    h.fg(&["init"]);
    let fake = h.work.join("fake-shell.sh");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\nprintf 'export TOKEN={}\\n' >> \"$HISTFILE\"\n",
            "Zx9Qw8Er5Ty2"
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = h.fg_env(
        &["record", "leaky"],
        &[("SHELL", fake.to_str().unwrap())],
        "",
    );
    assert_eq!(out.status.code(), Some(2), "{}", Harness::stderr(&out));
    assert!(!h.home.join("snippets/leaky.yaml").exists());
    // The refusal must not print the secret value anywhere.
    assert!(!Harness::stderr(&out).contains("Zx9Qw8Er5Ty2"));
}

#[test]
fn run_missing_snippet_is_exit_3_with_suggestions() {
    let h = Harness::new("missing");
    h.fg(&["init"]);
    h.write_snippet("ubuntu-setup", "name: ubuntu-setup\nsteps: [echo hi]\n");
    let out = h.fg(&["run", "ubuntu"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(
        Harness::stderr(&out).contains("ubuntu-setup"),
        "{}",
        Harness::stderr(&out)
    );
}

#[test]
fn variables_missing_then_supplied() {
    let h = Harness::new("vars");
    h.fg(&["init"]);
    h.write_snippet("greet", "name: greet\nsteps:\n  - echo hello {{WHO}}\n");
    let out = h.fg(&["run", "greet", "-y"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        Harness::stderr(&out).contains("WHO"),
        "{}",
        Harness::stderr(&out)
    );
    let out = h.fg(&["run", "greet", "-y", "WHO=world"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(Harness::stdout(&out).contains("hello world"));
}

#[test]
fn failing_step_propagates_code_and_always_runs() {
    let h = Harness::new("fail");
    h.fg(&["init"]);
    h.write_snippet(
        "boom",
        "name: boom\nsteps:\n  - exit 7\nalways:\n  - echo cleanup-done\n",
    );
    let out = h.fg(&["run", "boom", "-y"]);
    assert_eq!(out.status.code(), Some(7), "{}", Harness::stderr(&out));
    assert!(Harness::stdout(&out).contains("cleanup-done"));
}

#[test]
fn conditions_that_hold_let_steps_run() {
    let h = Harness::new("cond");
    h.fg(&["init"]);
    // `missing: <path>` holds when the path does NOT exist;
    // `exists: .` holds in the working directory.
    h.write_snippet(
        "cond",
        "name: cond\nsteps:\n  - command: echo ran-missing-ok\n    if: { missing: definitely-not-here-12345 }\n  - command: echo ran-exists-ok\n    if: { exists: . }\n",
    );
    let out = h.fg(&["run", "cond", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("ran-missing-ok"), "{stdout}");
    assert!(stdout.contains("ran-exists-ok"), "{stdout}");
    assert!(!Harness::stderr(&out).contains("skipped"));
}

#[test]
fn condition_unmet_skips_and_reports() {
    let h = Harness::new("cond2");
    h.fg(&["init"]);
    h.write_snippet(
        "cond2",
        "name: cond2\nsteps:\n  - command: echo nope\n    if: { exists: no-such-file-xyz }\n  - echo yes\n",
    );
    let out = h.fg(&["run", "cond2", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!Harness::stdout(&out).contains("nope"));
    assert!(Harness::stderr(&out).contains("skipped (condition not met)"));
}

#[test]
fn script_mode_runs_and_cleans_up() {
    let h = Harness::new("script");
    h.fg(&["init"]);
    h.write_snippet(
        "scripty",
        "name: scripty\nmode: script\nscript: |\n  X=hello\n  echo \"var is $X\"\n  read line < /dev/null || true\n",
    );
    let out = h.fg(&["run", "scripty", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(Harness::stdout(&out).contains("var is hello"));
    // No leftover run-* temp dirs.
    let tmp = h.home.join("tmp");
    let leftovers: Vec<_> = std::fs::read_dir(&tmp)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("run-"))
        .collect();
    assert_eq!(leftovers.len(), 0);
}

#[test]
fn tag_rename_delete_search_edit() {
    let h = Harness::new("manage");
    h.fg(&["init"]);
    h.write_snippet(
        "alpha",
        "name: alpha\ndescription: first snippet\nsteps: [echo a]\n",
    );

    let out = h.fg(&["tag", "alpha", "add", "CI", "deploy"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let out = h.fg(&["--json", "show", "alpha"]);
    assert!(Harness::stdout(&out).contains("\"ci\""));

    let out = h.fg(&["tag", "alpha", "remove", "ci"]);
    assert_eq!(out.status.code(), Some(0));
    let out = h.fg(&["search", "first"]);
    assert!(Harness::stdout(&out).contains("alpha"));
    let out = h.fg(&["search", "zzz-no-match"]);
    assert!(Harness::stdout(&out).contains("No snippets match"));

    let out = h.fg(&["rename", "alpha", "beta"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    assert!(h.home.join("snippets/beta.yaml").exists());
    assert!(!h.home.join("snippets/alpha.yaml").exists());

    // Edit with a fake editor that appends a step.
    let editor = h.work.join("editor.sh");
    std::fs::write(
        &editor,
        "#!/bin/sh\nprintf '  - echo edited\\n' >> \"$1\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // beta is steps: [echo a] in flow form; appending a block step after
    // a flow sequence is invalid YAML, so edit a block-style file:
    h.write_snippet("gamma", "name: gamma\nsteps:\n  - echo g\n");
    let out = h.fg_env(
        &["edit", "gamma"],
        &[("EDITOR", editor.to_str().unwrap())],
        "",
    );
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));
    let out = h.fg(&["show", "gamma"]);
    assert!(Harness::stdout(&out).contains("echo edited"));

    // Edit that breaks the file restores the original.
    let bad_editor = h.work.join("bad-editor.sh");
    std::fs::write(&bad_editor, "#!/bin/sh\nprintf 'not: [valid\\n' > \"$1\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bad_editor, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = h.fg_env(
        &["edit", "gamma"],
        &[("EDITOR", bad_editor.to_str().unwrap())],
        "",
    );
    assert_eq!(out.status.code(), Some(1), "{}", Harness::stderr(&out));
    let out = h.fg(&["show", "gamma"]);
    assert!(
        Harness::stdout(&out).contains("echo edited"),
        "original restored"
    );

    // Delete: non-TTY without -y refuses (2); with -y deletes.
    let out = h.fg(&["delete", "beta"]);
    assert_eq!(out.status.code(), Some(2));
    let out = h.fg(&["delete", "beta", "-y"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!h.home.join("snippets/beta.yaml").exists());
    let out = h.fg(&["delete", "beta", "-y"]);
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn retry_and_timeout_behaviour() {
    let h = Harness::new("retry");
    h.fg(&["init"]);
    // Fails twice then succeeds, using a counter file in the work dir.
    h.write_snippet(
        "flaky",
        "name: flaky\nsteps:\n  - command: 'c=$(cat count.txt 2>/dev/null || echo 0); c=$((c+1)); echo $c > count.txt; test $c -ge 3'\n    retry: 3\n",
    );
    let out = h.fg(&["run", "flaky", "-y"]);
    assert_eq!(out.status.code(), Some(0), "{}", Harness::stderr(&out));

    h.write_snippet(
        "slow",
        "name: slow\nsteps:\n  - command: 'sleep 5'\n    timeout: 200ms\n",
    );
    let out = h.fg(&["run", "slow", "-y"]);
    assert_eq!(out.status.code(), Some(124), "{}", Harness::stderr(&out));
}
