//! Server configuration from environment variables, with validation.
//!
//! Every variable is documented in `.env.example` and
//! `docs/running.md`. The startup summary printed by
//! [`Config::summary`] never contains the secret key's value.

use anyhow::{bail, Context, Result};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    /// Development: pretty logs, relaxed secret requirements.
    Dev,
    /// Production: JSON logs, `FORGEHUB_SECRET_KEY` required (≥ 32 bytes).
    Prod,
}

impl Env {
    /// Lowercase name for display.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Env::Dev => "dev",
            Env::Prod => "prod",
        }
    }
}

/// Validated server configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Deployment environment.
    pub env: Env,
    /// Address to bind (`FORGEHUB_BIND`).
    pub bind: SocketAddr,
    /// Public base URL (`FORGEHUB_BASE_URL`), no trailing slash.
    pub base_url: String,
    /// SQLite URL or path (`FORGEHUB_DATABASE_URL`).
    pub database_url: String,
    /// Cookie/token signing secret (`FORGEHUB_SECRET_KEY`). Never printed.
    pub secret_key: Option<String>,
    /// Whether public signup is open (`FORGEHUB_ALLOW_SIGNUP`).
    pub allow_signup: bool,
    /// Directory with the built web app (`FORGEHUB_WEB_DIR`).
    pub web_dir: PathBuf,
    /// Max request body for API routes (`FORGEHUB_BODY_LIMIT_KIB`).
    pub body_limit_bytes: usize,
    /// Per-key requests per minute (`FORGEHUB_RATE_LIMIT_PER_MINUTE`).
    pub rate_limit_per_minute: u32,
    /// API request timeout (`FORGEHUB_REQUEST_TIMEOUT_SECS`).
    pub request_timeout_secs: u64,
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => bail!("{key} must be true or false, got `{other}`"),
    }
}

fn parse_or<T: std::str::FromStr>(key: &str, value: &str, default: T) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    if value.trim().is_empty() {
        return Ok(default);
    }
    value
        .trim()
        .parse::<T>()
        .map_err(|e| anyhow::anyhow!("{key} is invalid: {e}"))
}

impl Config {
    /// Load and validate configuration from the process environment.
    ///
    /// # Errors
    /// On invalid values, an unparseable bind address, or — in prod —
    /// a missing/short secret key.
    pub fn from_env() -> Result<Self> {
        Self::from_vars(&|k| env_var(k))
    }

    /// Load from an arbitrary variable source (tests).
    ///
    /// # Errors
    /// As [`Config::from_env`].
    pub fn from_vars(get: &dyn Fn(&str) -> Option<String>) -> Result<Self> {
        let env = match get("FORGEHUB_ENV").as_deref() {
            None | Some("dev") => Env::Dev,
            Some("prod") => Env::Prod,
            Some(other) => bail!("FORGEHUB_ENV must be dev or prod, got `{other}`"),
        };
        let bind: SocketAddr = parse_or(
            "FORGEHUB_BIND",
            get("FORGEHUB_BIND").as_deref().unwrap_or_default(),
            SocketAddr::from(([127, 0, 0, 1], 8080)),
        )
        .context("parsing FORGEHUB_BIND")?;
        let base_url = get("FORGEHUB_BASE_URL")
            .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
            .trim_end_matches('/')
            .to_string();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            bail!("FORGEHUB_BASE_URL must start with http:// or https://, got `{base_url}`");
        }
        let database_url =
            get("FORGEHUB_DATABASE_URL").unwrap_or_else(|| "sqlite://forgehub.db".to_string());
        let secret_key = get("FORGEHUB_SECRET_KEY");
        if env == Env::Prod {
            let key = secret_key.as_deref().unwrap_or("");
            if key.len() < 32 {
                bail!(
                    "FORGEHUB_SECRET_KEY must be set to at least 32 bytes in production (got {})",
                    key.len()
                );
            }
        }
        // In dev a weak/missing key is not fatal: main() logs a
        // warning via `secret_is_weak`.
        let allow_signup = match get("FORGEHUB_ALLOW_SIGNUP") {
            Some(v) => parse_bool("FORGEHUB_ALLOW_SIGNUP", &v)?,
            None => true,
        };
        let web_dir =
            get("FORGEHUB_WEB_DIR").map_or_else(|| PathBuf::from("web/dist"), PathBuf::from);
        let body_limit_kib: usize = parse_or(
            "FORGEHUB_BODY_LIMIT_KIB",
            get("FORGEHUB_BODY_LIMIT_KIB")
                .as_deref()
                .unwrap_or_default(),
            256,
        )?;
        if body_limit_kib == 0 || body_limit_kib > 1024 * 64 {
            bail!("FORGEHUB_BODY_LIMIT_KIB must be between 1 and 65536");
        }
        let rate_limit_per_minute: u32 = parse_or(
            "FORGEHUB_RATE_LIMIT_PER_MINUTE",
            get("FORGEHUB_RATE_LIMIT_PER_MINUTE")
                .as_deref()
                .unwrap_or_default(),
            600,
        )?;
        let request_timeout_secs: u64 = parse_or(
            "FORGEHUB_REQUEST_TIMEOUT_SECS",
            get("FORGEHUB_REQUEST_TIMEOUT_SECS")
                .as_deref()
                .unwrap_or_default(),
            30,
        )?;
        if request_timeout_secs == 0 {
            bail!("FORGEHUB_REQUEST_TIMEOUT_SECS must be at least 1");
        }
        Ok(Self {
            env,
            bind,
            base_url,
            database_url,
            secret_key,
            allow_signup,
            web_dir,
            body_limit_bytes: body_limit_kib * 1024,
            rate_limit_per_minute,
            request_timeout_secs,
        })
    }

    /// A secret-free, multi-line summary for the startup log. The
    /// secret key appears only as set/unset plus its length.
    #[must_use]
    pub fn summary(&self) -> String {
        let secret = match &self.secret_key {
            Some(k) => format!("set ({} bytes, value hidden)", k.len()),
            None => "not set".to_string(),
        };
        format!(
            "env={} bind={} base_url={} database={} secret_key={} allow_signup={} web_dir={} body_limit={}KiB rate_limit={}/min timeout={}s",
            self.env.as_str(),
            self.bind,
            self.base_url,
            self.database_url,
            secret,
            self.allow_signup,
            self.web_dir.display(),
            self.body_limit_bytes / 1024,
            self.rate_limit_per_minute,
            self.request_timeout_secs,
        )
    }

    /// True when the secret key is missing or short in dev (the
    /// caller logs a warning; prod refuses to start instead).
    #[must_use]
    pub fn secret_is_weak(&self) -> bool {
        self.secret_key.as_deref().is_none_or(|k| k.len() < 32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    #[test]
    fn defaults_are_valid() {
        let cfg = Config::from_vars(&vars(&[])).unwrap();
        assert_eq!(cfg.env, Env::Dev);
        assert_eq!(cfg.bind.to_string(), "127.0.0.1:8080");
        assert!(cfg.allow_signup);
        assert_eq!(cfg.body_limit_bytes, 256 * 1024);
    }

    #[test]
    fn prod_requires_a_long_secret() {
        let err = Config::from_vars(&vars(&[("FORGEHUB_ENV", "prod")])).unwrap_err();
        assert!(format!("{err}").contains("FORGEHUB_SECRET_KEY"), "{err}");
        let ok = Config::from_vars(&vars(&[
            ("FORGEHUB_ENV", "prod"),
            ("FORGEHUB_SECRET_KEY", "x".repeat(32).as_str()),
        ]))
        .unwrap();
        assert_eq!(ok.env, Env::Prod);
    }

    #[test]
    fn summary_never_contains_the_secret() {
        let secret = "s".repeat(39);
        let cfg = Config::from_vars(&vars(&[("FORGEHUB_SECRET_KEY", secret.as_str())])).unwrap();
        let summary = cfg.summary();
        assert!(!summary.contains(&secret), "{summary}");
        assert!(
            summary.contains(&format!("set ({} bytes", secret.len())),
            "{summary}"
        );
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(Config::from_vars(&vars(&[("FORGEHUB_ENV", "staging")])).is_err());
        assert!(Config::from_vars(&vars(&[("FORGEHUB_BIND", "not-an-addr")])).is_err());
        assert!(Config::from_vars(&vars(&[("FORGEHUB_ALLOW_SIGNUP", "maybe")])).is_err());
        assert!(Config::from_vars(&vars(&[("FORGEHUB_BASE_URL", "ftp://x")])).is_err());
        assert!(Config::from_vars(&vars(&[("FORGEHUB_BODY_LIMIT_KIB", "0")])).is_err());
    }
}
