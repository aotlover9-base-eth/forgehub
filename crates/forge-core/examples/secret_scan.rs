//! Reads text on stdin and prints one `line N: kind` line per secret
//! finding; exits 1 when any finding exists, 0 when clean.
//!
//! Used by `scripts/scan-staged.sh` to scan `git diff --cached`, and
//! handy for piping any file through the same scanner the product uses:
//!
//! ```sh
//! git diff --cached | cargo run -q -p forge-core --example secret_scan
//! ```

use std::io::Read;

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("secret_scan: failed to read stdin");
        std::process::exit(2);
    }
    let findings = forge_core::scan::scan(&input);
    for f in &findings {
        println!("line {}: {}", f.line, f.kind.as_str());
    }
    if findings.is_empty() {
        std::process::exit(0);
    }
    std::process::exit(1);
}
