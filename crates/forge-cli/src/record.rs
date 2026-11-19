//! `fg record`: capture a shell session's history as a snippet.
//!
//! The recorder starts the user's shell interactively with `HISTFILE`
//! pointed at a private temp file, waits for the shell to exit, and
//! turns the recorded lines into steps. Every line is secret-scanned
//! (forge-core, ADR 0007): a finding refuses the save unless the user
//! explicitly accepts the redacted version.

use crate::exit::CliError;
use crate::store::Store;
use anyhow::{Context, Result};
use forge_core::model::StepSpec;
use forge_core::Snippet;
use std::io::BufRead;
use std::process::Command;

/// Turn raw history lines into commands: trim, drop blanks, the shell's
/// own exit commands, and consecutive duplicates.
#[must_use]
pub fn clean_history(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let cmd = line.trim().to_string();
        if cmd.is_empty() || cmd == "exit" || cmd == "logout" {
            continue;
        }
        if out.last() == Some(&cmd) {
            continue;
        }
        out.push(cmd);
    }
    out
}

/// Scan commands for secrets. On findings: print them (kind + command
/// number only), then either return the redacted commands (when the
/// user consents on a terminal) or a refusal error.
pub fn scan_and_maybe_redact(
    commands: &[String],
    stdin: &mut impl BufRead,
    is_tty: bool,
) -> Result<Vec<String>> {
    let mut findings: Vec<(usize, forge_core::scan::SecretKind)> = Vec::new();
    for (i, cmd) in commands.iter().enumerate() {
        for f in forge_core::scan::scan(cmd) {
            findings.push((i + 1, f.kind));
        }
    }
    if findings.is_empty() {
        return Ok(commands.to_vec());
    }
    eprintln!(
        "forgehub: secrets were found in the recorded commands; refusing to save them as-is:"
    );
    for (index, kind) in &findings {
        eprintln!("  command {index}: {}", kind.as_str());
    }
    if !is_tty {
        return Err(CliError::refused(
            "forgehub: recording contains secrets and there is no terminal to confirm a redacted save\n  what: the recorded commands contain likely secrets\n  why: a saved snippet must never contain a real credential\n  fix: re-record without the secret, or run `fg record` in a terminal to save a redacted version",
        )
        .into());
    }
    if crate::prompt::ask_yes_no(stdin, "Save a REDACTED version instead (secrets replaced)?") {
        Ok(commands
            .iter()
            .map(|c| forge_core::scan::redact(c))
            .collect())
    } else {
        Err(CliError::refused(
            "forgehub: recording discarded\n  what: you declined the redacted save\n  why: the original commands contain likely secrets\n  fix: re-record using variables like {{PASSWORD}} instead of literals",
        )
        .into())
    }
}

/// Run the interactive capture: spawn the shell with a private
/// `HISTFILE`, wait, and read the history back.
pub fn capture_session(store: &Store, shell: &str) -> Result<Vec<String>> {
    store.ensure_dirs()?;
    let hist = store
        .tmp_dir()
        .join(format!(".history-{}", std::process::id()));
    let _ = std::fs::remove_file(&hist);
    eprintln!("forgehub: recording with {shell}. Run your commands, then type `exit` to finish.");
    let status = Command::new(shell)
        .arg("-i")
        .env("HISTFILE", &hist)
        .status()
        .with_context(|| format!("starting shell `{shell}`"))?;
    if !status.success() {
        eprintln!("forgehub: note: the recording shell exited with {status}");
    }
    let text = std::fs::read_to_string(&hist).unwrap_or_default();
    let _ = std::fs::remove_file(&hist);
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    Ok(clean_history(&lines))
}

/// Build a snippet from captured commands.
#[must_use]
pub fn build_snippet(
    name: &str,
    description: &str,
    tags: &[String],
    commands: &[String],
) -> Snippet {
    let mut snippet = Snippet::new(name);
    snippet.description = description.to_string();
    snippet.tags = tags.to_vec();
    snippet.steps = commands
        .iter()
        .map(|c| StepSpec::from_command(c.clone()))
        .collect();
    snippet.created = forge_core::vars::builtins().get("TIMESTAMP").cloned();
    snippet
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn cursor(s: &str) -> Cursor<Vec<u8>> {
        Cursor::new(s.as_bytes().to_vec())
    }

    #[test]
    fn clean_history_drops_blanks_exits_and_dupes() {
        let lines: Vec<String> = ["", "ls", "ls", " pwd ", "exit", "whoami"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(clean_history(&lines), vec!["ls", "pwd", "whoami"]);
    }

    #[test]
    fn clean_commands_pass_scan() {
        let cmds = vec!["echo hi".to_string()];
        let out = scan_and_maybe_redact(&cmds, &mut cursor(""), false).unwrap();
        assert_eq!(out, cmds);
    }

    #[test]
    fn secret_refuses_without_tty() {
        let cmds = vec![format!("export TOKEN={}", "Zx9Qw8Er5Ty2")];
        let err = scan_and_maybe_redact(&cmds, &mut cursor(""), false).unwrap_err();
        assert_eq!(crate::exit::code_of(&err), 2);
    }

    #[test]
    fn secret_redacts_with_consent() {
        let secret = "Zx9Qw8Er5Ty2".to_string();
        let cmds = vec![format!("export TOKEN={secret}")];
        let out = scan_and_maybe_redact(&cmds, &mut cursor("y\n"), true).unwrap();
        assert!(!out[0].contains(&secret));
        assert!(out[0].contains("REDACTED"));
    }

    #[test]
    fn secret_discards_without_consent() {
        let cmds = vec![format!("export TOKEN={}", "Zx9Qw8Er5Ty2")];
        let err = scan_and_maybe_redact(&cmds, &mut cursor("n\n"), true).unwrap_err();
        assert_eq!(crate::exit::code_of(&err), 2);
    }

    #[test]
    fn build_snippet_shape() {
        let s = build_snippet("demo", "d", &["ci".to_string()], &["echo a".to_string()]);
        assert_eq!(s.name, "demo");
        assert_eq!(s.command_count(), 1);
        assert!(s.validate().is_ok());
    }
}
