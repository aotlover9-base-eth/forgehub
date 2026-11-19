//! The Step 4 commands: `init`, `record`, `run` (+ shorthand), `list`,
//! `show`, `search`, `tag`, `rename`, `delete`, `edit`.
//!
//! Output rules (spec-cli §3): human output on stdout, progress and
//! prompts on stderr, `--json` prints exactly one JSON value on stdout.
//! Untrusted text in the confirmation card is display-escaped (§8).

use crate::exit::CliError;
use crate::record;
use crate::runner;
use crate::store::Store;
use anyhow::{Context, Result};
use forge_core::{Mode, Snippet};
use std::io::BufRead;

/// Escape a string for safe terminal display: ANSI escapes, CR, bidi
/// controls and zero-width characters are neutralised (spec-cli §8).
#[must_use]
pub fn escape_display(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{1b}' => out.push_str("\\e"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("  "),
            '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{feff}' => {}
            c if c.is_control() => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    out
}

/// `fg init` — create the `ForgeHub` home, config and library dirs.
pub fn cmd_init(store: &Store, json: bool) -> Result<i32> {
    let created = store.ensure_dirs()?;
    let cfg_path = store.config_path();
    let mut config_created = false;
    if !cfg_path.exists() {
        store.save_config(&crate::store::Config::default())?;
        config_created = true;
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "home": store.root().display().to_string(),
                "created": created.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                "config_created": config_created,
            })
        );
    } else {
        println!("ForgeHub home: {}", store.root().display());
        for dir in &created {
            println!("  created {}", dir.display());
        }
        if config_created {
            println!("  created {}", cfg_path.display());
        }
        println!("Next: `fg record` to capture a shell session, or place a snippet YAML in {} and `fg run <name>`.", store.snippets_dir().display());
    }
    Ok(0)
}

/// `fg record` — capture a shell session and save it as a snippet.
pub fn cmd_record(
    store: &Store,
    stdin: &mut impl BufRead,
    name: Option<String>,
    description: Option<&str>,
    tags: &[String],
    force: bool,
) -> Result<i32> {
    let config = store.load_config()?;
    let shell = config
        .default_shell
        .clone()
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "bash".to_string());
    let commands = record::capture_session(store, &shell)?;
    if commands.is_empty() {
        eprintln!("forgehub: nothing was recorded; no snippet saved");
        return Ok(1);
    }
    let is_tty = crate::prompt::stdin_is_tty();
    let commands = record::scan_and_maybe_redact(&commands, stdin, is_tty)?;
    let name = match name {
        Some(n) => n,
        None => match crate::prompt::ask_line(stdin, "Name for this snippet:") {
            Some(n) if !n.is_empty() => n,
            _ => {
                return Err(CliError::failure(
                    "forgehub: recording discarded\n  what: no snippet name was given\n  why: a snippet needs a name\n  fix: run `fg record <name>`",
                )
                .into());
            }
        },
    };
    forge_core::names::validate_snippet_name(&name)?;
    if store.exists(&name) && !force {
        return Err(CliError::failure(format!(
            "forgehub: a snippet named `{name}` already exists\n  what: recording would overwrite it\n  why: overwrites are never silent\n  fix: choose another name, or pass --force"
        ))
        .into());
    }
    let tags = forge_core::names::normalize_tags(tags)?;
    let snippet = record::build_snippet(&name, description.unwrap_or_default(), &tags, &commands);
    snippet.validate()?;
    let path = store.save(&snippet)?;
    println!(
        "Saved `{name}` ({} commands) to {}",
        commands.len(),
        path.display()
    );
    Ok(0)
}

/// The confirmation card / dry-run plan for a snippet about to run.
pub fn render_plan(
    snippet: &Snippet,
    source_label: &str,
    values: &std::collections::BTreeMap<String, String>,
    hash: &str,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    // Writing to a String is infallible; ignore the results.
    let _ = writeln!(
        out,
        "About to run `{}` ({})",
        escape_display(&snippet.name),
        source_label
    );
    if !snippet.description.is_empty() {
        let _ = writeln!(out, "  {}", escape_display(&snippet.description));
    }
    let _ = writeln!(
        out,
        "  mode: {} \u{b7} shell: {} \u{b7} hash: {}",
        match snippet.effective_mode() {
            Mode::Script => "script",
            Mode::Steps => "steps",
        },
        snippet.effective_shell(),
        &hash[..8.min(hash.len())]
    );
    match snippet.effective_mode() {
        Mode::Script => {
            out.push_str("  script:\n");
            let body = forge_core::vars::substitute_lossy(
                snippet.script.as_deref().unwrap_or_default(),
                values,
            );
            for line in body.lines() {
                let _ = writeln!(out, "    {}", escape_display(line));
            }
        }
        Mode::Steps => {
            let steps = snippet.resolved_steps();
            for (i, step) in steps.iter().enumerate() {
                for cmd in step.commands() {
                    let rendered = forge_core::vars::substitute_lossy(cmd, values);
                    let _ = writeln!(out, "  {}. {}", i + 1, escape_display(&rendered));
                }
            }
            let always = snippet.always_steps();
            if !always.is_empty() {
                out.push_str("  always:\n");
                for step in &always {
                    for cmd in step.commands() {
                        let rendered = forge_core::vars::substitute_lossy(cmd, values);
                        let _ = writeln!(out, "    {}", escape_display(&rendered));
                    }
                }
            }
        }
    }
    let warnings = forge_core::lint::lint_snippet(snippet);
    if !warnings.is_empty() {
        out.push_str("  warnings:\n");
        for w in warnings {
            let _ = writeln!(out, "    line {}: {}", w.line, w.kind.message());
        }
    }
    out
}

/// Arguments shared by the run paths.
pub struct RunArgs {
    /// Snippet name (local for Step 4).
    pub name: String,
    /// `KEY=value` assignments.
    pub assignments: Vec<String>,
    /// Skip the confirmation prompt.
    pub yes: bool,
    /// Print the plan only.
    pub dry_run: bool,
    /// JSON output.
    pub json: bool,
}

/// `fg run <name> [KEY=value…]` and the `fg <name>` shorthand.
pub fn cmd_run(store: &Store, stdin: &mut impl BufRead, args: &RunArgs) -> Result<i32> {
    let snippet = match store.load(&args.name) {
        Ok(s) => s,
        Err(e) => {
            if crate::exit::code_of(&e) == 3 {
                let suggestions: Vec<String> = store
                    .names()
                    .into_iter()
                    .filter(|n| n.contains(&args.name) || args.name.contains(n.as_str()))
                    .take(3)
                    .collect();
                let mut msg = format!(
                    "forgehub: no local snippet named `{}`\n  what: the name was not found in {}\n  why: names must match exactly\n  fix: run `fg list` to see your snippets",
                    args.name,
                    store.snippets_dir().display()
                );
                if !suggestions.is_empty() {
                    use std::fmt::Write as _;
                    let _ = write!(msg, "\n  similar: {}", suggestions.join(", "));
                }
                return Err(CliError::not_found(msg).into());
            }
            return Err(e);
        }
    };
    let cli_values = forge_core::vars::parse_assignments(&args.assignments)?;
    let values = match forge_core::vars::resolve(&snippet, &cli_values) {
        Ok(v) => v,
        Err(forge_core::ForgeError::MissingVariables(missing)) => {
            return Err(CliError::failure(format!(
                "forgehub: missing values for variables: {}\n  what: these variables have no value from arguments, defaults or built-ins\n  why: a command with an empty variable would run the wrong thing\n  fix: {}",
                missing.join(", "),
                forge_core::vars::missing_hint(&missing)
            ))
            .into());
        }
        Err(e) => return Err(e.into()),
    };
    let hash = forge_core::hash::content_hash(&snippet);
    if args.dry_run {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "name": snippet.name,
                    "source": "local",
                    "hash": hash,
                    "dry_run": true,
                    "commands": snippet.all_command_texts(),
                })
            );
        } else {
            print!("{}", render_plan(&snippet, "local", &values, &hash));
            println!("Dry run: nothing was executed.");
        }
        return Ok(0);
    }
    if !args.yes {
        let config = store.load_config()?;
        if config.confirm_before_run {
            if !crate::prompt::stdin_is_tty() {
                return Err(CliError::refused(format!(
                    "forgehub: not confirmed\n  what: `{}` was not run\n  why: confirmation is on and there is no terminal to confirm on\n  fix: pass --yes (-y) if you have reviewed the snippet",
                    snippet.name
                ))
                .into());
            }
            eprint!("{}", render_plan(&snippet, "local", &values, &hash));
            if !crate::prompt::ask_yes_no(stdin, "Run it?") {
                eprintln!("forgehub: not run (declined)");
                return Ok(2);
            }
        }
    }
    let base = std::env::current_dir().context("reading the current directory")?;
    let outcome = runner::execute(&snippet, &values, &base, &store.tmp_dir())?;
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "name": snippet.name,
                "source": "local",
                "hash": hash,
                "exit_code": outcome.exit_code,
                "executed": outcome.executed,
                "skipped": outcome.skipped,
            })
        );
    }
    Ok(outcome.exit_code)
}

/// `fg list` — the local library.
pub fn cmd_list(store: &Store, json: bool) -> i32 {
    let snippets = store.list();
    if json {
        let arr: Vec<_> = snippets
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "tags": s.tags,
                    "mode": match s.effective_mode() { Mode::Script => "script", Mode::Steps => "steps" },
                    "commands": s.command_count(),
                    "hash": forge_core::hash::short_hash(s),
                })
            })
            .collect();
        println!("{}", serde_json::json!(arr));
        return 0;
    }
    if snippets.is_empty() {
        println!(
            "No snippets yet. Run `fg record` to create one, or add a YAML file to {}",
            store.snippets_dir().display()
        );
        return 0;
    }
    for s in &snippets {
        let desc = if s.description.is_empty() {
            String::new()
        } else {
            format!(" — {}", escape_display(&s.description))
        };
        println!("{} ({} commands){}", s.name, s.command_count(), desc);
        if !s.tags.is_empty() {
            println!("    tags: {}", s.tags.join(", "));
        }
    }
    0
}

/// `fg show <name>` — print the stored YAML (or JSON).
pub fn cmd_show(store: &Store, name: &str, json: bool) -> Result<i32> {
    let snippet = store.load(name)?;
    if json {
        let mut value = serde_json::to_value(&snippet).context("serialising snippet")?;
        value["hash"] = serde_json::Value::String(forge_core::hash::content_hash(&snippet));
        println!("{value}");
    } else {
        print!("{}", snippet.to_yaml()?);
    }
    Ok(0)
}

/// `fg search <query>` — substring match over name, description, tags
/// and commands.
pub fn cmd_search(store: &Store, query: &str, json: bool) -> i32 {
    let q = query.to_lowercase();
    let matches: Vec<Snippet> = store
        .list()
        .into_iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&q)
                || s.description.to_lowercase().contains(&q)
                || s.tags.iter().any(|t| t.contains(&q))
                || s.all_command_texts()
                    .iter()
                    .any(|c| c.to_lowercase().contains(&q))
        })
        .collect();
    if json {
        let arr: Vec<_> = matches
            .iter()
            .map(|s| serde_json::json!({"name": s.name, "description": s.description, "tags": s.tags}))
            .collect();
        println!("{}", serde_json::json!(arr));
        return 0;
    }
    if matches.is_empty() {
        println!("No snippets match `{query}`.");
    }
    for s in &matches {
        println!("{} — {}", s.name, escape_display(&s.description));
    }
    0
}

/// `fg tag <name> add|remove <tag>…`.
pub fn cmd_tag(
    store: &Store,
    name: &str,
    action: &str,
    tags: &[String],
    json: bool,
) -> Result<i32> {
    let mut snippet = store.load(name)?;
    match action {
        "add" => {
            for t in forge_core::names::normalize_tags(tags)? {
                if !snippet.tags.contains(&t) {
                    snippet.tags.push(t);
                }
            }
        }
        "remove" => {
            let removing = forge_core::names::normalize_tags(tags)?;
            snippet.tags.retain(|t| !removing.contains(t));
        }
        other => {
            return Err(CliError::failure(format!(
                "forgehub: unknown tag action `{other}`\n  what: expected `add` or `remove`\n  fix: fg tag {name} add <tag>…"
            ))
            .into());
        }
    }
    snippet.tags.sort();
    store.save(&snippet)?;
    if json {
        println!(
            "{}",
            serde_json::json!({"name": snippet.name, "tags": snippet.tags})
        );
    } else {
        println!("`{name}` tags: {}", snippet.tags.join(", "));
    }
    Ok(0)
}

/// `fg rename <old> <new>`.
pub fn cmd_rename(store: &Store, old: &str, new: &str, json: bool) -> Result<i32> {
    forge_core::names::validate_snippet_name(new)?;
    if store.exists(new) {
        return Err(CliError::failure(format!(
            "forgehub: a snippet named `{new}` already exists\n  what: rename would overwrite it\n  fix: delete `{new}` first or choose another name"
        ))
        .into());
    }
    let mut snippet = store.load(old)?;
    snippet.name = new.to_string();
    store.save(&snippet)?;
    store.delete(old)?;
    if json {
        println!("{}", serde_json::json!({"renamed_from": old, "name": new}));
    } else {
        println!("Renamed `{old}` to `{new}`.");
    }
    Ok(0)
}

/// `fg delete <name>` — confirms unless `--yes`.
pub fn cmd_delete(
    store: &Store,
    stdin: &mut impl BufRead,
    name: &str,
    yes: bool,
    json: bool,
) -> Result<i32> {
    if !store.exists(name) {
        return Err(
            CliError::not_found(format!("forgehub: no local snippet named `{name}`")).into(),
        );
    }
    if !yes {
        if !crate::prompt::stdin_is_tty() {
            return Err(CliError::refused(
                "forgehub: not confirmed\n  what: delete needs confirmation and there is no terminal\n  fix: pass --yes (-y) to delete without prompting",
            )
            .into());
        }
        if !crate::prompt::ask_yes_no(stdin, &format!("Delete `{name}` permanently?")) {
            println!("Kept `{name}`.");
            return Ok(2);
        }
    }
    store.delete(name)?;
    if json {
        println!("{}", serde_json::json!({"deleted": name}));
    } else {
        println!("Deleted `{name}`.");
    }
    Ok(0)
}

/// `fg edit <name>` — open in the editor; re-validate afterwards and
/// restore the original when the edit breaks the file.
pub fn cmd_edit(store: &Store, name: &str) -> Result<i32> {
    let path = store.path_for(name)?;
    if !path.exists() {
        return Err(
            CliError::not_found(format!("forgehub: no local snippet named `{name}`")).into(),
        );
    }
    let original = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let config = store.load_config()?;
    let editor = config
        .editor
        .clone()
        .or_else(|| std::env::var("EDITOR").ok())
        .or_else(|| std::env::var("VISUAL").ok())
        .unwrap_or_else(|| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("running editor `{editor}`"))?;
    if !status.success() {
        return Err(CliError::failure(format!(
            "forgehub: editor `{editor}` exited with {status}; the file was left as the editor saved it"
        ))
        .into());
    }
    let edited = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    match Snippet::from_bytes(&edited) {
        Ok(parsed) => {
            if parsed.name == name {
                println!("Saved `{name}`.");
            } else {
                // A rename inside the editor is honoured by moving the file.
                store.save(&parsed)?;
                store.delete(name)?;
                println!("Saved edits; the snippet is now named `{}`.", parsed.name);
            }
            Ok(0)
        }
        Err(e) => {
            std::fs::write(&path, &original)
                .with_context(|| format!("restoring {}", path.display()))?;
            Err(CliError::failure(format!(
                "forgehub: the edited file is not a valid snippet, so the original was restored\n  what: {e}\n  why: the library must never hold an unparseable snippet\n  fix: run `fg edit {name}` again and fix the error"
            ))
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_neutralises_ansi_and_bidi() {
        let dirty = "ok\u{1b}[31m red \u{202e}evil\u{200b}";
        let clean = escape_display(dirty);
        assert!(!clean.contains('\u{1b}'));
        assert!(!clean.contains('\u{202e}'));
        assert!(clean.contains("\\e"));
    }

    #[test]
    fn escape_keeps_plain_text() {
        assert_eq!(
            escape_display("apt update && apt upgrade"),
            "apt update && apt upgrade"
        );
    }
}
