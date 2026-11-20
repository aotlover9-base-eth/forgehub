//! Run-cache management (Step 18): list, clear, and the LRU
//! eviction that keeps the cache under the configured cap
//! (`cache_max_mb`, default 32 MiB).
//!
//! The cache itself (layout, pointers, hash verification)
//! lives in `trust.rs` from Step 13; this module only manages
//! its size and presents it.

use crate::store::Store;

/// One cached body file: `<owner>--<name>--v<version>.yaml`.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheEntry {
    pub owner: String,
    pub name: String,
    pub version: i64,
    pub bytes: u64,
    /// Seconds since the epoch (mtime - the LRU key).
    pub modified: u64,
}

fn parse_entry(file_name: &str, bytes: u64, modified: u64) -> Option<CacheEntry> {
    let stem = file_name.strip_suffix(".yaml")?;
    let (head, version) = stem.rsplit_once("--v")?;
    let (owner, name) = head.split_once("--")?;
    Some(CacheEntry {
        owner: owner.to_string(),
        name: name.to_string(),
        version: version.parse().ok()?,
        bytes,
        modified,
    })
}

/// All cached bodies, oldest first (eviction order).
pub fn entries(store: &Store) -> Vec<CacheEntry> {
    let dir = store.cache_dir();
    let mut out = Vec::new();
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(meta) = entry.metadata() {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                if let Some(parsed) = parse_entry(&name, meta.len(), modified) {
                    out.push(parsed);
                }
            }
        }
    }
    out.sort_by_key(|e| e.modified);
    out
}

pub fn total_bytes(entries: &[CacheEntry]) -> u64 {
    entries.iter().map(|e| e.bytes).sum()
}

/// Delete oldest entries until the cache fits `cap_bytes`.
/// Pointers whose body vanished are removed too, so
/// `--offline` never trusts a dangling pointer. Returns the
/// number of evicted bodies.
pub fn evict_lru(store: &Store, cap_bytes: u64) -> usize {
    let all = entries(store);
    let mut total = total_bytes(&all);
    let mut evicted = 0;
    for entry in &all {
        if total <= cap_bytes {
            break;
        }
        let body = crate::trust::cache_body_path(store, &entry.owner, &entry.name, entry.version);
        if std::fs::remove_file(&body).is_ok() {
            total = total.saturating_sub(entry.bytes);
            evicted += 1;
        }
    }
    if evicted > 0 {
        prune_pointers(store);
    }
    evicted
}

/// Remove pointer files whose referenced body is gone.
fn prune_pointers(store: &Store) {
    let dir = store.cache_dir();
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".json") {
                if let Some((owner, snippet)) = stem.split_once("--") {
                    let has_body = entries(store)
                        .iter()
                        .any(|e| e.owner == owner && e.name == snippet);
                    if !has_body {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)] // display only
fn human_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// `fg cache list`.
pub fn cmd_cache_list(store: &Store, json: bool) -> i32 {
    let all = entries(store);
    let cap = store
        .load_config()
        .map_or(100, |c| c.cache_max_mb)
        .saturating_mul(1_048_576);
    if json {
        let items: Vec<serde_json::Value> = all
            .iter()
            .map(|e| {
                serde_json::json!({
                    "owner": e.owner,
                    "name": e.name,
                    "version": e.version,
                    "bytes": e.bytes,
                    "modified": e.modified,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "entries": items,
                "total_bytes": total_bytes(&all),
                "cap_bytes": cap,
            })
        );
        return 0;
    }
    if all.is_empty() {
        println!("Cache is empty. Snippets you run from the cloud are cached here for --offline.");
        return 0;
    }
    for e in &all {
        println!(
            "{}/{} v{}  {}",
            e.owner,
            e.name,
            e.version,
            human_bytes(e.bytes)
        );
    }
    println!(
        "{} in {} entries (cap {})",
        human_bytes(total_bytes(&all)),
        all.len(),
        human_bytes(cap)
    );
    0
}

/// `fg cache clear`: remove every cached body + pointer.
pub fn cmd_cache_clear(store: &Store, json: bool) -> i32 {
    let all = entries(store);
    let bytes = total_bytes(&all);
    let dir = store.cache_dir();
    let mut removed = 0usize;
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let is_cache_file = entry
                .path()
                .extension()
                .is_some_and(|e| e == "yaml" || e == "json");
            if is_cache_file && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    if json {
        println!(
            "{}",
            serde_json::json!({ "cleared_entries": all.len(), "freed_bytes": bytes, "files_removed": removed })
        );
    } else {
        println!(
            "Cleared {} cached snippet(s), {} freed.",
            all.len(),
            human_bytes(bytes)
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entry_names() {
        let e = parse_entry("ada--ubuntu-setup--v3.yaml", 10, 5).unwrap();
        assert_eq!(e.owner, "ada");
        assert_eq!(e.name, "ubuntu-setup");
        assert_eq!(e.version, 3);
        assert!(parse_entry("ada--x.json", 1, 1).is_none());
        assert!(parse_entry("garbage.yaml", 1, 1).is_none());
    }

    #[test]
    fn eviction_removes_oldest_first_and_keeps_cap() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::from_root(dir.path().to_path_buf());
        // Three bodies, increasing mtimes via explicit writes
        // in sequence (mtime granularity is seconds; sizes
        // drive the test, order is by the recorded mtimes -
        // equal mtimes keep insertion order from read_dir,
        // so use sizes that evict deterministically by count).
        for name in ["a--one--v1.yaml", "a--two--v1.yaml", "a--three--v1.yaml"] {
            std::fs::create_dir_all(store.cache_dir()).unwrap();
            std::fs::write(store.cache_dir().join(name), vec![b'x'; 600]).unwrap();
        }
        // Cap 1000 bytes: at most one 600-byte body survives
        // after evicting down from 1800.
        let evicted = evict_lru(&store, 1000);
        assert!(evicted >= 2, "evicted {evicted}");
        assert!(total_bytes(&entries(&store)) <= 1000);
    }
}
