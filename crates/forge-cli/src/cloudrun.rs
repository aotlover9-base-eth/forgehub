//! Run from cloud (Step 13) - the headline flow with the full
//! safety UX:
//!
//! - Resolution: explicit `owner/name[@version]`, else the
//!   caller's own cloud snippet for a bare name (local snippets
//!   win earlier, in `cmds::cmd_run`).
//! - The fetched body is parsed and validated with forge-core
//!   and its locally computed hash MUST equal the server's hash
//!   (mismatch = hard error, nothing runs).
//! - Confirmation follows the trust model: own snippets
//!   `[y/N]`; other people's need a typed `yes` until the exact
//!   (owner, name, hash) is in `trust.db`; a changed hash shows
//!   a diff and re-confirms, and `-y` never bypasses a typed-yes
//!   or a changed-hash confirmation.
//! - Non-interactive stdin fails safe: card printed, exit 2.
//! - Every fetched immutable version is cached for offline
//!   replay (`--offline`) and as the diff baseline.

use std::io::BufRead;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::cloud::active_credential;
use crate::cmds::{escape_display, resolve_or_prompt};
use crate::exit::CliError;
use crate::prompt;
use crate::runner;
use crate::store::Store;
use crate::sync::{api_fail, session, Session};
use crate::trust::{cache_body, cache_latest, cache_put, TrustDb};

/// A parsed run reference: `name`, `owner/name`, with an
/// optional `@version` pin on either.
#[derive(Debug, PartialEq, Eq)]
pub struct RunRef {
    /// Explicit owner, when given.
    pub owner: Option<String>,
    /// Snippet name.
    pub name: String,
    /// Version pin from `@N`, when given.
    pub version: Option<i64>,
}

/// Parse a run reference. A `@N` suffix (digits only) is a
/// version pin; anything else after `@` is a validation error
/// surfaced by the caller.
pub fn parse_reference(text: &str) -> RunRef {
    let (head, version) = match text.rsplit_once('@') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            (head, tail.parse::<i64>().ok())
        }
        _ => (text, None),
    };
    match head.split_once('/') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() => RunRef {
            owner: Some(owner.to_string()),
            name: name.to_string(),
            version,
        },
        _ => RunRef {
            owner: None,
            name: head.to_string(),
            version,
        },
    }
}

/// A fetched (or cache-recovered) snippet ready to run.
pub struct Fetched {
    /// Owner username.
    pub owner: String,
    /// Snippet name.
    pub name: String,
    /// Version number.
    pub version: i64,
    /// Content hash (locally verified).
    pub hash: String,
    /// Visibility.
    pub visibility: String,
    /// Canonical forge-file body.
    pub body: String,
    /// The parsed snippet.
    pub snippet: forge_core::Snippet,
    /// Star count (0 when from cache).
    pub star_count: i64,
    /// Pull count (0 when from cache).
    pub pull_count: i64,
    /// True when served from the local cache (`--offline`).
    pub from_cache: bool,
}

/// Fetch one version with ONE request (`intent=run`), verify
/// its hash locally, and cache the immutable body.
fn fetch_and_cache(
    store: &Store,
    sess: &Session,
    owner: &str,
    name: &str,
    pin: Option<i64>,
    no_cache: bool,
) -> Result<Fetched> {
    let mut path = format!("/snippets/{owner}/{name}?intent=run");
    if let Some(v) = pin {
        use std::fmt::Write as _;
        let _ = write!(path, "&version={v}");
    }
    let (_, body, _) = sess
        .client
        .get(&path, Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    let text = body["body"].as_str().unwrap_or_default().to_string();
    let fetched = verify_body(owner, name, &body, &text, false)?;
    if !no_cache {
        let _ = cache_put(
            store,
            owner,
            name,
            fetched.version,
            &fetched.hash,
            &fetched.visibility,
            &text,
        );
    }
    Ok(fetched)
}

/// Parse + validate + hash-verify a fetched body.
fn verify_body(
    owner: &str,
    name: &str,
    body: &Value,
    text: &str,
    from_cache: bool,
) -> Result<Fetched> {
    let server_hash = body["hash"].as_str().unwrap_or_default().to_string();
    let snippet = forge_core::Snippet::from_yaml(text).map_err(|e| {
        CliError::failure(format!(
            "forgehub: the fetched snippet is not a valid forge file\n  what: {e}\n  fix: nothing was run; report the problem to the snippet's owner"
        ))
    })?;
    snippet.validate().map_err(|e| {
        CliError::failure(format!(
            "forgehub: the fetched snippet fails validation\n  what: {e}\n  fix: nothing was run"
        ))
    })?;
    let local_hash = forge_core::hash::content_hash(&snippet);
    if local_hash != server_hash {
        return Err(CliError::failure(format!(
            "forgehub: hash mismatch - refusing to run\n  what: the server claims sha256:{} but the received body hashes to sha256:{}\n  why: the content may have been altered in transit or the server is misbehaving\n  fix: do not run this snippet; try again later or contact the registry operator",
            server_hash.get(..12).unwrap_or(&server_hash),
            local_hash.get(..12).unwrap_or(&local_hash),
        ))
        .into());
    }
    Ok(Fetched {
        owner: owner.to_string(),
        name: name.to_string(),
        version: body["version"].as_i64().unwrap_or(0),
        hash: local_hash,
        visibility: body["visibility"].as_str().unwrap_or("private").to_string(),
        body: text.to_string(),
        snippet,
        star_count: body["star_count"].as_i64().unwrap_or(0),
        pull_count: body["pull_count"].as_i64().unwrap_or(0),
        from_cache,
    })
}

/// Recover a snippet from the cache for `--offline`.
fn fetch_offline(store: &Store, owner: &str, name: &str, pin: Option<i64>) -> Result<Fetched> {
    let cached = cache_latest(store, owner, name).ok_or_else(|| {
        CliError::failure(format!(
            "forgehub: nothing cached for {owner}/{name}\n  what: --offline can only replay snippets you have fetched before\n  fix: run once online (without --offline) to populate the cache"
        ))
    })?;
    let (version, hash, visibility, body_text) = match pin {
        Some(v) => {
            let text = cache_body(store, owner, name, v).ok_or_else(|| {
                CliError::failure(format!(
                    "forgehub: version {v} of {owner}/{name} is not in the cache\n  fix: fetch it once online"
                ))
            })?;
            let snippet = forge_core::Snippet::from_yaml(&text).map_err(|e| {
                CliError::failure(format!("forgehub: cached body no longer parses: {e}"))
            })?;
            (
                v,
                forge_core::hash::content_hash(&snippet),
                cached.visibility.clone(),
                text,
            )
        }
        None => (
            cached.version,
            cached.hash.clone(),
            cached.visibility.clone(),
            cached.body.clone(),
        ),
    };
    let body = serde_json::json!({
        "body": body_text,
        "hash": hash,
        "version": version,
        "visibility": visibility,
    });
    verify_body(owner, name, &body, &body_text, true)
}

// ---------------------------------------------------------------------------
// Trust decisions
// ---------------------------------------------------------------------------

/// How a run must be confirmed.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfirmKind {
    /// A `[y/N]` prompt; `-y` may skip it.
    YesNo,
    /// A typed `yes`; `-y` never skips it.
    TypedYes,
}

/// The pure trust decision: given ownership, the trusted hash
/// on record (if any) and the current hash, decide the
/// confirmation kind and whether the content changed since the
/// recorded approval.
#[must_use]
pub fn confirm_kind(
    own: bool,
    trusted_hash: Option<&str>,
    current_hash: &str,
) -> (ConfirmKind, bool) {
    let changed = trusted_hash.is_some_and(|h| h != current_hash);
    if changed {
        // Any change re-confirms from scratch: own → [y/N]
        // (but -y cannot skip), others → typed yes.
        return if own {
            (ConfirmKind::YesNo, true)
        } else {
            (ConfirmKind::TypedYes, true)
        };
    }
    if own {
        return (ConfirmKind::YesNo, false);
    }
    if trusted_hash == Some(current_hash) {
        return (ConfirmKind::YesNo, false);
    }
    (ConfirmKind::TypedYes, false)
}

/// Whether `-y` may skip this confirmation: only a plain
/// `[y/N]` for unchanged content.
#[must_use]
pub fn flag_yes_may_skip(kind: &ConfirmKind, changed: bool) -> bool {
    matches!(kind, ConfirmKind::YesNo) && !changed
}

// ---------------------------------------------------------------------------
// Card rendering
// ---------------------------------------------------------------------------

/// Wrap one display line at `width` columns (char-based, with a
/// continuation indent) - long lines are wrapped, never
/// truncated, so nothing can hide past the edge of the screen.
fn wrap_line(line: &str, width: usize, indent: &str) -> Vec<String> {
    if line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in line.split(' ') {
        if current.is_empty() {
            current = word.to_string();
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(std::mem::take(&mut current));
            current = format!("{indent}{word}");
        }
        // A single word longer than the width is hard-wrapped.
        while current.chars().count() > width {
            let head: String = current.chars().take(width).collect();
            out.push(head);
            current = format!(
                "{indent}{}",
                current.chars().skip(width).collect::<String>()
            );
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The confirmation card for a cloud run (roadmap §4 shape):
/// identity, provenance, EVERY command numbered and escaped,
/// lint warnings, and the variables that will be prompted.
pub fn render_card(fetched: &Fetched, own: bool, changed: bool, diff: Option<&str>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let short = fetched.hash.get(..8).unwrap_or(&fetched.hash);
    let _ = writeln!(
        out,
        "About to run `{}` (cloud: {}/{})",
        escape_display(&fetched.name),
        escape_display(&fetched.owner),
        escape_display(&fetched.name)
    );
    if !fetched.snippet.description.is_empty() {
        let _ = writeln!(
            out,
            "  {}",
            escape_display(fetched.snippet.description.as_str())
        );
    }
    let _ = writeln!(
        out,
        "  version {} · sha256:{} · {} · mode: {}",
        fetched.version,
        short,
        fetched.visibility,
        match fetched.snippet.effective_mode() {
            forge_core::Mode::Script => "script",
            forge_core::Mode::Steps => "steps",
        }
    );
    if fetched.from_cache {
        out.push_str("  (from the local cache - offline replay)\n");
    }
    if !own {
        let _ = writeln!(
            out,
            "  ⚠ This snippet is not yours. Owner: {} · ★ {} · {} pulls",
            escape_display(&fetched.owner),
            fetched.star_count,
            fetched.pull_count
        );
    }
    if changed {
        out.push_str(
            "  ⚠ This snippet CHANGED since you last ran it. Review the diff below carefully.\n",
        );
        if let Some(diff) = diff {
            for line in diff.lines() {
                // The diff is attacker-controlled text at the
                // most security-critical display in the product:
                // escape it like every other command line
                // (Step 19, T-EXEC-3).
                let _ = writeln!(out, "  {}", escape_display(line));
            }
        }
    }
    let warnings = forge_core::lint::lint_snippet(&fetched.snippet);
    if !warnings.is_empty() {
        out.push_str("  Warnings:\n");
        for w in &warnings {
            let _ = writeln!(out, "    line {}: {}", w.line, w.kind.message());
        }
    }
    out.push_str("  Commands:\n");
    for (i, cmd) in crate::cmds::show_commands(&fetched.snippet)
        .iter()
        .enumerate()
    {
        let escaped = escape_display(cmd);
        for (j, part) in wrap_line(&escaped, 100, "      ").iter().enumerate() {
            if j == 0 {
                let _ = writeln!(out, "  {:>2}. {part}", i + 1);
            } else {
                let _ = writeln!(out, "      {part}");
            }
        }
    }
    let vars = fetched.snippet.referenced_variables();
    if !vars.is_empty() {
        let _ = writeln!(out, "  Variables that may be prompted: {}", vars.join(", "));
    }
    out
}

// ---------------------------------------------------------------------------
// The run itself
// ---------------------------------------------------------------------------

/// Options for a cloud run (mirrors the `run` flags).
#[allow(clippy::struct_excessive_bools)]
pub struct CloudRunArgs {
    /// Skip confirmation where the trust model allows it.
    pub yes: bool,
    /// Print the card only.
    pub dry_run: bool,
    /// JSON output (for --dry).
    pub json: bool,
    /// Step-through execution.
    pub step: bool,
    /// Do not write the run cache.
    pub no_cache: bool,
    /// Cache-only mode.
    pub offline: bool,
    /// `KEY=value` assignments.
    pub assignments: Vec<String>,
}

/// The logged-in username when a credential exists, resolved
/// WITHOUT network access (stored username, else config).
fn local_username(store: &Store, stdin: &mut impl BufRead) -> Option<String> {
    if let Ok(Some((_, entry))) = active_credential(store, stdin, None) {
        if !entry.username.is_empty() {
            return Some(entry.username);
        }
    }
    store.load_config().ok().and_then(|c| c.username)
}

/// Run a cloud snippet end to end. `own_probe` distinguishes
/// the bare-name path (owner = me) from explicit references.
pub fn run_cloud(
    store: &Store,
    stdin: &mut impl BufRead,
    refr: &RunRef,
    args: &CloudRunArgs,
) -> Result<i32> {
    let my_name = local_username(store, stdin);
    let fetched = if args.offline {
        let owner = refr
            .owner
            .clone()
            .or_else(|| my_name.clone())
            .ok_or_else(|| {
                CliError::failure(
                    "forgehub: --offline needs an owner\n  what: a bare name cannot be resolved to a cache entry without knowing whose snippet it is\n  fix: use owner/name, or log in once so your username is known",
                )
            })?;
        fetch_offline(store, &owner, &refr.name, refr.version)?
    } else {
        let sess = session(store, stdin, None)?;
        let owner = refr.owner.clone().unwrap_or_else(|| sess.username.clone());
        if !args.json {
            eprintln!(
                "forgehub: fetching {owner}/{} from {}",
                refr.name, sess.registry
            );
        }
        fetch_and_cache(
            store,
            &sess,
            &owner,
            &refr.name,
            refr.version,
            args.no_cache,
        )?
    };
    run_fetched(store, stdin, &fetched, my_name.as_deref(), args)
}

/// The changed-hash diff: the previously cached body for the
/// trusted version against the current body, when the baseline
/// is still in the cache.
fn changed_diff(
    store: &Store,
    fetched: &Fetched,
    trusted: Option<&crate::trust::TrustRecord>,
) -> Option<String> {
    let t = trusted?;
    let old = cache_body(store, &fetched.owner, &fetched.name, t.version)?;
    let d = similar::TextDiff::from_lines(&old, &fetched.body);
    Some(
        d.unified_diff()
            .header(
                &format!(
                    "{}/{} v{} (last run)",
                    fetched.owner, fetched.name, t.version
                ),
                &format!(
                    "{}/{} v{} (now)",
                    fetched.owner, fetched.name, fetched.version
                ),
            )
            .to_string(),
    )
}

/// The confirmation gate: `Ok(true)` = proceed, `Ok(false)` =
/// declined (the caller exits 2), `Err(refused)` = the non-TTY
/// fail-safe. `-y` skips only where the trust model allows.
#[allow(clippy::too_many_arguments)]
fn confirm_run(
    stdin: &mut impl BufRead,
    fetched: &Fetched,
    own: bool,
    kind: &ConfirmKind,
    changed: bool,
    diff: Option<&str>,
    yes_flag: bool,
) -> Result<bool> {
    if yes_flag && flag_yes_may_skip(kind, changed) {
        return Ok(true);
    }
    if !prompt::stdin_is_tty() {
        // Fail safe: show the card, run nothing.
        eprint!("{}", render_card(fetched, own, changed, diff));
        return Err(CliError::refused(format!(
            "forgehub: not confirmed\n  what: `{}/{}` was not run\n  why: there is no terminal to confirm on{}\n  fix: run it once interactively to approve this exact content{}",
            fetched.owner,
            fetched.name,
            if changed {
                " and the snippet changed since your last run"
            } else if !own {
                " and someone else's snippet needs a typed confirmation the first time"
            } else {
                ""
            },
            if own && !changed {
                ", or pass --yes (-y) for your own unchanged snippets"
            } else {
                ""
            },
        ))
        .into());
    }
    eprint!("{}", render_card(fetched, own, changed, diff));
    let confirmed = match kind {
        ConfirmKind::YesNo => prompt::ask_yes_no(stdin, "Run this on this machine?"),
        ConfirmKind::TypedYes => prompt::ask_typed_yes(stdin, "Run this on this machine?"),
    };
    if !confirmed {
        eprintln!("forgehub: not run (declined)");
    }
    Ok(confirmed)
}

/// Everything after the fetch: trust, card, confirm, execute.
pub fn run_fetched(
    store: &Store,
    stdin: &mut impl BufRead,
    fetched: &Fetched,
    my_username: Option<&str>,
    args: &CloudRunArgs,
) -> Result<i32> {
    let own = my_username == Some(fetched.owner.as_str());
    let trust_db = TrustDb::open(&store.trust_db_path())?;
    let trusted = trust_db.get(&fetched.owner, &fetched.name)?;
    let trusted_hash = trusted.as_ref().map(|t| t.hash.as_str());
    let (kind, changed) = confirm_kind(own, trusted_hash, &fetched.hash);
    let diff = if changed {
        changed_diff(store, fetched, trusted.as_ref())
    } else {
        None
    };
    let cli_values = forge_core::vars::parse_assignments(&args.assignments)?;
    let values = resolve_or_prompt(&fetched.snippet, &cli_values, stdin, prompt::stdin_is_tty())?;
    if args.dry_run {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "name": fetched.name,
                    "owner": fetched.owner,
                    "version": fetched.version,
                    "hash": fetched.hash,
                    "source": "cloud",
                    "visibility": fetched.visibility,
                    "dry_run": true,
                    "commands": fetched.snippet.all_command_texts(),
                })
            );
        } else {
            print!("{}", render_card(fetched, own, changed, diff.as_deref()));
            println!("Dry run: nothing was executed.");
        }
        return Ok(0);
    }
    // Confirmation.
    if !confirm_run(
        stdin,
        fetched,
        own,
        &kind,
        changed,
        diff.as_deref(),
        args.yes,
    )? {
        return Ok(2);
    }
    // Execute exactly like a local run.
    let base = std::env::current_dir().context("reading the current directory")?;
    let history = crate::history::Db::open(&store.history_db_path()).ok();
    let run_id = history
        .as_ref()
        .and_then(|db| db.start_run(&fetched.name).ok());
    let started = std::time::Instant::now();
    let outcome = if args.step {
        runner::execute_stepwise(&fetched.snippet, &values, &base, &store.tmp_dir(), stdin)?
    } else {
        runner::execute(&fetched.snippet, &values, &base, &store.tmp_dir())?
    };
    if let (Some(db), Some(id)) = (&history, run_id) {
        let commands = u32::try_from(fetched.snippet.command_count()).unwrap_or(u32::MAX);
        let _ = db.finish_run(
            id,
            outcome.exit_code == 0,
            started.elapsed().as_millis(),
            commands,
        );
    }
    // The approval is recorded for the content that just ran.
    let _ = trust_db.record_run(
        &fetched.owner,
        &fetched.name,
        &fetched.hash,
        fetched.version,
    );
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "name": fetched.name,
                "owner": fetched.owner,
                "version": fetched.version,
                "hash": fetched.hash,
                "source": "cloud",
                "exit_code": outcome.exit_code,
                "executed": outcome.executed,
                "skipped": outcome.skipped,
            })
        );
    } else {
        runner::print_run_report(&outcome, started.elapsed(), &fetched.name, args.step);
    }
    Ok(outcome.exit_code)
}

// ---------------------------------------------------------------------------
// Suggestions + trust commands
// ---------------------------------------------------------------------------

/// Classic Levenshtein edit distance (for typo suggestions,
/// spec-cli §5: "similar local names by edit distance").
#[must_use]
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur: Vec<usize> = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Up to 3 suggestions for a missing name: similar local names
/// (edit distance ≤ 2, closest first; substring matches count
/// as distance 0), then (when logged in) top remote search
/// hits. Never run.
pub fn suggestions(store: &Store, stdin: &mut impl BufRead, name: &str) -> Vec<String> {
    let mut scored: Vec<(usize, String)> = store
        .names()
        .into_iter()
        .map(|n| {
            let d = if n.contains(name) || name.contains(n.as_str()) {
                0
            } else {
                edit_distance(&n, name)
            };
            (d, n)
        })
        .filter(|(d, _)| *d <= 2)
        .collect();
    scored.sort();
    let mut out: Vec<String> = scored.into_iter().map(|(_, n)| n).take(3).collect();
    if out.len() < 3 {
        if let Ok(sess) = session(store, stdin, None) {
            let encoded = name.replace(' ', "%20");
            if let Ok((_, body, _)) = sess.client.get(
                &format!("/search?q={encoded}&limit=3"),
                Some(&sess.entry.token),
            ) {
                if let Some(results) = body["results"].as_array() {
                    for r in results {
                        let hit = format!(
                            "{}/{}",
                            r["snippet"]["owner"].as_str().unwrap_or_default(),
                            r["snippet"]["name"].as_str().unwrap_or_default()
                        );
                        if !out.contains(&hit) {
                            out.push(hit);
                        }
                        if out.len() >= 3 {
                            break;
                        }
                    }
                }
            }
        }
    }
    out
}

/// `forgehub trust list`.
pub fn cmd_trust_list(store: &Store, json: bool) -> Result<i32> {
    let db = TrustDb::open(&store.trust_db_path())?;
    let records = db.list()?;
    if json {
        let arr: Vec<Value> = records
            .iter()
            .map(|r| {
                serde_json::json!({
                    "owner": r.owner,
                    "name": r.name,
                    "hash": r.hash,
                    "version": r.version,
                    "runs": r.runs,
                    "first_trusted_at": r.first_trusted_at,
                    "last_run_at": r.last_run_at,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "trusted": arr }));
        return Ok(0);
    }
    if records.is_empty() {
        println!("No trusted cloud snippets yet. Trust is recorded when you run one.");
        return Ok(0);
    }
    for r in &records {
        println!(
            "{}/{}  v{}  sha256:{}  ({} runs, trusted since {})",
            r.owner,
            r.name,
            r.version,
            r.hash.get(..8).unwrap_or(&r.hash),
            r.runs,
            r.first_trusted_at
        );
    }
    Ok(0)
}

/// `forgehub trust revoke <owner/name>`.
pub fn cmd_trust_revoke(store: &Store, reference: &str, json: bool) -> Result<i32> {
    let refr = parse_reference(reference);
    let owner = refr.owner.ok_or_else(|| {
        CliError::failure(format!(
            "forgehub: trust revoke needs an owner\n  what: `{reference}` has no owner\n  fix: fg trust revoke <owner>/<name>"
        ))
    })?;
    let db = TrustDb::open(&store.trust_db_path())?;
    let removed = db.revoke(&owner, &refr.name)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "owner": owner, "name": refr.name, "revoked": removed })
        );
    } else if removed {
        println!("Trust revoked for {}/{}.", owner, refr.name);
    } else {
        println!(
            "No trust recorded for {}/{} - nothing to revoke.",
            owner, refr.name
        );
    }
    Ok(0)
}

/// `forgehub trust <owner/name@hash|@version>` - approve an
/// exact piece of content without running it. The fetched
/// content is shown (card) and confirmed under the same rules
/// as a run; the approval is recorded only after confirmation.
pub fn cmd_trust_add(
    store: &Store,
    stdin: &mut impl BufRead,
    reference: &str,
    json: bool,
) -> Result<i32> {
    // Split the pin: @<digits> = version, @<hex> = hash.
    let (head, pin_hash, pin_version) = match reference.rsplit_once('@') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            (head.to_string(), None, tail.parse::<i64>().ok())
        }
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_hexdigit()) => {
            (head.to_string(), Some(tail.to_lowercase()), None)
        }
        _ => (reference.to_string(), None, None),
    };
    let refr = parse_reference(&head);
    let sess = session(store, stdin, None)?;
    let owner = refr.owner.clone().unwrap_or_else(|| sess.username.clone());
    let fetched = fetch_and_cache(store, &sess, &owner, &refr.name, pin_version, false)?;
    if let Some(want) = &pin_hash {
        if !fetched.hash.starts_with(want.as_str()) {
            return Err(CliError::failure(format!(
                "forgehub: hash mismatch\n  what: you asked to trust @{want} but {}/{} v{} hashes to {}\n  fix: check the hash with the snippet's owner; do not trust content you cannot verify",
                fetched.owner, fetched.name, fetched.version, fetched.hash
            ))
            .into());
        }
    }
    let own = sess.username == fetched.owner;
    if !prompt::stdin_is_tty() {
        eprint!("{}", render_card(&fetched, own, false, None));
        return Err(CliError::refused(
            "forgehub: not confirmed\n  what: trust needs a terminal to confirm on\n  fix: run this interactively once",
        )
        .into());
    }
    eprint!("{}", render_card(&fetched, own, false, None));
    let confirmed = if own {
        prompt::ask_yes_no(stdin, "Trust this exact content for future runs?")
    } else {
        prompt::ask_typed_yes(stdin, "Trust this exact content for future runs?")
    };
    if !confirmed {
        println!("Not trusted.");
        return Ok(2);
    }
    let db = TrustDb::open(&store.trust_db_path())?;
    db.record_trust(
        &fetched.owner,
        &fetched.name,
        &fetched.hash,
        fetched.version,
    )?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "owner": fetched.owner,
                "name": fetched.name,
                "version": fetched.version,
                "hash": fetched.hash,
                "trusted": true,
            })
        );
    } else {
        println!(
            "Trusted {}/{} v{} (sha256:{}). Future runs of this exact content will ask [y/N] instead of a typed yes.",
            fetched.owner,
            fetched.name,
            fetched.version,
            fetched.hash.get(..8).unwrap_or(&fetched.hash)
        );
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_parsing() {
        assert_eq!(
            parse_reference("anna/tool@3"),
            RunRef {
                owner: Some("anna".into()),
                name: "tool".into(),
                version: Some(3)
            }
        );
        assert_eq!(
            parse_reference("tool"),
            RunRef {
                owner: None,
                name: "tool".into(),
                version: None
            }
        );
        assert_eq!(
            parse_reference("tool@12"),
            RunRef {
                owner: None,
                name: "tool".into(),
                version: Some(12)
            }
        );
        assert_eq!(
            parse_reference("anna/tool"),
            RunRef {
                owner: Some("anna".into()),
                name: "tool".into(),
                version: None
            }
        );
    }

    #[test]
    fn trust_decision_matrix() {
        // Own, no record: [y/N].
        assert_eq!(confirm_kind(true, None, "h1"), (ConfirmKind::YesNo, false));
        // Own, same hash: [y/N].
        assert_eq!(
            confirm_kind(true, Some("h1"), "h1"),
            (ConfirmKind::YesNo, false)
        );
        // Own, changed: [y/N] but flagged changed (-y may not skip).
        let (k, changed) = confirm_kind(true, Some("h0"), "h1");
        assert_eq!(k, ConfirmKind::YesNo);
        assert!(changed);
        assert!(!flag_yes_may_skip(&k, changed));
        // Other, first time: typed yes.
        assert_eq!(
            confirm_kind(false, None, "h1"),
            (ConfirmKind::TypedYes, false)
        );
        // Other, exact hash trusted: [y/N], -y may skip.
        let (k, changed) = confirm_kind(false, Some("h1"), "h1");
        assert_eq!(k, ConfirmKind::YesNo);
        assert!(!changed);
        assert!(flag_yes_may_skip(&k, changed));
        // Other, changed: typed yes again.
        assert_eq!(
            confirm_kind(false, Some("h0"), "h1"),
            (ConfirmKind::TypedYes, true)
        );
        // Plain own [y/N]: -y may skip.
        assert!(flag_yes_may_skip(&ConfirmKind::YesNo, false));
        assert!(!flag_yes_may_skip(&ConfirmKind::TypedYes, false));
    }

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("hello", "hello"), 0);
        assert_eq!(edit_distance("helo", "hello"), 1);
        assert_eq!(edit_distance("hllo", "hello"), 1);
        assert_eq!(edit_distance("xyz", "hello"), 5);
        assert_eq!(edit_distance("", "ab"), 2);
    }

    #[test]
    fn changed_diff_in_card_is_escaped() {
        // Step 19 (T-EXEC-3): the changed-hash diff is the one
        // display an attacker most wants to corrupt - a hostile
        // added line must not smuggle raw ANSI/bidi into it.
        let snippet =
            forge_core::Snippet::from_yaml("name: tool\nmode: script\nscript: |\n  echo hi\n")
                .unwrap();
        let fetched = Fetched {
            owner: "mallory".into(),
            name: "tool".into(),
            version: 2,
            hash: "h2".into(),
            visibility: "public".into(),
            body: String::new(),
            snippet,
            star_count: 0,
            pull_count: 0,
            from_cache: false,
        };
        let diff = "+echo safe\n+\u{1b}[2K\rhidden-evil \u{202e}cmd";
        let card = render_card(&fetched, false, true, Some(diff));
        assert!(!card.contains('\u{1b}'), "card leaked raw ESC:\n{card}");
        assert!(!card.contains('\u{202e}'));
        assert!(card.contains("\\e"));
        assert!(card.contains("hidden-evil"));
    }

    #[test]
    fn confirmation_card_golden() {
        // Peak B3: the confirmation card is safety content - its
        // bytes are pinned so no refactor can quietly change what
        // the user approves. Any intentional change updates this
        // golden in the same commit, with the report saying so.
        let snippet = forge_core::Snippet::from_yaml(
            "name: deploy\ndescription: Deploy the app\nsteps:\n  - echo deploying v3\n  - mkdir -p /tmp/app && touch /tmp/app/ok\n",
        )
        .unwrap();
        let fetched = Fetched {
            owner: "alice".into(),
            name: "deploy".into(),
            version: 3,
            hash: "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".into(),
            visibility: "public".into(),
            body: String::new(),
            snippet,
            star_count: 12,
            pull_count: 34,
            from_cache: false,
        };
        let card = render_card(&fetched, false, false, None);
        assert_eq!(
            card,
            "About to run `deploy` (cloud: alice/deploy)\n  Deploy the app\n  version 3 · sha256:abcdef12 · public · mode: steps\n  \u{26a0} This snippet is not yours. Owner: alice · \u{2605} 12 · 34 pulls\n  Commands:\n   1. echo deploying v3\n   2. mkdir -p /tmp/app && touch /tmp/app/ok\n"
        );
    }

    #[test]
    fn wrapping_never_truncates() {
        let long = "word ".repeat(60);
        let wrapped = wrap_line(long.trim(), 40, "  ");
        assert!(wrapped.len() > 1);
        let rejoined: String = wrapped.join(" ");
        assert!(rejoined.contains("word"));
        assert_eq!(rejoined.matches("word").count(), 60);
        let one_word = "x".repeat(250);
        let wrapped = wrap_line(&one_word, 100, "");
        assert_eq!(
            wrapped.iter().map(|l| l.chars().count()).sum::<usize>(),
            250
        );
    }
}
