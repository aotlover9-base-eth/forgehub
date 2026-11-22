# ADR 0010 - Gist import: the server's one outbound fetch, allow-listed

- **Status:** Accepted (Peak WP C5, 2026-11-22)
- **Context:** Peak C5 adds GitHub Gist import: a user pastes a gist URL and the server fetches the gist's files and imports them as private snippets. Until now the ForgeHub server made **no outbound requests at all** - it is a purely inbound service. A server-side fetch of user-influenced URLs is the classic SSRF shape, so the network surface needs an explicit decision before the code exists.

## Decision

The server fetches gists under these rules, all enforced in `crates/forge-server/src/imports.rs`:

1. **The user supplies an identifier, never a URL to fetch.** The pasted URL is parsed against exactly two shapes - `https://gist.github.com/{user}/{id}` and `https://gist.github.com/{id}` - and only the gist `id` (hex/alphanumeric, length-capped) is extracted. The server then **constructs** the request URL itself: `{gist_api_base}/gists/{id}`. There is no code path in which user input becomes a request host, scheme or path.
2. **The API base is operator configuration, not user input.** `FORGEHUB_GIST_API_BASE` defaults to `https://api.github.com` (this is also what makes GitHub Enterprise bases possible: `https://host/api/v3`). Redirects are followed only to the configured base's host or `gist.githubusercontent.com` (where raw file URLs live), at most 3 hops; any other redirect target aborts the fetch.
3. **Public gists only; no credentials are ever sent.** The request carries a `User-Agent` and an `Accept` header and no `Authorization` - the server has no GitHub credentials and never will for this feature. A private gist answers 404 from GitHub and imports nothing.
4. **Caps everywhere.** 10 s timeout; the response is read with a hard byte cap (1 MiB); at most 20 files; any file over the 64 KiB snippet limit is skipped (as is a file GitHub marks `truncated`). The caller's per-user snippet limit applies to the batch.
5. **Fetched content is untrusted input.** Every file goes through the exact snippet-creation pipeline (`parse_submission`: forge-file parse or script wrap, full validation, secret scan). A file with secret findings is skipped with the reason recorded - its content is never stored. Name collisions with the caller's existing snippets skip, never overwrite. Everything imported lands **private**.

## Why

- **SSRF dies at rule 1.** The standard failure is `fetch(user_url)`; here the fetch target is built from a validated id and a configured base, so DNS-rebinding/internal-service tricks have no input to work with. The allow-list is therefore structural, not a filter bolted onto a general fetcher.
- **The alternative - fetching from the CLI or the browser - is worse.** The browser would hit CORS and would put the content through no server-side scan; the CLI cannot serve the web import flow at all. A single server-side implementation serves both the web UI and `fg import --from-gist`.
- **Operator override over hardcoding.** Self-hosters behind GitHub Enterprise are a real audience for a self-hosted tool; making the base configurable (with the default correct for everyone else) costs one config field.

## Alternatives rejected

- **Fetch the pasted URL directly with a host allow-list check.** Rejected: allow-lists on arbitrary URLs leak through redirects, URL parsing quirks and userinfo tricks; constructing the URL removes the class.
- **Import via the git clone protocol (`git clone https://gist.github.com/{id}.git`).** Rejected: shelling out to git adds a process surface, credential-helper surprises and a much larger parser problem (a whole working tree) for the same content.
- **Let users paste gist file contents manually.** That already works (it is just snippet creation); it is not an import feature.

## Consequences

- The server gains its first outbound dependency (`reqwest` with rustls native roots, promoted from dev-dependencies) and one config field. Operators who want zero outbound traffic can point `FORGEHUB_GIST_API_BASE` at an unreachable base or block egress; the feature fails closed with a clear error.
- Tests point the base at a local mock, so the suite never touches the real GitHub.
