//! Peak C2 integration tests: typed parameters at run time -
//! choice/number enforcement, yesno normalisation, and the
//! secret rules (masked in every display, used in execution,
//! refused with a default, clean error without a terminal).

use std::path::PathBuf;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_forgehub");

struct Rig {
    home: tempfile::TempDir,
}

impl Rig {
    fn p(&self, name: &str) -> PathBuf {
        self.home.path().join(name)
    }
    fn run(&self, args: &[&str]) -> (i32, String, String) {
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
        drop(child.stdin.take());
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }
    fn write_local(&self, name: &str, yaml_body: &str) {
        let dir = self.p("fg").join("snippets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.yaml")), yaml_body).unwrap();
    }
}

fn rig() -> Rig {
    Rig {
        home: tempfile::tempdir().unwrap(),
    }
}

const TYPED: &str = "name: typed\ndescription: typed params\nvariables:\n  REGION:\n    type: choice\n    choices: [eu, us]\n    default: eu\n  COUNT:\n    type: number\n  CONFIRM:\n    type: yesno\nmode: script\nscript: |\n  echo region={{REGION}} count={{COUNT}} confirm={{CONFIRM}}\n";

#[test]
fn choice_and_number_are_enforced() {
    let r = rig();
    r.write_local("typed", TYPED);
    let (code, out, err) = r.run(&["-y", "run", "typed", "REGION=us", "COUNT=3", "CONFIRM=yes"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("region=us count=3 confirm=yes"), "{out}");

    let (code, _, err) = r.run(&[
        "-y",
        "run",
        "typed",
        "REGION=moon",
        "COUNT=1",
        "CONFIRM=yes",
    ]);
    assert_ne!(code, 0);
    assert!(err.contains("REGION") && err.contains("eu, us"), "{err}");

    let (code, _, err) = r.run(&["-y", "run", "typed", "COUNT=many", "CONFIRM=yes"]);
    assert_ne!(code, 0);
    assert!(
        err.contains("COUNT") && err.contains("not a number"),
        "{err}"
    );
}

#[test]
fn yesno_is_normalised() {
    let r = rig();
    r.write_local("typed", TYPED);
    let (code, out, err) = r.run(&["-y", "run", "typed", "CONFIRM=True", "COUNT=1"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("confirm=yes"), "{out}");
    let (code, out, _) = r.run(&["-y", "run", "typed", "CONFIRM=n", "COUNT=1"]);
    assert_eq!(code, 0);
    assert!(out.contains("confirm=no"), "{out}");
    let (code, _, err) = r.run(&["-y", "run", "typed", "CONFIRM=perhaps", "COUNT=1"]);
    assert_ne!(code, 0);
    assert!(err.contains("yes/no"), "{err}");
}

const SECRET: &str = "name: vault\ndescription: secret demo\nvariables:\n  TOKEN:\n    type: secret\n    required: true\nmode: script\nscript: |\n  echo token-is-{{TOKEN}}\n";

#[test]
fn secret_is_masked_in_the_plan_but_executes() {
    let r = rig();
    r.write_local("vault", SECRET);
    // The dry-run plan shows the mask, never the value.
    let (code, out, err) = r.run(&["run", "vault", "--dry", "TOKEN=hunter2"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("••••••"), "plan masks the secret: {out}");
    assert!(!out.contains("hunter2"), "plan leaked the secret: {out}");
    // A real run uses the real value.
    let (code, out, err) = r.run(&["-y", "run", "vault", "TOKEN=hunter2"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("token-is-hunter2"), "{out}");
    // Without a value and without a terminal: a clean
    // missing-values error, no prompt attempt, no hang.
    let (code, _, err) = r.run(&["run", "vault"]);
    assert_ne!(code, 0);
    assert!(err.contains("TOKEN"), "{err}");
}

#[test]
fn secret_is_masked_in_step_labels() {
    let r = rig();
    r.write_local(
        "steps-vault",
        "name: steps-vault\ndescription: steps secret\nvariables:\n  TOKEN:\n    type: secret\n    required: true\nsteps:\n  - run: echo {{TOKEN}}\n",
    );
    let (code, out, err) = r.run(&["-y", "run", "steps-vault", "TOKEN=hunter2"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("hunter2"),
        "the step itself ran with the value: {out}"
    );
    assert!(
        !err.contains("hunter2"),
        "the step label (stderr) leaked the secret: {err}"
    );
    assert!(err.contains("••••••"), "label shows the mask: {err}");
}

#[test]
fn secret_with_a_default_is_refused() {
    let r = rig();
    r.write_local(
        "bad-vault",
        "name: bad-vault\ndescription: bad\nvariables:\n  TOKEN:\n    type: secret\n    default: hunter2\nsteps:\n  - run: echo hi\n",
    );
    let (code, _, err) = r.run(&["lint", "bad-vault"]);
    assert_ne!(code, 0);
    assert!(err.contains("secret"), "{err}");
    assert!(err.contains("default"), "{err}");
}
