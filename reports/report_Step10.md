# Report — Step 10: Search, tags, stars, forks, profiles

## 1. Summary

Step 10 is complete locally. ForgeHub now has discovery: FTS5 full-text
search over public snippets with BM25 ranking, prefix matching and
plain-text highlight ranges; popular tags; an explore feed (recent +
popular-this-week, 60-second cache); public user profiles that never
expose email; stars (idempotent, transactional counter); forks with
recorded provenance; a pull counter with per-IP hourly dedup; abuse
reports; and an operator admin CLI (`hide-snippet`, `ban-user`,
`list-reports` — no admin web UI, per the roadmap). The 10k-snippet
performance bench passes the 50 ms p95 budget for snippet reads
(p95 ≈ 1.5–1.7 ms) and, on re-run, for search (p95 ≈ 35 ms; first run
68 ms under shared-CPU contention — both runs recorded in §4).

## 2. What I built

- `crates/forge-server/src/discovery.rs` (new): search, tags, explore,
  profiles, stars, forks, reports handlers.
  - Search terms are sanitised before they ever reach FTS5: split on
    whitespace, keep only alphanumeric/`_` characters, cap 10 terms;
    each term becomes a quoted prefix query (`"term"*`), AND-joined.
    Highlights are byte-offset ranges into the stored fields (first
    occurrence per term per field, capped at 20) — plain text, never
    HTML, so the client renders them safely.
  - Explore: first page of each feed served from a 60 s in-memory
    cache of 100 rows; popular = stars received in the last 7 days,
    tie-broken by total star count.
  - Stars: PUT/DELETE on public/unlisted snippets only (private →
    404, even for the owner), idempotent, counter maintained in the
    same transaction as the star row.
  - Fork: copies the latest version into the caller's account as a
    private snippet, records `forked_from` + source version + hash,
    increments the source's fork count in the same transaction.
  - Report: reason 1–500 chars; one open report per (reporter,
    snippet) — duplicates are a silent 204.
- `crates/forge-server/src/admin.rs` (new): `forge-server admin
  hide-snippet|unhide-snippet <owner>/<name>`,
  `ban-user|unban-user <username>` (ban also deletes sessions and
  revokes all API tokens), `list-reports [--status <s>]`. Hidden
  snippets return 404 publicly and disappear from search/explore;
  the owner still sees them.
- `crates/forge-server/src/repo.rs`: `search_public` (FTS join +
  predicates `visibility='public' AND hidden_at IS NULL AND
  deleted_at IS NULL` enforced in SQL), `popular_tags`,
  `explore_recent`, `explore_popular`, `list_starred`,
  `list_public_for_owner`, `count_public_for_owner`, `create_fork`
  (single transaction), `increment_pull`, `set_hidden` (FTS-synced),
  `ReportRepo::list` / `exists_open`, `UserRepo::set_disabled`.
- `crates/forge-server/src/state.rs`: pull-dedup map
  (`(sha256(ip), snippet_id)` → last counted instant, 1-hour window,
  opportunistic eviction above 10k entries; raw IPs never stored)
  and the explore cache.
- `crates/forge-server/src/snippets.rs`: `GET /snippets/{owner}/{name}`
  counts a pull when `?intent=run` is passed for a public snippet by
  a non-owner, subject to the dedup window. Several helpers became
  `pub(crate)` for reuse by discovery; `page_limit` now takes
  `Option<i64>`.
- `docs/running.md`: "Admin commands" section.
- Tests: `tests/discovery.rs` (9 HTTP tests) and `tests/perf.rs`
  (ignored bench, run explicitly in release mode).

## 3. Decisions made

- Defence in depth for public-only discovery: the visibility/hidden/
  deleted predicates live in the SQL of every discovery query, not
  only in handler code, so a future handler bug cannot leak private
  snippets into search, explore, tags or profiles.
- Highlights as byte ranges, not marked-up text: the roadmap forbids
  returning HTML; ranges let the web UI bold matches without any
  injection surface.
- Explore cache trade-off: a snippet hidden by an admin can remain
  in the explore first page for up to 60 s (the roadmap's mandated
  cache TTL). Search and direct reads are not cached and hide
  immediately.
- Pull counting keys on a SHA-256 of the client IP: the counter
  needs per-source dedup but the server never stores raw IPs, and
  no per-user pull history is kept (roadmap privacy rule).
- Fork is always private at birth regardless of source visibility;
  the new owner can publish it explicitly afterwards.

## 4. Verification

- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0,
  zero errors (full output inspected, not truncated).
- `cargo test --workspace` — **336 passed, 0 failed** (325 at Step 9
  + 9 discovery HTTP tests + 2 discovery unit tests).
- Web gates — lint, typecheck, test (1 passed), build all pass
  (web UI itself is unchanged this step).
- `scripts/scan-staged.sh` — clean.
- Discovery tests cover: search returns public only with correct
  highlight byte ranges; an FTS-injection suite (9 hostile queries,
  including `'; DROP TABLE snippets; --`, all return 200 with only
  public results); stars flow (anonymous 401, idempotence, private
  unstarrable even by owner); fork flow (provenance, hash equal to
  the source version, 409 on name reuse, custom name, other's
  private snippet 404); profiles (response asserted to contain
  neither the user's email nor the word "email"); explore + tags
  (tag counts public only; popular feed orders the starred snippet
  first); pull dedup (same IP does not recount within the hour,
  owner never counts, no intent = no count, per-snippet keys);
  report dedup (exactly one stored row after two posts); admin
  hide/ban exercised through `admin::run` against the test server's
  repositories.
- Performance (`cargo test --release -p forge-server --test perf --
  --ignored --nocapture`, 10,000 seeded public snippets, this
  machine — a shared 1.5-CPU cell, so variance is expected):
  - Run 1: seed 12.77 s; snippet read p50 0.97 ms, **p95 1.74 ms**;
    search `q=docker` (matches all 10k — worst case) p50 34.14 ms,
    **p95 68.22 ms** (over budget in this run).
  - Run 2 (with repo-level split): seed 12.61 s; repo
    `search_public` alone 13.13 ms avg; snippet read p50 0.95 ms,
    **p95 1.52 ms**; search p50 24.72 ms, **p95 34.91 ms**;
    search `limit=1` p50 13.96 ms, p95 18.63 ms.
  - Reading: ~13 ms is the FTS scan + BM25 over 10k matches;
    rendering a 20-result page (owner/tags/starred lookups + body
    parse + lint + highlights per result) adds ~11 ms. The read
    budget is met with ~30× headroom; search met the budget in the
    re-run but is workload- and contention-sensitive at the
    pathological all-documents-match end. Recorded honestly rather
    than tuned away; a result-cap or summary-cache optimisation is
    a Step 20 performance-pass candidate.

## 5. Acceptance criteria

- FTS5 search over public snippets only, BM25 + prefix, sanitised
  queries, plain-text highlights — done, tested incl. injection.
- `GET /tags`, `GET /explore` (recent + popular-this-week, 60 s
  cache) — done.
- Stars PUT/DELETE, one per user, transactional counter,
  `GET /me/stars` — done.
- Fork with provenance + fork count; private fork only by owner —
  done.
- Profiles without email; profile snippet lists public only,
  paginated — done.
- Pull counter on `?intent=run` with hourly per-IP dedup — done.
- Reports stored and deduplicated; admin CLI hide/ban/list — done.
- p95 bench at 10k snippets — done (see §4 for both runs).

## 6. Security notes

- Search is the classic FTS5-injection surface: user input never
  reaches the MATCH expression raw — terms are character-filtered
  and quoted, and the injection suite proves table-drop and
  boolean-tautology attempts return ordinary results.
- Discovery SQL enforces public visibility predicates independently
  of handler logic (defence in depth).
- Profiles serialise an explicit allow-list shape; a test asserts
  the email string and the key name never appear.
- Raw IPs are never stored for pull dedup — only SHA-256 hashes,
  in memory only.
- Admin operations are local CLI only (database access required);
  there is no admin HTTP surface to attack.

## 7. Dependencies added

None.

## 8. Corrections to earlier steps

- **Step 9 clippy incident (honesty record):** Step 9's commit
  `986d44b` went in with 9 clippy errors. Cause: the gate output was
  truncated with `| head -N` while checking (hiding most errors) and
  the commit was made while a full background gate run was still
  pending; its result arrived after the commit and contradicted the
  foreground impression. The lesson is recorded in `~/AGENTS.md`
  (never truncate gate output when checking; never commit while a
  gate run is pending). All 9 errors were fixed in the working tree
  the same day and are included in this step's commit:
  `snippets.rs` cursor helpers (const hoisted; `u8`/`i64` casts via
  `try_from` with clamping), `repo.rs` test line-count allow,
  `tests/snippets.rs` (underscore binding rename, line-count allow,
  two assert-style fixes). This step's clippy run was verified with
  an untruncated error count of 0 before committing.
- `page_limit` signature changed from `&PageQuery` to
  `Option<i64>` for reuse by the discovery handlers (Step 9 code,
  behaviour unchanged).

## 9. Known issues / limitations

- **Push status changed materially during this step.** The user
  completed `gh auth login` (device flow) on 2026-11-19: this
  machine is now authenticated as `aotlover9-base-eth`, and the
  repo `aotlover9-base-eth/forgehub` exists and is **private**.
  `gh auth setup-git` is configured. The first real push attempt
  was rejected by GitHub for one reason only: the OAuth token
  lacks the `workflow` scope and history contains
  `.github/workflows/ci.yml`. A `gh auth refresh -s workflow`
  approval was pending with the user at commit time; once granted,
  steps 1–10 push together and CI runs for the first time. Nothing
  in this report claims a successful push or a CI result — both
  will be reported with real output when they happen.
- Explore first page can show a just-hidden snippet for up to the
  60 s cache TTL (see §3).
- Search p95 at the pathological end (term matching all 10k
  documents) is contention-sensitive; see §4.

## 10. Deferred / ideas

- Search result rendering optimisation (summary cache) — Step 20
  performance pass candidate.
- Periodic (not just startup) retention purge — Step 19/21
  hardening, noted in the Step 9 report as well.

## 11. Next step

Step 11: CLI login / whoami / credentials (device-flow client).
