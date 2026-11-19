//! `forgehub update --check` (Step 18): ask the registry's
//! `/version` (or GitHub releases) whether a newer `ForgeHub`
//! exists. It NEVER downloads or replaces anything — it
//! prints where to get the upgrade.

use anyhow::Result;

use crate::cloud::ApiClient;
use crate::exit::CliError;
use crate::store::Store;

/// The outcome of an update check.
#[derive(Debug, PartialEq)]
pub enum UpdateStatus {
    /// The running version is the newest known.
    Latest,
    /// A newer version exists.
    Available {
        latest: String,
        /// Where to get it.
        source: String,
    },
}

/// Compare dotted versions numerically (`0.10.0` > `0.9.9`).
/// Non-numeric suffixes (e.g. `-rc1`) are ignored per segment.
pub fn version_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .map(|seg| {
                seg.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    let (l, c) = (parts(latest), parts(current));
    for i in 0..l.len().max(c.len()) {
        let (a, b) = (
            l.get(i).copied().unwrap_or(0),
            c.get(i).copied().unwrap_or(0),
        );
        if a != b {
            return a > b;
        }
    }
    false
}

/// Query a registry's `/api/v1/version` (answers
/// `{"version": "x.y.z"}`).
pub fn check_version_endpoint(registry: &str) -> Result<Option<String>> {
    let client = ApiClient::new(registry)?;
    let (_, body, _) = client.get("/version", None)?;
    Ok(body["version"].as_str().map(str::to_string))
}

/// Query GitHub's latest-release API for the `ForgeHub` repo.
/// `api_base` is a parameter so tests can point it at a fake
/// server.
pub fn check_github(api_base: &str) -> Result<Option<String>> {
    let http = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("forgehub/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let body: serde_json::Value = http
        .get(format!(
            "{}/repos/aotlover9-base-eth/forgehub/releases/latest",
            api_base.trim_end_matches('/')
        ))
        .header("accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json()?;
    Ok(body["tag_name"].as_str().map(str::to_string))
}

/// The check itself: registry first (it is the deployment's
/// own source of truth), GitHub as a fallback.
pub fn run_check(store: &Store, registry_flag: Option<&str>) -> Result<UpdateStatus> {
    let current = env!("CARGO_PKG_VERSION");
    // 1. The configured registry, if any.
    if let Ok(Some(registry)) = crate::cloud::resolve_registry(store, registry_flag) {
        if let Ok(Some(latest)) = check_version_endpoint(&registry) {
            return Ok(if version_newer(&latest, current) {
                UpdateStatus::Available {
                    latest,
                    source: format!("{registry} (registry version)"),
                }
            } else {
                UpdateStatus::Latest
            });
        }
    }
    // 2. GitHub releases (FORGEHUB_GITHUB_API overrides the
    // API base, for mirrors and tests).
    let api_base = std::env::var("FORGEHUB_GITHUB_API")
        .unwrap_or_else(|_| "https://api.github.com".to_string());
    match check_github(&api_base) {
        Ok(Some(latest)) => Ok(if version_newer(&latest, current) {
            UpdateStatus::Available {
                latest: latest.clone(),
                source: format!(
                    "https://github.com/aotlover9-base-eth/forgehub/releases/tag/{latest}"
                ),
            }
        } else {
            UpdateStatus::Latest
        }),
        _ => Err(CliError::failure(
            "forgehub: could not check for updates\n  what: neither the registry nor GitHub answered\n  fix: check your connection, or look at https://github.com/aotlover9-base-eth/forgehub/releases",
        )
        .into()),
    }
}

/// `fg update --check`.
pub fn cmd_update_check(store: &Store, registry_flag: Option<&str>, json: bool) -> Result<i32> {
    let current = env!("CARGO_PKG_VERSION");
    match run_check(store, registry_flag)? {
        UpdateStatus::Latest => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "current": current, "latest": current, "update_available": false })
                );
            } else {
                println!("forgehub {current} — you are on the latest known version.");
            }
            Ok(0)
        }
        UpdateStatus::Available { latest, source } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "current": current, "latest": latest, "update_available": true, "source": source })
                );
            } else {
                println!("forgehub {latest} is available (you run {current}).");
                println!("  get it: {source}");
                println!("  this command never updates anything itself.");
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_is_numeric() {
        assert!(version_newer("0.10.0", "0.9.9"));
        assert!(version_newer("v1.0.0", "0.9.9"));
        assert!(!version_newer("0.1.0", "0.1.0"));
        assert!(!version_newer("0.1.0", "0.2.0"));
        assert!(version_newer("0.1.1", "0.1.0"));
    }
}
