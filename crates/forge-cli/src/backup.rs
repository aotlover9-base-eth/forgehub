//! `fg backup` / `fg restore`: a portable `.tar` archive of the local
//! library (config + snippets + a hash manifest), written and read by a
//! minimal ustar implementation so the CLI needs no external `tar` and
//! no archive dependency.
//!
//! Restore verifies every file against the manifest's SHA-256 before
//! writing anything, and refuses archive entries with absolute or
//! parent-traversing names.

use crate::exit::CliError;
use crate::store::Store;
use anyhow::{Context, Result};
use forge_core::Snippet;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// One file in an archive.
#[derive(Debug, Clone)]
pub struct TarEntry {
    /// Archive-relative name (forward slashes, no `..`, not absolute).
    pub name: String,
    /// File contents.
    pub data: Vec<u8>,
}

fn octal(value: u64, width: usize) -> String {
    format!("{value:0width$o}", width = width - 1) + "\0"
}

fn write_header(out: &mut Vec<u8>, name: &str, size: usize) {
    let mut header = [0u8; 512];
    let name_bytes = name.as_bytes();
    header[..name_bytes.len()].copy_from_slice(name_bytes);
    let fields: [(usize, String); 6] = [
        (100, octal(0o644, 8)),        // mode
        (108, octal(0, 8)),            // uid
        (116, octal(0, 8)),            // gid
        (124, octal(size as u64, 12)), // size
        (136, octal(0, 12)),           // mtime (0: reproducible)
        (148, "        ".to_string()), // checksum placeholder (spaces)
    ];
    for (offset, text) in fields {
        header[offset..offset + text.len()].copy_from_slice(text.as_bytes());
    }
    header[156] = b'0'; // typeflag: regular file
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum: u32 = header.iter().map(|b| u32::from(*b)).sum();
    let checksum_text = format!("{checksum:06o}\0 ");
    header[148..148 + checksum_text.len()].copy_from_slice(checksum_text.as_bytes());
    out.extend_from_slice(&header);
}

/// Serialise entries to a ustar archive.
#[must_use]
pub fn write_tar(entries: &[TarEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        write_header(&mut out, &entry.name, entry.data.len());
        out.extend_from_slice(&entry.data);
        let pad = (512 - (entry.data.len() % 512)) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    out.extend(std::iter::repeat_n(0u8, 1024));
    out
}

fn parse_octal(bytes: &[u8]) -> Option<u64> {
    let text: String = bytes
        .iter()
        .take_while(|b| **b != 0 && **b != b' ')
        .map(|b| *b as char)
        .collect();
    u64::from_str_radix(text.trim(), 8).ok()
}

/// Parse a ustar archive.
///
/// # Errors
/// On truncated data, bad headers, or unsafe entry names.
pub fn read_tar(data: &[u8]) -> Result<Vec<TarEntry>> {
    let mut entries = Vec::new();
    let mut offset = 0usize;
    while offset + 512 <= data.len() {
        let header = &data[offset..offset + 512];
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let name_end = header[..100].iter().position(|b| *b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&header[..name_end]).into_owned();
        if name.is_empty() || name.starts_with('/') || name.split('/').any(|p| p == "..") {
            return Err(CliError::failure(format!(
                "forgehub: unsafe path in archive: `{name}`\n  what: archive entries must be relative and stay inside the ForgeHub home\n  fix: restore a backup produced by `fg backup`"
            ))
            .into());
        }
        let size =
            usize::try_from(parse_octal(&header[124..136]).context("parsing tar header size")?)
                .context("tar entry size does not fit in memory")?;
        let start = offset + 512;
        let end = start.checked_add(size).context("tar entry size overflow")?;
        if end > data.len() {
            anyhow::bail!("archive is truncated at entry `{name}`");
        }
        entries.push(TarEntry {
            name,
            data: data[start..end].to_vec(),
        });
        offset = start + size.div_ceil(512) * 512;
    }
    Ok(entries)
}

/// The backup manifest (JSON inside the archive).
#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    format: u32,
    created: String,
    files: BTreeMap<String, String>, // name -> sha256 hex
}

/// `fg backup [name] [--out file]` - write an archive of the config
/// plus all snippets (or one named snippet). The default location is
/// the store's `backups/` directory with a timestamped name
/// (spec-cli §4). Archives never contain credentials: the config
/// holds settings only, and credentials live outside the `ForgeHub`
/// home (Step 11) and are never read here.
/// Returns the archive path.
pub fn cmd_backup(store: &Store, name: Option<&str>, out: Option<&str>) -> Result<PathBuf> {
    let timestamp = forge_core::vars::builtins()
        .get("TIMESTAMP")
        .cloned()
        .unwrap_or_else(|| "unknown-time".to_string());
    let out_path = out.map_or_else(
        || {
            store
                .backups_dir()
                .join(format!("forgehub-backup-{timestamp}.tar"))
        },
        PathBuf::from,
    );
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    let mut entries: Vec<TarEntry> = Vec::new();
    let mut hashes = BTreeMap::new();
    let mut add = |name: String, data: Vec<u8>| {
        hashes.insert(name.clone(), forge_core::hash::sha256_hex(&data));
        entries.push(TarEntry { name, data });
    };
    let config_path = store.config_path();
    if config_path.exists() {
        add(
            "config.yaml".to_string(),
            std::fs::read(&config_path)
                .with_context(|| format!("reading {}", config_path.display()))?,
        );
    }
    let snippets = match name {
        Some(n) => vec![store.load(n)?],
        None => store.list(),
    };
    for snippet in snippets {
        let path = store.path_for(&snippet.name)?;
        add(
            format!("snippets/{}.yaml", snippet.name),
            std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
        );
    }
    let manifest = Manifest {
        format: 1,
        created: timestamp,
        files: hashes,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).context("serialising manifest")?;
    entries.insert(
        0,
        TarEntry {
            name: "manifest.json".to_string(),
            data: manifest_json.into_bytes(),
        },
    );
    let count = entries.len() - 1;
    std::fs::write(&out_path, write_tar(&entries))
        .with_context(|| format!("writing {}", out_path.display()))?;
    println!(
        "Wrote {} ({} files: config + snippets)",
        out_path.display(),
        count
    );
    Ok(out_path)
}

/// The newest backup archive in the store's `backups/` directory
/// (by modification time), for `fg restore` with no argument.
#[must_use]
pub fn newest_backup(store: &Store) -> Option<PathBuf> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(store.backups_dir()).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("tar") {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            candidates.push((modified, path));
        }
    }
    candidates.sort();
    candidates.pop().map(|(_, p)| p)
}

/// `fg restore` - verify an archive against its manifest, then write
/// the snippets back (and the config only with `with_config`).
/// Returns the number of snippets restored.
pub fn cmd_restore(store: &Store, archive: &PathBuf, with_config: bool) -> Result<usize> {
    let data = std::fs::read(archive).with_context(|| format!("reading {}", archive.display()))?;
    let entries = read_tar(&data)?;
    let manifest_entry = entries
        .iter()
        .find(|e| e.name == "manifest.json")
        .ok_or_else(|| {
            CliError::failure(
                "forgehub: this archive has no manifest.json\n  what: it was not produced by `fg backup` (or is corrupted)\n  fix: restore a ForgeHub backup file",
            )
        })?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_entry.data).context("parsing the backup manifest")?;
    if manifest.format != 1 {
        anyhow::bail!("unsupported backup format {}", manifest.format);
    }
    // Verify everything before writing anything.
    for entry in &entries {
        if entry.name == "manifest.json" {
            continue;
        }
        let expected = manifest.files.get(&entry.name).ok_or_else(|| {
            CliError::failure(format!(
                "forgehub: archive entry `{}` is not in the manifest; refusing to restore",
                entry.name
            ))
        })?;
        let actual = forge_core::hash::sha256_hex(&entry.data);
        if &actual != expected {
            return Err(CliError::failure(format!(
                "forgehub: hash mismatch for `{}`\n  what: the archive is corrupted or was modified\n  why: restore must never write unverified content\n  fix: restore from an intact backup",
                entry.name
            ))
            .into());
        }
    }
    store.ensure_dirs()?;
    let mut restored = 0usize;
    for entry in &entries {
        if let Some(rest) = entry.name.strip_prefix("snippets/") {
            let name = rest.strip_suffix(".yaml").unwrap_or(rest);
            let snippet = Snippet::from_bytes(&entry.data)
                .with_context(|| format!("parsing restored snippet `{name}`"))?;
            if snippet.name != name {
                anyhow::bail!(
                    "archive entry `{}` contains a snippet named `{}`",
                    entry.name,
                    snippet.name
                );
            }
            store.save(&snippet)?;
            restored += 1;
        } else if entry.name == "config.yaml" && with_config {
            std::fs::write(store.config_path(), &entry.data)
                .with_context(|| format!("writing {}", store.config_path().display()))?;
            println!("Restored config.yaml");
        }
    }
    println!("Restored {restored} snippet(s) from {}", archive.display());
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tar_roundtrip() {
        let entries = vec![
            TarEntry {
                name: "a.txt".into(),
                data: b"hello".to_vec(),
            },
            TarEntry {
                name: "dir/b.yaml".into(),
                data: vec![7u8; 1500],
            },
        ];
        let parsed = read_tar(&write_tar(&entries)).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].data, b"hello");
        assert_eq!(parsed[1].data.len(), 1500);
    }

    #[test]
    fn tar_rejects_traversal() {
        let entries = vec![TarEntry {
            name: "../evil".into(),
            data: b"x".to_vec(),
        }];
        assert!(read_tar(&write_tar(&entries)).is_err());
    }

    #[test]
    fn tar_truncated_errors() {
        let mut bytes = write_tar(&[TarEntry {
            name: "a".into(),
            data: b"hello world".to_vec(),
        }]);
        bytes.truncate(520);
        assert!(read_tar(&bytes).is_err());
    }
}
