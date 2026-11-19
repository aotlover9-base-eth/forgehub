#![forbid(unsafe_code)]
//! `ForgeHub` CLI binary (`forgehub`, aliased to `fg`).
//!
//! Local commands (`init`, `record`, `run`, `list`, `show`, `search`,
//! `tag`, `rename`, `delete`, `edit`, `parameterize`, `clone`,
//! `import`, `export`, `analyze`, `stats`, `backup`, `restore`,
//! `doctor`, `alias`, `completions`) work fully offline. Cloud
//! commands arrive in Steps 11–13; the name resolution order in
//! spec-cli §6 already reserves their place.

mod analyze;
mod backup;
mod cmds;
mod exit;
mod history;
mod parity;
mod prompt;
mod record;
mod runner;
mod store;

use clap::{Parser, Subcommand};
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
use std::io::BufRead;
use std::process::ExitCode;

/// Subcommand names the shorthand must not swallow. Mirrors the
/// reserved-word list in `forge-core` for the commands that exist or
/// are planned; an unknown first word is treated as a snippet name.
const KNOWN_COMMANDS: &[&str] = &[
    "run",
    "init",
    "record",
    "list",
    "show",
    "search",
    "tag",
    "rename",
    "delete",
    "edit",
    "login",
    "logout",
    "whoami",
    "push",
    "pull",
    "fork",
    "status",
    "diff",
    "clone",
    "import",
    "export",
    "analyze",
    "parameterize",
    "stats",
    "backup",
    "restore",
    "doctor",
    "alias",
    "completions",
    "ui",
    "cache",
    "update",
    "trust",
    "remote",
    "help",
];

/// Dynamic completion: local snippet names for name-taking commands.
/// Called by the shell through the `clap_complete` env protocol
/// (`COMPLETE=<shell> forgehub`); never runs during normal use.
fn complete_snippet_name(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Ok(store) = store::Store::open() else {
        return Vec::new();
    };
    let prefix = current.to_string_lossy();
    store
        .names()
        .into_iter()
        .filter(|n| n.starts_with(prefix.as_ref()))
        .map(CompletionCandidate::new)
        .collect()
}

#[derive(Parser)]
#[command(
    name = "forgehub",
    version,
    about = "ForgeHub — a cloud home for shell command snippets",
    long_about = "Save shell command snippets once, run them anywhere with `fg <name>`. \
                  Local commands work fully offline; cloud commands (login, push, pull, fork) sync with your ForgeHub server."
)]
struct Cli {
    /// Print machine-readable JSON instead of human output.
    #[arg(long, global = true)]
    json: bool,

    /// Skip confirmation prompts (never bypasses hash-change or
    /// third-party trust prompts; see spec-cli §7).
    #[arg(long, short = 'y', global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create the `ForgeHub` home and config.
    Init,
    /// Record a shell session as a snippet.
    Record {
        /// Name for the snippet (prompted when omitted).
        name: Option<String>,
        /// Short description.
        #[arg(long)]
        description: Option<String>,
        /// Comma-separated tags.
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Overwrite an existing snippet with the same name.
        #[arg(long)]
        force: bool,
    },
    /// Run a snippet by name.
    Run {
        /// Snippet name (local in this step; owner/name and cloud in later steps).
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
        /// Print the plan and execute nothing (alias: --dry).
        #[arg(long = "dry-run", alias = "dry")]
        dry_run: bool,
        /// KEY=value variable assignments.
        #[arg(value_name = "KEY=value")]
        assignments: Vec<String>,
    },
    /// List local snippets.
    List {
        /// Only show snippets with this tag.
        #[arg(long)]
        tag: Option<String>,
        /// Show mode, shell and hash for each snippet.
        #[arg(long)]
        long: bool,
    },
    /// Show a snippet (card + numbered commands).
    Show {
        /// Snippet name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
    },
    /// Search local snippets (name, description, tags, commands).
    Search {
        /// Search text.
        query: String,
    },
    /// Add or remove tags: fg tag <name> add|remove <tag>…
    Tag {
        /// Snippet name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
        /// `add` or `remove`.
        action: String,
        /// Tags to add or remove.
        tags: Vec<String>,
    },
    /// Rename a local snippet.
    Rename {
        /// Current name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        old: String,
        /// New name.
        new: String,
    },
    /// Delete a local snippet.
    Delete {
        /// Snippet name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
    },
    /// Edit a snippet in $EDITOR; invalid edits are rolled back.
    Edit {
        /// Snippet name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
    },
    /// Turn repeated literals into variables.
    Parameterize {
        /// Snippet name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
        /// A literal to parameterize (repeat with --as; pairs apply directly).
        #[arg(long = "literal", value_name = "TEXT")]
        literals: Vec<String>,
        /// Variable name for the preceding --literal.
        #[arg(long = "as", value_name = "NAME")]
        as_names: Vec<String>,
    },
    /// Duplicate a local snippet under a new name.
    Clone {
        /// Existing name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
        /// New name.
        new: String,
    },
    /// Import snippet file(s) (`ForgeHub` or Forgenesis format).
    Import {
        /// A snippet file, or a directory of snippet files.
        /// Optional with --from-forgenesis.
        file: Option<std::path::PathBuf>,
        /// Import from the Forgenesis workflows directory instead
        /// ($`FORGENESIS_HOME/workflows` or ~/.forgenesis/workflows).
        #[arg(long)]
        from_forgenesis: bool,
        /// Save under a different name (single file only).
        #[arg(long)]
        name: Option<String>,
        /// Overwrite existing snippets with the same name.
        #[arg(long)]
        force: bool,
    },
    /// Export a snippet as YAML (to stdout or a file).
    Export {
        /// Snippet name.
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: String,
        /// Write to this file instead of stdout.
        #[arg(long, short = 'o')]
        out: Option<std::path::PathBuf>,
    },
    /// Mine shell history for repeated command sequences.
    Analyze {
        /// Minimum repetitions before a sequence is reported.
        #[arg(long)]
        min_count: Option<u32>,
        /// Maximum number of candidates to show.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Longest command sequence to look for.
        #[arg(long)]
        max_len: Option<usize>,
        /// Print candidates only; create nothing.
        #[arg(long = "dry", alias = "dry-run")]
        dry: bool,
        /// Read this history file instead of auto-detecting one.
        #[arg(long)]
        file: Option<std::path::PathBuf>,
    },
    /// Show run statistics from history.db.
    Stats {
        /// Maximum number of snippets in the ranking.
        #[arg(long, default_value_t = 10)]
        limit: u32,
        /// Explain the saved-time formula.
        #[arg(long)]
        explain: bool,
    },
    /// Write a backup archive (default: backups/forgehub-backup-<timestamp>.tar).
    Backup {
        /// Back up only this snippet (plus the config).
        #[arg(add = ArgValueCompleter::new(complete_snippet_name))]
        name: Option<String>,
        /// Output file instead of the default location.
        #[arg(long, short = 'o')]
        out: Option<String>,
    },
    /// Restore snippets from a backup archive.
    Restore {
        /// Backup file (default: the newest in the backups directory).
        file: Option<std::path::PathBuf>,
        /// Also restore config.yaml from the archive.
        #[arg(long)]
        config: bool,
        /// Overwrite without asking (same as -y).
        #[arg(long)]
        force: bool,
    },
    /// Check the environment and library health.
    Doctor {
        /// Only run checks for this topic (e.g. alias, rust, python).
        topic: Option<String>,
    },
    /// Manage the `fg` shell alias (install|uninstall|show).
    Alias {
        /// install, uninstall or show.
        action: String,
        /// Shell to manage (bash, zsh or fish; default: $SHELL or bash).
        #[arg(long)]
        shell: Option<String>,
    },
    /// Print shell completions (dynamic: local snippet names included).
    Completions {
        /// Shell: bash, zsh, fish, powershell or elvish.
        shell: clap_complete::Shell,
    },
}

/// Dispatch for `import` (path resolution lives here so `run_cli`
/// stays a flat match).
fn dispatch_import(
    store: &store::Store,
    file: Option<std::path::PathBuf>,
    from_forgenesis: bool,
    name: Option<&str>,
    force: bool,
    json: bool,
) -> anyhow::Result<i32> {
    let target = if from_forgenesis {
        parity::forgenesis_workflows_dir()
    } else {
        file.ok_or_else(|| {
            exit::CliError::failure(
                "forgehub: import needs a file or directory\n  what: no path was given\n  fix: fg import <path>, or fg import --from-forgenesis",
            )
        })?
    };
    parity::cmd_import(store, &target, name, force, json)
}

/// Dispatch for `restore` (default archive + confirmation gate).
fn dispatch_restore(
    store: &store::Store,
    stdin: &mut impl BufRead,
    file: Option<std::path::PathBuf>,
    config: bool,
    force: bool,
    yes: bool,
) -> anyhow::Result<i32> {
    let archive = match file {
        Some(f) => f,
        None => backup::newest_backup(store).ok_or_else(|| {
            exit::CliError::failure(
                "forgehub: no backups found\n  what: the backups directory has no archives\n  fix: run `fg backup` first, or pass an archive path",
            )
        })?,
    };
    if !yes && !force {
        if !prompt::stdin_is_tty() {
            return Err(exit::CliError::refused(
                "forgehub: not confirmed\n  what: restore overwrites local snippets and needs confirmation\n  fix: pass --yes (-y) or --force to restore without prompting",
            )
            .into());
        }
        if !prompt::ask_yes_no(
            stdin,
            "Restore will overwrite local snippets with the archive's versions. Continue?",
        ) {
            println!("Restore cancelled.");
            return Ok(2);
        }
    }
    backup::cmd_restore(store, &archive, config).map(|_| 0)
}

/// The `completions` registration snippet for a shell (dynamic
/// completion through the COMPLETE env protocol).
fn completions_snippet(shell: clap_complete::Shell) -> anyhow::Result<&'static str> {
    Ok(match shell {
        clap_complete::Shell::Bash => "source <(COMPLETE=bash forgehub)",
        clap_complete::Shell::Zsh => "source <(COMPLETE=zsh forgehub)",
        clap_complete::Shell::Fish => "COMPLETE=fish forgehub | source",
        clap_complete::Shell::PowerShell => {
            "$env:COMPLETE = \"powershell\"; forgehub | Out-String | Invoke-Expression; Remove-Item Env:\\COMPLETE"
        }
        clap_complete::Shell::Elvish => "eval (E:COMPLETE=elvish forgehub | slurp)",
        _ => {
            return Err(exit::CliError::failure(
                "forgehub: unsupported shell for completions",
            )
            .into())
        }
    })
}

/// Dispatch for `parameterize` (validates the --literal/--as pairs).
fn dispatch_parameterize(
    store: &store::Store,
    stdin: &mut impl BufRead,
    name: &str,
    literals: Vec<String>,
    as_names: Vec<String>,
    yes: bool,
    json: bool,
) -> anyhow::Result<i32> {
    if literals.len() != as_names.len() {
        return Err(exit::CliError::failure(format!(
            "forgehub: --literal and --as must come in pairs\n  what: got {} --literal and {} --as\n  fix: fg parameterize {name} --literal TEXT --as NAME",
            literals.len(),
            as_names.len()
        ))
        .into());
    }
    let pairs: Vec<(String, String)> = literals.into_iter().zip(as_names).collect();
    parity::cmd_parameterize(store, stdin, name, yes, json, &pairs)
}

fn run_cli(cli: Cli, stdin: &mut impl BufRead) -> anyhow::Result<i32> {
    let store = store::Store::open()?;
    match cli.command {
        Command::Init => cmds::cmd_init(&store, cli.json),
        Command::Record {
            name,
            description,
            tags,
            force,
        } => cmds::cmd_record(&store, stdin, name, description.as_deref(), &tags, force),
        Command::Run {
            name,
            dry_run,
            assignments,
        } => cmds::cmd_run(
            &store,
            stdin,
            &cmds::RunArgs {
                name,
                assignments,
                yes: cli.yes,
                dry_run,
                json: cli.json,
            },
        ),
        Command::List { tag, long } => Ok(cmds::cmd_list(&store, tag.as_deref(), long, cli.json)),
        Command::Show { name } => cmds::cmd_show(&store, &name, cli.json),
        Command::Search { query } => Ok(cmds::cmd_search(&store, &query, cli.json)),
        Command::Tag { name, action, tags } => {
            cmds::cmd_tag(&store, &name, &action, &tags, cli.json)
        }
        Command::Rename { old, new } => cmds::cmd_rename(&store, &old, &new, cli.json),
        Command::Delete { name } => cmds::cmd_delete(&store, stdin, &name, cli.yes, cli.json),
        Command::Edit { name } => cmds::cmd_edit(&store, &name),
        Command::Parameterize {
            name,
            literals,
            as_names,
        } => dispatch_parameterize(&store, stdin, &name, literals, as_names, cli.yes, cli.json),
        Command::Clone { name, new } => parity::cmd_clone(&store, &name, &new, cli.json),
        Command::Import {
            file,
            from_forgenesis,
            name,
            force,
        } => dispatch_import(
            &store,
            file,
            from_forgenesis,
            name.as_deref(),
            force,
            cli.json,
        ),
        Command::Export { name, out } => parity::cmd_export(&store, &name, out.as_ref()),
        Command::Analyze {
            min_count,
            limit,
            max_len,
            dry,
            file,
        } => parity::cmd_analyze(
            &store,
            stdin,
            &parity::AnalyzeArgs {
                file,
                min_count,
                max_len,
                limit,
                dry,
            },
            cli.yes,
            cli.json,
        ),
        Command::Stats { limit, explain } => parity::cmd_stats(&store, limit, explain, cli.json),
        Command::Backup { name, out } => {
            backup::cmd_backup(&store, name.as_deref(), out.as_deref()).map(|_| 0)
        }
        Command::Restore {
            file,
            config,
            force,
        } => dispatch_restore(&store, stdin, file, config, force, cli.yes),
        Command::Doctor { topic } => parity::cmd_doctor(&store, topic.as_deref(), cli.json),
        Command::Alias { action, shell } => {
            parity::cmd_alias(stdin, &action, shell.as_deref(), cli.yes, cli.json)
        }
        Command::Completions { shell } => {
            println!("{}", completions_snippet(shell)?);
            Ok(0)
        }
    }
}

fn main() -> ExitCode {
    // Dynamic shell completion (clap_complete env protocol). When
    // COMPLETE is set by a shell integration this handles the request
    // and exits; otherwise it returns immediately.
    clap_complete::CompleteEnv::with_factory(<Cli as clap::CommandFactory>::command).complete();

    // Shorthand: `fg <name> [KEY=value…]` means `fg run <name> …` when
    // the first word is not a known command or flag (spec-cli §6 step 1:
    // subcommands always win over snippet names).
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut argv: Vec<String> = vec![std::env::args().next().unwrap_or_else(|| "forgehub".into())];
    if let Some(first) = raw.first() {
        if !first.starts_with('-') && !KNOWN_COMMANDS.contains(&first.as_str()) {
            argv.push("run".to_string());
        }
    }
    argv.extend(raw);

    let cli = match Cli::try_parse_from(&argv) {
        Ok(c) => c,
        Err(e) => {
            // clap handles --help/--version and usage errors itself.
            e.exit();
        }
    };
    let stdin = std::io::stdin();
    let mut lock = stdin.lock();
    match run_cli(cli, &mut lock) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(exit::code_of(&err))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_string_is_not_empty() {
        assert_ne!(env!("CARGO_PKG_VERSION"), "");
    }

    #[test]
    fn known_commands_cover_step4_set() {
        for cmd in [
            "run", "init", "record", "list", "show", "search", "tag", "rename", "delete", "edit",
        ] {
            assert!(super::KNOWN_COMMANDS.contains(&cmd), "{cmd}");
        }
    }
}
