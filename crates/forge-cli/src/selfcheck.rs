//! `forgehub self-check` (Peak C10): verify the installed
//! binary against the published release checksums.
//!
//! What it can and cannot prove, stated plainly:
//! - It always reports this binary's own SHA-256 and
//!   version - useful on its own when comparing installs.
//! - The releases publish `SHA256SUMS` over the release
//!   ARCHIVES (the .tar.gz files), not over the extracted
//!   binary, so a binary cannot be matched against them
//!   directly. With `--archive <file>` the archive's hash
//!   is compared against the published entry - a real
//!   end-to-end verification of a downloaded release.
//! - The sums come from `--sums <file>` when given
//!   (offline verification), else from the release page
//!   over HTTPS. When the network is unavailable - or the
//!   version has no published release - the command
//!   explains the manual steps instead of failing, unless
//!   an archive verification was explicitly requested.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

/// The release download base for this project.
const RELEASE_BASE: &str = "https://github.com/aotlover9-base-eth/forgehub/releases/download";

/// Why fetching the published sums did not work.
#[derive(Debug)]
enum FetchFail {
    /// No network route (timeout, refused, DNS).
    Offline(String),
    /// The server answered 404 (unpublished version, or the
    /// private repository needs the `gh` flow).
    NotFound,
    /// Any other HTTP status or transport problem.
    Other(String),
}

/// The release archive name for a platform, or None when
/// no archive is published for it (Linux only, per ADR
/// 0008: `x86_64` and `aarch64` tarballs).
fn asset_name_for(os: &str, arch: &str, version: &str) -> Option<String> {
    let triple = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        _ => return None,
    };
    Some(format!("forgehub-{version}-{triple}.tar.gz"))
}

/// This machine's release archive name, when one exists.
fn asset_name(version: &str) -> Option<String> {
    asset_name_for(std::env::consts::OS, std::env::consts::ARCH, version)
}

/// Where the published sums live for a version. The
/// `FORGEHUB_SUMS_URL` environment variable overrides the
/// URL wholesale (mirrors and tests).
fn sums_url(version: &str) -> String {
    if let Ok(url) = std::env::var("FORGEHUB_SUMS_URL") {
        if !url.trim().is_empty() {
            return url;
        }
    }
    format!("{RELEASE_BASE}/v{version}/SHA256SUMS")
}

/// Parse a SHA256SUMS file into name -> lowercase hex hash.
/// Tolerates the `*` binary marker, blank lines and
/// comments; malformed lines are skipped.
fn parse_sums(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let name = name.trim_start_matches('*');
        // Entries may carry a path; only the file name keys.
        let name = name.rsplit('/').next().unwrap_or(name);
        out.insert(name.to_string(), hash.to_lowercase());
    }
    out
}

/// SHA-256 of a file's bytes, lowercase hex.
fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("cannot read {} to hash it", path.display()))?;
    Ok(forge_core::hash::sha256_hex(&bytes))
}

/// Fetch the published sums over HTTPS with the CLI's
/// usual timeouts (5 s connect, 10 s total).
fn fetch_sums(url: &str) -> Result<String, FetchFail> {
    let http = reqwest::blocking::Client::builder()
        .user_agent(concat!("forgehub/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| FetchFail::Other(format!("cannot build the HTTP client: {e}")))?;
    let resp = http.get(url).send().map_err(|e| {
        if e.is_timeout() {
            FetchFail::Offline(format!("request to {url} timed out"))
        } else if e.is_connect() {
            FetchFail::Offline(format!(
                "cannot connect to {url} (connection refused or DNS failure)"
            ))
        } else {
            FetchFail::Other(format!("cannot reach {url}: {e}"))
        }
    })?;
    let status = resp.status();
    if status.as_u16() == 404 {
        return Err(FetchFail::NotFound);
    }
    if !status.is_success() {
        return Err(FetchFail::Other(format!("{url} answered HTTP {status}")));
    }
    resp.text()
        .map_err(|e| FetchFail::Other(format!("cannot read the response from {url}: {e}")))
}

/// The manual verification steps, printed whenever the
/// automatic path cannot complete.
fn manual_steps(version: &str) -> String {
    format!(
        "To verify a downloaded release by hand:\n  gh release download v{version} --repo aotlover9-base-eth/forgehub\n  sha256sum -c SHA256SUMS\n(the release repository is private, so plain HTTPS downloads need the authenticated gh flow above; sha256sum -c must answer OK for the archive you installed from.)"
    )
}

/// `forgehub self-check [--archive <file>] [--sums <file>]`.
/// Returns the process exit code: 0 when the state is
/// reported or an archive verifies, 1 on any mismatch or
/// when a requested verification cannot be completed.
pub fn cmd_self_check(archive: Option<&Path>, sums_path: Option<&Path>, json: bool) -> Result<i32> {
    let version = env!("CARGO_PKG_VERSION");
    let exe = std::env::current_exe().context("cannot locate the running binary")?;
    let binary_sha256 = sha256_file(&exe)?;
    let asset = asset_name(version);

    // Resolve the published sums: local file first, then
    // the network. `sums_note` explains an unavailable set.
    let mut sums: Option<BTreeMap<String, String>> = None;
    let mut sums_note: Option<String> = None;
    if let Some(path) = sums_path {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read the sums file {}", path.display()))?;
        sums = Some(parse_sums(&text));
    } else {
        match fetch_sums(&sums_url(version)) {
            Ok(text) => sums = Some(parse_sums(&text)),
            Err(FetchFail::NotFound) => {
                sums_note = Some(format!(
                    "no published SHA256SUMS found for v{version} (the version may be unpublished, or the release needs the authenticated gh flow)"
                ));
            }
            Err(FetchFail::Offline(why)) => {
                sums_note = Some(format!("offline: {why}"));
            }
            Err(FetchFail::Other(why)) => {
                sums_note = Some(why);
            }
        }
    }

    let published = sums
        .as_ref()
        .and_then(|s| asset.as_ref().and_then(|a| s.get(a)).cloned());

    // Archive verification, when requested.
    let mut archive_report: Option<(String, String, bool)> = None; // (name, hash, matches)
    let mut exit = 0;
    if let Some(archive_path) = archive {
        let hash = sha256_file(archive_path)?;
        let name = archive_path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let expected = sums.as_ref().and_then(|s| s.get(&name));
        if let Some(expected) = expected {
            let matches = *expected == hash;
            if !matches {
                exit = 1;
            }
            archive_report = Some((name, hash, matches));
        } else {
            // No sums at all, or no entry for this file:
            // the requested verification cannot complete.
            exit = 1;
            archive_report = Some((name, hash, false));
        }
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "version": version,
                "binary": exe.display().to_string(),
                "binary_sha256": binary_sha256,
                "release_asset": asset,
                "published_archive_sha256": published,
                "sums_note": sums_note,
                "archive": archive_report.as_ref().map(|(name, hash, matches)| {
                    serde_json::json!({ "file": name, "sha256": hash, "matches_published": matches })
                }),
            })
        );
        return Ok(exit);
    }

    println!("forgehub self-check");
    println!("  version: {version}");
    println!("  binary: {}", exe.display());
    println!("  binary sha256: {binary_sha256}");
    if let Some(asset_name) = &asset {
        println!("  release asset for this platform: {asset_name}");
    }
    match (&published, &sums_note) {
        (Some(hash), _) => println!("  published archive sha256: {hash}"),
        (None, Some(note)) => println!("  published checksums unavailable: {note}"),
        (None, None) => {
            println!("  published checksums: v{version} has no entry for this platform's archive");
        }
    }
    if let Some((name, hash, matches)) = &archive_report {
        println!("  archive {name} sha256: {hash}");
        if *matches {
            println!("  result: the archive matches the published checksum");
        } else if sums.is_some() {
            println!(
                "  result: MISMATCH or no published entry for {name} - do not install from this archive"
            );
        } else {
            println!("  result: could not verify - the published checksums were unavailable");
        }
    }
    if sums.is_none() || (archive.is_none() && published.is_none()) {
        println!("{}", manual_steps(version));
    }
    Ok(exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_cover_the_published_platforms_only() {
        assert_eq!(
            asset_name_for("linux", "x86_64", "0.1.6").as_deref(),
            Some("forgehub-0.1.6-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert_eq!(
            asset_name_for("linux", "aarch64", "0.1.6").as_deref(),
            Some("forgehub-0.1.6-aarch64-unknown-linux-gnu.tar.gz")
        );
        assert_eq!(asset_name_for("windows", "x86_64", "0.1.6"), None);
        assert_eq!(asset_name_for("macos", "aarch64", "0.1.6"), None);
    }

    #[test]
    fn parse_sums_tolerates_markers_paths_and_junk() {
        let text = "\
# release sums
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  forgehub-0.1.6-x86_64-unknown-linux-gnu.tar.gz
E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855 *dist/forgehub-0.1.6-aarch64-unknown-linux-gnu.tar.gz
not a sums line
1234  too-short.tar.gz
";
        let sums = parse_sums(text);
        assert_eq!(sums.len(), 2);
        assert_eq!(
            sums["forgehub-0.1.6-x86_64-unknown-linux-gnu.tar.gz"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Uppercase hashes normalise; path prefixes key by name.
        assert_eq!(
            sums["forgehub-0.1.6-aarch64-unknown-linux-gnu.tar.gz"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_file_matches_the_known_vector() {
        let dir = std::env::temp_dir().join(format!("fg-selfcheck-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("abc.bin");
        std::fs::write(&file, b"abc").expect("write");
        assert_eq!(
            sha256_file(&file).expect("hash"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
