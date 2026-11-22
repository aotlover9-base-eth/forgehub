# Report - Peak C4: public snippet pages

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** C4 (Phase C - new features).
- **Status:** Done. Server-rendered public pages at
  `/s/{owner}/{name}` for public snippets, with canonical
  and Open Graph metadata and the CLI install line; the web
  snippet page links to it.

## 1. Decisions

- **Public only.** Private snippets 404 (existence never
  leaks), and so do **unlisted** ones: unlisted means
  "reachable in the app by link", not "published as a
  page" - giving unlisted snippets an indexable,
  metadata-rich page would break that promise. Missing
  users/snippets get the identical 404 page.
- **No JavaScript required.** The page is plain HTML with
  one inline `<style>` block (allowed by the CSP's
  `style-src 'unsafe-inline'`; there are no external
  assets at all). The copy buttons are progressive
  enhancement: a tiny self-hosted script at
  `/s-assets/copy.js` (served by the same module) wires
  them to the clipboard; without it, the install/run lines
  sit in read-only inputs the reader can select and copy.
- **Canonical/OG URLs** use the configured
  `FORGEHUB_BASE_URL` (`state.config.base_url`), so a
  deployment behind its real domain emits correct absolute
  URLs. `og:` title/description/url + `twitter:card`
  summary are emitted; there is deliberately no `og:image`
  (no suitable self-hosted asset exists, and hotlinking
  one would break the no-third-party rule).
- **Escaping.** Every interpolated value passes through a
  local `esc()` (text + attribute contexts); the hostile-
  content test proves a `<script>` in the description and
  an `onerror` img in the body render as inert text, and
  that the only `<script>` on the page is the self-hosted
  helper.
- The route lives outside `/api/v1` and outside the SPA
  fallback (`pages::routes()` merged into the app router
  with its own state), and inherits the global security
  headers layer - the test asserts the CSP header is
  present and starts with `default-src 'self'`.

## 2. Page contents

Title/byline (owner link), version + short hash + updated
time + star/fork/pull counts, description, tags, the
declared parameters (names and types only, parsed from the
stored body - a parse failure degrades to no list, never a
500), the **install line** `fg pull owner/name` and the
**run line** `fg owner/name` with copy buttons and an
honest note that ForgeHub shows the commands and asks
before running anything, then the full forge file in a
`<pre>`. Footer: "No trackers; this page works without
JavaScript."

## 3. Web

The snippet page's "Run it" card gains a **Public page**
link (new tab) when the snippet is public - the
discoverability path from the app to the shareable page.

## 4. Tests

Server integration (+3, `tests/public_pages.rs`): the full
page (title, canonical with the configured base URL, OG
tags, body, both CLI lines, copy wiring, CSP header, the
copy.js asset itself); hostile content escaped (see
above); private + unlisted + missing owner/snippet all
404 with no body leak. Web: no new test - the link is a
conditional anchor on an already-covered page (the
snippet suite stays green).

## 5. Docs

`docs/api.md` endpoint map gained the Collections row
(owed since C1) and the Public pages row.

## 6. Gates

Local: `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets -- -D warnings` clean (two
`map().unwrap_or_default()` lints in the new code fixed in
this WP); `cargo test --workspace` 508 passed / 0 failed
(505 at C3 + 3). Web: typecheck clean; the snippet suite
stays green; full web suite in CI. CI verification of this
commit's head is recorded in PROGRESS.md.

## 7. Next

C5 - import helpers (history files + GitHub Gists, ADR
first for the Gist network surface).
