//! Cloud commands (Step 11): `login`, `logout`, `whoami`, the
//! credentials store, and the HTTP client they share.
//!
//! Design notes:
//! - The HTTP client is **reqwest (blocking, rustls)**, constructed
//!   lazily inside cloud commands only. Local commands never build
//!   one, so their startup path is unchanged (re-measured in the
//!   Step 11 report). Proxy env vars (`HTTP_PROXY`, `HTTPS_PROXY`,
//!   `NO_PROXY`) are honoured by reqwest's system-proxy default.
//! - Retries: idempotent GETs retry transport errors and
//!   502/503/504 twice with backoff. POST/DELETE never retry — the
//!   device poll loop is its own retry mechanism.
//! - Tokens are never printed. Anything the server returns is shown
//!   through [`redact`] when a token is in scope, and the token
//!   prefix (first 8 chars of the random part) is the most that is
//!   ever displayed.
//! - Credentials live in `$XDG_CONFIG_HOME/forgehub/credentials.toml`
//!   (or `~/.config/forgehub/credentials.toml`), directory 0700,
//!   file 0600, keyed by registry URL, written atomically. A file
//!   with group/other permissions is refused (with a fix offer on a
//!   TTY) rather than read.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::exit::CliError;
use crate::prompt;
use crate::store::Store;

// ---------------------------------------------------------------------------
// Registry resolution
// ---------------------------------------------------------------------------

/// Validate and normalise a registry base URL: `http`/`https` with a
/// host, no credentials/query/fragment, trailing slashes trimmed.
pub fn normalize_registry(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let bad = |why: &str| {
        CliError::failure(format!(
            "forgehub: invalid registry URL {raw:?}\n  what: {why}\n  fix: pass a base URL like https://forgehub.example or http://127.0.0.1:8080"
        ))
    };
    let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    else {
        return Err(bad("the URL must start with http:// or https://").into());
    };
    if rest.is_empty() || rest.contains(['@', '?', '#', ' ']) {
        return Err(bad("the URL must be a bare scheme + host[:port][/path]").into());
    }
    Ok(trimmed.to_string())
}

/// Registry resolution order: `--registry` flag, `FORGEHUB_REGISTRY`,
/// config `server`. Returns the normalised URL.
pub fn resolve_registry(store: &Store, flag: Option<&str>) -> Result<Option<String>> {
    if let Some(f) = flag {
        return Ok(Some(normalize_registry(f)?));
    }
    if let Ok(env_url) = std::env::var("FORGEHUB_REGISTRY") {
        if !env_url.trim().is_empty() {
            return Ok(Some(normalize_registry(&env_url)?));
        }
    }
    if let Ok(cfg) = store.load_config() {
        if let Some(server) = cfg.server {
            if !server.trim().is_empty() {
                return Ok(Some(normalize_registry(&server)?));
            }
        }
    }
    Ok(None)
}

fn no_registry_error() -> CliError {
    CliError::failure(
        "forgehub: no registry configured\n  what: login needs a ForgeHub server URL\n  fix: fg login --registry https://your-server.example (or set FORGEHUB_REGISTRY)",
    )
}

// ---------------------------------------------------------------------------
// Credentials store
// ---------------------------------------------------------------------------

/// One registry's stored credential. The token itself is only ever
/// read to be sent back to its own registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredEntry {
    /// The bearer token (`fgh_…`). Never printed.
    pub token: String,
    /// Username as confirmed by the server at login time.
    pub username: String,
    /// Server-assigned token name, when known (device flow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_name: Option<String>,
    /// Granted scopes, when known (device flow).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Token expiry (ISO-8601), when the token has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// The credentials file: credentials keyed by normalised registry
/// URL, so several registries can be used side by side.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CredFile {
    #[serde(default)]
    pub registries: BTreeMap<String, CredEntry>,
}

/// The displayable prefix of a token: the first 8 characters of the
/// random part (the server's own definition), or a short stub for
/// foreign-shaped tokens.
#[must_use]
pub fn token_prefix(token: &str) -> String {
    let random = token.strip_prefix("fgh_").unwrap_or(token);
    random.chars().take(8).collect()
}

/// Replace any occurrence of `token` in `text` with its prefix stub,
/// so server messages can be shown without leaking credentials.
#[must_use]
pub fn redact(text: &str, token: &str) -> String {
    if token.is_empty() {
        return text.to_string();
    }
    text.replace(token, &format!("fgh_{}…", token_prefix(token)))
}

/// Path of the credentials file (XDG-aware).
pub fn credentials_path() -> Result<PathBuf> {
    let dir = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            let home = std::env::var_os("HOME")
                .context("neither XDG_CONFIG_HOME nor HOME is set; cannot locate credentials")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(dir.join("forgehub").join("credentials.toml"))
}

#[cfg(unix)]
fn file_mode(path: &std::path::Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .ok()
        .map(|md| md.permissions().mode() & 0o777)
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

/// Load the credentials file, enforcing permissions: a file that is
/// group/other-readable is refused — on a TTY the user is offered a
/// chmod fix; non-interactively the error says exactly what to run.
pub fn load_credentials(stdin: &mut impl BufRead) -> Result<CredFile> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(CredFile::default());
    }
    #[cfg(unix)]
    if let Some(mode) = file_mode(&path) {
        if mode & 0o077 != 0 {
            if prompt::stdin_is_tty() {
                eprintln!(
                    "forgehub: {} has permissions {mode:03o} — credentials must not be group/other readable.",
                    path.display()
                );
                if prompt::ask_yes_no(stdin, "Fix to 0600 now?") {
                    set_mode(&path, 0o600)
                        .with_context(|| format!("cannot chmod {}", path.display()))?;
                } else {
                    return Err(CliError::refused(
                        "forgehub: credentials not read\n  what: the file is readable by other users\n  fix: chmod 600 the file, or answer yes to the fix",
                    )
                    .into());
                }
            } else {
                return Err(CliError::failure(format!(
                    "forgehub: credentials not read\n  what: {} has permissions {mode:03o}; tokens must not be group/other readable\n  fix: chmod 600 {}",
                    path.display(),
                    path.display()
                ))
                .into());
            }
        }
    }
    load_credentials_file()
}

/// Parse the credentials file WITHOUT the permission gate or any
/// prompting (doctor uses this; the permission problem is its own
/// check there). Command paths must use [`load_credentials`].
pub fn load_credentials_file() -> Result<CredFile> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(CredFile::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("{} is not valid TOML", path.display()))
}

/// Persist the credentials file atomically: ensure the directory
/// (0700), write a temp file (0600) in it, rename over the target.
/// An empty file (no registries) removes the file instead.
pub fn save_credentials(creds: &CredFile) -> Result<()> {
    let path = credentials_path()?;
    if creds.registries.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("cannot remove {}", path.display()))?;
        }
        return Ok(());
    }
    let dir = path
        .parent()
        .context("credentials path has no parent directory")?;
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    #[cfg(unix)]
    set_mode(dir, 0o700).with_context(|| format!("cannot chmod {}", dir.display()))?;
    let text = toml::to_string(creds).context("cannot serialise credentials")?;
    let tmp = dir.join(".credentials.toml.tmp");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    set_mode(&tmp, 0o600)?; // in case a stale tmp existed with wider perms
    std::fs::rename(&tmp, &path).with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

/// A `ForgeHub` API error in the server's envelope shape.
#[derive(Debug)]
pub struct ApiFailure {
    /// HTTP status.
    pub status: u16,
    /// Machine code from the envelope ("" when unparseable).
    pub code: String,
    /// Human message from the envelope.
    pub message: String,
    /// Structured details from the envelope, when present (e.g.
    /// secret findings on a 422, `latest_version` on a 412).
    /// Boxed to keep the error type small.
    pub details: Option<Box<Value>>,
    /// `Retry-After` seconds on a 429, when the server sent it.
    pub retry_after: Option<u64>,
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}
impl std::error::Error for ApiFailure {}

/// Thin client over the `ForgeHub` API. See the module docs for the
/// retry and redaction rules.
pub struct ApiClient {
    base: String,
    http: reqwest::blocking::Client,
}

impl ApiClient {
    /// Client with the standard timeouts (5 s connect, 15 s total).
    pub fn new(registry: &str) -> Result<Self> {
        Self::with_timeout(registry, Duration::from_secs(15))
    }

    /// Client with a custom overall timeout (doctor uses 3 s).
    pub fn with_timeout(registry: &str, timeout: Duration) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(concat!("forgehub/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(timeout)
            .build()
            .context("cannot build the HTTP client")?;
        Ok(Self {
            base: registry.to_string(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.base)
    }

    fn send(
        &self,
        req: reqwest::blocking::RequestBuilder,
    ) -> Result<(u16, Value, reqwest::header::HeaderMap), ApiFailure> {
        let resp = req.send().map_err(|e| ApiFailure {
            status: 0,
            code: "network_error".into(),
            message: format!("cannot reach {}: {e}", self.base),
            details: None,
            retry_after: None,
        })?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let retry_after = headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let body: Value = resp.json().unwrap_or(Value::Null);
        if status >= 400 {
            let (code, message) = match (&body["error"]["code"], &body["error"]["message"]) {
                (Value::String(c), Value::String(m)) => (c.clone(), m.clone()),
                _ => (String::new(), format!("server returned HTTP {status}")),
            };
            let details = body["error"].get("details").cloned().map(Box::new);
            return Err(ApiFailure {
                status,
                code,
                message,
                details,
                retry_after,
            });
        }
        Ok((status, body, headers))
    }

    /// A request with extra headers (e.g. `If-Match` on PUT).
    /// Never retried.
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        token: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> Result<(u16, Value, reqwest::header::HeaderMap), ApiFailure> {
        let mut req = match method {
            "PUT" => self.http.put(self.url(path)),
            "PATCH" => self.http.patch(self.url(path)),
            "DELETE" => self.http.delete(self.url(path)),
            _ => self.http.post(self.url(path)),
        };
        if let Some(b) = body {
            req = req.json(b);
        }
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        for (k, v) in extra_headers {
            req = req.header(*k, *v);
        }
        self.send(req)
    }

    /// GET with retries (transport errors and 502/503/504, twice,
    /// with backoff) — GETs are idempotent.
    pub fn get(
        &self,
        path: &str,
        token: Option<&str>,
    ) -> Result<(u16, Value, reqwest::header::HeaderMap), ApiFailure> {
        let mut delay = Duration::from_millis(300);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut req = self.http.get(self.url(path));
            if let Some(t) = token {
                req = req.bearer_auth(t);
            }
            match self.send(req) {
                Ok(ok) => return Ok(ok),
                Err(e) if attempt < 3 && (e.status == 0 || [502, 503, 504].contains(&e.status)) => {
                    std::thread::sleep(delay);
                    delay *= 3;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// POST a JSON body. Never retried (not idempotent in general).
    pub fn post_json(
        &self,
        path: &str,
        body: &Value,
        token: Option<&str>,
    ) -> Result<(u16, Value, reqwest::header::HeaderMap), ApiFailure> {
        let mut req = self.http.post(self.url(path)).json(body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        self.send(req)
    }

    /// DELETE. Never retried.
    pub fn delete(
        &self,
        path: &str,
        token: Option<&str>,
    ) -> Result<(u16, Value, reqwest::header::HeaderMap), ApiFailure> {
        let mut req = self.http.delete(self.url(path));
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        self.send(req)
    }
}

// ---------------------------------------------------------------------------
// Device-flow polling (pure state machine, unit-tested)
// ---------------------------------------------------------------------------

/// What a poll response means for the login loop.
#[derive(Debug, PartialEq, Eq)]
pub enum PollVerdict {
    /// `status: approved` — the access token is in the body.
    Approved,
    /// Keep waiting.
    Pending,
    /// The user denied the request in the browser.
    Denied,
    /// The code expired before approval.
    Expired,
}

/// Classify a `POST /auth/device/token` response body.
#[must_use]
pub fn poll_verdict(body: &Value) -> PollVerdict {
    match body["status"].as_str() {
        Some("approved") => PollVerdict::Approved,
        Some("denied") => PollVerdict::Denied,
        Some("expired") => PollVerdict::Expired,
        _ => PollVerdict::Pending,
    }
}

/// The next poll interval: `slow_down` adds 5 seconds (RFC 8628
/// §3.5); anything else keeps the current interval.
#[must_use]
pub fn next_interval(current: u64, body: &Value) -> u64 {
    if body["error"].as_str() == Some("slow_down") {
        current + 5
    } else {
        current
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// This machine's name for the device flow (`client_name`): 1–80
/// chars, no control characters (the server validates the same).
fn client_name() -> String {
    let raw = std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| {
            std::fs::read_to_string("/proc/sys/kernel/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        });
    let mut name: String = raw
        .unwrap_or_else(|| "this device".into())
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if name.is_empty() {
        name = "this device".into();
    }
    name
}

/// Try to open the browser at `url`. Skipped silently on SSH
/// sessions and headless Linux; all failures are ignored — the URL
/// is always printed as well.
fn try_open_browser(url: &str) {
    if std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some() {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return;
        }
        let _ = std::process::Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// The device-code login flow. Returns the stored credential entry.
fn device_login(client: &ApiClient, registry: &str, json: bool) -> Result<CredEntry> {
    let name = client_name();
    let (_, code_body, _) = client
        .post_json(
            "/auth/device/code",
            &serde_json::json!({ "client_name": name }),
            None,
        )
        .map_err(|e| {
            CliError::failure(format!(
                "forgehub: login failed: {}",
                redact(&e.message, "")
            ))
        })?;
    let device_code = code_body["device_code"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let user_code = code_body["user_code"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let verify_url = code_body["verification_uri_complete"]
        .as_str()
        .or_else(|| code_body["verification_uri"].as_str())
        .unwrap_or_default()
        .to_string();
    let interval = code_body["interval"].as_u64().unwrap_or(5).max(1);
    let expires_in = code_body["expires_in"].as_u64().unwrap_or(600);

    if !json {
        println!("Log in to ForgeHub");
        println!("  registry: {registry}");
        println!("  open:     {verify_url}");
        println!("  code:     {user_code}");
        println!(
            "Waiting for approval… (the code expires in {} minutes)",
            expires_in / 60
        );
    }
    try_open_browser(&verify_url);

    let (token, scopes) = poll_for_approval(client, &device_code, interval, expires_in)?;
    let (_, me, _) = client.get("/me", Some(&token)).map_err(|e| {
        CliError::failure(format!(
            "forgehub: login failed: {}",
            redact(&e.message, &token)
        ))
    })?;
    let username = me["username"].as_str().unwrap_or_default().to_string();
    let today = chrono::Utc::now().format("%Y-%m-%d");
    Ok(CredEntry {
        token,
        username,
        token_name: Some(format!("CLI on {name} ({today})")),
        scopes,
        expires_at: None,
    })
}

/// Poll the device-token endpoint until approval, denial, or
/// expiry, honouring the server interval and `slow_down` backoff.
/// Returns (access token, scopes).
fn poll_for_approval(
    client: &ApiClient,
    device_code: &str,
    mut interval: u64,
    expires_in: u64,
) -> Result<(String, Vec<String>)> {
    let deadline = std::time::Instant::now() + Duration::from_secs(expires_in + 5);
    loop {
        std::thread::sleep(Duration::from_secs(interval));
        let (_, body, _) = client
            .post_json(
                "/auth/device/token",
                &serde_json::json!({ "device_code": device_code }),
                None,
            )
            .map_err(|e| {
                CliError::failure(format!(
                    "forgehub: login failed while polling: {}",
                    redact(&e.message, "")
                ))
            })?;
        match poll_verdict(&body) {
            PollVerdict::Approved => {
                let issued = body["access_token"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                if issued.is_empty() {
                    return Err(CliError::failure(
                        "forgehub: login failed\n  what: the server approved the login but returned no token\n  fix: try again; if it persists, report a server bug",
                    )
                    .into());
                }
                let scopes: Vec<String> = body["scopes"].as_array().map_or_else(Vec::new, |a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                });
                return Ok((issued, scopes));
            }
            PollVerdict::Denied => {
                return Err(CliError::refused(
                    "forgehub: login denied\n  what: the request was denied in the browser\n  fix: run `fg login` again and approve the request",
                )
                .into());
            }
            PollVerdict::Expired => {
                return Err(CliError::failure(
                    "forgehub: login code expired\n  what: the code was not approved in time\n  fix: run `fg login` again",
                )
                .into());
            }
            PollVerdict::Pending => {
                interval = next_interval(interval, &body);
                if std::time::Instant::now() > deadline {
                    return Err(CliError::failure(
                        "forgehub: login timed out\n  what: the code was not approved in time\n  fix: run `fg login` again",
                    )
                    .into());
                }
            }
        }
    }
}

/// Persist a successful login: credentials entry + config
/// `server`/`username` (a config write failure is a warning, not a
/// failed login — the credential is already stored).
fn finish_login(
    store: &Store,
    stdin: &mut impl BufRead,
    registry: &str,
    entry: &CredEntry,
    json: bool,
) -> Result<i32> {
    let mut creds = load_credentials(stdin)?;
    creds.registries.insert(registry.to_string(), entry.clone());
    save_credentials(&creds)?;
    if let Ok(mut cfg) = store.load_config() {
        cfg.server = Some(registry.to_string());
        cfg.username = Some(entry.username.clone());
        if store.save_config(&cfg).is_err() {
            eprintln!("forgehub: warning: could not update config.yaml (credentials were saved)");
        }
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "username": entry.username,
                "registry": registry,
                "token_prefix": token_prefix(&entry.token),
                "scopes": entry.scopes,
            })
        );
    } else {
        println!("Logged in as {} on {registry}.", entry.username);
        println!(
            "  token: fgh_{}… (stored in {}, never printed in full)",
            token_prefix(&entry.token),
            credentials_path()
                .map_or_else(|_| "credentials.toml".into(), |p| p.display().to_string())
        );
    }
    Ok(0)
}

/// `fg login [--registry URL] [--token]`.
pub fn cmd_login(
    store: &Store,
    stdin: &mut impl BufRead,
    registry_flag: Option<&str>,
    token_flag: bool,
    json: bool,
) -> Result<i32> {
    let registry = resolve_registry(store, registry_flag)?.ok_or_else(no_registry_error)?;
    let client = ApiClient::new(&registry)?;
    let entry = if token_flag {
        // Token from stdin only — never argv, where it would land
        // in shell history and `ps`. (Read through the caller's
        // stdin handle: the process stdin lock is already held by
        // `main`, so a second handle would deadlock.)
        let mut raw = String::new();
        stdin
            .read_to_string(&mut raw)
            .context("cannot read the token from stdin")?;
        let token = raw.trim().to_string();
        if token.is_empty() {
            return Err(CliError::failure(
                "forgehub: no token on stdin\n  what: --token expects the token piped on stdin\n  fix: print the token | fg login --token",
            )
            .into());
        }
        let (_, me, _) = client.get("/me", Some(&token)).map_err(|e| {
            if e.status == 401 {
                CliError::auth(
                    "forgehub: that token is not valid for this registry\n  what: the server rejected it (401)\n  fix: check the token, or use `fg login` (device flow) instead",
                )
            } else {
                CliError::failure(format!(
                    "forgehub: login failed: {}",
                    redact(&e.message, &token)
                ))
            }
        })?;
        CredEntry {
            token,
            username: me["username"].as_str().unwrap_or_default().to_string(),
            token_name: None,
            scopes: Vec::new(),
            expires_at: None,
        }
    } else {
        device_login(&client, &registry, json)?
    };
    finish_login(store, stdin, &registry, &entry, json)
}

/// Which credential a command should use, after resolution:
/// (registry, entry). `FORGEHUB_TOKEN` wins over the file and is
/// never stored.
pub(crate) fn active_credential(
    store: &Store,
    stdin: &mut impl BufRead,
    registry_flag: Option<&str>,
) -> Result<Option<(String, CredEntry)>> {
    if let Ok(env_token) = std::env::var("FORGEHUB_TOKEN") {
        if !env_token.trim().is_empty() {
            let registry = resolve_registry(store, registry_flag)?.ok_or_else(|| {
                CliError::failure(
                    "forgehub: FORGEHUB_TOKEN is set but no registry is configured\n  what: the token needs a server to talk to\n  fix: pass --registry <url> or set FORGEHUB_REGISTRY",
                )
            })?;
            return Ok(Some((
                registry,
                CredEntry {
                    token: env_token.trim().to_string(),
                    username: String::new(),
                    token_name: None,
                    scopes: Vec::new(),
                    expires_at: None,
                },
            )));
        }
    }
    let creds = load_credentials(stdin)?;
    if let Some(registry) = resolve_registry(store, registry_flag)? {
        return Ok(creds
            .registries
            .get(&registry)
            .map(|e| (registry, e.clone())));
    }
    if creds.registries.len() == 1 {
        let (registry, entry) = creds.registries.iter().next().expect("one entry");
        return Ok(Some((registry.clone(), entry.clone())));
    }
    Ok(None)
}

/// `fg whoami [--registry URL]`.
pub fn cmd_whoami(
    store: &Store,
    stdin: &mut impl BufRead,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let Some((registry, entry)) = active_credential(store, stdin, registry_flag)? else {
        return Err(CliError::auth(
            "forgehub: not logged in\n  what: no stored credentials match this registry\n  fix: fg login",
        )
        .into());
    };
    let client = ApiClient::new(&registry)?;
    let (_, me, _) = client.get("/me", Some(&entry.token)).map_err(|e| {
        if e.status == 401 {
            CliError::auth(
                "forgehub: the stored token is invalid or expired\n  what: the server rejected it (401)\n  fix: fg login",
            )
        } else {
            CliError::failure(format!(
                "forgehub: whoami failed: {}",
                redact(&e.message, &entry.token)
            ))
        }
    })?;
    let username = me["username"].as_str().unwrap_or_default().to_string();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "username": username,
                "registry": registry,
                "token_name": entry.token_name,
                "token_prefix": token_prefix(&entry.token),
                "scopes": entry.scopes,
                "expires_at": entry.expires_at,
            })
        );
        return Ok(0);
    }
    println!("{username}");
    println!("  registry: {registry}");
    match &entry.token_name {
        Some(name) => println!("  token:    {name} · fgh_{}…", token_prefix(&entry.token)),
        None => println!("  token:    fgh_{}…", token_prefix(&entry.token)),
    }
    if entry.scopes.is_empty() {
        println!("  scopes:   (unknown — token was provided directly)");
    } else {
        println!("  scopes:   {}", entry.scopes.join(", "));
    }
    match &entry.expires_at {
        Some(exp) => println!("  expires:  {exp}"),
        None => println!("  expires:  never"),
    }
    Ok(0)
}

/// `fg logout [--registry URL] [--revoke]`.
pub fn cmd_logout(
    store: &Store,
    stdin: &mut impl BufRead,
    registry_flag: Option<&str>,
    revoke: bool,
    json: bool,
) -> Result<i32> {
    // FORGEHUB_TOKEN is environment-provided; there is nothing
    // local to log out of.
    let creds = load_credentials(stdin)?;
    let registry = match resolve_registry(store, registry_flag)? {
        Some(r) => Some(r),
        None if creds.registries.len() == 1 => creds.registries.keys().next().cloned(),
        None => None,
    };
    let Some(registry) = registry else {
        if json {
            println!("{}", serde_json::json!({ "logged_out": false }));
        } else {
            println!("Not logged in.");
        }
        return Ok(0);
    };
    let Some(entry) = creds.registries.get(&registry).cloned() else {
        if json {
            println!(
                "{}",
                serde_json::json!({ "registry": registry, "logged_out": false })
            );
        } else {
            println!("Not logged in to {registry}.");
        }
        return Ok(0);
    };

    let mut exit_code = 0;
    let mut revoked = false;
    if revoke {
        let client = ApiClient::new(&registry)?;
        match client.delete("/tokens/current", Some(&entry.token)) {
            Ok(_) => revoked = true,
            Err(e) if e.status == 401 => {
                // Already invalid server-side: the goal is met.
                revoked = true;
            }
            Err(e) => {
                eprintln!(
                    "forgehub: warning: could not revoke the token server-side ({}). \
                     It stays valid until it expires or you revoke it from the web UI. \
                     Local credentials were still removed.",
                    redact(&e.message, &entry.token)
                );
                exit_code = 1;
            }
        }
    }
    let mut creds = creds;
    creds.registries.remove(&registry);
    save_credentials(&creds)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "registry": registry,
                "username": entry.username,
                "revoked": revoked,
                "logged_out": true,
            })
        );
    } else {
        println!("Logged out of {registry} (was {}).", entry.username);
        if revoke && revoked {
            println!("  token revoked server-side");
        }
    }
    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_normalisation() {
        assert_eq!(
            normalize_registry("https://x.example/").unwrap(),
            "https://x.example"
        );
        assert_eq!(
            normalize_registry(" http://127.0.0.1:8080 ").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert!(normalize_registry("x.example").is_err());
        assert!(normalize_registry("ftp://x.example").is_err());
        assert!(normalize_registry("https://user@x.example").is_err());
        assert!(normalize_registry("https://").is_err());
    }

    #[test]
    fn prefix_uses_the_servers_definition() {
        assert_eq!(token_prefix("fgh_ABCDEFGHrest"), "ABCDEFGH");
        assert_eq!(token_prefix("short"), "short");
    }

    #[test]
    fn redaction_hides_the_token() {
        let out = redact("token fgh_SECRET99x leaked", "fgh_SECRET99x");
        assert!(!out.contains("SECRET99x"), "{out}");
        assert!(out.contains("fgh_SECRET99…"), "{out}");
    }

    #[test]
    fn poll_state_machine() {
        assert_eq!(
            poll_verdict(&serde_json::json!({"status": "approved"})),
            PollVerdict::Approved
        );
        assert_eq!(
            poll_verdict(
                &serde_json::json!({"status": "pending", "error": "authorization_pending"})
            ),
            PollVerdict::Pending
        );
        assert_eq!(
            poll_verdict(&serde_json::json!({"status": "denied", "error": "access_denied"})),
            PollVerdict::Denied
        );
        assert_eq!(
            poll_verdict(&serde_json::json!({"status": "expired", "error": "expired_token"})),
            PollVerdict::Expired
        );
        assert_eq!(
            next_interval(5, &serde_json::json!({"error": "slow_down"})),
            10
        );
        assert_eq!(
            next_interval(5, &serde_json::json!({"error": "authorization_pending"})),
            5
        );
    }

    #[test]
    fn credentials_toml_roundtrip() {
        let mut creds = CredFile::default();
        creds.registries.insert(
            "http://127.0.0.1:8080".into(),
            CredEntry {
                token: concat!("fgh_", "ABCDEFGHxxxx").into(),
                username: "anna".into(),
                token_name: Some("CLI on host (2026-11-19)".into()),
                scopes: vec!["read".into(), "write".into()],
                expires_at: None,
            },
        );
        let text = toml::to_string(&creds).unwrap();
        let back: CredFile = toml::from_str(&text).unwrap();
        let entry = &back.registries["http://127.0.0.1:8080"];
        assert_eq!(entry.username, "anna");
        assert_eq!(entry.scopes, ["read", "write"]);
    }
}
