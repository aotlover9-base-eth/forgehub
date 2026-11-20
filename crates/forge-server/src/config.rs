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
    /// Session idle expiry (`FORGEHUB_SESSION_IDLE_SECS`, default 7 days).
    pub session_idle_secs: u64,
    /// Session absolute expiry (`FORGEHUB_SESSION_ABSOLUTE_SECS`, default 30 days).
    pub session_absolute_secs: u64,
    /// Base delay for login-throttle backoff (`FORGEHUB_LOGIN_BACKOFF_BASE_MS`, default 1000).
    pub login_backoff_base_ms: u64,
    /// Minimum seconds between device-code polls (`FORGEHUB_DEVICE_POLL_INTERVAL_SECS`, default 5).
    pub device_poll_interval_secs: u64,
    /// Per-user snippet cap (`FORGEHUB_MAX_SNIPPETS_PER_USER`, default 500).
    pub max_snippets_per_user: i64,
    /// Per-snippet version cap (`FORGEHUB_MAX_VERSIONS_PER_SNIPPET`, default 200).
    pub max_versions_per_snippet: i64,
    /// SMTP submission URL for outgoing mail (`FORGEHUB_SMTP_URL`,
    /// e.g. `smtps://user:pass@smtp.example.com:465`). Never printed:
    /// it usually carries a password.
    pub smtp_url: Option<String>,
    /// From address for outgoing mail (`FORGEHUB_MAIL_FROM`).
    pub mail_from: String,
}

/// How outgoing mail will be handled, for the startup summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailMode {
    /// Real SMTP delivery (`FORGEHUB_SMTP_URL` is set).
    Smtp,
    /// Dev only: reset links are written to the server log
    /// instead of being mailed (the console mailer).
    Console,
    /// No way to send mail: reset requests are accepted but
    /// nothing is delivered (prod without SMTP configured).
    Disabled,
}

impl MailMode {
    /// Lowercase name for the startup summary.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            MailMode::Smtp => "smtp",
            MailMode::Console => "console (dev: links go to the server log)",
            MailMode::Disabled => "disabled (set FORGEHUB_SMTP_URL to send mail)",
        }
    }
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

/// Session lifetimes and the login-throttle base, parsed together
/// because the absolute lifetime must cover the idle lifetime.
/// Returns `(idle_secs, absolute_secs, backoff_base_ms)`.
///
/// # Errors
/// On invalid values or `absolute < idle`.
fn session_settings(get: &dyn Fn(&str) -> Option<String>) -> Result<(u64, u64, u64)> {
    let idle: u64 = parse_or(
        "FORGEHUB_SESSION_IDLE_SECS",
        get("FORGEHUB_SESSION_IDLE_SECS")
            .as_deref()
            .unwrap_or_default(),
        7 * 24 * 3600,
    )?;
    let absolute: u64 = parse_or(
        "FORGEHUB_SESSION_ABSOLUTE_SECS",
        get("FORGEHUB_SESSION_ABSOLUTE_SECS")
            .as_deref()
            .unwrap_or_default(),
        30 * 24 * 3600,
    )?;
    if idle == 0 || absolute < idle {
        bail!(
            "FORGEHUB_SESSION_ABSOLUTE_SECS must be >= FORGEHUB_SESSION_IDLE_SECS (and idle at least 1)"
        );
    }
    let backoff_ms: u64 = parse_or(
        "FORGEHUB_LOGIN_BACKOFF_BASE_MS",
        get("FORGEHUB_LOGIN_BACKOFF_BASE_MS")
            .as_deref()
            .unwrap_or_default(),
        1000,
    )?;
    Ok((idle, absolute, backoff_ms))
}

/// Device-flow poll interval (`FORGEHUB_DEVICE_POLL_INTERVAL_SECS`).
fn device_poll_interval(get: &dyn Fn(&str) -> Option<String>) -> Result<u64> {
    parse_or(
        "FORGEHUB_DEVICE_POLL_INTERVAL_SECS",
        get("FORGEHUB_DEVICE_POLL_INTERVAL_SECS")
            .as_deref()
            .unwrap_or_default(),
        5,
    )
}

/// Snippet caps (`FORGEHUB_MAX_SNIPPETS_PER_USER`,
/// `FORGEHUB_MAX_VERSIONS_PER_SNIPPET`); both must be positive.
fn snippet_limits(get: &dyn Fn(&str) -> Option<String>) -> Result<(i64, i64)> {
    let per_user: i64 = parse_or(
        "FORGEHUB_MAX_SNIPPETS_PER_USER",
        get("FORGEHUB_MAX_SNIPPETS_PER_USER")
            .as_deref()
            .unwrap_or_default(),
        500,
    )?;
    let per_snippet: i64 = parse_or(
        "FORGEHUB_MAX_VERSIONS_PER_SNIPPET",
        get("FORGEHUB_MAX_VERSIONS_PER_SNIPPET")
            .as_deref()
            .unwrap_or_default(),
        200,
    )?;
    if per_user < 1 || per_snippet < 1 {
        bail!("snippet limits must be at least 1");
    }
    Ok((per_user, per_snippet))
}

impl Config {
    /// Load and validate configuration from the process environment.
    ///
    /// # Errors
    /// On invalid values, an unparseable bind address, or - in prod -
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
        let (session_idle_secs, session_absolute_secs, login_backoff_base_ms) =
            session_settings(get)?;
        let device_poll_interval_secs = device_poll_interval(get)?;
        let (max_snippets_per_user, max_versions_per_snippet) = snippet_limits(get)?;
        let smtp_url = get("FORGEHUB_SMTP_URL");
        if let Some(url) = &smtp_url {
            if !url.starts_with("smtp://") && !url.starts_with("smtps://") {
                bail!("FORGEHUB_SMTP_URL must start with smtp:// or smtps://");
            }
        }
        let mail_from = get("FORGEHUB_MAIL_FROM")
            .unwrap_or_else(|| "ForgeHub <no-reply@localhost>".to_string());
        if !mail_from.contains('@') {
            bail!("FORGEHUB_MAIL_FROM must contain an email address, got `{mail_from}`");
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
            session_idle_secs,
            session_absolute_secs,
            login_backoff_base_ms,
            device_poll_interval_secs,
            max_snippets_per_user,
            max_versions_per_snippet,
            smtp_url,
            mail_from,
        })
    }

    /// The mail mode this configuration produces (see [`MailMode`]).
    #[must_use]
    pub fn mail_mode(&self) -> MailMode {
        if self.smtp_url.is_some() {
            MailMode::Smtp
        } else if self.env == Env::Dev {
            MailMode::Console
        } else {
            MailMode::Disabled
        }
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
            "env={} bind={} base_url={} database={} secret_key={} allow_signup={} web_dir={} body_limit={}KiB rate_limit={}/min timeout={}s session_idle={}s session_absolute={}s mail={}",
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
            self.session_idle_secs,
            self.session_absolute_secs,
            self.mail_mode().as_str(),
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
    fn mail_modes_and_validation() {
        let dev = Config::from_vars(&vars(&[])).unwrap();
        assert_eq!(dev.mail_mode(), MailMode::Console);
        let smtp = Config::from_vars(&vars(&[(
            "FORGEHUB_SMTP_URL",
            "smtps://user:hunter2@smtp.example.com:465",
        )]))
        .unwrap();
        assert_eq!(smtp.mail_mode(), MailMode::Smtp);
        assert!(
            !smtp.summary().contains("hunter2"),
            "the summary must never leak the SMTP password: {}",
            smtp.summary()
        );
        let secret = "x".repeat(32);
        let prod = Config::from_vars(&vars(&[
            ("FORGEHUB_ENV", "prod"),
            ("FORGEHUB_SECRET_KEY", secret.as_str()),
        ]))
        .unwrap();
        assert_eq!(prod.mail_mode(), MailMode::Disabled);
        assert!(Config::from_vars(&vars(&[("FORGEHUB_SMTP_URL", "http://x")])).is_err());
        assert!(Config::from_vars(&vars(&[("FORGEHUB_MAIL_FROM", "not-an-address")])).is_err());
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
