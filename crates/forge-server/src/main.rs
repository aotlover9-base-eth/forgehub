#![forbid(unsafe_code)]
//! `ForgeHub` server binary.
//!
//! Step 1 provides only the skeleton entry point so the workspace builds.
//! The axum server, configuration, database and migrations arrive in Step 6.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V" | "version") => {
            println!("forge-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h" | "help") => {
            println!(
                "forge-server {} — {} API and web server\n\nUsage: forge-server [--version] [--help]\n\nThe HTTP server is implemented in Step 6; see roadmap.md.",
                env!("CARGO_PKG_VERSION"),
                forge_core::PRODUCT
            );
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "forge-server: argument `{other}` is not implemented yet (skeleton from Step 1)\n  what: the server skeleton only supports --version and --help\n  why: the HTTP server arrives in Step 6\n  fix: run `forge-server --help`"
            );
            ExitCode::from(2)
        }
        None => {
            println!(
                "forge-server {} skeleton — HTTP server arrives in Step 6",
                env!("CARGO_PKG_VERSION")
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
