# Report - Step 9: Snippet and version API

- **Date/time:** 2026-11-19T12:52:54+05:30
- **Commit:** this report is part of the Step 9 commit - see `git log --oneline -n 1` (tag `step-9`)
- **Status:** ⚠️ Done with caveats - all Step 9 acceptance criteria met and proven below; the caveat is the carried-over push blocker (§9), unchanged since Step 5.

## 1. Summary
The core data API is complete: snippets can be created, read, versioned, metadata-patched, renamed (with a 30-day redirect), soft-deleted, diffed and downloaded raw, with all parsing/validation/hashing done by `forge-core` so server hashes are byte-identical to CLI hashes (asserted in tests against `forge_core::hash::content_hash`). Every write is secret-scanned (422 with kinds + line numbers, never values); danger lints ride along as warnings. The authorisation matrix - every endpoint × {anonymous, other user, owner, read token, write token} × {private, unlisted, public} - is tested, with private existence never leaking (404 everywhere, including writes by non-owners). 325 workspace tests pass.

## 2. What I built
- **`src/snippets.rs`** (~1,100 lines incl. unit tests): all openapi snippet routes plus `POST /snippets/validate` (Step 2 contract; shares the submission pipeline, persists nothing) and `GET /me/snippets` (visibility/tag filters, updated/created/name sorts, opaque base64url cursors).
  - **Submission pipeline** (`parse_submission`): 64 KiB cap (413) → forge-file parse, or wrap-as-script for plain text (a forge-looking but broken YAML is a 400, not silently wrapped - the `looks_like_forge_file` heuristic) → name must match the request name → request description/tags override the file's → `validate()` → secret scan of the *submitted* text (so finding line numbers match the user's file) → canonical YAML via `to_yaml()` + spec §9 hash. Stored bodies are therefore always canonical forge files.
  - **Reads**: `GET /snippets/{owner}/{name}[?version=N]` with ETag/`If-None-Match` (304), versions list (metadata only), single version, server-side unified diff via the `similar` crate (3 lines of context, 256 KiB output cap → 413), `raw` as `text/plain`. Responses carry the parsed summary (mode, steps count, variables, `uses_sudo`, lint warnings) and fork provenance.
  - **Writes**: POST (201; duplicate name 409; per-user cap), PUT (new immutable version; `If-Match` required; identical hash → 200 `unchanged: true`), PATCH (metadata only, no version; rename with redirect), DELETE (soft delete). Cookie writes need CSRF; bearer writes need the `write` scope (read token → 403 `insufficient_scope`).
  - **Rename policy** (documented in the module docs): redirect lives 30 days, is stored against the snippet id so rename chains resolve to the current name, reads follow it (response carries `renamed_from`), writes do not, and creating a new snippet with the old name shadows it.
  - **Delete policy**: soft delete hides the snippet from everyone including the owner; the name stays reserved during the 30-day retention (recreate → 409), then the startup purge hard-deletes it (cascading versions/tags/stars) and drops expired rename redirects.
- **Repository additions** (`src/repo.rs`): `update_metadata` now reconciles tags too; new `rename`, `resolve_rename`, `count_for_owner`, `list_for_owner` (QueryBuilder, fixed sort fragments only), `purge_deleted`; **`add_version` gained transactional optimistic concurrency** - an optional expected-latest is re-checked *inside* the write transaction, so two racing PUTs with the same `If-Match` cannot both append (self-review fix, §8; covered by a repo test).
- **Config**: `FORGEHUB_MAX_SNIPPETS_PER_USER` (500), `FORGEHUB_MAX_VERSIONS_PER_SNIPPET` (200); documented in `.env.example` and `docs/running.md`. Retention purge runs at startup in `main.rs` and logs what it removed.
- **Route/spec sync**: openapi.yaml already described this step exactly (including required `If-Match`, 412, and the validate endpoint) - verified route-by-route against the implementation while writing the handlers; no spec changes were needed. A literal route-list-diff test was judged brittle (path templates vs axum patterns); instead every route family is exercised by HTTP tests asserting handler-specific error codes (`snippet_not_found` vs the fallback's `not_found`).

## 3. Decisions made
- **PUT applies description/tags from the body** (a PUT carries a full file) via `update_metadata` after the version append, without creating an extra version; visibility is *not* changed by PUT (PATCH-only, per openapi).
- **Plain-script detection**: text that fails forge parsing and doesn't start with forge keys within its first 40 lines is a script; anything that looks like a forge file but fails to parse is a 400 with the parser's message. This keeps `fg push` of a raw shell script working while never silently mangling a broken forge file.
- **Cursors are offsets in base64url** - opaque to clients, stable under the three fixed orderings, and trivially verifiable; keyset cursors can replace them if lists ever outgrow offset sanity.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass, zero warnings |
| Tests | `cargo test --workspace` | **325 passed, 0 failed** (forge-server: 23+4 unit, 9 auth HTTP, 7 HTTP, 8 token/device HTTP, 10 snippet HTTP; forge-core 199; forge-cli 65) |
| Web | `npm run lint && npm run typecheck && npm run test && npm run build` | pass (1 web test) |
| Secret scan | `scripts/scan-staged.sh` | clean |
| Read matrix | `tests/snippets.rs::read_authorisation_matrix` | all 6 caller kinds × 3 visibilities on detail + versions + raw + version-1 |
| Hash parity | `create_normalises_and_hashes_like_the_cli` | server response hash == `forge_core::hash::content_hash` of the stored body; plain scripts wrapped; steps summary (steps_count 2, uses_sudo true) |
| Writes | PUT suite | missing If-Match 400, stale 412 with `latest_version` detail, identical body `unchanged: true`, v1 immutable after v2/v3, non-owner PUT 404 on a *public* snippet, read token 403, write token 200 |
| Secrets & input | create suite | 422 `secret_detected` with findings and the secret string asserted **absent** from the response; name mismatch 400; broken YAML 400; 70 KiB body 413; reserved name 400; uppercase name 400; duplicate 409; no-CSRF session write 403 |
| PATCH/rename/delete | suites | metadata patch creates no version; old name resolves with `renamed_from`; PUT to old name 404; rename onto existing name 409; new snippet shadows redirect; delete → 404 for all, recreate 409 during retention |
| Lists | me/snippets suite | filters (visibility, tag), name sort, two-page cursor walk, anonymous 401, garbage cursor 400; per-user cap suite (limit=2 → third create 400) |
| Validate | suite | valid → `valid: true` + hash; secret → 422 + `secret_findings`; broken file → `valid: false` + errors, nothing persisted |

## 5. Acceptance criteria
- [x] Private snippets never leak (404 for non-owners) - tested across all endpoints (detail, versions, version, diff, raw, and every write verb).
- [x] Every write creates an immutable version with a stable hash identical to what the CLI will compute. - version rows are append-only (no update path exists in the repo layer); hash parity asserted against forge-core in tests.
- [x] Secret scanning rejects; lints warn. - 422 with findings on create/PUT; lint warnings present in every snippet summary, never blocking.

## 6. Security notes
Advances T-EXEC-2 (server never stores a secret-shaped body; findings leak neither value nor existence), T-ACCT-2/T-DOS-1 (owner-only writes, scope-gated bearer writes, body/version/count caps), and the privacy rule that private existence is indistinguishable from absence - including on rename resolution, where lookup of another user's old name yields the same 404. The transactional If-Match closes the lost-update race the roadmap called out. `intent=run` pull counting deliberately lands in Step 10 with its dedup/rate-limit design, as the roadmap sequences it.

## 7. Dependencies added
Runtime: `similar` 2 (unified diffs). Dev-only: `forge-core` path dependency for the integration tests (hash-parity assertions).

## 8. Corrections to earlier steps
- **Step 6 (repo)**: `add_version` signature extended with the expected-latest check (its Step 6 tests updated to pass `None` and to cover the raced case); `update_metadata` extended with tag reconciliation (Step 6 caller/test updated). No behaviour regressions: full suite green.
- **Step 7 (auth)**: `fmt_ts` is now `pub` (the binary's retention job formats with it); no semantics changed.

## 9. Known issues / limitations
- **Push blocker (carried over, unchanged):** repo returns 404, no GitHub auth on this machine; CI has never run. Needed: create the empty private repo `aotlover9-base-eth/forgehub` and authenticate (`gh auth login`, PAT via Secure Vault, or SSH key). Steps 1–9 (9 commits, tags `step-1`…`step-9`) publish with one push.
- The retention purge runs at startup only; a long-lived server purges on restart. A periodic job is a natural Step 19/21 hardening item.
- `starred_by_me`/star endpoints, search and profiles are Step 10, per the roadmap.

## 10. Deferred / ideas
None.

## 11. Next step
Step 10 - discovery: FTS5 search with injection-safe query handling, popular tags, explore, stars, forks, public profiles, the `intent=run` pull counter with per-IP dedup, snippet reports, and the minimal `forge-server admin` CLI; closes with the 10k-snippet p95 benchmark.
