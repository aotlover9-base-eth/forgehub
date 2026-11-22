//! `fg history` and `fg rollback` (Peak C3): cloud version
//! history, and rolling a snippet back by publishing an old
//! version's content as a NEW version. History is never
//! rewritten - versions are immutable, a rollback is just
//! another version whose body happens to be an older one.
//!
//! Local-only snippets have no version history (they are
//! files); both commands say so plainly instead of failing
//! mysteriously.

use std::io::BufRead;

use anyhow::Result;
use serde_json::{json, Value};

use crate::exit::CliError;
use crate::output as out;
use crate::store::Store;
use crate::sync::{api_fail, render_diff, session, Session};

/// Split a reference: `name` (own) or `owner/name`.
fn split_ref(refr: &str, own_username: &str) -> (String, String) {
    match refr.split_once('/') {
        Some((owner, name)) => (owner.to_string(), name.to_string()),
        None => (own_username.to_string(), refr.to_string()),
    }
}

fn local_note(name: &str) -> i32 {
    println!("`{name}` is a local snippet - local files have no version history.");
    println!(
        "  tip: `fg push {name}` starts its cloud history; after that, `fg history {name}` works."
    );
    0
}

fn snippet_path(owner: &str, name: &str) -> String {
    format!("/snippets/{owner}/{name}")
}

/// GET a snippet's JSON (latest version view), mapping errors
/// the house way.
fn fetch_snippet(sess: &Session, owner: &str, name: &str) -> Result<Value> {
    let (_, body, _) = sess
        .client
        .get(&snippet_path(owner, name), Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    Ok(body)
}

/// `fg history <ref>`: the version list of a cloud snippet.
pub fn cmd_history(
    store: &Store,
    stdin: &mut impl BufRead,
    refr: &str,
    registry: Option<&str>,
    json_out: bool,
    limit: u32,
) -> Result<i32> {
    let sess = match session(store, stdin, registry) {
        Ok(s) => s,
        Err(e) => {
            // Logged out: a local snippet of this name gets the
            // honest explanation instead of an auth error.
            if !refr.contains('/') && store.exists(refr) {
                return Ok(local_note(refr));
            }
            return Err(e);
        }
    };
    let (owner, name) = split_ref(refr, &sess.username);
    // Existence (and read access) first, so a local-only name
    // gets the note rather than a bare 404.
    match fetch_snippet(&sess, &owner, &name) {
        Ok(_) => {}
        Err(e) => {
            if crate::exit::code_of(&e) == 3 && !refr.contains('/') && store.exists(&name) {
                return Ok(local_note(&name));
            }
            return Err(e);
        }
    }
    let (_, body, _) = sess
        .client
        .get(
            &format!("{}/versions?limit={limit}", snippet_path(&owner, &name)),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&body["versions"]).unwrap_or_default()
        );
        return Ok(0);
    }
    let versions = body["versions"].as_array().cloned().unwrap_or_default();
    println!("history of {owner}/{name} (by {owner}; newest first):");
    if versions.is_empty() {
        println!("  (no versions yet)");
        return Ok(0);
    }
    for (i, v) in versions.iter().enumerate() {
        let when = v["created_at"]
            .as_str()
            .unwrap_or_default()
            .replace('T', " ");
        let when = when.get(..16).unwrap_or(&when);
        let message = v["message"].as_str().unwrap_or_default();
        let current = if i == 0 { " (current)" } else { "" };
        if message.is_empty() {
            println!(
                "  v{}  {}  {}{}",
                v["version"].as_i64().unwrap_or_default(),
                when,
                v["short_hash"].as_str().unwrap_or_default(),
                current
            );
        } else {
            println!(
                "  v{}  {}  {}  {}{}",
                v["version"].as_i64().unwrap_or_default(),
                when,
                v["short_hash"].as_str().unwrap_or_default(),
                message,
                current
            );
        }
    }
    Ok(0)
}

/// `fg rollback <ref> <version>`: publish the target version's
/// content as a new version, after showing the diff and asking.
pub fn cmd_rollback(
    store: &Store,
    stdin: &mut impl BufRead,
    refr: &str,
    version: i64,
    registry: Option<&str>,
    yes: bool,
) -> Result<i32> {
    let sess = session(store, stdin, registry)?;
    let (owner, name) = split_ref(refr, &sess.username);
    if owner != sess.username {
        return Err(CliError::failure(format!(
            "forgehub: cannot roll back {owner}/{name}\n  what: rollback publishes a new version, and only the owner can publish\n  fix: roll back your own snippets (see `fg history`)"
        ))
        .into());
    }
    let latest = fetch_snippet(&sess, &owner, &name)?;
    let latest_version = latest["version"].as_i64().unwrap_or_default();
    let latest_hash = latest["hash"].as_str().unwrap_or_default().to_string();
    let (_, target, _) = sess
        .client
        .get(
            &format!("{}/versions/{version}", snippet_path(&owner, &name)),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    let target_hash = target["hash"].as_str().unwrap_or_default().to_string();
    if target_hash == latest_hash {
        println!(
            "{}",
            out::style().success(&format!(
                "{owner}/{name} is already at v{version}'s content - nothing to roll back"
            ))
        );
        return Ok(0);
    }
    // Show exactly what would change (current -> target).
    let (_, diff_body, _) = sess
        .client
        .get(
            &format!(
                "{}/diff?from={latest_version}&to={version}",
                snippet_path(&owner, &name)
            ),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    println!("Roll back {owner}/{name} from v{latest_version} to the content of v{version}?");
    println!(
        "  this publishes a NEW version (v{}) with v{version}'s content; history is not rewritten.",
        latest_version + 1
    );
    if let Some(diff) = diff_body["diff"].as_str() {
        for line in render_diff(diff, out::style()) {
            println!("{line}");
        }
    }
    if !yes {
        if !crate::prompt::stdin_is_tty() {
            return Err(CliError::refused(
                "forgehub: not confirmed\n  what: rollback needs confirmation and there is no terminal\n  fix: pass --yes (-y) if you have reviewed the diff",
            )
            .into());
        }
        if !crate::prompt::ask_yes_no(stdin, "Publish the rollback?") {
            println!("not rolled back");
            return Ok(2);
        }
    }
    let body = target["body"].as_str().unwrap_or_default().to_string();
    let (_, resp, _) = sess
        .client
        .request(
            "PUT",
            &snippet_path(&owner, &name),
            Some(&json!({
                "name": name,
                "body": body,
                "message": format!("Rollback to v{version}"),
            })),
            Some(&sess.entry.token),
            &[("If-Match", &latest_version.to_string())],
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    let new_version = resp["version"].as_i64().unwrap_or(latest_version + 1);
    println!(
        "{}",
        out::style().success(&format!(
            "rolled back {owner}/{name}: v{latest_version} -> v{new_version} (content of v{version})"
        ))
    );
    Ok(0)
}
