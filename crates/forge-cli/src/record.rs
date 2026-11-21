//! `fg record`: capture a shell session's history as a snippet.
//!
//! The recorder starts the user's shell interactively, arranged so
//! the session's history lands in a private temp file, waits for the
//! shell to exit, and turns the recorded lines into steps. The finish
//! word is `end` (not `exit`, which reads as "close my terminal"):
//! bash gets an `end` shell function via a generated `--rcfile`;
//! zsh - where `end` is also a reserved word - gets an Enter-key
//! (zle) widget via generated `ZDOTDIR` startup files; fish - where
//! `end` is a reserved keyword and `HISTFILE` is ignored - gets an
//! Enter-key binding plus an `XDG_DATA_HOME` redirect so its own
//! history file lands in the temp dir. Unrecognized shells keep
//! the plain `HISTFILE` spawn and the honest `exit` instruction.
//! Every line is secret-scanned (forge-core, ADR 0007): a finding
//! refuses the save unless the user explicitly accepts the redacted
//! version.

use crate::exit::CliError;
use crate::store::Store;
use anyhow::{Context, Result};
use forge_core::model::StepSpec;
use forge_core::Snippet;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Turn raw history lines into commands: trim, drop blanks, the shell's
/// own exit commands and the recorder's `end` finish word, and
/// consecutive duplicates.
#[must_use]
pub fn clean_history(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let cmd = line.trim().to_string();
        if cmd.is_empty() || cmd == "exit" || cmd == "logout" || cmd == "end" {
            continue;
        }
        if out.last() == Some(&cmd) {
            continue;
        }
        out.push(cmd);
    }
    out
}

/// Which shell family the recorder is launching, from the shell
/// path's basename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    /// bash: `--rcfile` startup file defines `end`.
    Bash,
    /// zsh: `ZDOTDIR` startup files install the `end` Enter widget.
    Zsh,
    /// fish: Enter-key binding + `XDG_DATA_HOME` history redirect.
    Fish,
    /// Anything else: plain spawn, `exit` is the finish word.
    Other,
}

/// Classify a shell path (or bare name) into a [`ShellKind`].
#[must_use]
pub fn shell_kind(shell: &str) -> ShellKind {
    match Path::new(shell).file_name().and_then(|s| s.to_str()) {
        Some("bash") => ShellKind::Bash,
        Some("zsh") => ShellKind::Zsh,
        Some("fish") => ShellKind::Fish,
        _ => ShellKind::Other,
    }
}

/// Single-quote a path for embedding in a POSIX-shell startup file.
fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

/// The zsh finish mechanism, appended to the generated `.zshrc`.
/// `end` is a reserved word in zsh (the `foreach` terminator), so -
/// exactly as in fish - a function or alias named `end` cannot even
/// be parsed; an Enter-key widget is used instead: when the whole
/// line is exactly `end` it leaves the shell, and otherwise it runs
/// the normal `accept-line` (so multi-line constructs, whose buffer
/// is longer than the single word, behave exactly as usual). Because
/// the widget exits before `accept-line`, the word `end` itself
/// never enters the history file. Verified against zsh 5.9.
const ZSH_FINISH_RC: &str = r#"__forgehub_finish() {
  if [[ "$BUFFER" =~ '^[[:space:]]*end[[:space:]]*$' ]]; then
    exit
  fi
  zle accept-line
}
zle -N __forgehub_finish
bindkey '^M' __forgehub_finish
bindkey '^J' __forgehub_finish
bindkey -M vicmd '^M' __forgehub_finish
"#;

/// fish `--init-command` payload: bind Enter to a wrapper that exits
/// when the whole command line is exactly `end`, and otherwise runs
/// the reader's normal `execute` (so incomplete multi-line blocks -
/// whose own closing `end` line is part of a longer buffer - behave
/// exactly as usual). A function cannot be named `end` in fish (it is
/// a reserved keyword, verified against fish 3.7), and abbreviations
/// do not expand for keyword tokens, so the binding is the mechanism.
/// Because the wrapper exits before `execute`, the word `end` itself
/// never enters the history file.
const FISH_FINISH_INIT: &str = "function __forgehub_finish; \
    if test (count (commandline)) -eq 1; \
    and test (commandline | string trim) = end; exit; end; \
    commandline -f execute; end; \
    bind \\r __forgehub_finish; bind \\n __forgehub_finish";

/// Everything needed to spawn a recording shell and read its history
/// back, computed purely so it can be unit-tested.
#[derive(Debug)]
pub struct LaunchPlan {
    /// Extra argv after the shell program.
    pub args: Vec<String>,
    /// Extra environment for the child shell.
    pub env: Vec<(String, PathBuf)>,
    /// Directories to create before spawning.
    pub dirs: Vec<PathBuf>,
    /// Files to write before spawning (startup files).
    pub files: Vec<(PathBuf, String)>,
    /// Where the session's history lands.
    pub history_path: PathBuf,
    /// True when `history_path` is in fish's `- cmd:` format.
    pub fish_format: bool,
    /// The word the instruction tells the user to type to finish.
    pub finish_word: &'static str,
    /// Files/dirs to remove after the history has been read.
    pub cleanup_paths: Vec<PathBuf>,
}

/// Build the [`LaunchPlan`] for a shell family. All paths live under
/// the store's temp dir and are made unique with the process id.
#[must_use]
pub fn launch_plan(kind: ShellKind, tmp_dir: &Path, pid: u32) -> LaunchPlan {
    let hist = tmp_dir.join(format!(".history-{pid}"));
    let hist_env = || ("HISTFILE".to_string(), hist.clone());
    match kind {
        ShellKind::Bash => {
            let rc = tmp_dir.join(format!(".record-bashrc-{pid}"));
            let rc_text = format!(
                "# Written by `forgehub record` for this session only.\n\
                 if [ -f \"$HOME/.bashrc\" ]; then . \"$HOME/.bashrc\"; fi\n\
                 HISTFILE={}\n\
                 end() {{ exit; }}\n",
                sh_quote(&hist)
            );
            LaunchPlan {
                args: vec![
                    "--rcfile".to_string(),
                    rc.to_string_lossy().into_owned(),
                    "-i".to_string(),
                ],
                env: vec![hist_env()],
                dirs: Vec::new(),
                files: vec![(rc.clone(), rc_text)],
                history_path: hist,
                fish_format: false,
                finish_word: "end",
                cleanup_paths: vec![rc],
            }
        }
        ShellKind::Zsh => {
            let zdotdir = tmp_dir.join(format!(".record-zdotdir-{pid}"));
            let zshenv = format!(
                "# Written by `forgehub record` for this session only.\n\
                 if [ -f \"$HOME/.zshenv\" ]; then source \"$HOME/.zshenv\"; fi\n\
                 ZDOTDIR={}\n",
                sh_quote(&zdotdir)
            );
            // SAVEHIST is forced because a stock zsh defaults it to
            // 0, which would silently save no history at all; the
            // file it writes to is our temp HISTFILE either way.
            let zshrc = format!(
                "# Written by `forgehub record` for this session only.\n\
                 if [ -f \"$HOME/.zshrc\" ]; then source \"$HOME/.zshrc\"; fi\n\
                 HISTFILE={}\n\
                 SAVEHIST=10000\n",
                sh_quote(&hist)
            ) + ZSH_FINISH_RC;
            LaunchPlan {
                args: vec!["-i".to_string()],
                env: vec![("ZDOTDIR".to_string(), zdotdir.clone()), hist_env()],
                dirs: Vec::new(),
                files: vec![
                    (zdotdir.join(".zshenv"), zshenv),
                    (zdotdir.join(".zshrc"), zshrc),
                ],
                history_path: hist,
                fish_format: false,
                finish_word: "end",
                cleanup_paths: vec![zdotdir],
            }
        }
        ShellKind::Fish => {
            let xdg = tmp_dir.join(format!(".record-fish-{pid}"));
            LaunchPlan {
                args: vec![
                    "-i".to_string(),
                    "--init-command".to_string(),
                    FISH_FINISH_INIT.to_string(),
                ],
                env: vec![("XDG_DATA_HOME".to_string(), xdg.clone())],
                dirs: vec![xdg.clone()],
                files: Vec::new(),
                history_path: xdg.join("fish").join("fish_history"),
                fish_format: true,
                finish_word: "end",
                cleanup_paths: vec![xdg],
            }
        }
        ShellKind::Other => LaunchPlan {
            args: vec!["-i".to_string()],
            env: vec![hist_env()],
            dirs: Vec::new(),
            files: Vec::new(),
            history_path: hist,
            fish_format: false,
            finish_word: "exit",
            cleanup_paths: Vec::new(),
        },
    }
}

/// Extract raw commands from history-file text. Fish history uses its
/// `- cmd:` record format (parsed with the same parser `fg analyze`
/// uses, so escaped multi-line commands survive whole); other shells
/// store one command per line.
#[must_use]
pub fn commands_from_history(text: &str, fish_format: bool) -> Vec<String> {
    if fish_format {
        crate::analyze::parse_history_text(text)
            .into_iter()
            .map(|entry| entry.command)
            .collect()
    } else {
        text.lines().map(str::to_string).collect()
    }
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

/// Run the interactive capture: prepare the per-shell launch plan,
/// spawn the shell, wait, and read the history back.
pub fn capture_session(store: &Store, shell: &str) -> Result<Vec<String>> {
    store.ensure_dirs()?;
    let plan = launch_plan(shell_kind(shell), &store.tmp_dir(), std::process::id());
    let _ = std::fs::remove_file(&plan.history_path);
    for dir in &plan.dirs {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    for (path, contents) in &plan.files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    }
    eprintln!(
        "forgehub: recording with {shell}. Run your commands, then type `{}` to finish.",
        plan.finish_word
    );
    let mut command = Command::new(shell);
    command.args(&plan.args);
    for (key, value) in &plan.env {
        command.env(key, value);
    }
    let status = command
        .status()
        .with_context(|| format!("starting shell `{shell}`"))?;
    if !status.success() {
        eprintln!("forgehub: note: the recording shell exited with {status}");
    }
    let text = std::fs::read_to_string(&plan.history_path).unwrap_or_default();
    let _ = std::fs::remove_file(&plan.history_path);
    for path in &plan.cleanup_paths {
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(clean_history(&commands_from_history(
        &text,
        plan.fish_format,
    )))
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
    fn clean_history_drops_the_end_finish_word() {
        let lines: Vec<String> = ["ls", "end", "logout", "pwd", "end"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(clean_history(&lines), vec!["ls", "pwd"]);
    }

    #[test]
    fn shell_kind_from_paths_and_names() {
        assert_eq!(shell_kind("/usr/bin/bash"), ShellKind::Bash);
        assert_eq!(shell_kind("bash"), ShellKind::Bash);
        assert_eq!(shell_kind("/bin/zsh"), ShellKind::Zsh);
        assert_eq!(shell_kind("/usr/bin/fish"), ShellKind::Fish);
        assert_eq!(shell_kind("fish"), ShellKind::Fish);
        assert_eq!(shell_kind("/bin/sh"), ShellKind::Other);
        assert_eq!(shell_kind("/usr/bin/nu"), ShellKind::Other);
        assert_eq!(shell_kind(""), ShellKind::Other);
    }

    fn plan(kind: ShellKind) -> LaunchPlan {
        launch_plan(kind, Path::new("/tmp/fh-tmp"), 42)
    }

    fn env_has(plan: &LaunchPlan, key: &str) -> bool {
        plan.env.iter().any(|(k, _)| k == key)
    }

    #[test]
    fn bash_plan_defines_end_and_restores_histfile() {
        let plan = plan(ShellKind::Bash);
        assert_eq!(plan.finish_word, "end");
        assert!(!plan.fish_format);
        assert!(env_has(&plan, "HISTFILE"));
        assert_eq!(plan.args[0], "--rcfile");
        assert_eq!(plan.args[2], "-i");
        assert_eq!(plan.files.len(), 1);
        let rc = &plan.files[0].1;
        // The user's own rc is sourced first, then our HISTFILE is
        // restored (theirs may set one), then `end` is defined.
        assert!(rc.contains("if [ -f \"$HOME/.bashrc\" ]; then . \"$HOME/.bashrc\"; fi"));
        assert!(rc.contains("HISTFILE='/tmp/fh-tmp/.history-42'"));
        assert!(rc.contains("end() { exit; }"));
        assert!(rc.find("end()").unwrap() > rc.find(".bashrc").unwrap());
        assert_eq!(plan.history_path, Path::new("/tmp/fh-tmp/.history-42"));
    }

    #[test]
    fn zsh_plan_uses_zdotdir_startup_files() {
        let plan = plan(ShellKind::Zsh);
        assert_eq!(plan.finish_word, "end");
        assert!(!plan.fish_format);
        assert!(env_has(&plan, "ZDOTDIR"));
        assert!(env_has(&plan, "HISTFILE"));
        assert_eq!(plan.args, vec!["-i".to_string()]);
        assert_eq!(plan.files.len(), 2);
        let zshenv = &plan.files[0];
        assert!(zshenv.0.ends_with(".zshenv"));
        assert!(zshenv.1.contains("source \"$HOME/.zshenv\""));
        assert!(zshenv
            .1
            .contains("ZDOTDIR='/tmp/fh-tmp/.record-zdotdir-42'"));
        let zshrc = &plan.files[1];
        assert!(zshrc.0.ends_with(".zshrc"));
        assert!(zshrc.1.contains("source \"$HOME/.zshrc\""));
        assert!(zshrc.1.contains("HISTFILE='/tmp/fh-tmp/.history-42'"));
        assert!(zshrc.1.contains("SAVEHIST=10000"));
        // `end` is a reserved word in zsh: the finish mechanism is
        // the Enter-key widget, never a function definition.
        assert!(!zshrc.1.contains("end()"));
        assert!(zshrc.1.contains("__forgehub_finish()"));
        assert!(zshrc.1.contains("zle -N __forgehub_finish"));
        assert!(zshrc.1.contains("bindkey '^M' __forgehub_finish"));
        assert!(zshrc.1.contains("bindkey '^J' __forgehub_finish"));
        assert_eq!(plan.history_path, Path::new("/tmp/fh-tmp/.history-42"));
    }

    #[test]
    fn fish_plan_uses_xdg_and_enter_binding() {
        let plan = plan(ShellKind::Fish);
        assert_eq!(plan.finish_word, "end");
        assert!(plan.fish_format);
        // fish ignores HISTFILE; history must come from XDG_DATA_HOME.
        assert!(env_has(&plan, "XDG_DATA_HOME"));
        assert!(!env_has(&plan, "HISTFILE"));
        assert_eq!(plan.args[0], "-i");
        assert_eq!(plan.args[1], "--init-command");
        let init = &plan.args[2];
        assert!(init.contains("function __forgehub_finish"));
        assert!(init.contains("= end"));
        assert!(init.contains("bind \\r __forgehub_finish"));
        assert!(init.contains("bind \\n __forgehub_finish"));
        assert_eq!(
            plan.history_path,
            Path::new("/tmp/fh-tmp/.record-fish-42/fish/fish_history")
        );
        assert_eq!(
            plan.dirs,
            vec![Path::new("/tmp/fh-tmp/.record-fish-42").to_path_buf()]
        );
    }

    #[test]
    fn other_plan_is_plain_histfile_with_exit_instruction() {
        let plan = plan(ShellKind::Other);
        assert_eq!(plan.finish_word, "exit");
        assert!(!plan.fish_format);
        assert_eq!(plan.args, vec!["-i".to_string()]);
        assert!(env_has(&plan, "HISTFILE"));
        assert_eq!(plan.files, Vec::new());
        assert_eq!(plan.history_path, Path::new("/tmp/fh-tmp/.history-42"));
    }

    #[test]
    fn fish_history_commands_parse_and_clean() {
        // Real fish_history shape, including an escaped multi-line
        // command (a function definition whose own closing `end`
        // line is part of the command and must survive whole) and a
        // manually typed `exit`.
        let text = "- cmd: echo one\n  when: 1700000000\n\
                    - cmd: function foo\\necho in-foo\\nend\n  when: 1700000001\n\
                    - cmd: exit\n  when: 1700000002\n";
        let raw = commands_from_history(text, true);
        assert_eq!(
            raw,
            vec!["echo one", "function foo\necho in-foo\nend", "exit"]
        );
        assert_eq!(
            clean_history(&raw),
            vec!["echo one", "function foo\necho in-foo\nend"]
        );
    }

    #[test]
    fn plain_history_commands_are_lines() {
        let raw = commands_from_history("ls\npwd\n", false);
        assert_eq!(raw, vec!["ls", "pwd"]);
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
