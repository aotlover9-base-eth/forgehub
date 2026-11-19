#![forbid(unsafe_code)]
//! `ForgeHub` CLI binary (`forgehub`, aliased to `fg`).
//!
//! Step 1 provides only the skeleton entry point so the workspace builds.
//! Local commands (`init`, `record`, `run`, management) arrive in Step 4,
//! local parity in Step 5, and cloud commands in Steps 11-13.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V" | "version") => {
            println!("{} {}", forge_core::BINARY_NAME, env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h" | "help") => {
            println!(
                "{} {} — a cloud home for shell command snippets\n\nUsage: {} [--version] [--help]\n\nCommands are implemented from Step 4 onward; see roadmap.md.",
                forge_core::PRODUCT,
                env!("CARGO_PKG_VERSION"),
                forge_core::BINARY_NAME
            );
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "forgehub: command `{other}` is not implemented yet (skeleton from Step 1)\n  what: the CLI skeleton only supports --version and --help\n  why: local commands arrive in Step 4\n  fix: run `forgehub --help`"
            );
            ExitCode::from(2)
        }
        None => {
            println!(
                "{} {} — run `{} --help`",
                forge_core::PRODUCT,
                env!("CARGO_PKG_VERSION"),
                forge_core::BINARY_NAME
            );
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_string_is_not_empty() {
        assert_ne!(env!("CARGO_PKG_VERSION"), "");
    }
}
