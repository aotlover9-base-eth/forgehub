//! The runner: executes a snippet's steps (or its script body) with
//! substitution, conditions, retries, timeouts, background and parallel
//! steps, and `always` steps. Semantics follow spec-cli §9 and the
//! Forgenesis runner where the two agree.
//!
//! Script mode follows ADR 0004: the body is written to a file in a
//! fresh `0700` directory under the store's `tmp/`, executed as
//! `<shell> <file>`, and removed on every exit path (a drop guard).

use anyhow::{Context, Result};
use forge_core::model::{Condition, Step};
use forge_core::{Mode, Snippet};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Outcome of a run.
#[derive(Debug)]
pub struct RunOutcome {
    /// Process exit code to return (0 = success).
    pub exit_code: i32,
    /// Number of steps actually executed (skips excluded).
    pub executed: usize,
    /// Number of steps skipped by conditions.
    pub skipped: usize,
    /// 1-based index of the step whose failure stopped the run
    /// (Peak B3, for the re-run hint). `None` on success, for
    /// script mode (one unit of work), for `always`-step failures
    /// and for a user abort in step mode.
    pub failed_step: Option<usize>,
}

/// The post-run summary line (Peak B3), shared by the local and
/// cloud run paths so both read identically: exit code, step
/// counts and wall-clock seconds.
#[must_use]
pub fn summary_line(outcome: &RunOutcome, elapsed: Duration) -> String {
    format!(
        "exit code {} · {} executed, {} skipped · {:.1}s",
        outcome.exit_code,
        outcome.executed,
        outcome.skipped,
        elapsed.as_secs_f64()
    )
}

/// The failure hint (Peak B3): when a step stopped the run, say
/// which one and how to step through it. `None` when the run did
/// not stop on a step failure.
#[must_use]
pub fn failure_hint(outcome: &RunOutcome, name: &str) -> Option<String> {
    let step = outcome.failed_step?;
    if outcome.exit_code == 0 {
        return None;
    }
    Some(format!(
        "forgehub: step {step} failed (exit {}) - re-run with `fg run {name} --step` to step through it",
        outcome.exit_code
    ))
}

/// Print the summary line (stdout) and, unless the run already
/// stepped through, the failure hint (stderr).
pub fn print_run_report(outcome: &RunOutcome, elapsed: Duration, name: &str, step_mode: bool) {
    println!("{}", summary_line(outcome, elapsed));
    if let Some(hint) = failure_hint(outcome, name).filter(|_| !step_mode) {
        eprintln!("{hint}");
    }
}

fn subst(text: &str, values: &BTreeMap<String, String>) -> String {
    forge_core::vars::substitute_lossy(text, values)
}

fn subst_map(
    map: &BTreeMap<String, String>,
    values: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    map.iter()
        .map(|(k, v)| (k.clone(), subst(v, values)))
        .collect()
}

/// Effective env for a step: snippet env + step env, both substituted.
fn effective_env(
    snippet: &Snippet,
    step: Option<&Step>,
    values: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut env = subst_map(&snippet.env, values);
    if let Some(step) = step {
        env.extend(subst_map(&step.env, values));
    }
    env
}

/// Effective working directory for a step.
fn effective_cwd(
    snippet: &Snippet,
    step: Option<&Step>,
    values: &BTreeMap<String, String>,
    base: &Path,
) -> PathBuf {
    let raw = step
        .and_then(|s| s.cwd.as_deref())
        .or(snippet.cwd.as_deref());
    match raw {
        Some(raw) => {
            let substituted = subst(raw, values);
            let expanded = forge_core::vars::expand_path(&substituted, values);
            forge_core::vars::resolve_against(base, &expanded)
        }
        None => base.to_path_buf(),
    }
}

fn which_on_path(program: &str, env: &BTreeMap<String, String>) -> bool {
    if program.contains('/') {
        return Path::new(program).exists();
    }
    let path_var = env
        .get("PATH")
        .cloned()
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();
    path_var
        .split(':')
        .any(|dir| Path::new(dir).join(program).exists())
}

/// Evaluate a condition; every present field must hold (AND).
fn condition_met(
    cond: &Condition,
    snippet: &Snippet,
    step: &Step,
    values: &BTreeMap<String, String>,
    base: &Path,
) -> bool {
    let cwd = effective_cwd(snippet, Some(step), values, base);
    let env = effective_env(snippet, Some(step), values);
    if let Some(p) = &cond.exists {
        let path = forge_core::vars::resolve_against(
            &cwd,
            &forge_core::vars::expand_path(&subst(p, values), values),
        );
        if !path.exists() {
            return false;
        }
    }
    if let Some(p) = &cond.missing {
        let path = forge_core::vars::resolve_against(
            &cwd,
            &forge_core::vars::expand_path(&subst(p, values), values),
        );
        if path.exists() {
            return false;
        }
    }
    if let Some(w) = &cond.which {
        if !which_on_path(&subst(w, values), &env) {
            return false;
        }
    }
    if let Some(name) = &cond.env {
        let present = env
            .get(name)
            .cloned()
            .or_else(|| std::env::var(name).ok())
            .is_some_and(|v| !v.is_empty());
        if !present {
            return false;
        }
    }
    if let Some(eq) = &cond.equals {
        if eq.len() == 2 && subst(&eq[0], values) != subst(&eq[1], values) {
            return false;
        }
    }
    if let Some(script) = &cond.shell {
        let status = Command::new(snippet.effective_shell())
            .arg("-c")
            .arg(subst(script, values))
            .current_dir(&cwd)
            .envs(&env)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(status, Ok(s) if s.success()) {
            return false;
        }
    }
    true
}

enum WaitOutcome {
    Exited(std::process::ExitStatus),
    TimedOut,
    SpawnFailed(std::io::Error),
}

fn wait_with_timeout(child: &mut Child, timeout: Option<Duration>) -> WaitOutcome {
    let Some(limit) = timeout else {
        return match child.wait() {
            Ok(s) => WaitOutcome::Exited(s),
            Err(e) => WaitOutcome::SpawnFailed(e),
        };
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return WaitOutcome::Exited(status),
            Ok(None) => {
                if start.elapsed() >= limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    return WaitOutcome::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return WaitOutcome::SpawnFailed(e),
        }
    }
}

fn status_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    // Killed by a signal: 128 + signal (SIGINT 2 -> 130, per spec-cli).
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

/// Run one command once. Returns the exit code (nonzero on failure).
fn run_once(
    shell: &str,
    command: &str,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    quiet: bool,
    background: bool,
    timeout: Option<Duration>,
) -> i32 {
    let mut cmd = Command::new(shell);
    cmd.arg("-c").arg(command).current_dir(cwd).envs(env);
    if quiet {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("forgehub: could not start `{shell}`: {e}");
            return 1;
        }
    };
    if background {
        // Detach: do not wait, do not kill on exit. The child keeps our
        // stdio (already inherited unless quiet).
        std::mem::forget(child);
        return 0;
    }
    match wait_with_timeout(&mut child, timeout) {
        WaitOutcome::Exited(status) => status_code(status),
        WaitOutcome::TimedOut => {
            eprintln!("forgehub: step timed out and was killed");
            124
        }
        WaitOutcome::SpawnFailed(e) => {
            eprintln!("forgehub: waiting on child failed: {e}");
            1
        }
    }
}

fn run_command_with_retries(
    shell: &str,
    command: &str,
    step: &Step,
    cwd: &Path,
    env: &BTreeMap<String, String>,
) -> i32 {
    let attempts = step.retry.as_ref().map_or(1, |r| r.times.max(1));
    let delay = step
        .retry
        .as_ref()
        .and_then(|r| r.delay.as_deref())
        .and_then(|d| forge_core::vars::parse_duration(d).ok());
    let timeout = step
        .timeout
        .as_deref()
        .and_then(|d| forge_core::vars::parse_duration(d).ok());
    let mut code = 1;
    for attempt in 1..=attempts {
        code = run_once(
            shell,
            command,
            cwd,
            env,
            step.quiet,
            step.background,
            timeout,
        );
        if code == 0 {
            return 0;
        }
        if attempt < attempts {
            eprintln!("forgehub: attempt {attempt}/{attempts} failed (exit {code}); retrying");
            if let Some(d) = delay {
                std::thread::sleep(d);
            }
        }
    }
    code
}

fn run_step(
    snippet: &Snippet,
    step: &Step,
    values: &BTreeMap<String, String>,
    base: &Path,
    index: usize,
    total: usize,
) -> i32 {
    let shell = snippet.effective_shell();
    let cwd = effective_cwd(snippet, Some(step), values, base);
    let env = effective_env(snippet, Some(step), values);
    let code = if let Some(command) = &step.command {
        let rendered = subst(command, values);
        // Peak C2: the label is display only - when it falls
        // back to the command text, secrets stay masked (the
        // executed text above keeps the real values).
        let masked;
        let label = if let Some(name) = &step.name {
            name.as_str()
        } else {
            masked = subst(command, &forge_core::vars::display_values(snippet, values));
            masked.as_str()
        };
        eprintln!("[{}/{}] {}", index + 1, total, label);
        run_command_with_retries(shell, &rendered, step, &cwd, &env)
    } else {
        // Parallel step: run all branches concurrently; the step fails
        // if any branch fails.
        eprintln!(
            "[{}/{}] parallel ({} commands)",
            index + 1,
            total,
            step.parallel.len()
        );
        let handles: Vec<_> = step
            .parallel
            .iter()
            .map(|command| {
                let rendered = subst(command, values);
                let shell = shell.to_string();
                let cwd = cwd.clone();
                let env = env.clone();
                let step = step.clone();
                std::thread::spawn(move || {
                    run_command_with_retries(&shell, &rendered, &step, &cwd, &env)
                })
            })
            .collect();
        let mut worst = 0;
        for h in handles {
            let code = h.join().unwrap_or(1);
            if code != 0 && worst == 0 {
                worst = code;
            }
        }
        worst
    };
    if code == 0 {
        if let Some(wait) = &step.wait {
            if let Ok(d) = forge_core::vars::parse_duration(wait) {
                std::thread::sleep(d);
            }
        }
    }
    code
}

/// A directory that is removed on drop (script-mode temp files).
struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_script(
    snippet: &Snippet,
    values: &BTreeMap<String, String>,
    base: &Path,
    tmp_root: &Path,
) -> Result<i32> {
    let body = subst(snippet.script.as_deref().unwrap_or_default(), values);
    std::fs::create_dir_all(tmp_root)
        .with_context(|| format!("creating {}", tmp_root.display()))?;
    let dir = tmp_root.join(format!(
        "run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::create_dir(&dir).with_context(|| format!("creating {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let _guard = TempDirGuard(dir.clone());
    let file = dir.join("script.sh");
    std::fs::write(&file, body).with_context(|| format!("writing {}", file.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700))?;
    }
    let cwd = effective_cwd(snippet, None, values, base);
    let env = effective_env(snippet, None, values);
    let status = Command::new(snippet.effective_shell())
        .arg(&file)
        .current_dir(&cwd)
        .envs(&env)
        .status()
        .with_context(|| format!("running script with {}", snippet.effective_shell()))?;
    Ok(status_code(status))
}

/// Execute a snippet. `base` is the directory relative paths resolve
/// against (normally the caller's cwd); `tmp_root` hosts script-mode
/// temp files.
///
/// # Errors
/// Only for script-mode setup failures (temp dir/file); step failures
/// are reported in the outcome's exit code, not as errors.
pub fn execute(
    snippet: &Snippet,
    values: &BTreeMap<String, String>,
    base: &Path,
    tmp_root: &Path,
) -> Result<RunOutcome> {
    if snippet.effective_mode() == Mode::Script {
        let code = run_script(snippet, values, base, tmp_root)?;
        return Ok(RunOutcome {
            exit_code: code,
            executed: 1,
            skipped: 0,
            failed_step: None,
        });
    }
    let steps = snippet.resolved_steps();
    let always = snippet.always_steps();
    let total = steps.len();
    let mut executed = 0usize;
    let mut skipped = 0usize;
    let mut result = 0i32;
    let mut failed_step = None;
    for (i, step) in steps.iter().enumerate() {
        if let Some(cond) = &step.condition {
            if !condition_met(cond, snippet, step, values, base) {
                eprintln!(
                    "[{}/{}] skipped (condition not met): {}",
                    i + 1,
                    total,
                    step.label()
                );
                skipped += 1;
                continue;
            }
        }
        executed += 1;
        let code = run_step(snippet, step, values, base, i, total);
        if code != 0 {
            if step.continue_on_error {
                eprintln!(
                    "forgehub: step {} failed (exit {code}); continuing (continue_on_error)",
                    i + 1
                );
            } else {
                result = code;
                failed_step = Some(i + 1);
                break;
            }
        }
    }
    // `always` steps run even after a failure.
    for step in &always {
        if let Some(cond) = &step.condition {
            if !condition_met(cond, snippet, step, values, base) {
                continue;
            }
        }
        let code = run_step(snippet, step, values, base, 0, always.len());
        if code != 0 && result == 0 {
            result = code;
        }
    }
    Ok(RunOutcome {
        exit_code: result,
        executed,
        skipped,
        failed_step,
    })
}

/// Step-through execution (`--step`, Step 13): like
/// [`execute`], but before each main step the user is shown the
/// step and asked; `y` runs it, `n`/Enter skips it, `q` aborts
/// the run (exit code 2 - refused). Script mode has a single
/// unit of work, so it behaves exactly like [`execute`].
/// `always` steps are not prompted (they are cleanup by
/// definition) and still run after an abort.
///
/// # Errors
/// Only for script-mode setup failures.
pub fn execute_stepwise(
    snippet: &Snippet,
    values: &BTreeMap<String, String>,
    base: &Path,
    tmp_root: &Path,
    stdin: &mut impl std::io::BufRead,
) -> Result<RunOutcome> {
    if snippet.effective_mode() == Mode::Script {
        eprintln!("forgehub: --step has no effect on script-mode snippets (one unit of work)");
        return execute(snippet, values, base, tmp_root);
    }
    let steps = snippet.resolved_steps();
    let always = snippet.always_steps();
    let total = steps.len();
    let mut executed = 0usize;
    let mut skipped = 0usize;
    let mut result = 0i32;
    let mut failed_step = None;
    let mut aborted = false;
    for (i, step) in steps.iter().enumerate() {
        if let Some(cond) = &step.condition {
            if !condition_met(cond, snippet, step, values, base) {
                eprintln!(
                    "[{}/{}] skipped (condition not met): {}",
                    i + 1,
                    total,
                    step.label()
                );
                skipped += 1;
                continue;
            }
        }
        // Peak C2: the step-through display masks secrets; the
        // run itself (below) uses the real values.
        let display_values = forge_core::vars::display_values(snippet, values);
        let display = if let Some(command) = &step.command {
            subst(command, &display_values)
        } else if !step.parallel.is_empty() {
            step.parallel
                .iter()
                .map(|c| subst(c, &display_values))
                .collect::<Vec<_>>()
                .join(" ; ")
        } else {
            step.label()
        };
        eprintln!(
            "[{}/{}] {} - {}",
            i + 1,
            total,
            step.label(),
            crate::cmds::escape_display(&display)
        );
        match crate::prompt::ask_step(stdin) {
            crate::prompt::StepChoice::Run => {}
            crate::prompt::StepChoice::Skip => {
                eprintln!("  skipped by user");
                skipped += 1;
                continue;
            }
            crate::prompt::StepChoice::Abort => {
                eprintln!("forgehub: run aborted by user");
                aborted = true;
                result = 2;
                break;
            }
        }
        executed += 1;
        let code = run_step(snippet, step, values, base, i, total);
        if code != 0 {
            if step.continue_on_error {
                eprintln!(
                    "forgehub: step {} failed (exit {code}); continuing (continue_on_error)",
                    i + 1
                );
            } else {
                result = code;
                failed_step = Some(i + 1);
                break;
            }
        }
    }
    for step in &always {
        if let Some(cond) = &step.condition {
            if !condition_met(cond, snippet, step, values, base) {
                continue;
            }
        }
        let code = run_step(snippet, step, values, base, 0, always.len());
        if code != 0 && result == 0 {
            result = code;
        }
    }
    let _ = aborted; // reflected in the exit code
    Ok(RunOutcome {
        exit_code: result,
        executed,
        skipped,
        failed_step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(exit_code: i32, failed_step: Option<usize>) -> RunOutcome {
        RunOutcome {
            exit_code,
            executed: 3,
            skipped: 1,
            failed_step,
        }
    }

    #[test]
    fn summary_line_shape_is_stable() {
        assert_eq!(
            summary_line(&outcome(0, None), Duration::from_millis(1500)),
            "exit code 0 · 3 executed, 1 skipped · 1.5s"
        );
        assert_eq!(
            summary_line(&outcome(3, Some(2)), Duration::from_secs(2)),
            "exit code 3 · 3 executed, 1 skipped · 2.0s"
        );
    }

    #[test]
    fn failure_hint_names_the_step_and_the_step_flag() {
        let hint = failure_hint(&outcome(3, Some(2)), "deploy").expect("hint");
        assert!(hint.contains("step 2 failed (exit 3)"), "{hint}");
        assert!(hint.contains("fg run deploy --step"), "{hint}");
        assert!(
            failure_hint(&outcome(0, Some(2)), "deploy").is_none(),
            "no hint on success"
        );
        assert!(
            failure_hint(&outcome(1, None), "deploy").is_none(),
            "no hint without a step"
        );
    }

    #[test]
    fn execute_records_the_failing_step() {
        let snippet =
            Snippet::from_yaml("name: t\nsteps:\n  - \"true\"\n  - \"exit 3\"\n  - \"true\"\n")
                .unwrap();
        let base = std::env::temp_dir();
        let outcome = execute(&snippet, &BTreeMap::new(), &base, &base).unwrap();
        assert_eq!(outcome.exit_code, 3);
        assert_eq!(outcome.executed, 2, "the third step never runs");
        assert_eq!(outcome.failed_step, Some(2));
    }

    #[test]
    fn execute_success_has_no_failed_step() {
        let snippet = Snippet::from_yaml("name: t\nsteps:\n  - \"true\"\n").unwrap();
        let base = std::env::temp_dir();
        let outcome = execute(&snippet, &BTreeMap::new(), &base, &base).unwrap();
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.failed_step, None);
    }
}
