//! Public snippet pages (Peak C4): server-rendered HTML at
//! `/s/{owner}/{name}` for PUBLIC snippets only. The page is
//! the shareable face of a snippet - it works with JavaScript
//! disabled, carries Open Graph tags and a canonical URL,
//! loads no trackers and nothing third-party (the one script
//! is a self-hosted copy-button helper, and the page is fully
//! usable without it). Private and unlisted snippets answer
//! the same 404 as missing ones: unlisted means "reachable
//! in the app by link", not "published as a page".

use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

/// Escape a string for HTML text and attribute contexts. All
/// interpolated values pass through here - descriptions and
/// bodies are attacker-controlled.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const COPY_JS: &str = "document.querySelectorAll('[data-copy]').forEach(function (btn) {\n  btn.addEventListener('click', function () {\n    var input = document.getElementById(btn.getAttribute('data-copy'));\n    if (!input) return;\n    input.select();\n    if (navigator.clipboard) { navigator.clipboard.writeText(input.value); }\n    btn.textContent = 'Copied';\n    setTimeout(function () { btn.textContent = 'Copy'; }, 2000);\n  });\n});\n";

async fn copy_js() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        COPY_JS,
    )
        .into_response()
}

fn not_found_page() -> Response {
    let html = "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Not found - ForgeHub</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n</head>\n<body>\n<h1>Not found</h1>\n<p>This snippet is private, unlisted, renamed or deleted. Public snippet pages live at <code>/s/owner/name</code>.</p>\n<p><a href=\"/\">ForgeHub</a></p>\n</body>\n</html>\n";
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

async fn public_snippet_page(
    State(state): State<AppState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let Some(owner_row) = state.repos.users.by_username(&owner).await? else {
        return Ok(not_found_page());
    };
    let Some(snippet) = state
        .repos
        .snippets
        .by_owner_name(owner_row.id, &name)
        .await?
    else {
        return Ok(not_found_page());
    };
    if snippet.visibility != "public" || snippet.hidden_at.is_some() || snippet.deleted_at.is_some()
    {
        return Ok(not_found_page());
    }
    let Some(version) = state.repos.snippets.latest_version(snippet.id).await? else {
        return Ok(not_found_page());
    };
    let tags = state.repos.snippets.tags(snippet.id).await?;
    let base = state.config.base_url.trim_end_matches('/');
    let canonical = format!("{base}/s/{owner}/{name}");
    let title = format!("{name} by {owner} - ForgeHub");
    let description = if snippet.description.is_empty() {
        format!("A shell snippet by {owner} on ForgeHub. Install it with the ForgeHub CLI.")
    } else {
        snippet.description.clone()
    };
    let pull_line = format!("fg pull {owner}/{name}");
    let run_line = format!("fg {owner}/{name}");
    let short_hash = &version.hash[..8.min(version.hash.len())];
    let updated = snippet.updated_at.replace('T', " ");
    let updated = updated.get(..16).unwrap_or(&updated);

    // Declared parameters (names + types only) from the parsed
    // body; a parse failure here means corrupt stored data -
    // the page still renders, just without the list.
    let params_html =
        forge_core::Snippet::from_yaml(&version.body).map_or(String::new(), |parsed| {
            if parsed.variables.is_empty() {
                String::new()
            } else {
                let items: Vec<String> = parsed
                    .variables
                    .iter()
                    .map(|(pname, spec)| {
                        let param = spec.resolve();
                        format!(
                            "<li><code>{}</code> ({})</li>",
                            esc(pname),
                            esc(param.kind())
                        )
                    })
                    .collect();
                format!("<h2>Parameters</h2>\n<ul>\n{}\n</ul>\n", items.join("\n"))
            }
        });

    let tags_html = if tags.is_empty() {
        String::new()
    } else {
        let chips: Vec<String> = tags.iter().map(|t| esc(t)).collect();
        format!("<p>Tags: {}</p>\n", chips.join(", "))
    };

    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"description\" content=\"{description_attr}\">\n<link rel=\"canonical\" href=\"{canonical}\">\n<meta property=\"og:type\" content=\"article\">\n<meta property=\"og:site_name\" content=\"ForgeHub\">\n<meta property=\"og:title\" content=\"{title_attr}\">\n<meta property=\"og:description\" content=\"{description_attr}\">\n<meta property=\"og:url\" content=\"{canonical}\">\n<meta name=\"twitter:card\" content=\"summary\">\n<meta name=\"twitter:title\" content=\"{title_attr}\">\n<meta name=\"twitter:description\" content=\"{description_attr}\">\n<style>body{{font-family:system-ui,sans-serif;max-width:52rem;margin:2rem auto;padding:0 1rem;line-height:1.5}}pre{{overflow-x:auto;background:#f4f4f5;padding:1rem;border-radius:.5rem}}code{{font-family:ui-monospace,monospace}}input.copyline{{width:100%;font-family:ui-monospace,monospace;padding:.5rem;box-sizing:border-box}}</style>\n</head>\n<body>\n<header><a href=\"/\">ForgeHub</a></header>\n<main>\n<h1>{name}</h1>\n<p>by <a href=\"/{owner}\">{owner}</a> · version {version_no} · sha256:{short_hash} · updated {updated} UTC · ★ {stars} · {forks} forks · {pulls} pulls</p>\n{description_block}{tags_html}<h2>Install with the ForgeHub CLI</h2>\n<p><label for=\"pull-line\">Pull it into your library:</label></p>\n<p><input class=\"copyline\" id=\"pull-line\" readonly value=\"{pull_line}\"> <button type=\"button\" data-copy=\"pull-line\">Copy</button></p>\n<p><label for=\"run-line\">Or run it directly:</label></p>\n<p><input class=\"copyline\" id=\"run-line\" readonly value=\"{run_line}\"> <button type=\"button\" data-copy=\"run-line\">Copy</button></p>\n<p>ForgeHub asks before running anything, and shows you the exact commands first. Get the CLI from the <a href=\"/\">ForgeHub home page</a>.</p>\n{params_html}<h2>Contents</h2>\n<pre><code>{body}</code></pre>\n</main>\n<footer><p>Rendered by ForgeHub. No trackers; this page works without JavaScript.</p></footer>\n<script src=\"/s-assets/copy.js\" defer></script>\n</body>\n</html>\n",
        title = esc(&title),
        title_attr = esc(&title),
        description_attr = esc(&description),
        description_block = if snippet.description.is_empty() {
            String::new()
        } else {
            format!("<p>{}</p>\n", esc(&snippet.description))
        },
        name = esc(&name),
        owner = esc(&owner),
        version_no = version.version,
        short_hash = esc(short_hash),
        updated = esc(updated),
        stars = snippet.star_count,
        forks = snippet.fork_count,
        pulls = snippet.pull_count,
        body = esc(&version.body),
    );
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}

/// The public-pages router (state applied by the caller).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/s/{owner}/{name}", get(public_snippet_page))
        .route("/s-assets/copy.js", get(copy_js))
}
