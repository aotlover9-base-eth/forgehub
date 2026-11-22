//! `fg collection ...` (Peak C1): manage cloud collections -
//! ordered snippet runbooks - and the collection fallback for
//! `fg run` (see `try_run_collection`).
//!
//! A collection never widens access: the server filters its
//! items through the snippet read rules for the caller, so the
//! CLI can only ever run what the caller could run directly.
//! HTTP errors arrive as `ApiFailure` from the client (any
//! status of 400 or more) and are mapped by `sync::api_fail`
//! like everywhere else.

use std::io::BufRead;

use anyhow::Result;
use serde_json::{json, Value};

use crate::cloudrun::{CloudRunArgs, RunRef};
use crate::exit::CliError;
use crate::output as out;
use crate::store::Store;
use crate::sync::{api_fail, session, Session};

/// Shared context for the collection subcommands (the global
/// flags every cloud command receives).
pub struct Args<'a> {
    /// The local store.
    pub store: &'a Store,
    /// `--json` output.
    pub json: bool,
    /// `-y`: skip the delete confirmation.
    pub yes: bool,
    /// `--registry` flag value, when given.
    pub registry: Option<&'a str>,
}

/// Split a collection reference: `name` (own) or `owner/name`.
fn split_ref(refr: &str, own_username: &str) -> (String, String) {
    match refr.split_once('/') {
        Some((owner, name)) => (owner.to_string(), name.to_string()),
        None => (own_username.to_string(), refr.to_string()),
    }
}

/// Split a snippet reference, which must be `owner/name`.
fn split_snippet_ref(refr: &str) -> Result<(String, String)> {
    refr.split_once('/')
        .filter(|(o, n)| !o.is_empty() && !n.is_empty() && !n.contains('/'))
        .map(|(o, n)| (o.to_string(), n.to_string()))
        .ok_or_else(|| {
            CliError::failure(format!(
                "forgehub: bad snippet reference `{refr}`\n  what: collection items are named owner/name\n  fix: pass the snippet as owner/name, e.g. anna/hello"
            ))
            .into()
        })
}

fn check_visibility(value: &str) -> Result<()> {
    match value {
        "private" | "unlisted" | "public" => Ok(()),
        other => Err(CliError::failure(format!(
            "forgehub: unknown visibility `{other}`\n  what: visibility must be private, unlisted or public\n  fix: fg collection create <name> --visibility private"
        ))
        .into()),
    }
}

/// Print a success line in the house style.
fn success(message: &str) {
    println!("{}", out::style().success(message));
}

fn collection_path(owner: &str, name: &str) -> String {
    format!("/collections/{owner}/{name}")
}

/// GET a collection detail body.
fn fetch_collection(sess: &Session, owner: &str, name: &str) -> Result<Value> {
    let (_, body, _) = sess
        .client
        .get(&collection_path(owner, name), Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    Ok(body)
}

fn print_collection_detail(body: &Value) {
    let owner = body["owner"].as_str().unwrap_or_default();
    let name = body["name"].as_str().unwrap_or_default();
    let full_ref = format!("{owner}/{name}");
    let mut pairs: Vec<(&str, &str)> = vec![
        ("collection", full_ref.as_str()),
        (
            "visibility",
            body["visibility"].as_str().unwrap_or_default(),
        ),
    ];
    if let Some(desc) = body["description"].as_str().filter(|d| !d.is_empty()) {
        pairs.push(("description", desc));
    }
    println!("{}", out::style().kv(&pairs));
    let items = body["items"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        println!("  (no snippets yet - add one with `fg collection add {name} <owner/name>`)");
        return;
    }
    println!("  items ({}):", items.len());
    for (i, item) in items.iter().enumerate() {
        let desc = item["description"].as_str().unwrap_or_default();
        let version = item["latest_version"].as_i64().unwrap_or_default();
        let vis = item["visibility"].as_str().unwrap_or_default();
        let refr = format!(
            "{}/{}",
            item["owner"].as_str().unwrap_or_default(),
            item["name"].as_str().unwrap_or_default()
        );
        if desc.is_empty() {
            println!("  {:>2}. {refr} (v{version}, {vis})", i + 1);
        } else {
            println!("  {:>2}. {refr} - {desc} (v{version}, {vis})", i + 1);
        }
    }
    if let Some(total) = body["total_items"].as_i64() {
        let visible = i64::try_from(items.len()).unwrap_or(i64::MAX);
        if total > visible {
            println!(
                "  ({} more item(s) are not readable by you and are hidden)",
                total - visible
            );
        }
    }
}

/// `fg collection list`.
pub fn cmd_list(ctx: &Args<'_>, stdin: &mut impl BufRead) -> Result<i32> {
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (_, body, _) = sess
        .client
        .get("/me/collections", Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if ctx.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&body["collections"]).unwrap_or_default()
        );
        return Ok(0);
    }
    let rows = body["collections"].as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        success("no collections yet - create one with `fg collection create <name>`");
        return Ok(0);
    }
    for row in &rows {
        let desc = row["description"].as_str().unwrap_or_default();
        let line = format!(
            "{} ({}, {} item(s))",
            row["name"].as_str().unwrap_or_default(),
            row["visibility"].as_str().unwrap_or_default(),
            row["item_count"].as_i64().unwrap_or_default(),
        );
        if desc.is_empty() {
            println!("{line}");
        } else {
            println!("{line} - {desc}");
        }
    }
    Ok(0)
}

/// `fg collection create <name>`.
pub fn cmd_create(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    name: &str,
    description: Option<&str>,
    visibility: &str,
) -> Result<i32> {
    check_visibility(visibility)?;
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (_, body, _) = sess
        .client
        .post_json(
            "/collections",
            &json!({
                "name": name,
                "description": description.unwrap_or(""),
                "visibility": visibility,
            }),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if ctx.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    } else {
        success(&format!(
            "created collection {}/{} ({})",
            sess.username, name, visibility
        ));
        println!("  add snippets with `fg collection add {name} <owner/name>`");
    }
    Ok(0)
}

/// `fg collection show <ref>`.
pub fn cmd_show(ctx: &Args<'_>, stdin: &mut impl BufRead, refr: &str) -> Result<i32> {
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = split_ref(refr, &sess.username);
    let body = fetch_collection(&sess, &owner, &name)?;
    if ctx.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    } else {
        print_collection_detail(&body);
    }
    Ok(0)
}

/// `fg collection add <collection> <owner/name>`. Adding an
/// item that is already present is a friendly no-op (the
/// server's 409 is reported as success-with-note).
pub fn cmd_add(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    collection: &str,
    snippet_ref: &str,
) -> Result<i32> {
    let (so, sn) = split_snippet_ref(snippet_ref)?;
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = split_ref(collection, &sess.username);
    let path = format!("{}/items/{so}/{sn}", collection_path(&owner, &name));
    match sess
        .client
        .request("PUT", &path, None, Some(&sess.entry.token), &[])
    {
        Ok(_) => {
            success(&format!("added {so}/{sn} to {owner}/{name}"));
            Ok(0)
        }
        Err(e) if e.status == 409 => {
            success(&format!("{so}/{sn} is already in {owner}/{name}"));
            Ok(0)
        }
        Err(e) => Err(api_fail(&e, &sess.entry.token).into()),
    }
}

/// `fg collection remove <collection> <owner/name>`.
pub fn cmd_remove(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    collection: &str,
    snippet_ref: &str,
) -> Result<i32> {
    let (so, sn) = split_snippet_ref(snippet_ref)?;
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = split_ref(collection, &sess.username);
    let path = format!("{}/items/{so}/{sn}", collection_path(&owner, &name));
    sess.client
        .delete(&path, Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    success(&format!("removed {so}/{sn} from {owner}/{name}"));
    Ok(0)
}

/// `fg collection order <collection> <owner/name>...`.
pub fn cmd_order(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    collection: &str,
    items: &[String],
) -> Result<i32> {
    for refr in items {
        split_snippet_ref(refr)?;
    }
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = split_ref(collection, &sess.username);
    let path = format!("{}/reorder", collection_path(&owner, &name));
    sess.client
        .post_json(&path, &json!({ "items": items }), Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    success(&format!(
        "order updated for {owner}/{name} ({} items)",
        items.len()
    ));
    Ok(0)
}

/// `fg collection visibility <collection> <level>`.
pub fn cmd_visibility(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    collection: &str,
    visibility: &str,
) -> Result<i32> {
    check_visibility(visibility)?;
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = split_ref(collection, &sess.username);
    let (_, body, _) = sess
        .client
        .request(
            "PATCH",
            &collection_path(&owner, &name),
            Some(&json!({ "visibility": visibility })),
            Some(&sess.entry.token),
            &[],
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if ctx.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    } else {
        success(&format!("{owner}/{name} is now {visibility}"));
    }
    Ok(0)
}

/// `fg collection share <collection> [--days N]` (Peak C6):
/// mint a read-only share link. The full URL is printed once;
/// the server stores only its hash.
pub fn cmd_share(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    collection: &str,
    days: i64,
) -> Result<i32> {
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = own_collection_ref(&sess, collection)?;
    let (_, body, _) = sess
        .client
        .post_json(
            &format!("{}/share-links", collection_path(&owner, &name)),
            &json!({ "expires_in_days": days }),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if ctx.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
        return Ok(0);
    }
    let base = sess.registry.trim_end_matches('/');
    println!(
        "{}",
        out::style().success(&format!(
            "share link for {owner}/{name} (expires {})",
            body["expires_at"].as_str().unwrap_or_default()
        ))
    );
    println!("  {base}{}", body["url"].as_str().unwrap_or_default());
    println!("  Anyone with this link can read the collection's current snippets.");
    println!("  It is shown once - the server stores only a hash of it.");
    println!(
        "  Revoke it with: fg collection unshare {name} {}",
        body["id"]
    );
    Ok(0)
}

/// `fg collection shares <collection>`: list share links
/// (prefixes only - full tokens are never stored).
pub fn cmd_shares(ctx: &Args<'_>, stdin: &mut impl BufRead, collection: &str) -> Result<i32> {
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = own_collection_ref(&sess, collection)?;
    let (_, body, _) = sess
        .client
        .get(
            &format!("{}/share-links", collection_path(&owner, &name)),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    if ctx.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
        return Ok(0);
    }
    let links = body["links"].as_array().cloned().unwrap_or_default();
    if links.is_empty() {
        println!("no share links for {owner}/{name}");
        return Ok(0);
    }
    for link in &links {
        let state = if link["active"].as_bool() == Some(true) {
            "active"
        } else if link["revoked_at"].is_string() {
            "revoked"
        } else {
            "expired"
        };
        println!(
            "  #{}  fgl_{}...  {}  expires {}",
            link["id"],
            link["prefix"].as_str().unwrap_or_default(),
            state,
            link["expires_at"].as_str().unwrap_or_default()
        );
    }
    Ok(0)
}

/// `fg collection unshare <collection> <id>`: revoke a link.
pub fn cmd_unshare(
    ctx: &Args<'_>,
    stdin: &mut impl BufRead,
    collection: &str,
    id: i64,
) -> Result<i32> {
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = own_collection_ref(&sess, collection)?;
    sess.client
        .delete(
            &format!("{}/share-links/{id}", collection_path(&owner, &name)),
            Some(&sess.entry.token),
        )
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    success(&format!("revoked share link #{id} for {owner}/{name}"));
    Ok(0)
}

/// Resolve a collection reference that must be the caller's
/// own (share-link management is owner-only, ADR 0011).
fn own_collection_ref(sess: &Session, collection: &str) -> Result<(String, String)> {
    let (owner, name) = split_ref(collection, &sess.username);
    if owner != sess.username {
        return Err(CliError::failure(format!(
            "forgehub: {owner}/{name} is not yours\n  what: share links are managed by the collection's owner\n  fix: collections you own are listed by `fg collection list`"
        ))
        .into());
    }
    Ok((owner, name))
}

/// `fg collection delete <collection>` (asks unless -y).
pub fn cmd_delete(ctx: &Args<'_>, stdin: &mut impl BufRead, collection: &str) -> Result<i32> {
    let sess = session(ctx.store, stdin, ctx.registry)?;
    let (owner, name) = split_ref(collection, &sess.username);
    if owner != sess.username {
        return Err(CliError::failure(format!(
            "forgehub: cannot delete {owner}/{name}\n  what: only the owner can delete a collection\n  fix: collections you own are listed by `fg collection list`"
        ))
        .into());
    }
    if !ctx.yes {
        if !crate::prompt::stdin_is_tty() {
            return Err(CliError::refused(
                "forgehub: not confirmed\n  what: delete needs confirmation and there is no terminal\n  fix: pass --yes (-y) to delete without prompting",
            )
            .into());
        }
        let detail = fetch_collection(&sess, &owner, &name)?;
        let count = detail["item_count"].as_i64().unwrap_or_default();
        let prompt = format!(
            "Delete collection {owner}/{name} ({count} item(s))? The snippets themselves are not deleted."
        );
        if !crate::prompt::ask_yes_no(stdin, &prompt) {
            println!("Kept `{owner}/{name}`.");
            return Ok(2);
        }
    }
    sess.client
        .delete(&collection_path(&owner, &name), Some(&sess.entry.token))
        .map_err(|e| api_fail(&e, &sess.entry.token))?;
    success(&format!("deleted collection {owner}/{name}"));
    Ok(0)
}

// ------------------------------------------------------------ run

/// The `fg run` collection fallback: when a snippet reference
/// does not resolve, try a collection of that name (own
/// collections for a bare name, the named owner's otherwise).
/// Returns `None` when no readable collection matches, so the
/// caller keeps its original not-found error. Offline runs
/// skip this (collections are not cached).
pub fn try_run_collection(
    store: &Store,
    stdin: &mut impl BufRead,
    owner: Option<&str>,
    name: &str,
    args: &CloudRunArgs,
) -> Option<Result<i32>> {
    if args.offline {
        return None;
    }
    let sess = session(store, stdin, None).ok()?;
    let owner = owner.map_or_else(|| sess.username.clone(), str::to_string);
    let (status, body, _) = sess
        .client
        .get(&collection_path(&owner, name), Some(&sess.entry.token))
        .ok()?;
    if status != 200 {
        return None;
    }
    Some(run_collection_members(store, stdin, &body, args))
}

/// Run every member in order through the standard cloud-run
/// trust flow. Stops at the first member that fails, is
/// declined, or exits non-zero - a runbook that half-ran must
/// never look like a full run.
fn run_collection_members(
    store: &Store,
    stdin: &mut impl BufRead,
    body: &Value,
    args: &CloudRunArgs,
) -> Result<i32> {
    let owner = body["owner"].as_str().unwrap_or_default();
    let name = body["name"].as_str().unwrap_or_default();
    let items = body["items"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        success(&format!("collection {owner}/{name} has no snippets to run"));
        return Ok(0);
    }
    let total = items.len();
    if !args.json {
        eprintln!("forgehub: running collection {owner}/{name} ({total} snippets, in order)");
    }
    for (i, item) in items.iter().enumerate() {
        let io = item["owner"].as_str().unwrap_or_default();
        let iname = item["name"].as_str().unwrap_or_default();
        if !args.json {
            eprintln!("forgehub: [{}/{}] {io}/{iname}", i + 1, total);
        }
        let rref = RunRef {
            owner: Some(io.to_string()),
            name: iname.to_string(),
            version: None,
        };
        match crate::cloudrun::run_cloud(store, stdin, &rref, args) {
            Ok(0) => {}
            Ok(code) => {
                eprintln!(
                    "forgehub: collection run stopped at {io}/{iname} ({}/{total}) - it exited with code {code}",
                    i + 1
                );
                return Ok(code);
            }
            Err(e) => {
                eprintln!(
                    "forgehub: collection run stopped at {io}/{iname} ({}/{total})",
                    i + 1
                );
                return Err(e);
            }
        }
    }
    if !args.json {
        success(&format!(
            "collection {owner}/{name}: all {total} snippets ran"
        ));
    }
    Ok(0)
}
