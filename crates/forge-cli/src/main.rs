#![forbid(unsafe_code)]
//! `ForgeHub` CLI binary (`forgehub`, aliased to `fg`).
//!
//! Step 4 implements the local command set (`init`, `record`, `run`,
//! `list`, `show`, `search`, `tag`, `rename`, `delete`, `edit`) plus the
//! `fg <name>` shorthand. Cloud commands arrive in Steps 11–13; the name
//! resolution order in spec-cli §5 already reserves their place.

mod cmds;
mod exit;
mod prompt;
mod record;
mod runner;
mod store;

use clap::{Parser, Subcommand};
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
        name: String,
        /// Print the plan and execute nothing.
        #[arg(long)]
        dry_run: bool,
        /// KEY=value variable assignments.
        #[arg(value_name = "KEY=value")]
        assignments: Vec<String>,
    },
    /// List local snippets.
    List,
    /// Show a snippet's stored YAML.
    Show {
        /// Snippet name.
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
        name: String,
        /// `add` or `remove`.
        action: String,
        /// Tags to add or remove.
        tags: Vec<String>,
    },
    /// Rename a local snippet.
    Rename {
        /// Current name.
        old: String,
        /// New name.
        new: String,
    },
    /// Delete a local snippet.
    Delete {
        /// Snippet name.
        name: String,
    },
    /// Edit a snippet in $EDITOR; invalid edits are rolled back.
    Edit {
        /// Snippet name.
        name: String,
    },
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
        Command::List => Ok(cmds::cmd_list(&store, cli.json)),
        Command::Show { name } => cmds::cmd_show(&store, &name, cli.json),
        Command::Search { query } => Ok(cmds::cmd_search(&store, &query, cli.json)),
        Command::Tag { name, action, tags } => {
            cmds::cmd_tag(&store, &name, &action, &tags, cli.json)
        }
        Command::Rename { old, new } => cmds::cmd_rename(&store, &old, &new, cli.json),
        Command::Delete { name } => cmds::cmd_delete(&store, stdin, &name, cli.yes, cli.json),
        Command::Edit { name } => cmds::cmd_edit(&store, &name),
    }
}

fn main() -> ExitCode {
    // Shorthand: `fg <name> [KEY=value…]` means `fg run <name> …` when
    // the first word is not a known command or flag (spec-cli §5 step 1:
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
