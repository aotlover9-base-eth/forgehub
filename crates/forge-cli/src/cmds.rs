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
            | '\u{2066}'..='\u{2069}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{ad}'
            | '\u{feff}' => {}
            c if c.is_control() => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    out
}

/// `fg init` - create the `ForgeHub` home, config and library dirs.
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

/// `fg record` - capture a shell session and save it as a snippet.
/// Record a session; also returns the saved snippet's name
/// (Step 12's `record --push` hook needs it).
pub fn cmd_record_inner(
    store: &Store,
    stdin: &mut impl BufRead,
    name: Option<String>,
    description: Option<&str>,
    tags: &[String],
    force: bool,
) -> Result<(i32, Option<String>)> {
    let config = store.load_config()?;
    let shell = config
        .default_shell
        .clone()
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "bash".to_string());
    let commands = record::capture_session(store, &shell)?;
    if commands.is_empty() {
        eprintln!("forgehub: nothing was recorded; no snippet saved");
        return Ok((1, None));
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
    Ok((0, Some(name)))
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
#[allow(clippy::struct_excessive_bools)]
pub struct RunArgs {
    /// Snippet name or `owner/name[@version]` reference.
    pub name: String,
    /// `KEY=value` assignments.
    pub assignments: Vec<String>,
    /// Skip the confirmation prompt.
    pub yes: bool,
    /// Print the plan only.
    pub dry_run: bool,
    /// JSON output.
    pub json: bool,
    /// Step-through execution (Step 13).
    pub step: bool,
    /// Do not write the run cache (cloud runs).
    pub no_cache: bool,
    /// Cache-only mode (cloud runs).
    pub offline: bool,
}

impl RunArgs {
    /// The cloud-run view of these options.
    #[must_use]
    pub fn to_cloud(&self) -> crate::cloudrun::CloudRunArgs {
        crate::cloudrun::CloudRunArgs {
            yes: self.yes,
            dry_run: self.dry_run,
            json: self.json,
            step: self.step,
            no_cache: self.no_cache,
            offline: self.offline,
            assignments: self.assignments.clone(),
        }
    }
}

/// The not-found error for a run reference, with up to 3
/// suggestions (similar local names, then remote search hits
/// when logged in). A suggestion is never auto-run (spec-cli §5).
fn run_not_found(store: &Store, stdin: &mut impl BufRead, name: &str) -> CliError {
    let suggestions = crate::cloudrun::suggestions(store, stdin, name);
    let mut msg = format!(
        "forgehub: no snippet named `{name}`\n  what: not found locally, and no cloud snippet matched either\n  why: names must match exactly\n  fix: run `fg list` locally, or `fg remote search <text>` in the cloud"
    );
    if !suggestions.is_empty() {
        use std::fmt::Write as _;
        let _ = write!(msg, "\n  did you mean: {}", suggestions.join(", "));
    }
    CliError::not_found(msg)
}

/// `fg run <name> [KEY=value…]` and the `fg <name>` shorthand.
///
/// Resolution (Step 13): an explicit `owner/name` or an `@N`
/// pin goes to the cloud; a bare name runs the local snippet
/// when one exists, else the caller's own cloud snippet, else
/// a not-found error with suggestions.
pub fn cmd_run(store: &Store, stdin: &mut impl BufRead, args: &RunArgs) -> Result<i32> {
    let refr = crate::cloudrun::parse_reference(&args.name);
    if refr.owner.is_some() || refr.version.is_some() {
        return crate::cloudrun::run_cloud(store, stdin, &refr, &args.to_cloud());
    }
    let snippet = match store.load(&args.name) {
        Ok(s) => s,
        Err(e) => {
            if crate::exit::code_of(&e) == 3 {
                // Cloud fallback: my own cloud snippet of this
                // name (needs a credential, or --offline cache).
                let can_cloud = args.offline
                    || matches!(
                        crate::cloud::active_credential(store, stdin, None),
                        Ok(Some(_))
                    );
                if can_cloud {
                    match crate::cloudrun::run_cloud(store, stdin, &refr, &args.to_cloud()) {
                        Err(ce) if crate::exit::code_of(&ce) == 3 => {
                            return Err(run_not_found(store, stdin, &args.name).into());
                        }
                        other => return other,
                    }
                }
                return Err(run_not_found(store, stdin, &args.name).into());
            }
            return Err(e);
        }
    };
    let cli_values = forge_core::vars::parse_assignments(&args.assignments)?;
    let values = resolve_or_prompt(&snippet, &cli_values, stdin, crate::prompt::stdin_is_tty())?;
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
    // Record the run in history.db (spec-cli §9). Best-effort: a
    // broken history DB must never break a run - `fg doctor` reports
    // DB health. The row is started now and finished after execution;
    // if this process is killed mid-run the row stays unfinished and
    // stats counts it as interrupted.
    let history = crate::history::Db::open(&store.history_db_path()).ok();
    let run_id = history
        .as_ref()
        .and_then(|db| db.start_run(&snippet.name).ok());
    let started = std::time::Instant::now();
    let outcome = if args.step {
        runner::execute_stepwise(&snippet, &values, &base, &store.tmp_dir(), stdin)?
    } else {
        runner::execute(&snippet, &values, &base, &store.tmp_dir())?
    };
    if let (Some(db), Some(id)) = (&history, run_id) {
        let commands = u32::try_from(snippet.command_count()).unwrap_or(u32::MAX);
        let _ = db.finish_run(
            id,
            outcome.exit_code == 0,
            started.elapsed().as_millis(),
            commands,
        );
    }
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

/// Resolve variable values; on a terminal, prompt for each missing
/// variable (spec-snippet-format §7 stage 5) before giving up. The
/// final error lists **all** still-missing variables at once.
pub fn resolve_or_prompt(
    snippet: &Snippet,
    cli_values: &std::collections::BTreeMap<String, String>,
    stdin: &mut impl BufRead,
    interactive: bool,
) -> Result<std::collections::BTreeMap<String, String>> {
    match forge_core::vars::resolve(snippet, cli_values) {
        Ok(v) => Ok(v),
        Err(forge_core::ForgeError::MissingVariables(missing)) => {
            if !interactive {
                return Err(missing_error(&missing).into());
            }
            let mut values = cli_values.clone();
            for name in &missing {
                let description = snippet
                    .variables
                    .get(name)
                    .and_then(|spec| spec.resolve().description)
                    .unwrap_or_default();
                let question = if description.is_empty() {
                    format!("Value for {name}:")
                } else {
                    format!("Value for {name} ({description}):")
                };
                match crate::prompt::ask_line(stdin, &question) {
                    Some(answer) if !answer.is_empty() => {
                        values.insert(name.clone(), answer);
                    }
                    _ => {}
                }
            }
            match forge_core::vars::resolve(snippet, &values) {
                Ok(v) => Ok(v),
                Err(forge_core::ForgeError::MissingVariables(still)) => {
                    Err(missing_error(&still).into())
                }
                Err(e) => Err(e.into()),
            }
        }
        Err(e) => Err(e.into()),
    }
}

fn missing_error(missing: &[String]) -> CliError {
    CliError::failure(format!(
        "forgehub: missing values for variables: {}\n  what: these variables have no value from arguments, defaults or built-ins\n  why: a command with an empty variable would run the wrong thing\n  fix: {}",
        missing.join(", "),
        forge_core::vars::missing_hint(missing)
    ))
}

/// `fg list [--tag t] [--long]` - the local library. JSON shape is
/// spec-cli §3: `{ "workflows": [{ "name", "description", "tags",
/// "mode", "origin" }] }`.
pub fn cmd_list(store: &Store, tag: Option<&str>, long: bool, json: bool) -> i32 {
    let snippets: Vec<Snippet> = store
        .list()
        .into_iter()
        .filter(|s| tag.is_none_or(|t| s.tags.iter().any(|x| x == t)))
        .collect();
    if json {
        let arr: Vec<_> = snippets
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "tags": s.tags,
                    "mode": match s.effective_mode() { Mode::Script => "script", Mode::Steps => "steps" },
                    "origin": "local",
                })
            })
            .collect();
        println!("{}", serde_json::json!({"workflows": arr}));
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
            format!(" - {}", escape_display(&s.description))
        };
        println!("{} ({} commands){}", s.name, s.command_count(), desc);
        if !s.tags.is_empty() {
            println!("    tags: {}", s.tags.join(", "));
        }
        if long {
            println!(
                "    mode: {} · shell: {} · hash: {}",
                match s.effective_mode() {
                    Mode::Script => "script",
                    Mode::Steps => "steps",
                },
                s.effective_shell(),
                forge_core::hash::short_hash(s)
            );
        }
    }
    0
}

/// The ordered command strings a snippet would run (spec-cli §3,
/// `show`): steps mode flattens the resolved steps; script mode is
/// the body as a single entry.
#[must_use]
pub fn show_commands(snippet: &Snippet) -> Vec<String> {
    match snippet.effective_mode() {
        Mode::Script => snippet
            .script
            .clone()
            .unwrap_or_default()
            .lines()
            .map(ToString::to_string)
            .collect(),
        Mode::Steps => snippet
            .resolved_steps()
            .iter()
            .flat_map(|s| s.commands())
            .map(ToString::to_string)
            .collect(),
    }
}

/// `fg show <name>` - a summary card with numbered commands; the raw
/// YAML is one `fg export` away. JSON shape is spec-cli §3:
/// `{ "name", "description", "tags", "mode", "shell", "variables",
/// "commands" }`.
pub fn cmd_show(store: &Store, name: &str, json: bool) -> Result<i32> {
    let snippet = store.load(name)?;
    if json {
        let mut variables = serde_json::Map::new();
        for (name, spec) in &snippet.variables {
            let param = spec.resolve();
            let mut entry = serde_json::Map::new();
            if let Some(d) = &param.description {
                entry.insert("description".into(), serde_json::Value::String(d.clone()));
            }
            if let Some(d) = &param.default {
                entry.insert("default".into(), serde_json::Value::String(d.clone()));
            }
            entry.insert("required".into(), serde_json::Value::Bool(param.required));
            variables.insert(name.clone(), serde_json::Value::Object(entry));
        }
        println!(
            "{}",
            serde_json::json!({
                "name": snippet.name,
                "description": snippet.description,
                "tags": snippet.tags,
                "mode": match snippet.effective_mode() { Mode::Script => "script", Mode::Steps => "steps" },
                "shell": snippet.effective_shell(),
                "variables": variables,
                "commands": show_commands(&snippet),
            })
        );
        return Ok(0);
    }
    let description = if snippet.description.is_empty() {
        "(no description)".to_string()
    } else {
        escape_display(&snippet.description)
    };
    println!("{} - {}", snippet.name, description);
    println!(
        "  mode: {} · shell: {} · hash: {}",
        match snippet.effective_mode() {
            Mode::Script => "script",
            Mode::Steps => "steps",
        },
        snippet.effective_shell(),
        forge_core::hash::short_hash(&snippet)
    );
    if !snippet.tags.is_empty() {
        println!("  tags: {}", snippet.tags.join(", "));
    }
    if !snippet.variables.is_empty() {
        println!("  variables:");
        for (name, spec) in &snippet.variables {
            let param = spec.resolve();
            let mut line = format!("    {name}");
            if param.required {
                line.push_str(" (required)");
            }
            if let Some(d) = &param.default {
                use std::fmt::Write as _;
                let _ = write!(line, " [default: {}]", escape_display(d));
            }
            if let Some(d) = &param.description {
                if !d.is_empty() {
                    use std::fmt::Write as _;
                    let _ = write!(line, " - {}", escape_display(d));
                }
            }
            println!("{line}");
        }
    }
    println!("  commands:");
    match snippet.effective_mode() {
        Mode::Script => {
            for (i, line) in snippet
                .script
                .as_deref()
                .unwrap_or_default()
                .lines()
                .enumerate()
            {
                println!("    {}. {}", i + 1, escape_display(line));
            }
        }
        Mode::Steps => {
            for (i, cmd) in show_commands(&snippet).iter().enumerate() {
                println!("    {}. {}", i + 1, escape_display(cmd));
            }
        }
    }
    Ok(0)
}

/// `fg search <query>` - substring match over name, description, tags
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
        println!("{} - {}", s.name, escape_display(&s.description));
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

/// `fg delete <name>` - confirms unless `--yes`.
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

/// `fg edit <name>` - open in the editor; re-validate afterwards and
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
    fn escape_drops_isolates_and_other_invisibles() {
        // Step 19 extension: bidi isolates (U+2066–2069), the
        // Arabic letter mark, Mongolian vowel separator and
        // the soft hyphen are all dropped; CR becomes visible.
        let dirty = "a\u{2066}b\u{2069}c\u{61c}d\u{180e}e\u{ad}f\rg";
        let clean = escape_display(dirty);
        assert_eq!(clean, "abcdef\\rg");
        assert!(!clean.contains('\u{7f}'));
        assert_eq!(escape_display("x\u{7f}y"), "x\u{fffd}y");
    }

    #[test]
    fn escape_keeps_plain_text() {
        assert_eq!(
            escape_display("apt update && apt upgrade"),
            "apt update && apt upgrade"
        );
    }

    #[test]
    fn prompt_resolves_missing_variable() {
        let s = Snippet::from_yaml(
            "name: g\nvariables:\n  WHO: { description: 'who to greet', required: true }\nsteps:\n  - echo {{WHO}}\n",
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"world\n".to_vec());
        let values =
            resolve_or_prompt(&s, &std::collections::BTreeMap::new(), &mut input, true).unwrap();
        assert_eq!(values["WHO"], "world");
    }

    #[test]
    fn prompt_empty_answer_stays_missing() {
        let s = Snippet::from_yaml("name: g\nsteps:\n  - echo {{WHO}}\n").unwrap();
        let mut input = std::io::Cursor::new(b"\n".to_vec());
        let err = resolve_or_prompt(&s, &std::collections::BTreeMap::new(), &mut input, true)
            .unwrap_err();
        assert!(format!("{err}").contains("WHO"));
    }

    #[test]
    fn missing_without_tty_lists_all() {
        let s = Snippet::from_yaml("name: g\nsteps:\n  - echo {{A}} {{B}}\n").unwrap();
        let mut input = std::io::Cursor::new(Vec::new());
        let err = resolve_or_prompt(&s, &std::collections::BTreeMap::new(), &mut input, false)
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains('A') && msg.contains('B'), "{msg}");
        assert!(msg.contains("A=value B=value"), "{msg}");
    }
}
