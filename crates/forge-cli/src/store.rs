//! Local storage: `$FORGEHUB_HOME` (default `~/.forgehub`), the config
//! file, and the local snippet library (one YAML file per snippet).
//!
//! Snippet names are validated by `forge-core` before they ever become
//! file names, so a name can never escape the snippets directory.

use anyhow::{Context, Result};
use forge_core::Snippet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CLI configuration (`config.yaml` in the `ForgeHub` home).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Shell for `record` and script mode (`bash`, `sh`, `zsh`, `fish`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_shell: Option<String>,
    /// Editor for `fg edit` (falls back to `$EDITOR`, then `vi`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// Ask before running (the confirmation card). Default true.
    #[serde(default = "default_true")]
    pub confirm_before_run: bool,
    /// Logged-in username (Step 11+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Server base URL (Step 11+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Seconds of human overhead (recall + typing) credited per
    /// automated command by `fg stats` (Forgenesis default: 12).
    #[serde(default = "default_overhead")]
    pub human_overhead_seconds: f64,
    /// Minimum repetitions before `fg analyze` reports a sequence.
    #[serde(default = "default_analyze_min_count")]
    pub analyze_min_count: u32,
    /// Longest command sequence `fg analyze` looks for.
    #[serde(default = "default_analyze_max_len")]
    pub analyze_max_len: usize,
    /// Shell-history location override for `fg analyze`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_file: Option<String>,
    /// Commands `fg analyze` treats as noise (exact head word, full
    /// command, or a `prefix*` pattern).
    #[serde(default = "default_ignores")]
    pub ignore_commands: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_overhead() -> f64 {
    12.0
}

fn default_analyze_min_count() -> u32 {
    3
}

fn default_analyze_max_len() -> usize {
    8
}

/// The Forgenesis noise list: commands that are never workflows.
pub fn default_ignores() -> Vec<String> {
    [
        "ls",
        "ll",
        "la",
        "cd",
        "clear",
        "pwd",
        "exit",
        "history",
        "which",
        "man",
        "htop",
        "top",
        "fg",
        "bg",
        "jobs",
        "echo",
        "cat",
        "less",
        "more",
        "tree",
        "whoami",
        "date",
        "reset",
        "forgehub",
        "forgenesis",
        "sudo su",
        "vi",
        "vim",
        "nano",
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

impl Config {
    /// Is `cmd` noise for history analysis? Matches the head word, the
    /// whole command, or a `prefix*` pattern (Forgenesis semantics).
    #[must_use]
    pub fn is_ignored(&self, cmd: &str) -> bool {
        let head = cmd.split_whitespace().next().unwrap_or("");
        if head.is_empty() {
            return true;
        }
        self.ignore_commands.iter().any(|pat| {
            let pat = pat.trim();
            match pat.strip_suffix('*') {
                Some(prefix) => cmd.starts_with(prefix),
                None => head == pat || cmd == pat,
            }
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_shell: None,
            editor: None,
            confirm_before_run: true,
            username: None,
            server: None,
            human_overhead_seconds: default_overhead(),
            analyze_min_count: default_analyze_min_count(),
            analyze_max_len: default_analyze_max_len(),
            history_file: None,
            ignore_commands: default_ignores(),
        }
    }
}

/// The local store rooted at the `ForgeHub` home directory.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Open the store at `$FORGEHUB_HOME` or `~/.forgehub`.
    ///
    /// # Errors
    /// When neither `FORGEHUB_HOME` nor `HOME` is set.
    pub fn open() -> Result<Self> {
        let root = match std::env::var_os("FORGEHUB_HOME") {
            Some(h) if !h.is_empty() => PathBuf::from(h),
            _ => {
                let home = std::env::var_os("HOME").context(
                    "neither FORGEHUB_HOME nor HOME is set; cannot find the ForgeHub home",
                )?;
                PathBuf::from(home).join(".forgehub")
            }
        };
        Ok(Self { root })
    }

    /// The `ForgeHub` home directory.
    #[must_use]
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Directory holding local snippet files.
    #[must_use]
    pub fn snippets_dir(&self) -> PathBuf {
        self.root.join("snippets")
    }

    /// Directory for private temporary files (recordings, script runs).
    #[must_use]
    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// Directory holding timestamped backup archives.
    #[must_use]
    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// The run-history database (`history.db`).
    #[must_use]
    pub fn history_db_path(&self) -> PathBuf {
        self.root.join("history.db")
    }

    /// The config file path.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.yaml")
    }

    /// Create the directory layout. Idempotent; returns the paths that
    /// were newly created.
    ///
    /// # Errors
    /// On filesystem failures.
    pub fn ensure_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        for dir in [
            self.root.clone(),
            self.snippets_dir(),
            self.tmp_dir(),
            self.backups_dir(),
        ] {
            if !dir.exists() {
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("creating {}", dir.display()))?;
                created.push(dir);
            }
        }
        Ok(created)
    }

    /// Load the config; a missing file means defaults.
    ///
    /// # Errors
    /// When the file exists but cannot be read or parsed.
    pub fn load_config(&self) -> Result<Config> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    /// Save the config.
    ///
    /// # Errors
    /// On filesystem failures.
    pub fn save_config(&self, cfg: &Config) -> Result<()> {
        self.ensure_dirs()?;
        let text = serde_yaml::to_string(cfg).context("serialising config")?;
        std::fs::write(self.config_path(), text)
            .with_context(|| format!("writing {}", self.config_path().display()))?;
        Ok(())
    }

    fn snippet_path(&self, name: &str) -> Result<PathBuf> {
        forge_core::names::validate_snippet_name(name)?;
        Ok(self.snippets_dir().join(format!("{name}.yaml")))
    }

    /// Does a local snippet with this name exist?
    #[must_use]
    pub fn exists(&self, name: &str) -> bool {
        self.snippet_path(name).is_ok_and(|p| p.exists())
    }

    /// Load a local snippet by name.
    ///
    /// # Errors
    /// [`crate::exit::CliError`] kind `NotFound` (exit 3) when absent;
    /// a parse/IO error otherwise.
    pub fn load(&self, name: &str) -> Result<Snippet> {
        let path = self.snippet_path(name)?;
        if !path.exists() {
            return Err(crate::exit::CliError::not_found(format!(
                "no local snippet named `{name}`"
            ))
            .into());
        }
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let snippet =
            Snippet::from_bytes(&bytes).with_context(|| format!("parsing {}", path.display()))?;
        Ok(snippet)
    }

    /// Save a snippet (validating first), replacing any existing file.
    ///
    /// # Errors
    /// On validation or filesystem failures.
    pub fn save(&self, snippet: &Snippet) -> Result<PathBuf> {
        snippet.validate()?;
        self.ensure_dirs()?;
        let path = self.snippet_path(&snippet.name)?;
        let tmp = self
            .tmp_dir()
            .join(format!(".save-{}-{}.tmp", snippet.name, std::process::id()));
        std::fs::write(&tmp, snippet.to_yaml()?)
            .with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("moving {} into place", path.display()))?;
        Ok(path)
    }

    /// The cloud-run cache directory (Step 13): immutable
    /// version bodies for offline replay and changed-hash diffs.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Path of the trust database (Step 13).
    #[must_use]
    pub fn trust_db_path(&self) -> PathBuf {
        self.root.join("trust.db")
    }

    /// Path of the cloud provenance sidecar for a local snippet
    /// (Step 12): `<name>.meta.json` next to the snippet file.
    pub fn meta_path(&self, name: &str) -> Result<PathBuf> {
        forge_core::names::validate_snippet_name(name)?;
        Ok(self.snippets_dir().join(format!("{name}.meta.json")))
    }

    /// Read the provenance sidecar text, when one exists.
    #[must_use]
    pub fn read_meta(&self, name: &str) -> Option<String> {
        let path = self.meta_path(name).ok()?;
        std::fs::read_to_string(path).ok()
    }

    /// Write the provenance sidecar (atomically, like snippets).
    pub fn write_meta(&self, name: &str, json: &str) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.meta_path(name)?;
        let tmp = self
            .tmp_dir()
            .join(format!(".meta-{}-{}.tmp", name, std::process::id()));
        std::fs::write(&tmp, json).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("moving {} into place", path.display()))?;
        Ok(())
    }

    /// Remove the provenance sidecar, when one exists.
    pub fn remove_meta(&self, name: &str) {
        if let Ok(path) = self.meta_path(name) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// All local snippets, sorted by name. Files that fail to parse are
    /// reported on stderr and skipped (one bad file must not break the
    /// whole library listing).
    #[must_use]
    pub fn list(&self) -> Vec<Snippet> {
        let mut out = Vec::new();
        let dir = self.snippets_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            match std::fs::read(&path).map(|b| Snippet::from_bytes(&b)) {
                Ok(Ok(snippet)) => out.push(snippet),
                Ok(Err(e)) => {
                    eprintln!("forgehub: warning: skipping {}: {e}", path.display());
                }
                Err(e) => {
                    eprintln!("forgehub: warning: skipping {}: {e}", path.display());
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Names of all local snippets, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.list().iter().map(|s| s.name.clone()).collect()
    }

    /// Delete a local snippet file.
    ///
    /// # Errors
    /// `NotFound` when absent; IO errors otherwise.
    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.snippet_path(name)?;
        if !path.exists() {
            return Err(crate::exit::CliError::not_found(format!(
                "no local snippet named `{name}`"
            ))
            .into());
        }
        std::fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
        Ok(())
    }

    /// The file path for a snippet (for `edit`).
    ///
    /// # Errors
    /// When the name is invalid.
    pub fn path_for(&self, name: &str) -> Result<PathBuf> {
        self.snippet_path(name)
    }
}
