//! Import helpers (Peak C5): shell-history analysis for the
//! web import flow, and GitHub Gist import.
//!
//! History: the client uploads a history file's text (capped
//! at 64 KiB); the server runs the shared mining pipeline
//! (`forge_core::history_mine` - the same code as `fg
//! analyze`, secrets filtered before anything is returned)
//! and answers with candidates the user picks from. Nothing
//! is stored by the analyze call itself.
//!
//! Gist: the server's only outbound fetch, governed by ADR
//! 0010 - the user supplies a gist id (parsed out of the
//! pasted URL), the server constructs the request against
//! the configured API base, sends no credentials, caps
//! everything, and puts every file through the ordinary
//! snippet-creation pipeline (`parse_submission`, secret
//! scan included). Imports land private; collisions skip.

use crate::auth::{body, RequireUser};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::snippets::{check_write_scope_and_csrf, parse_submission};
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use serde::Deserialize;

/// Plan cap for the history-file upload (C5).
const MAX_HISTORY_BYTES: usize = 64 * 1024;
/// ADR 0010 caps for the gist fetch.
const MAX_GIST_FILES: usize = 20;
const MAX_GIST_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_SNIPPET_BYTES: usize = 64 * 1024;

/// Built-in noise commands for the server-side history filter
/// (the CLI's default `ignore_commands`; the server has no
/// per-user list, so the defaults are the honest choice).
const NOISE_COMMANDS: &[&str] = &[
    "ls", "ll", "la", "cd", "clear", "pwd", "exit", "history", "which", "man", "htop",
];

fn is_noise(cmd: &str) -> bool {
    let head = cmd.split_whitespace().next().unwrap_or("");
    NOISE_COMMANDS.contains(&head) && cmd.split_whitespace().count() == 1
}

// ------------------------------------------------------------ history

#[derive(Deserialize)]
struct HistoryInput {
    text: String,
}

async fn analyze_history(
    State(_state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    result: Result<Json<HistoryInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    if req.text.len() > MAX_HISTORY_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "history_too_large",
            "History files are limited to 64 KiB for analysis",
        ));
    }
    let (candidates, parsed, kept) =
        forge_core::history_mine::mine_history_text(&req.text, &is_noise, 4, 2, 25);
    let out: Vec<serde_json::Value> = candidates
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.suggested_name(),
                "commands": c.commands,
                "count": c.count,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "parsed": parsed,
        "kept": kept,
        "candidates": out,
    }))
    .into_response())
}

// ------------------------------------------------------------ gist

#[derive(Deserialize)]
struct GistInput {
    url: String,
}

/// Extract the gist id from a pasted URL (ADR 0010 rule 1).
/// Accepts `https://gist.github.com/{user}/{id}` and
/// `https://gist.github.com/{id}` (trailing slash tolerated).
/// Anything else - another host, another scheme, extra path
/// segments, a malformed id - is a validation error, never a
/// fetch.
fn parse_gist_id(url: &str) -> Result<String, ApiError> {
    let bad = || {
        ApiError::validation(
            "That does not look like a gist URL (expected https://gist.github.com/<user>/<id>)",
        )
    };
    let rest = url
        .trim()
        .strip_prefix("https://gist.github.com/")
        .ok_or_else(bad)?;
    let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
    let id = match parts.len() {
        1 => parts[0],
        2 => parts[1],
        _ => return Err(bad()),
    };
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(bad());
    }
    Ok(id.to_string())
}

/// Build the gist HTTP client (ADR 0010): 10 s timeout,
/// redirects only to the API base's host or
/// gist.githubusercontent.com, at most 3 hops.
fn gist_client(base: &str) -> Result<reqwest::Client, ApiError> {
    let base_host = reqwest::Url::parse(base)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() > 3 {
            return attempt.error("too many redirects");
        }
        match attempt.url().host_str() {
            Some(host) if host == base_host || host == "gist.githubusercontent.com" => {
                attempt.follow()
            }
            _ => attempt.error("redirect to a host that is not on the allow-list"),
        }
    });
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(policy)
        .user_agent("ForgeHub gist import")
        .build()
        .map_err(|_| ApiError::internal())
}

/// One gist file, as the import loop needs it.
struct GistFile {
    filename: String,
    content: Option<String>,
    truncated: bool,
}

/// Pull the files array out of a GitHub gist JSON document.
fn gist_files(doc: &serde_json::Value) -> Vec<GistFile> {
    let mut files: Vec<GistFile> = doc["files"].as_object().map_or_else(Vec::new, |map| {
        map.values()
            .map(|f| GistFile {
                filename: f["filename"].as_str().unwrap_or_default().to_string(),
                content: f["content"].as_str().map(str::to_string),
                truncated: f["truncated"].as_bool().unwrap_or(false),
            })
            .collect()
    });
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

/// Derive a snippet name from a gist filename: strip the
/// extension, slugify, and validate; falls back to `gist`
/// when nothing usable remains.
fn name_from_filename(filename: &str) -> String {
    let stem = filename.rsplit('/').next().unwrap_or(filename);
    let stem = stem.split('.').next().unwrap_or(stem);
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase();
    if cleaned.is_empty() || forge_core::names::validate_snippet_name(&cleaned).is_err() {
        "gist".to_string()
    } else {
        cleaned
    }
}

async fn import_gist(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    result: Result<Json<GistInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let id = parse_gist_id(&req.url)?;
    let base = state.config.gist_api_base.trim_end_matches('/');
    let client = gist_client(base)?;
    let resp = client
        .get(format!("{base}/gists/{id}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_GATEWAY,
                "gist_unreachable",
                "Could not reach the GitHub API; try again in a moment",
            )
        })?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::not_found(
            "gist_not_found",
            "Gist not found - only public gists can be imported",
        ));
    }
    if !resp.status().is_success() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "gist_unreachable",
            format!("The GitHub API answered {}", resp.status()),
        ));
    }
    // Read with a hard cap (ADR 0010 rule 4).
    let mut bytes = Vec::new();
    let mut resp = resp;
    while let Some(chunk) = resp.chunk().await.map_err(|_| ApiError::internal())? {
        if bytes.len() + chunk.len() > MAX_GIST_RESPONSE_BYTES {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "gist_too_large",
                "That gist is too large to import (over 1 MiB of metadata)",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let doc: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "gist_unreachable",
            "The GitHub API returned an unreadable response",
        )
    })?;
    let files = gist_files(&doc);
    let (created, skipped) = import_gist_files(&state, ctx.0.user.id, &files).await?;
    Ok(Json(serde_json::json!({ "created": created, "skipped": skipped })).into_response())
}

/// Import each gist file as a private snippet, in filename
/// order; every file that cannot be imported is skipped with
/// a plain-language reason (Peak C5, ADR 0010).
async fn import_gist_files(
    state: &AppState,
    user_id: i64,
    files: &[GistFile],
) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>), ApiError> {
    let mut created: Vec<serde_json::Value> = Vec::new();
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    let mut skip = |file: &str, reason: &str| {
        skipped.push(serde_json::json!({ "file": file, "reason": reason }));
    };
    let mut count = state.repos.snippets.count_for_owner(user_id).await?;
    for (index, file) in files.iter().enumerate() {
        if index >= MAX_GIST_FILES {
            skip(&file.filename, "gist file limit reached (20 per import)");
            continue;
        }
        if file.truncated {
            skip(&file.filename, "file is truncated by GitHub (too large)");
            continue;
        }
        let Some(content) = &file.content else {
            skip(&file.filename, "file has no readable content");
            continue;
        };
        if content.len() > MAX_SNIPPET_BYTES {
            skip(
                &file.filename,
                "file is larger than the 64 KiB snippet limit",
            );
            continue;
        }
        if count >= state.config.max_snippets_per_user {
            skip(&file.filename, "your snippet limit is reached");
            continue;
        }
        // Forge files name themselves; anything else is named
        // after the file. parse_submission decides which case
        // applies and runs the full validation + secret scan.
        let probe = forge_core::Snippet::from_yaml(content).ok();
        let name = probe
            .as_ref()
            .map_or_else(|| name_from_filename(&file.filename), |s| s.name.clone());
        if state
            .repos
            .snippets
            .by_owner_name(user_id, &name)
            .await?
            .is_some()
        {
            skip(
                &file.filename,
                &format!("you already have a snippet named {name}"),
            );
            continue;
        }
        let parsed = match parse_submission(&name, None, None, None, content) {
            Ok(p) => p,
            Err(e) => {
                skip(&file.filename, &e.message);
                continue;
            }
        };
        let now = now_iso();
        match state
            .repos
            .snippets
            .create_with_version(
                user_id,
                &name,
                &parsed.description,
                "private",
                parsed.mode,
                &parsed.canonical_body,
                &parsed.hash,
                Some("Imported from a GitHub gist"),
                &parsed.tags,
                &now,
            )
            .await
        {
            Ok(_) => {
                count += 1;
                created.push(serde_json::json!({ "name": name, "file": file.filename }));
            }
            Err(crate::repo::RepoError::Conflict(_)) => {
                skip(
                    &file.filename,
                    &format!("you already have a snippet named {name}"),
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok((created, skipped))
}

/// Import routes (merged into the API router).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/imports/history/analyze", post(analyze_history))
        .route("/imports/gist", post(import_gist))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gist_id_parsing() {
        assert_eq!(
            parse_gist_id("https://gist.github.com/anna/0123abcdEF").unwrap(),
            "0123abcdEF"
        );
        assert_eq!(
            parse_gist_id("https://gist.github.com/0123abcd").unwrap(),
            "0123abcd"
        );
        assert_eq!(
            parse_gist_id("https://gist.github.com/anna/0123abcd/").unwrap(),
            "0123abcd"
        );
        for bad in [
            "https://github.com/anna/0123abcd",
            "http://gist.github.com/anna/0123abcd",
            "https://gist.github.com/anna/0123abcd/extra",
            "https://gist.github.com/anna/not%20hex!",
            "https://gist.github.com/",
            "gist.github.com/anna/0123abcd",
        ] {
            assert!(parse_gist_id(bad).is_err(), "must refuse: {bad}");
        }
    }

    #[test]
    fn filename_names() {
        assert_eq!(name_from_filename("deploy.sh"), "deploy");
        assert_eq!(name_from_filename("My Setup.bash"), "my-setup");
        assert_eq!(name_from_filename("weird name!.txt"), "weird-name");
        assert_eq!(name_from_filename("..."), "gist");
    }
}
