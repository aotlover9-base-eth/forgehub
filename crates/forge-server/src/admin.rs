//! Minimal admin CLI (Step 10): `forge-server admin <command>`.
//! There is deliberately no admin web UI in v1; moderation is an
//! operator action run against the server's own database.
//!
//! Commands:
//! - `hide-snippet <owner>/<name>` / `unhide-snippet <owner>/<name>`
//! - `ban-user <username>` / `unban-user <username>`
//! - `list-reports [--status open|resolved|dismissed]`
//!
//! Banning disables the account and immediately revokes its
//! sessions and API tokens. Hiding removes a snippet from every
//! public surface (detail 404s for non-owners, search, explore,
//! profiles) while leaving the owner's own access intact.

use crate::db::now_iso;
use crate::repo::Repos;
use anyhow::{bail, Context, Result};

fn usage() -> &'static str {
    "usage: forge-server admin <hide-snippet|unhide-snippet> <owner>/<name>\n       \
     forge-server admin <ban-user|unban-user> <username>\n       \
     forge-server admin list-reports [--status <status>]"
}

/// Run one admin command against the repositories. `args` are the
/// arguments after `admin`.
///
/// # Errors
/// Unknown commands, missing targets, and storage failures.
pub async fn run(repos: &Repos, args: &[String]) -> Result<()> {
    let (cmd, rest) = args
        .split_first()
        .map_or(("", &[][..]), |(c, r)| (c.as_str(), r));
    match cmd {
        "hide-snippet" | "unhide-snippet" => {
            let target = rest.first().context(usage())?;
            let (owner, name) = target.split_once('/').context("expected <owner>/<name>")?;
            let user = repos
                .users
                .by_username(owner)
                .await?
                .with_context(|| format!("no such user: {owner}"))?;
            let snippet = repos
                .snippets
                .by_owner_name(user.id, name)
                .await?
                .with_context(|| format!("no such snippet: {target}"))?;
            repos
                .snippets
                .set_hidden(snippet.id, cmd == "hide-snippet", &now_iso())
                .await?;
            println!(
                "{} {target}",
                if cmd == "hide-snippet" {
                    "hid"
                } else {
                    "unhid"
                }
            );
            Ok(())
        }
        "ban-user" | "unban-user" => {
            let username = rest.first().context(usage())?;
            let user = repos
                .users
                .by_username(username)
                .await?
                .with_context(|| format!("no such user: {username}"))?;
            let ban = cmd == "ban-user";
            repos.users.set_disabled(user.id, ban, &now_iso()).await?;
            if ban {
                // A ban takes effect immediately: sessions and
                // tokens die with the account.
                repos.sessions.delete_for_user(user.id).await?;
                repos
                    .tokens
                    .revoke_all_for_user(user.id, &now_iso())
                    .await?;
            }
            println!("{} {username}", if ban { "banned" } else { "unbanned" });
            Ok(())
        }
        "list-reports" => {
            let status = rest
                .iter()
                .position(|a| a == "--status")
                .and_then(|i| rest.get(i + 1))
                .map(String::as_str);
            let reports = repos.reports.list(status, 100).await?;
            if reports.is_empty() {
                println!("no reports");
                return Ok(());
            }
            for r in &reports {
                let snippet = repos.snippets.by_id(r.snippet_id).await?;
                let reporter = repos.users.by_id(r.reporter_id).await?;
                let target = match &snippet {
                    Some(s) => {
                        let owner = repos.users.by_id(s.owner_id).await?;
                        format!(
                            "{}/{}",
                            owner.map_or("?".to_string(), |u| u.username),
                            s.name
                        )
                    }
                    None => format!("snippet#{}", r.snippet_id),
                };
                println!(
                    "#{} [{}] {} reported by {} at {}: {}",
                    r.id,
                    r.status,
                    target,
                    reporter.map_or("?".to_string(), |u| u.username),
                    r.created_at,
                    r.reason,
                );
            }
            Ok(())
        }
        _ => bail!("unknown admin command\n{}", usage()),
    }
}
