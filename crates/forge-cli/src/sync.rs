//! Cloud sync commands (Step 12): `push`, `pull`, `status`,
//! `diff`, `remote …`, `fork`, and the `record --push` hook.
//!
//! Invariants:
//! - A push is validated and secret-scanned locally first; a
//!   snippet with secret findings never leaves the machine.
//! - A pull never overwrites a local snippet without `--force`
//!   or an explicit confirmation, and what it fetched is shown
//!   (escaped) before it is saved when overwriting.
//! - Provenance lives in a `<name>.meta.json` sidecar
//!   (`{registry, owner, name, version, hash}`) so `status` can
//!   tell ahead / behind / diverged apart by hash.
//! - Hashes are forge-core content hashes on both sides; the
//!   round-trip equality is asserted by integration tests.

use std::fmt::Write as _;
use std::io::BufRead;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cloud::{active_credential, ApiClient, ApiFailure, CredEntry};
use crate::cmds::escape_display;
use crate::exit::CliError;
use crate::prompt;
use crate::store::Store;

// ---------------------------------------------------------------------------
// Session + error mapping
// ---------------------------------------------------------------------------

/// A logged-in cloud session for one registry.
pub struct Session {
    /// Normalised registry base URL.
    pub registry: String,
    /// The stored credential (token inside; never printed).
    pub entry: CredEntry,
    /// HTTP client bound to the registry.
    pub client: ApiClient,
    /// The account username (from the credential, or `GET /me`
    /// when the credential does not record it).
    pub username: String,
}

/// Resolve the active credential into a session, or fail with the
/// exact fix (`forgehub login`, exit 4).
pub fn session(
    store: &Store,
    stdin: &mut impl BufRead,
    registry_flag: Option<&str>,
) -> Result<Session> {
    let Some((registry, entry)) = active_credential(store, stdin, registry_flag)? else {
        return Err(CliError::auth(
            "forgehub: not logged in\n  what: cloud commands need a stored credential\n  fix: forgehub login",
        )
        .into());
    };
    let client = ApiClient::new(&registry)?;
    let username = if entry.username.is_empty() {
        let (_, me, _) = client
            .get("/me", Some(&entry.token))
            .map_err(|e| api_fail(&e, &entry.token))?;
        me["username"].as_str().unwrap_or_default().to_string()
    } else {
        entry.username.clone()
    };
    Ok(Session {
        registry,
        entry,
        client,
        username,
    })
}

/// Map an API failure to the CLI error the roadmap's error UX
/// requires: 401 → auth (exit 4), 404 → not found (exit 3, noting
/// private snippets look identical), 412 → conflict with the
/// recovery commands named, 422 → the secret findings listed,
/// 429 → the wait time named.
pub fn api_fail(e: &ApiFailure, token: &str) -> CliError {
    let msg = crate::cloud::redact(&e.message, token);
    match e.status {
        401 => CliError::auth(format!(
            "forgehub: the stored token is invalid or expired (401)\n  what: {msg}\n  fix: forgehub login"
        )),
        404 => CliError::not_found(format!(
            "forgehub: not found (404)\n  what: {msg} — private snippets are indistinguishable from missing ones\n  fix: check the owner/name, or ask the owner to share it"
        )),
        412 => {
            let latest = e
                .details
                .as_ref()
                .and_then(|d| d["latest_version"].as_i64())
                .map_or_else(|| "a newer version".to_string(), |v| format!("v{v}"));
            CliError::failure(format!(
                "forgehub: version conflict (412)\n  what: the remote already has {latest}; someone (maybe you, on another machine) pushed first\n  fix: `fg diff <name>` to compare, `fg pull <name> --force` to take the remote, or edit and push again"
            ))
        }
        422 => {
            let mut out =
                String::from("forgehub: the server refused the snippet (422): secret material detected\n");
            if let Some(findings) = e.details.as_ref().and_then(|d| d["findings"].as_array()) {
                for f in findings {
                    let _ = writeln!(
                        out,
                        "  line {}: {}",
                        f["line"].as_u64().unwrap_or(0),
                        f["kind"].as_str().unwrap_or("secret")
                    );
                }
            }
            out.push_str("  fix: move secrets into variables or your secret manager, then push again");
            CliError::refused(out)
        }
        429 => CliError::failure(format!(
            "forgehub: rate limited (429)\n  what: {msg}\n  fix: wait {} and retry",
            e.retry_after
                .map_or_else(|| "a minute".to_string(), |s| format!("{s}s"))
        )),
        0 => CliError::failure(format!(
            "forgehub: cannot reach the server\n  what: {msg}\n  fix: check the registry URL and your network; local commands keep working offline"
        )),
        _ => CliError::failure(format!(
            "forgehub: server error ({} {})\n  what: {msg}",
            e.status, e.code
        )),
    }
}

// ---------------------------------------------------------------------------
// Provenance sidecar
// ---------------------------------------------------------------------------

/// Cloud provenance for a local snippet (spec-cli §: the `.meta`
/// sidecar shape).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    /// Registry the snippet came from / goes to.
    pub registry: String,
    /// Remote owner username.
    pub owner: String,
    /// Remote snippet name.
    pub name: String,
    /// Remote version at last sync.
    pub version: i64,
    /// Remote content hash at last sync.
    pub hash: String,
}

/// Load the sidecar for a local snippet, when one exists and
/// parses.
#[must_use]
pub fn load_meta(store: &Store, local_name: &str) -> Option<Meta> {
    let text = store.read_meta(local_name)?;
    serde_json::from_str(&text).ok()
}

/// Persist the sidecar for a local snippet.
pub fn save_meta(store: &Store, local_name: &str, meta: &Meta) -> Result<()> {
    store.write_meta(local_name, &serde_json::to_string(meta)?)
}

/// The web URL for a snippet (profile-style path on the registry).
#[must_use]
pub fn web_url(registry: &str, owner: &str, name: &str) -> String {
    format!("{registry}/{owner}/{name}")
}

/// Split an `owner/name` or bare `name` reference. Bare names
/// resolve against `default_owner`.
pub fn parse_reference(reference: &str, default_owner: &str) -> (String, String) {
    match reference.split_once('/') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() => {
            (owner.to_string(), name.to_string())
        }
        _ => (default_owner.to_string(), reference.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Push
// ---------------------------------------------------------------------------

/// What a push did.
#[derive(Debug, PartialEq, Eq)]
pub enum PushStatus {
    /// Created remotely at v1.
    Created,
    /// A new version was appended.
    Updated,
    /// The remote already had exactly this content.
    Unchanged,
}

/// The outcome of one push.
pub struct PushOutcome {
    /// Remote owner.
    pub owner: String,
    /// Remote name.
    pub name: String,
    /// Latest remote version after the push.
    pub version: i64,
    /// Content hash after the push.
    pub hash: String,
    /// What happened.
    pub status: PushStatus,
}

/// Validate + secret-scan a local snippet for pushing. Refuses
/// (exit 2) with the findings listed when the scan hits.
fn checked_local(store: &Store, name: &str) -> Result<(forge_core::Snippet, String)> {
    let snippet = store.load(name)?;
    snippet.validate().map_err(|e| {
        CliError::failure(format!(
            "forgehub: `{name}` is not valid\n  what: {e}\n  fix: correct the snippet (see `fg show {name}`) and push again"
        ))
    })?;
    let findings = forge_core::scan::scan_snippet(&snippet);
    if !findings.is_empty() {
        let mut out = format!(
            "forgehub: refusing to push `{name}`\n  what: the local secret scan found possible secrets — they never leave this machine:\n"
        );
        for f in &findings {
            let _ = writeln!(out, "  line {}: {}", f.line, f.kind.as_str());
        }
        out.push_str("  fix: move secrets into variables or your secret manager, then push again");
        return Err(CliError::refused(out).into());
    }
    let yaml = snippet.to_yaml()?;
    Ok((snippet, yaml))
}

/// Push one local snippet. `remote_name` defaults to the local
/// name; `visibility` of None keeps/uses the defaults (private for
/// new, unchanged for existing).
pub fn push_one(
    store: &Store,
    sess: &Session,
    name: &str,
    remote_name: Option<&str>,
    visibility: Option<&str>,
    message: Option<&str>,
) -> Result<PushOutcome> {
    let (snippet, yaml) = checked_local(store, name)?;
    let remote_name = remote_name.unwrap_or(name).to_string();
    let local_hash = forge_core::hash::content_hash(&snippet);
    let detail_path = format!("/snippets/{}/{}", sess.username, remote_name);
    let existing = match sess.client.get(&detail_path, Some(&sess.entry.token)) {
        Ok((_, body, _)) => Some(body),
        Err(e) if e.status == 404 => None,
        Err(e) => return Err(api_fail(&e, &sess.entry.token).into()),
    };
    let outcome = match existing {
        None => push_create(
            sess,
            &snippet,
            &yaml,
            &remote_name,
            visibility,
            message,
            &local_hash,
        )?,
        Some(remote) => {
            let remote_hash = remote["hash"].as_str().unwrap_or_default().to_string();
            let remote_version = remote["version"].as_i64().unwrap_or(0);
            if remote_hash == local_hash {
                // Apply a visibility change even when content is
                // unchanged.
                apply_visibility(sess, &detail_path, &remote, visibility)?;
                PushOutcome {
                    owner: sess.username.clone(),
                    name: remote_name.clone(),
                    version: remote_version,
                    hash: remote_hash,
                    status: PushStatus::Unchanged,
                }
            } else {
                // Optimistic concurrency is based on the version
                // this local copy was last synced at (the sidecar),
                // NOT the version just read: if the remote moved
                // since the sync, the server's 412 is the honest
                // answer. Without a sidecar baseline there is no
                // safe If-Match, so refuse and point at pull/diff.
                let baseline = load_meta(store, name).filter(|m| {
                    m.registry == sess.registry && m.owner == sess.username && m.name == remote_name
                });
                let Some(baseline) = baseline else {
                    return Err(CliError::failure(format!(
                        "forgehub: version conflict\n  what: {}/{remote_name} already exists on the server (v{remote_version}) and this local copy has no sync baseline for it — pushing would silently overwrite someone else's version\n  fix: `fg diff {name}` to compare, `fg pull {remote_name} --as {name}` to fetch theirs first, or push under a new name with --as",
                        sess.username
                    ))
                    .into());
                };
                let body = serde_json::json!({
                    "name": remote_name,
                    "description": snippet.description,
                    "tags": snippet.tags,
                    "body": yaml,
                    "message": message,
                });
                let if_match = format!("\"{}\"", baseline.version);
                let (_, updated, _) = sess
                    .client
                    .request(
                        "PUT",
                        &detail_path,
                        Some(&body),
                        Some(&sess.entry.token),
                        &[("If-Match", &if_match)],
                    )
                    .map_err(|e| api_fail(&e, &sess.entry.token))?;
                let version = updated["version"].as_i64().unwrap_or(remote_version + 1);
                let hash = updated["hash"].as_str().unwrap_or(&local_hash).to_string();
                apply_visibility(sess, &detail_path, &remote, visibility)?;
                PushOutcome {
                    owner: sess.username.clone(),
                    name: remote_name.clone(),
                    version,
                    hash,
                    status: if updated["unchanged"].as_bool() == Some(true) {
                        PushStatus::Unchanged
                    } else {
                        PushStatus::Updated
                    },
                }
            }
        }
    };
    save_meta(
        store,
        name,
        &Meta {
            registry: sess.registry.clone(),
            owner: outcome.owner.clone(),
            name: outcome.name.clone(),
            version: outcome.version,
            hash: outcome.hash.clone(),
        },
    )?;
    Ok(outcome)
}

/// PATCH the visibility when the caller asked for one and it
/// differs from the remote's current value.
fn apply_visibility(
    sess: &Session,
    detail_path: &str,
    remote: &Value,
    visibility: Option<&str>,
) -> Result<()> {
    if let Some(vis) = visibility {
        if remote["visibility"].as_str() != Some(vis) {
            sess.client
                .request(
                    "PATCH",
                    detail_path,
                    Some(&serde_json::json!({ "visibility": vis })),
                    Some(&sess.entry.token),
                    &[],
                )
                .map_err(|e| api_fail(&e, &sess.entry.token))?;
        }
    }
    Ok(())
}

/// The create branch of a push (no remote snippet yet).
fn push_create(
    sess: &Session,
    snippet: &forge_core::Snippet,
    yaml: &str,
    remote_name: &str,
    visibility: Option<&str>,
    message: Option<&str>,
    local_hash: &str,
) -> Result<PushOutcome> {
    let body = serde_json::json!({
        "name": remote_name,
        "description": snippet.description,
        "tags": snippet.tags,
        "visibility": visibility.unwrap_or("private"),
        "body": yaml,
        "message": message,
    });
    let (_, created, _) = sess
        .client
        .post_json("/snippets", &body, Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    Ok(PushOutcome {
        owner: sess.username.clone(),
        name: remote_name.to_string(),
        version: created["version"].as_i64().unwrap_or(1),
        hash: created["hash"].as_str().unwrap_or(local_hash).to_string(),
        status: PushStatus::Created,
    })
}

fn print_push_outcome(registry: &str, outcome: &PushOutcome, json: bool) {
    let url = web_url(registry, &outcome.owner, &outcome.name);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "owner": outcome.owner,
                "name": outcome.name,
                "version": outcome.version,
                "hash": outcome.hash,
                "url": url,
                "status": match outcome.status {
                    PushStatus::Created => "created",
                    PushStatus::Updated => "updated",
                    PushStatus::Unchanged => "unchanged",
                },
            })
        );
        return;
    }
    let short = outcome.hash.get(..8).unwrap_or(&outcome.hash);
    match outcome.status {
        PushStatus::Unchanged => println!(
            "{}/{} is up to date (v{}, sha256:{short})",
            outcome.owner, outcome.name, outcome.version
        ),
        _ => println!(
            "{}/{} v{} sha256:{short}\n  {url}",
            outcome.owner, outcome.name, outcome.version
        ),
    }
}

/// `fg push <name>|--all …`.
#[allow(clippy::too_many_arguments)]
pub fn cmd_push(
    store: &Store,
    stdin: &mut impl BufRead,
    name: Option<&str>,
    all: bool,
    visibility: Option<&str>,
    message: Option<&str>,
    as_name: Option<&str>,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    if all {
        let names: Vec<String> = store.list().iter().map(|s| s.name.clone()).collect();
        if names.is_empty() {
            println!("No local snippets to push.");
            return Ok(0);
        }
        let mut failed = 0;
        let mut rows: Vec<(String, String)> = Vec::new();
        for n in &names {
            match push_one(store, &sess, n, None, visibility, message) {
                Ok(outcome) => {
                    let label = match outcome.status {
                        PushStatus::Created => format!("created v{}", outcome.version),
                        PushStatus::Updated => format!("updated v{}", outcome.version),
                        PushStatus::Unchanged => "up to date".to_string(),
                    };
                    rows.push((n.clone(), label));
                }
                Err(e) => {
                    failed += 1;
                    rows.push((n.clone(), format!("FAILED: {}", first_line(&e.to_string()))));
                }
            }
        }
        if json {
            let arr: Vec<Value> = rows
                .iter()
                .map(|(n, r)| serde_json::json!({ "name": n, "result": r }))
                .collect();
            println!("{}", serde_json::json!({ "results": arr }));
        } else {
            let width = rows.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
            for (n, r) in &rows {
                println!("{n:<width$}  {r}");
            }
            println!(
                "{} pushed, {} up to date, {} failed",
                rows.iter()
                    .filter(|(_, r)| r.starts_with("created") || r.starts_with("updated"))
                    .count(),
                rows.iter()
                    .filter(|(_, r)| r.as_str() == "up to date")
                    .count(),
                failed
            );
        }
        return Ok(i32::from(failed > 0));
    }
    let name = name.ok_or_else(|| {
        CliError::failure(
            "forgehub: push needs a snippet name\n  what: no name was given\n  fix: fg push <name>, or fg push --all",
        )
    })?;
    let outcome = push_one(store, &sess, name, as_name, visibility, message)?;
    print_push_outcome(&sess.registry, &outcome, json);
    Ok(0)
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).to_string()
}

// ---------------------------------------------------------------------------
// Pull
// ---------------------------------------------------------------------------

/// Fetch a snippet detail (latest or pinned version).
fn fetch_snippet(sess: &Session, owner: &str, name: &str, version: Option<i64>) -> Result<Value> {
    let mut path = format!("/snippets/{owner}/{name}");
    if let Some(v) = version {
        let _ = write!(path, "?version={v}");
    }
    let (_, body, _) = sess
        .client
        .get(&path, Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    Ok(body)
}

/// `fg pull <owner/name|name> [--as local] [--version N] [--force]`.
#[allow(clippy::too_many_arguments)]
pub fn cmd_pull(
    store: &Store,
    stdin: &mut impl BufRead,
    reference: &str,
    as_name: Option<&str>,
    version: Option<i64>,
    force: bool,
    yes: bool,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let (owner, name) = parse_reference(reference, &sess.username);
    let remote = fetch_snippet(&sess, &owner, &name, version)?;
    let body = remote["body"].as_str().unwrap_or_default().to_string();
    let remote_hash = remote["hash"].as_str().unwrap_or_default().to_string();
    let remote_version = remote["version"].as_i64().unwrap_or(0);
    // The fetched text is untrusted until it parses and validates.
    let snippet = forge_core::Snippet::from_yaml(&body).map_err(|e| {
        CliError::failure(format!(
            "forgehub: the fetched snippet is not a valid forge file\n  what: {e}\n  fix: nothing was saved; report the problem to the snippet's owner"
        ))
    })?;
    snippet.validate().map_err(|e| {
        CliError::failure(format!(
            "forgehub: the fetched snippet fails validation\n  what: {e}\n  fix: nothing was saved"
        ))
    })?;
    let local_name = as_name.unwrap_or(&name).to_string();
    let overwriting = store.exists(&local_name);
    if overwriting && !force && !yes {
        // Show exactly what would replace the local copy
        // (display-escaped, spec-cli §8) before asking.
        if !json {
            println!(
                "{} — {}",
                escape_display(&format!("{owner}/{name}")),
                escape_display(snippet.description.as_str())
            );
            println!(
                "  v{remote_version} · sha256:{} · from {}",
                remote_hash.get(..8).unwrap_or(&remote_hash),
                sess.registry
            );
            for (i, cmd) in crate::cmds::show_commands(&snippet).iter().enumerate() {
                println!("  {:>2}. {}", i + 1, escape_display(cmd));
            }
        }
        if !prompt::stdin_is_tty() {
            return Err(CliError::refused(format!(
                "forgehub: not confirmed\n  what: a local snippet named `{local_name}` already exists and pulling would overwrite it\n  fix: pass --force to overwrite, or --as <name> to save it under a different name"
            ))
            .into());
        }
        if !prompt::ask_yes_no(
            stdin,
            &format!("Overwrite local `{local_name}` with {owner}/{name} v{remote_version}?"),
        ) {
            println!("Pull cancelled; nothing was overwritten.");
            return Ok(2);
        }
    }
    let mut to_save = snippet;
    to_save.name.clone_from(&local_name);
    store.save(&to_save)?;
    save_meta(
        store,
        &local_name,
        &Meta {
            registry: sess.registry.clone(),
            owner: owner.clone(),
            name: name.clone(),
            version: remote_version,
            hash: remote_hash.clone(),
        },
    )?;
    let url = web_url(&sess.registry, &owner, &name);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "owner": owner,
                "name": name,
                "version": remote_version,
                "hash": remote_hash,
                "url": url,
                "saved_as": local_name,
            })
        );
    } else {
        let short = remote_hash.get(..8).unwrap_or(&remote_hash);
        println!("{owner}/{name} v{remote_version} sha256:{short} → saved as `{local_name}`");
        println!("  {url}");
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// Status + diff
// ---------------------------------------------------------------------------

/// The sync state of one local snippet against its remote.
#[derive(Debug, PartialEq, Eq)]
pub enum SyncState {
    /// Local and remote hashes are equal.
    Unchanged,
    /// Local changed since the last sync; remote did not.
    Ahead,
    /// Remote changed since the last sync; local did not.
    Behind,
    /// Both changed since the last sync (or no baseline exists).
    Diverged,
    /// No remote snippet exists.
    LocalOnly,
    /// No local snippet exists (remote-only listing entry).
    RemoteOnly,
}

impl SyncState {
    /// Stable label for text and JSON output.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            SyncState::Unchanged => "unchanged",
            SyncState::Ahead => "ahead",
            SyncState::Behind => "behind",
            SyncState::Diverged => "diverged",
            SyncState::LocalOnly => "local-only",
            SyncState::RemoteOnly => "remote-only",
        }
    }
}

/// Compare local vs remote using the sidecar baseline:
/// equal hashes → unchanged; only local moved → ahead; only
/// remote moved → behind; both moved (differently) → diverged.
/// Without a sidecar, equality is all that can be said: equal →
/// unchanged, different → diverged.
#[must_use]
pub fn sync_state(
    local_hash: Option<&str>,
    remote_hash: Option<&str>,
    meta: Option<&Meta>,
) -> SyncState {
    match (local_hash, remote_hash) {
        (None, Some(_)) => SyncState::RemoteOnly,
        (Some(l), Some(r)) if l == r => SyncState::Unchanged,
        (Some(l), Some(r)) => match meta {
            Some(m) => {
                let local_moved = l != m.hash;
                let remote_moved = r != m.hash;
                match (local_moved, remote_moved) {
                    (true, false) => SyncState::Ahead,
                    (false, true) => SyncState::Behind,
                    _ => SyncState::Diverged,
                }
            }
            None => SyncState::Diverged,
        },
        // No remote (with or without a local copy): nothing to
        // sync against.
        _ => SyncState::LocalOnly,
    }
}

struct StatusRow {
    name: String,
    owner: String,
    state: SyncState,
    local_version: Option<i64>,
    remote_version: Option<i64>,
}

fn status_row(
    store: &Store,
    sess: &Session,
    local_name: &str,
    owner: &str,
    remote_name: &str,
) -> Result<StatusRow> {
    let local_hash = if store.exists(local_name) {
        let snippet = store.load(local_name)?;
        Some(forge_core::hash::content_hash(&snippet))
    } else {
        None
    };
    let meta = load_meta(store, local_name);
    let remote = match sess.client.get(
        &format!("/snippets/{owner}/{remote_name}"),
        Some(&sess.entry.token),
    ) {
        Ok((_, body, _)) => Some(body),
        Err(e) if e.status == 404 => None,
        Err(e) => return Err(api_fail(&e, &sess.entry.token).into()),
    };
    let (remote_hash, remote_version) = match &remote {
        Some(r) => (
            Some(r["hash"].as_str().unwrap_or_default().to_string()),
            r["version"].as_i64(),
        ),
        None => (None, None),
    };
    Ok(StatusRow {
        name: local_name.to_string(),
        owner: owner.to_string(),
        state: sync_state(local_hash.as_deref(), remote_hash.as_deref(), meta.as_ref()),
        local_version: meta.as_ref().map(|m| m.version),
        remote_version,
    })
}

/// `fg status [name]`.
pub fn cmd_status(
    store: &Store,
    stdin: &mut impl BufRead,
    name: Option<&str>,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let mut rows: Vec<StatusRow> = Vec::new();
    if let Some(n) = name {
        let meta = load_meta(store, n);
        let (owner, remote_name) = match &meta {
            Some(m) => (m.owner.clone(), m.name.clone()),
            None => (sess.username.clone(), n.to_string()),
        };
        rows.push(status_row(store, &sess, n, &owner, &remote_name)?);
    } else {
        let mut seen: Vec<String> = Vec::new();
        for snippet in store.list() {
            let meta = load_meta(store, &snippet.name);
            let (owner, remote_name) = match &meta {
                Some(m) => (m.owner.clone(), m.name.clone()),
                None => (sess.username.clone(), snippet.name.clone()),
            };
            seen.push(remote_name.clone());
            rows.push(status_row(
                store,
                &sess,
                &snippet.name,
                &owner,
                &remote_name,
            )?);
        }
        // Remote-only snippets (mine, not saved locally).
        let (_, mine, _) = sess
            .client
            .get("/me/snippets?limit=100&sort=name", Some(&sess.entry.token))
            .map_err(|e| api_fail(&e, &sess.entry.token))?;
        if let Some(arr) = mine["snippets"].as_array() {
            for s in arr {
                let rname = s["name"].as_str().unwrap_or_default().to_string();
                if !seen.contains(&rname) && !store.exists(&rname) {
                    rows.push(StatusRow {
                        name: rname,
                        owner: sess.username.clone(),
                        state: SyncState::RemoteOnly,
                        local_version: None,
                        remote_version: s["version"].as_i64(),
                    });
                }
            }
        }
    }
    if json {
        let arr: Vec<Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "owner": r.owner,
                    "state": r.state.label(),
                    "local_version": r.local_version,
                    "remote_version": r.remote_version,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "snippets": arr }));
        return Ok(0);
    }
    if rows.is_empty() {
        println!("No local snippets and nothing on {} either.", sess.registry);
        return Ok(0);
    }
    let width = rows.iter().map(|r| r.name.len()).max().unwrap_or(0);
    for r in &rows {
        println!(
            "{:<width$}  {:<11} (local v{}, remote v{})",
            r.name,
            r.state.label(),
            r.local_version
                .map_or_else(|| "—".into(), |v| v.to_string()),
            r.remote_version
                .map_or_else(|| "—".into(), |v| v.to_string()),
        );
    }
    Ok(0)
}

/// `fg diff <name>` — unified diff, local vs remote latest.
pub fn cmd_diff(
    store: &Store,
    stdin: &mut impl BufRead,
    name: &str,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let local = store.load(name)?;
    let local_yaml = local.to_yaml()?;
    let meta = load_meta(store, name);
    let (owner, remote_name) = match &meta {
        Some(m) => (m.owner.clone(), m.name.clone()),
        None => (sess.username.clone(), name.to_string()),
    };
    let remote = fetch_snippet(&sess, &owner, &remote_name, None)?;
    let remote_yaml = remote["body"].as_str().unwrap_or_default();
    let diff = similar::TextDiff::from_lines(remote_yaml, &local_yaml);
    let unified = diff
        .unified_diff()
        .header(
            &format!(
                "{owner}/{remote_name} v{}",
                remote["version"].as_i64().unwrap_or(0)
            ),
            &format!("local {name}"),
        )
        .to_string();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "owner": owner,
                "name": remote_name,
                "remote_version": remote["version"],
                "changed": remote["hash"].as_str() != Some(&forge_core::hash::content_hash(&local)),
                "diff": unified,
            })
        );
        return Ok(0);
    }
    if unified.trim().is_empty()
        || remote["hash"].as_str() == Some(&forge_core::hash::content_hash(&local))
    {
        println!("{owner}/{remote_name} and local `{name}` are identical.");
    } else {
        print!("{unified}");
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// Remote subcommands
// ---------------------------------------------------------------------------

fn print_snippet_table(snippets: &[Value], show_owner: bool) {
    if snippets.is_empty() {
        println!("(none)");
        return;
    }
    for s in snippets {
        let owner = s["owner"].as_str().unwrap_or_default();
        let name = s["name"].as_str().unwrap_or_default();
        let vis = s["visibility"].as_str().unwrap_or_default();
        let version = s["version"].as_i64().unwrap_or(0);
        let desc = s["description"].as_str().unwrap_or_default();
        if show_owner {
            println!(
                "{owner}/{name}  [{vis}] v{version}  {}",
                escape_display(desc)
            );
        } else {
            println!("{name}  [{vis}] v{version}  {}", escape_display(desc));
        }
    }
}

fn snippet_page(body: &Value) -> Vec<Value> {
    body["snippets"].as_array().cloned().unwrap_or_default()
}

/// `fg remote list [--mine|--stars]`.
pub fn cmd_remote_list(
    store: &Store,
    stdin: &mut impl BufRead,
    stars: bool,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let path = if stars {
        "/me/stars?limit=100"
    } else {
        "/me/snippets?limit=100&sort=name"
    };
    let (_, body, _) = sess
        .client
        .get(path, Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    let snippets = snippet_page(&body);
    if json {
        println!("{}", serde_json::json!({ "snippets": snippets }));
        return Ok(0);
    }
    if stars {
        println!("Starred by {}:", sess.username);
        print_snippet_table(&snippets, true);
    } else {
        println!("{}'s snippets on {}:", sess.username, sess.registry);
        print_snippet_table(&snippets, false);
    }
    Ok(0)
}

/// `fg remote show <owner/name>`.
pub fn cmd_remote_show(
    store: &Store,
    stdin: &mut impl BufRead,
    reference: &str,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let (owner, name) = parse_reference(reference, &sess.username);
    let remote = fetch_snippet(&sess, &owner, &name, None)?;
    if json {
        println!("{remote}");
        return Ok(0);
    }
    println!(
        "{}/{} — {}",
        escape_display(&owner),
        escape_display(&name),
        escape_display(remote["description"].as_str().unwrap_or_default())
    );
    println!(
        "  visibility: {} · v{} · sha256:{}",
        remote["visibility"].as_str().unwrap_or_default(),
        remote["version"].as_i64().unwrap_or(0),
        remote["hash"]
            .as_str()
            .unwrap_or_default()
            .get(..8)
            .unwrap_or_default()
    );
    println!(
        "  stars: {} · forks: {} · pulls: {}",
        remote["star_count"].as_i64().unwrap_or(0),
        remote["fork_count"].as_i64().unwrap_or(0),
        remote["pull_count"].as_i64().unwrap_or(0)
    );
    if let Some(tags) = remote["tags"].as_array() {
        if !tags.is_empty() {
            let tags: Vec<&str> = tags.iter().filter_map(Value::as_str).collect();
            println!("  tags: {}", tags.join(", "));
        }
    }
    if let Ok(snippet) = forge_core::Snippet::from_yaml(remote["body"].as_str().unwrap_or_default())
    {
        println!("  commands:");
        for (i, cmd) in crate::cmds::show_commands(&snippet).iter().enumerate() {
            println!("  {:>2}. {}", i + 1, escape_display(cmd));
        }
    }
    Ok(0)
}

/// `fg remote delete <name>` — confirmation, then soft-delete.
pub fn cmd_remote_delete(
    store: &Store,
    stdin: &mut impl BufRead,
    name: &str,
    yes: bool,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let remote = fetch_snippet(&sess, &sess.username.clone(), name, None)?;
    if !yes {
        if !prompt::stdin_is_tty() {
            return Err(CliError::refused(format!(
                "forgehub: not confirmed\n  what: deleting {}/{name} v{} needs confirmation\n  fix: pass --yes (-y) to delete without prompting",
                sess.username,
                remote["version"].as_i64().unwrap_or(0)
            ))
            .into());
        }
        println!(
            "This will delete {}/{} ({} visibility, v{}, {} stars). It can be restored by an admin for 30 days.",
            sess.username,
            name,
            remote["visibility"].as_str().unwrap_or_default(),
            remote["version"].as_i64().unwrap_or(0),
            remote["star_count"].as_i64().unwrap_or(0)
        );
        if !prompt::ask_yes_no(stdin, "Delete it?") {
            println!("Delete cancelled.");
            return Ok(2);
        }
    }
    sess.client
        .delete(
            &format!("/snippets/{}/{name}", sess.username),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    // The sidecar baseline no longer refers to anything.
    if load_meta(store, name).is_some_and(|m| m.name == name && m.owner == sess.username) {
        store.remove_meta(name);
    }
    if json {
        println!(
            "{}",
            serde_json::json!({ "owner": sess.username, "name": name, "deleted": true })
        );
    } else {
        println!("Deleted {}/{name}.", sess.username);
    }
    Ok(0)
}

/// `fg remote visibility <name> <public|unlisted|private>`.
pub fn cmd_remote_visibility(
    store: &Store,
    stdin: &mut impl BufRead,
    name: &str,
    visibility: &str,
    yes: bool,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    if !["public", "unlisted", "private"].contains(&visibility) {
        return Err(CliError::failure(format!(
            "forgehub: unknown visibility `{visibility}`\n  what: expected public, unlisted or private\n  fix: fg remote visibility {name} public"
        ))
        .into());
    }
    let sess = session(store, stdin, registry_flag)?;
    if visibility == "public" && !yes {
        if !prompt::stdin_is_tty() {
            return Err(CliError::refused(
                "forgehub: not confirmed\n  what: making a snippet public needs confirmation\n  fix: review it for secrets first (`fg show <name>`), then pass --yes (-y)",
            )
            .into());
        }
        println!(
            "Making {}/{name} public means anyone can read and run it. Review it for secrets, tokens and internal hostnames first.",
            sess.username
        );
        if !prompt::ask_yes_no(stdin, "Make it public?") {
            println!("Visibility unchanged.");
            return Ok(2);
        }
    }
    let (_, updated, _) = sess
        .client
        .request(
            "PATCH",
            &format!("/snippets/{}/{name}", sess.username),
            Some(&serde_json::json!({ "visibility": visibility })),
            Some(&sess.entry.token),
            &[],
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if json {
        println!("{updated}");
    } else {
        println!("{}/{name} is now {visibility}.", sess.username);
    }
    Ok(0)
}

/// `fg remote search <query>`.
pub fn cmd_remote_search(
    store: &Store,
    stdin: &mut impl BufRead,
    query: &str,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let encoded = url_encode(query);
    let (_, body, _) = sess
        .client
        .get(
            &format!("/search?q={encoded}&limit=20"),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    let results = body["results"].as_array().cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::json!({ "results": results }));
        return Ok(0);
    }
    if results.is_empty() {
        println!("No public snippets match {query:?}.");
        return Ok(0);
    }
    for r in &results {
        let s = &r["snippet"];
        println!(
            "{}/{}  ★{}  {}",
            s["owner"].as_str().unwrap_or_default(),
            s["name"].as_str().unwrap_or_default(),
            s["star_count"].as_i64().unwrap_or(0),
            escape_display(s["description"].as_str().unwrap_or_default())
        );
    }
    Ok(0)
}

/// Minimal percent-encoding for a query value.
fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Fork
// ---------------------------------------------------------------------------

/// `fg fork <owner/name> [--as name]` — server fork, then an
/// optional pull (prompted on a TTY).
pub fn cmd_fork(
    store: &Store,
    stdin: &mut impl BufRead,
    reference: &str,
    as_name: Option<&str>,
    registry_flag: Option<&str>,
    json: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry_flag)?;
    let (owner, name) = parse_reference(reference, &sess.username);
    let mut body = serde_json::json!({});
    if let Some(n) = as_name {
        body["name"] = Value::String(n.to_string());
    }
    let (_, forked, _) = sess
        .client
        .post_json(
            &format!("/snippets/{owner}/{name}/fork"),
            &body,
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    let fork_name = forked["name"].as_str().unwrap_or_default().to_string();
    let fork_version = forked["version"].as_i64().unwrap_or(1);
    let fork_hash = forked["hash"].as_str().unwrap_or_default().to_string();
    let url = web_url(&sess.registry, &sess.username, &fork_name);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "owner": sess.username,
                "name": fork_name,
                "version": fork_version,
                "hash": fork_hash,
                "url": url,
                "forked_from": { "owner": owner, "name": name },
            })
        );
        return Ok(0);
    }
    let short = fork_hash.get(..8).unwrap_or(&fork_hash);
    println!(
        "Forked {owner}/{name} → {}/{fork_name} v{fork_version} sha256:{short} (private)",
        sess.username
    );
    println!("  {url}");
    if prompt::stdin_is_tty()
        && prompt::ask_yes_no(
            stdin,
            &format!("Pull {fork_name} to your local library now?"),
        )
    {
        return cmd_pull(
            store,
            stdin,
            &format!("{}/{fork_name}", sess.username),
            None,
            None,
            false,
            false,
            registry_flag,
            json,
        );
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// record --push
// ---------------------------------------------------------------------------

/// Whether `record --push` should offer the push, and how.
#[derive(Debug, PartialEq, Eq)]
pub enum RecordPush {
    /// Not logged in: print a note and skip.
    SkipNotLoggedIn,
    /// Non-interactive without -y: cannot ask; skip with a note.
    SkipNonInteractive,
    /// Ask `[y/N]`.
    Ask,
}

/// The pure decision behind `record --push`.
#[must_use]
pub fn record_push_decision(logged_in: bool, is_tty: bool, yes: bool) -> RecordPush {
    if !logged_in {
        return RecordPush::SkipNotLoggedIn;
    }
    if yes || is_tty {
        return RecordPush::Ask;
    }
    RecordPush::SkipNonInteractive
}

/// The `record --push` hook: after a successful save, offer to
/// push (only when logged in, per the roadmap).
pub fn after_record_push(
    store: &Store,
    stdin: &mut impl BufRead,
    name: &str,
    yes: bool,
    json: bool,
) -> Result<i32> {
    let logged_in = matches!(active_credential(store, stdin, None), Ok(Some(_)));
    match record_push_decision(logged_in, prompt::stdin_is_tty(), yes) {
        RecordPush::SkipNotLoggedIn => {
            if !json {
                println!("(Not logged in — skipping the cloud push. `fg login` enables it.)");
            }
            Ok(0)
        }
        RecordPush::SkipNonInteractive => {
            if !json {
                println!("(Non-interactive — not pushing. Run `fg push {name}` when ready.)");
            }
            Ok(0)
        }
        RecordPush::Ask => {
            if !yes && !prompt::ask_yes_no(stdin, "Push to cloud now?") {
                return Ok(0);
            }
            let sess = session(store, stdin, None)?;
            let outcome = push_one(store, &sess, name, None, None, None)?;
            print_push_outcome(&sess.registry, &outcome, json);
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_parsing() {
        assert_eq!(
            parse_reference("anna/docker-x", "me"),
            ("anna".into(), "docker-x".into())
        );
        assert_eq!(
            parse_reference("docker-x", "me"),
            ("me".into(), "docker-x".into())
        );
    }

    #[test]
    fn sync_state_matrix() {
        let meta = Meta {
            registry: "r".into(),
            owner: "a".into(),
            name: "n".into(),
            version: 2,
            hash: "base".into(),
        };
        assert_eq!(
            sync_state(Some("base"), Some("base"), Some(&meta)),
            SyncState::Unchanged
        );
        assert_eq!(
            sync_state(Some("new-local"), Some("base"), Some(&meta)),
            SyncState::Ahead
        );
        assert_eq!(
            sync_state(Some("base"), Some("new-remote"), Some(&meta)),
            SyncState::Behind
        );
        assert_eq!(
            sync_state(Some("x"), Some("y"), Some(&meta)),
            SyncState::Diverged
        );
        assert_eq!(sync_state(Some("x"), Some("x"), None), SyncState::Unchanged);
        assert_eq!(sync_state(Some("x"), Some("y"), None), SyncState::Diverged);
        assert_eq!(
            sync_state(Some("x"), None, Some(&meta)),
            SyncState::LocalOnly
        );
        assert_eq!(sync_state(None, Some("x"), None), SyncState::RemoteOnly);
    }

    #[test]
    fn record_push_decisions() {
        assert_eq!(
            record_push_decision(false, true, false),
            RecordPush::SkipNotLoggedIn
        );
        assert_eq!(
            record_push_decision(true, false, false),
            RecordPush::SkipNonInteractive
        );
        assert_eq!(record_push_decision(true, false, true), RecordPush::Ask);
        assert_eq!(record_push_decision(true, true, false), RecordPush::Ask);
    }

    #[test]
    fn url_encoding() {
        assert_eq!(url_encode("docker cleanup"), "docker%20cleanup");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }
}
