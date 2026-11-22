//! `fg config get|set|list` (Peak B9; plan section 5, spec-cli).
//!
//! The user-facing face of `config.yaml`. Five keys are exposed:
//! `registry_url`, `color`, `shell`, `default_visibility` and
//! `cache_size_mb`. Writes are validated before they are saved,
//! reads show the effective value and where it came from (the
//! config file or the built-in default). Secrets never live in
//! this file: tokens stay in the credential store (see
//! `cloud.rs`); `config.yaml` holds preferences only, and this
//! command refuses keys it does not know rather than inventing
//! new storage.

use crate::cloud;
use crate::exit::CliError;
use crate::output::{self, ColorMode};
use crate::store::{Config, Store};
use anyhow::Result;

/// The largest cache cap `config set` accepts (10 GiB, in MiB).
const MAX_CACHE_MB: u64 = 10_240;

/// Which `Config` slot a user-facing key maps to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    RegistryUrl,
    Color,
    Shell,
    DefaultVisibility,
    CacheSizeMb,
}

/// One user-facing config key: canonical name, accepted
/// aliases, the slot it maps to, and the built-in default the
/// rest of the CLI falls back to when it is unset (`None` when
/// there is no default, only "unset").
struct KeySpec {
    key: &'static str,
    aliases: &'static [&'static str],
    field: Field,
    builtin: Option<&'static str>,
    summary: &'static str,
}

const KEYS: &[KeySpec] = &[
    KeySpec {
        key: "registry_url",
        aliases: &["server", "registry"],
        field: Field::RegistryUrl,
        builtin: None,
        summary: "ForgeHub server base URL (config key: server)",
    },
    KeySpec {
        key: "color",
        aliases: &["colour"],
        field: Field::Color,
        builtin: Some("auto"),
        summary: "terminal colour: auto, always or never",
    },
    KeySpec {
        key: "shell",
        aliases: &[],
        field: Field::Shell,
        builtin: None,
        summary: "shell for record and script mode (config key: default_shell)",
    },
    KeySpec {
        key: "default_visibility",
        aliases: &["visibility"],
        field: Field::DefaultVisibility,
        builtin: Some("private"),
        summary: "visibility for snippets a push creates (config key: default_visibility)",
    },
    KeySpec {
        key: "cache_size_mb",
        aliases: &["cache_size"],
        field: Field::CacheSizeMb,
        builtin: Some("100"),
        summary: "run-cache size cap in MiB (config key: cache_max_mb)",
    },
];

/// Resolve a user-typed key (canonical name or alias).
fn spec_for(key: &str) -> Option<&'static KeySpec> {
    KEYS.iter()
        .find(|s| s.key == key || s.aliases.contains(&key))
}

/// The key names `config get`/`set` accept, for error messages.
fn valid_keys() -> String {
    KEYS.iter().map(|s| s.key).collect::<Vec<_>>().join(", ")
}

fn unknown_key(key: &str) -> CliError {
    CliError::failure(format!(
        "forgehub: unknown config key `{key}`\n  what: `fg config` only manages: {}\n  fix: fg config list shows every key; other settings live in config.yaml (see fg doctor)",
        valid_keys()
    ))
}

/// The raw value currently stored for a key, if any.
fn stored(cfg: &Config, spec: &KeySpec) -> Option<String> {
    match spec.field {
        Field::RegistryUrl => cfg.server.clone(),
        Field::Color => cfg.color.clone(),
        Field::Shell => cfg.default_shell.clone(),
        Field::DefaultVisibility => cfg.default_visibility.clone(),
        Field::CacheSizeMb => Some(cfg.cache_max_mb.to_string()),
    }
}

/// The effective value: the stored one, else the built-in
/// default, else `None` (the key is simply unset).
fn effective(cfg: &Config, spec: &KeySpec) -> Option<String> {
    stored(cfg, spec).or_else(|| spec.builtin.map(str::to_string))
}

/// Validate `value` for `spec` and store its normalised form.
/// Returns the normalised value that was stored.
fn apply(cfg: &mut Config, spec: &KeySpec, value: &str) -> Result<String, CliError> {
    let bad = |why: &str, fix: &str| {
        CliError::failure(format!(
            "forgehub: invalid value for {}\n  what: {why}\n  fix: {fix}",
            spec.key
        ))
    };
    match spec.field {
        Field::RegistryUrl => {
            let url = cloud::normalize_registry(value).map_err(|e| {
                // normalize_registry's message is a full
                // what/why/fix block for login; re-frame its
                // first line as this key's "what".
                let first = e
                    .to_string()
                    .lines()
                    .next()
                    .unwrap_or("invalid URL")
                    .trim()
                    .to_string();
                bad(
                    &first,
                    "pass a base URL like https://forgehub.example or http://127.0.0.1:8080",
                )
            })?;
            cfg.server = Some(url.clone());
            Ok(url)
        }
        Field::Color => {
            let mode = ColorMode::from_config(&value.to_lowercase()).ok_or_else(|| {
                bad(
                    "color must be auto, always or never",
                    "fg config set color auto",
                )
            })?;
            cfg.color = Some(mode.as_config().to_string());
            Ok(mode.as_config().to_string())
        }
        Field::Shell => {
            let known = ["bash", "sh", "zsh", "fish"].contains(&value);
            let path = std::path::Path::new(value);
            if known || (path.is_absolute() && path.is_file()) {
                cfg.default_shell = Some(value.to_string());
                Ok(value.to_string())
            } else {
                Err(bad(
                    "shell must be bash, sh, zsh or fish, or the absolute path of an installed shell",
                    "fg config set shell fish",
                ))
            }
        }
        Field::DefaultVisibility => {
            if ["private", "unlisted", "public"].contains(&value) {
                cfg.default_visibility = Some(value.to_string());
                Ok(value.to_string())
            } else {
                Err(bad(
                    "visibility must be private, unlisted or public",
                    "fg config set default_visibility private",
                ))
            }
        }
        Field::CacheSizeMb => {
            let n: u64 = value.parse().map_err(|_| {
                bad(
                    "cache size must be a whole number of MiB",
                    "fg config set cache_size_mb 100",
                )
            })?;
            if n == 0 || n > MAX_CACHE_MB {
                return Err(bad(
                    &format!("cache size must be between 1 and {MAX_CACHE_MB} MiB"),
                    "fg config set cache_size_mb 100",
                ));
            }
            cfg.cache_max_mb = n;
            Ok(n.to_string())
        }
    }
}

/// One key rendered for `config get` / `config list`.
struct Row {
    key: &'static str,
    value: Option<String>,
    source: &'static str, // "config", "default" or "unset"
    summary: &'static str,
}

fn row(cfg: &Config, spec: &KeySpec) -> Row {
    let stored = stored(cfg, spec);
    let source = if stored.is_some() && spec.field != Field::CacheSizeMb {
        "config"
    } else if spec.field == Field::CacheSizeMb {
        // cache_max_mb always has a value in the struct; it
        // counts as "config" only when the file actually
        // carries the key, which the struct cannot tell apart
        // from the default - report it as the effective value
        // either way.
        "effective"
    } else if spec.builtin.is_some() {
        "default"
    } else {
        "unset"
    };
    Row {
        key: spec.key,
        value: effective(cfg, spec),
        source,
        summary: spec.summary,
    }
}

fn row_json(r: &Row) -> serde_json::Value {
    serde_json::json!({
        "key": r.key,
        "value": r.value,
        "source": r.source,
        "summary": r.summary,
    })
}

fn print_row(r: &Row) {
    let shown = r.value.as_deref().unwrap_or("(not set)");
    println!("{:<20} {} ({})", r.key, shown, r.source);
}

/// `fg config list`.
pub fn cmd_config_list(store: &Store, json: bool) -> Result<i32> {
    let cfg = store.load_config()?;
    let rows: Vec<Row> = KEYS.iter().map(|s| row(&cfg, s)).collect();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": store.config_path().display().to_string(),
                "config": rows.iter().map(row_json).collect::<Vec<_>>(),
            })
        );
        return Ok(0);
    }
    let style = output::style();
    println!("{}", style.heading("Config"));
    println!(
        "  {}",
        style.dim(&store.config_path().display().to_string())
    );
    for r in &rows {
        print_row(r);
    }
    println!();
    for r in &rows {
        println!("  {} - {}", r.key, r.summary);
    }
    Ok(0)
}

/// `fg config get <key>`.
pub fn cmd_config_get(store: &Store, key: &str, json: bool) -> Result<i32> {
    let spec = spec_for(key).ok_or_else(|| unknown_key(key))?;
    let cfg = store.load_config()?;
    let r = row(&cfg, spec);
    if json {
        println!("{}", row_json(&r));
    } else {
        print_row(&r);
    }
    Ok(0)
}

/// `fg config set <key> <value>`: validate, save, report.
pub fn cmd_config_set(store: &Store, key: &str, value: &str, json: bool) -> Result<i32> {
    let spec = spec_for(key).ok_or_else(|| unknown_key(key))?;
    let mut cfg = store.load_config()?;
    let normalised = apply(&mut cfg, spec, value)?;
    store.save_config(&cfg)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "key": spec.key,
                "value": normalised,
                "path": store.config_path().display().to_string(),
            })
        );
    } else {
        println!("{} = {}", spec.key, normalised);
        println!(
            "  saved to {}",
            output::style().dim(&store.config_path().display().to_string())
        );
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(key: &str) -> &'static KeySpec {
        spec_for(key).unwrap()
    }

    #[test]
    fn aliases_resolve_to_canonical_keys() {
        assert_eq!(spec_for("colour").unwrap().key, "color");
        assert_eq!(spec_for("server").unwrap().key, "registry_url");
        assert_eq!(spec_for("cache_size").unwrap().key, "cache_size_mb");
        assert!(spec_for("token").is_none());
        assert!(spec_for("password").is_none());
    }

    #[test]
    fn color_values_are_validated_and_normalised() {
        let mut cfg = Config::default();
        assert_eq!(apply(&mut cfg, spec("color"), "NEVER").unwrap(), "never");
        assert_eq!(cfg.color.as_deref(), Some("never"));
        assert!(apply(&mut cfg, spec("color"), "sometimes").is_err());
        // A refused value leaves the stored one untouched.
        assert_eq!(cfg.color.as_deref(), Some("never"));
    }

    #[test]
    fn registry_url_is_normalised_and_validated() {
        let mut cfg = Config::default();
        let stored = apply(&mut cfg, spec("registry_url"), "https://Example.com/").unwrap();
        assert_eq!(cfg.server.as_deref(), Some(stored.as_str()));
        assert!(apply(&mut cfg, spec("registry_url"), "not a url").is_err());
        assert!(apply(&mut cfg, spec("registry_url"), "ftp://x.example").is_err());
    }

    #[test]
    fn shell_accepts_known_names_and_real_paths_only() {
        let mut cfg = Config::default();
        assert_eq!(apply(&mut cfg, spec("shell"), "fish").unwrap(), "fish");
        assert!(apply(&mut cfg, spec("shell"), "/bin/sh").is_ok());
        assert!(apply(&mut cfg, spec("shell"), "powershell").is_err());
        assert!(apply(&mut cfg, spec("shell"), "/no/such/shell").is_err());
    }

    #[test]
    fn visibility_and_cache_ranges_are_validated() {
        let mut cfg = Config::default();
        assert_eq!(
            apply(&mut cfg, spec("default_visibility"), "unlisted").unwrap(),
            "unlisted"
        );
        assert!(apply(&mut cfg, spec("default_visibility"), "friends").is_err());
        assert_eq!(
            apply(&mut cfg, spec("cache_size_mb"), "250").unwrap(),
            "250"
        );
        assert_eq!(cfg.cache_max_mb, 250);
        assert!(apply(&mut cfg, spec("cache_size_mb"), "0").is_err());
        assert!(apply(&mut cfg, spec("cache_size_mb"), "99999999").is_err());
        assert!(apply(&mut cfg, spec("cache_size_mb"), "1.5").is_err());
    }

    #[test]
    fn effective_values_fall_back_to_builtins() {
        let cfg = Config::default();
        assert_eq!(effective(&cfg, spec("color")).as_deref(), Some("auto"));
        assert_eq!(
            effective(&cfg, spec("default_visibility")).as_deref(),
            Some("private")
        );
        assert_eq!(
            effective(&cfg, spec("cache_size_mb")).as_deref(),
            Some("100")
        );
        assert_eq!(effective(&cfg, spec("registry_url")), None);
        assert_eq!(effective(&cfg, spec("shell")), None);
    }

    #[test]
    fn set_then_get_round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::from_root(dir.path().to_path_buf());
        assert_eq!(cmd_config_set(&store, "color", "always", false).unwrap(), 0);
        assert_eq!(cmd_config_get(&store, "color", false).unwrap(), 0);
        let cfg = store.load_config().unwrap();
        assert_eq!(cfg.color.as_deref(), Some("always"));
        // Unknown keys are refused, never written.
        assert!(cmd_config_set(&store, "api_token", "x", false).is_err());
    }
}
