# Threat model

- **Status:** Living document. Written in Step 2; every row is re-verified with evidence in Step 19.
- **Assets:** user accounts and passwords; session cookies; API/device tokens; snippet content (private snippets are confidential); the user's local machine (the CLI runs code there - the highest-value asset); the server's database; the project's supply chain.
- **Actors:** malicious snippet author; account attacker; network observer (limited - TLS is assumed at deployment, Step 21); abusive registered user; curious non-owner user; compromised dependency.
- **Method:** STRIDE per threat. "Step" is the step that implements the mitigation; Step 19 verifies all of them.

| ID | Threat (STRIDE) | Scenario | Mitigation | Step |
|----|-----------------|----------|------------|------|
| T-EXEC-1 | Malicious public snippet (Elevation / Tampering) | A public snippet runs destructive or exfiltrating commands on a visitor's machine | Never auto-run; full numbered display of every command; owner, version, short hash in the card; `[y/N]` default No; third-party first run requires typed `yes`; danger lints (sudo, `rm -rf`, `curl\|sh`, `dd`, `mkfs`, fork bomb, …) shown as warnings; honest docs that this is real execution, not a sandbox | 13 (lints 3) |
| T-EXEC-2 | Snippet swapping after trust / TOCTOU (Tampering) | An author edits a popular snippet after users trusted it; the next `fg name` runs different code silently | Immutable versions; content hash (SHA-256, spec-snippet-format §9) recomputed locally and compared with the server's; trust is keyed to `(owner, name, hash)`; a changed hash shows a unified **diff** and re-requires confirmation; `-y` never bypasses a hash change; version pinning `@N` | 9, 13 |
| T-EXEC-3 | Hidden-text display attacks (Spoofing) | ANSI escapes, `\r`, bidi overrides or zero-width chars hide a second command in the confirm display | Display escaping rules (spec-cli §8) applied at **every** display site; malicious fixtures tested in Steps 13, 18, 19 | 13, 19 |
| T-EXEC-4 | Typosquatting (Spoofing) | `fg alice/ubuntu-setup` vs a lookalike owner/name | Reserved words; ASCII-only lowercase names; suggestions are never auto-run (spec-cli §6); the card always shows the exact owner; search shows owner prominently | 3, 9, 13 |
| T-ACCT-1 | Account takeover via credential stuffing / brute force (Spoofing) | Password guessing against login | argon2id (tuned ~50–100 ms), per-IP **and** per-account throttling with backoff, generic failure messages, dummy hash on unknown user, common-password denylist, minimum length 10 | 7 |
| T-ACCT-2 | Session hijacking / fixation (Spoofing / Elevation) | Stolen or fixated session cookie | 256-bit random session ids, only SHA-256 stored; `HttpOnly`, `Secure` (prod), `SameSite=Lax`; rotation on login and on password change; idle + absolute expiry; logout-all; password change revokes other sessions | 7 |
| T-ACCT-3 | Token theft (Information disclosure) | CLI token leaks from disk, logs or a paste | Tokens are 256-bit random, stored **hashed**, shown once; credential file `0600` in a `0700` dir, atomic writes, permission check on read; tokens never logged or printed (`whoami` shows prefix only); scoped (`read`/`write`); revocable, revocation is immediate; tokens cannot mint/revoke tokens or change the password | 8, 11 |
| T-ACCT-4 | Device-code phishing / brute force (Spoofing) | Attacker guesses a `user_code`, or tricks a user into approving the attacker's CLI | User codes: 8 chars from an unambiguous alphabet, 600 s expiry, single use, attempt lockout per code and per IP; approval page (cookie + CSRF) shows the client name and scope in plain language and never auto-approves; polling interval enforced server-side (`slow_down`); device codes stored hashed | 8, 17 |
| T-WEB-1 | XSS via user content (Tampering / Elevation) | Snippet name/description/body, bio or search highlight containing HTML/JS runs in another user's browser | All user content rendered as text (React escaping); never `dangerouslySetInnerHTML` with user data; highlights returned as text ranges, not HTML; CSP with no inline scripts; hostile-content tests | 9, 10, 16, 19 |
| T-WEB-2 | CSRF (Tampering) | A malicious site submits a mutating request with the victim's cookie | Origin/Host check **and** per-session `X-CSRF-Token` on every cookie-authenticated mutation; `SameSite=Lax`; bearer-token requests are exempt by design (no ambient authority) | 7 |
| T-WEB-3 | Private-snippet enumeration / existence leak (Information disclosure) | Probing `owner/name` to learn whether a private snippet exists | Private snippets return **404, not 403**, to everyone but the owner, on every endpoint including versions, diff, raw, star and fork; search/explore/profiles only ever expose public snippets; unlisted snippets are readable by direct address only and never listed | 9, 10 |
| T-WEB-4 | User enumeration (Information disclosure) | Signup/login responses reveal whether an email or username exists | Generic auth errors; the one unavoidable case (choosing a taken username at signup) is documented in Step 7's report; timing blunting via dummy hash on login | 7 |
| T-DOS-1 | Large-body / quota abuse (Denial of service) | 64 KiB+ bodies, snippet/version spam, expensive requests | Hard size limits (spec-snippet-format §1); request body limit 256 KiB (Step 6); per-user quotas (500 snippets, 200 versions/snippet, 25 tokens); rate limits on signup, login, device, token, search, push and report endpoints with `Retry-After` | 6–10 |
| T-DOS-2 | Parser bombs (Denial of service) | YAML anchors/aliases (billion laughs), deep nesting, pathological FTS queries | Anchors/aliases and YAML tags rejected; nesting depth ≤ 16; step caps; FTS input sanitised to quoted terms (no user-controlled operators); search length limits and rate limits | 3, 10, 19 |
| T-DOS-3 | Regex / lint denial of service | Secret scanner or lints run on hostile input with catastrophic patterns | Scanner uses bounded, non-backtracking patterns over a ≤ 64 KiB input; scanner performance is benchmarked in Step 3 and fuzz-tested in Step 19 | 3, 19 |
| T-SECRET-1 | Secrets committed into snippets (Information disclosure) | A user records/pushes a command containing a real token or password, making it permanent and possibly public | Secret scanning in `forge-core` runs at `record` (refuse), at `push` (CLI refuses; server rejects 422 with line/kind, never the value), and in the web editor (blocking findings); findings never echo the secret | 3, 4, 9, 12, 16 |
| T-SUPPLY-1 | Installer / release supply chain (Tampering) | A tampered CLI binary or `install.sh` runs on user machines | Installer downloads over HTTPS and **verifies SHA-256 checksums** before installing; releases publish checksums (and attestations where feasible); the installer is short, commented and reviewable, and never edits shell rc files without asking; `cargo deny`/`cargo audit`/`npm audit` gates; CI and release actions pinned by SHA (Step 19/21) | 19, 21 |
| T-SUPPLY-2 | Dependency compromise (Tampering) | A malicious crate/npm release enters the build | Lockfiles committed; `cargo deny` (licences, advisories, bans) and audits reviewed from Step 19; boring, well-maintained dependencies only, each justified in its step report (agent.md R12) | 19 (all steps) |
| T-LOCAL-1 | Local state tampering / symlink attacks (Tampering) | Another local process or a malicious archive replaces workflows, the temp script, or credentials | Script temp dir `0700` with `O_EXCL` file creation (ADR 0004); credential permission checks; restore never overwrites without confirmation and backups never contain credentials; `~/.forgehub` perms checked by `doctor` | 4, 5, 11, 19 |
| T-ABUSE-1 | Spam / abuse content (Repudiation / DoS) | Mass public snippets, abusive content | Report endpoint (rate-limited); admin CLI to hide snippets and ban users (Step 10); hidden content returns 404 publicly; signup can be closed (`FORGEHUB_ALLOW_SIGNUP=false`) | 6, 10 |
| T-PRIV-1 | Tracking / pull-count misuse (Information disclosure) | Popularity numbers become per-user tracking | `pull_count` is an aggregate counter only, deduplicated per IP+snippet per hour, with **no per-user pull history stored**; no telemetry anywhere (agent.md R7) | 10 |

## Step 19 verification log (2026-11-19)

Every row was attacked against the running system (local only). Evidence
lives in `crates/forge-server/tests/security.rs`, `crates/forge-cli/tests/step18.rs`,
the unit/property suites, and `reports/report_Step19.md`.

| ID | Attack attempted | Result |
|----|------------------|--------|
| T-EXEC-1 | Reviewed the run path end to end | Card shows owner/version/hash + every command numbered; typed `yes` for third parties; `-y` matrix unit-tested (Steps 13/18 suites green) |
| T-EXEC-2 | Hash-change flow re-tested | Changed hash → diff + re-confirm; `-y` never bypasses (Step 13 tests); the diff display itself was found **unescaped** - fixed this step (see T-EXEC-3) |
| T-EXEC-3 | ANSI/CR/bidi/zero-width payloads in names, descriptions, bodies, diffs - at every display site (`show`, card, `diff`, `pull`, `remote`, TUI) | **Three holes found and fixed:** the changed-hash diff in the confirmation card, `fg diff` human output, and all TUI rendering (ratatui `TestBackend` test proves no raw ESC/bidi reaches the frame). `escape_display` extended to bidi isolates U+2066–2069, U+061C, U+180E, U+00AD |
| T-EXEC-4 | Lookalike names | Names/usernames are ASCII-lowercase by validation on both tiers; card always shows the exact owner; suggestions never auto-run (Step 13) |
| T-ACCT-1 | Credential stuffing simulation; X-Forwarded-For rotation to reset the throttle | Throttle keys on the socket address (ConnectInfo), never XFF - rotating XFF does not buy attempts; 6th attempt is 429 even with the correct password (`security.rs`). Generic failures + dummy hash (Step 7 tests) |
| T-ACCT-2 | Session fixation (planted cookie at login); cookie-flag audit in prod mode | Login always issues a fresh id (test); prod cookies carry `Secure; HttpOnly; SameSite=Lax; Path=/` (test) |
| T-ACCT-3 | Forged bearer tokens; token scope abuse | Forged/malformed tokens → generic 401 (test); tokens cannot manage tokens or change password (`session_required` 403, Step 8/17 tests); CLI never prints tokens; credential-file permission checks (Step 11) |
| T-ACCT-4 | Device-code guessing / approval confusion | Lookup lockout per IP (Step 8 tests); approval page shows client name + plain-language scope, never auto-approves (Step 17); poll `slow_down` enforced (Step 8) |
| T-WEB-1 | XSS payloads in every field; highlight injection | React text rendering everywhere, highlights as byte ranges (Step 16/17 tests incl. hostile content); CSP enforced `script-src 'self'` verified live by header script |
| T-WEB-2 | CSRF: missing token, another session's token, mismatched Origin | All rejected 403 (tests, incl. two new cross-session/Origin cases in `security.rs`) |
| T-WEB-3 | Private-snippet probing on read/versions/diff/raw/star/fork | Uniform 404 (Step 9/10 tests); hidden-by-admin snippets also 404 and leave explore (live admin smoke, Step 19) |
| T-WEB-4 | Signup/login enumeration | Generic errors; username-taken is the documented exception (Step 7) |
| T-DOS-1 | 300 KiB request body; quota checks | Rejected by the 256 KiB limit - the rejection surfaced as **400, fixed to 413 `body_too_large`** this step (test). Quotas enforced (Step 9/10) |
| T-DOS-2 | Billion-laughs YAML, 200-deep nesting, hostile FTS queries (`OR`/`NEAR`/unbalanced quotes/10k chars/`DROP TABLE` string) | All rejected or handled without 500 (new forge-core bomb tests; `security.rs` search tests). Anchors/aliases/tags rejected pre-parse; depth ≤ 16 |
| T-DOS-3 | Scanner/lints on hostile input | Bounded patterns over ≤ 64 KiB; proptest no-panic properties (parser, scanner, lints) green |
| T-SECRET-1 | Pushing snippets with planted secrets | Scanner refuses at record/push; server 422 with kind+line only (Steps 3/9/12 tests); findings never contain values (corpus tests) |
| T-SUPPLY-1 | Installer/release chain | Prepared in Step 21; this step: CI actions pinned to commit SHAs, minimal workflow permissions, `cargo deny`/`cargo audit`/`npm audit` jobs added |
| T-SUPPLY-2 | Dependency audit | **Real CVE found and fixed:** SQLite WAL page-cache flaw (CVE-2026-119375) via bundled `libsqlite3-sys` 0.30.1 → upgraded rusqlite 0.41 + sqlx 0.9, unifying on libsqlite3-sys 0.39.1 (SQLite 3.54.1). `cargo deny check` green (advisories/licenses/bans/sources); three unmaintained-crate ignores documented in `deny.toml` + known-gaps. `npm audit --omit=dev` runs in CI (this sandbox's npm audit endpoint is policy-blocked; recorded in the Step 19 report) |
| T-LOCAL-1 | Local state attacks | Temp script dir/file 0700 with drop-guard cleanup (Step 4); credential perms enforced on read (Step 11); backup/restore never touches credentials and verifies hashes before writing (Step 5); `doctor` checks home hygiene |
| T-ABUSE-1 | Spam/abuse handling | Report endpoint deduped + rate-limited (Step 10); admin hide/ban/list-reports exercised live this step (hide → 404 + explore removal; ban → login 401, sessions/tokens revoked) |
| T-PRIV-1 | Pull-count tracking review | Aggregate counter only, IP-hash hourly dedup, no per-user history stored (Step 10); no telemetry anywhere |

Additional Step 19 hardening landed with the verification: retention purge
now also runs every 6 h in-process (was startup-only), and the CLI warns at
login (and in `doctor`) when a registry is plain `http://` to a non-loopback
host.

## Out of scope / accepted risks (honest list)

- A user who reads a malicious snippet, understands it, and still confirms **will** run it. That is the product working as designed; the mitigations are visibility and consent, not prevention.
- TLS termination and host security are deployment concerns (Step 21 documents Caddy/Fly setups); the application assumes HTTPS in production and sets `Secure` cookies there.
- Account recovery does not exist in v1 (no email sending): a lost password means a lost account. This is documented in `docs/known-gaps.md` from Step 7, not hidden.
- The server operator can read the database, including private snippet bodies (they are not end-to-end encrypted). The web UI and docs must never claim otherwise.
