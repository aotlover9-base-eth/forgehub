//! Build metadata (Step 21): embed the git commit and build
//! date so `forgehub --version` can report exactly what a
//! binary was built from. Falls back gracefully outside a
//! git checkout (e.g. a crates.io source tarball).

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=FORGEHUB_BUILD_SHA={sha}");
    // Rebuild when HEAD moves.
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    // Build date: SOURCE_DATE_EPOCH when set (reproducible
    // builds), else today (UTC).
    let date = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|ts| {
            Command::new("date")
                .args(["-u", "-d", &format!("@{ts}"), "+%Y-%m-%d"])
                .output()
                .ok()
        })
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("date")
                .args(["-u", "+%Y-%m-%d"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=FORGEHUB_BUILD_DATE={date}");
}
