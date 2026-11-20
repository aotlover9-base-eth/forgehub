# Step 19 Report - Security hardening pass

## 1. Summary

An adversarial pass over the whole product, driven by `docs/threat-model.md`. Every one of the 21 threat rows was attacked against the running system and the results recorded in a new "Step 19 verification log" in the threat model itself. The pass found and fixed real defects: three terminal-escape holes (one in the security-critical changed-hash diff), a wrong status code for oversized bodies (400 → 413), and a **real dependency CVE** (SQLite WAL page-cache flaw) that is now eliminated from the tree. `cargo deny` and `cargo audit` are green and wired into CI alongside `npm audit`; all GitHub Actions are pinned to commit SHAs.

## 2. What was found and fixed

- **T-EXEC-3 terminal injection (3 holes).** (a) The changed-hash diff in the run confirmation card - the single most security-critical display in the product - printed diff lines raw; an attacker could hide the malicious change behind ANSI/CR sequences. Now escaped line-by-line (regression test in `cloudrun.rs`). (b) `fg diff` human output printed the unified diff raw; now escaped per line (the `--json` form was already safe by encoding). (c) The Step 18 TUI rendered names/descriptions/commands into the terminal buffer unescaped; all draw paths now pass through `escape_display`, proven by a ratatui `TestBackend` test asserting no raw ESC/bidi reaches the frame. `escape_display` itself was extended: bidi isolates U+2066–2069, U+061C, U+180E and soft hyphen U+00AD are now dropped too.
- **Oversized-body status (T-DOS-1).** The shared JSON-body helper mapped the body-limit rejection to 400 `validation_failed`; it now returns **413 `body_too_large`**, matching the snippet endpoints' shape (test in `security.rs`).
- **Dependency CVE (T-SUPPLY-2).** `cargo deny` flagged CVE-2026-119375: bundled SQLite < 3.54.0 (via `libsqlite3-sys` 0.30.1) can return uninitialised page-cache memory when opening a crafted DB+WAL pair. The CLI's history/trust databases used the bundled copy via rusqlite 0.32, and sqlx 0.8 pinned the same `libsqlite3-sys`, which also blocked the upgrade (cargo `links` conflict). Fixed properly: **rusqlite 0.32 → 0.41 and sqlx 0.8 → 0.9**, unifying the whole graph on `libsqlite3-sys` 0.39.1 (SQLite 3.54.1, patched). No advisory ignore was needed for the CVE - the vulnerable code is gone.
- **Retention purge schedule.** Soft-deleted snippets/rename redirects were purged only at server startup; a long-lived server now also purges every 6 hours (failure logged, never fatal).
- **Plain-HTTP registry warning.** `fg login` now warns on stderr before any credential is sent when the registry is `http://` to a non-loopback host (loopback/`localhost`/`::1` exempt), and `fg doctor` gained a matching "registry transport" check. Unit + integration tests cover the detection (including an IPv6-literal parsing bug the unit test caught and fixed).

## 3. Verification evidence (all run this step)

- **New server attack suite** `crates/forge-server/tests/security.rs` (9 tests): prod cookie flags (`Secure; HttpOnly; SameSite=Lax`), session fixation (planted cookie not honoured), cross-session CSRF token rejected, mismatched Origin rejected with a valid token, **X-Forwarded-For rotation does not reset the login throttle** (throttle keys on the socket address; 6th attempt 429 even with the correct password), hostile FTS queries (operators, unbalanced quotes, 10 000-char query, `DROP TABLE` string) never 500, NUL/ESC/bidi content never 500 and round-trips as inert text, oversized body → 413, forged bearer tokens → generic 401.
- **Parser bombs** (forge-core): billion-laughs anchor expansion and 200-deep nesting rejected in < 2 s; NUL/invalid-UTF-8 never panic; existing proptest properties (parser/scanner/lint no-panic, hash determinism) green.
- **Live header script** against the real server: CSP enforced (`default-src 'self'; script-src 'self'; frame-ancestors 'none'`), `nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy`, `Permissions-Policy` on SPA and API; the raw endpoint serves `text/plain` with `nosniff` + DENY (cannot be framed or sniffed as HTML).
- **Live admin smoke**: `hide-snippet` → snippet 404s and leaves explore; `unhide-snippet` restores it; `ban-user` → login 401 (sessions/tokens revoked); `list-reports` works.
- **Supply chain**: `cargo deny check` → *advisories ok, bans ok, licenses ok, sources ok* (config tightened from the Step 1 draft: yanked/wildcards/unknown sources now `deny`; path deps given explicit versions; CDLA-Permissive-2.0 allowed for webpki-roots cert data; three *unmaintained*-class ignores - chrono, unsafe-libyaml, paste - each justified in `deny.toml` and `docs/known-gaps.md`). `cargo audit` → 0 errors, 5 warnings (the three unmaintained crates plus two soundness notices for ratatui's pinned `lru` 0.12 - APIs ForgeHub does not exercise; resolves with a future ratatui bump; recorded in known-gaps via the deny notes). CI gained a supply-chain job running all three audits; every action is pinned to a commit SHA; workflow permissions stay `contents: read` (the audit job adds `checks: write` only).
- **Docs**: `SECURITY.md` (repo root: reporting, supported versions, scope) and `docs/security.md` (user/operator guidance) written; threat model gained the per-row verification log.

## 4. Tests and gates (real output)

- `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` - **0 errors**.
- `cargo test --workspace` - **408 passed, 0 failed** (Step 18: 391; +17 new tests).
- Web: lint clean, **45 tests passed** (web code untouched this step).
- `cargo deny check` green; `cargo audit` 0 errors / 5 documented warnings.

## 5. Deviations and problems

- **`npm audit --omit=dev` cannot run in this sandbox**: the environment's npm proxy rejects the audit endpoint (`policy_denied` from a managed-registry rule), even though the configured registry is npmjs.org. The audit therefore runs in CI (new supply-chain job), where the public endpoint is reachable; its first CI result will be verified with the Step 19 push, and any finding will be fixed inside Step 20's commit per the standing rule.
- **cargo-fuzz not used**: it needs a nightly toolchain; the roadmap's fallback (proptest-based fuzzing) is in place since Step 3 and was extended this step with the bomb corpus.
- An environment failure during this step (the 512 MB `/tmp` tmpfs filled by a leftover `cargo install` temp dir, silently breaking builds) was diagnosed and cleaned; the lesson is recorded in `AGENTS.md`.

## 6. Corrections to earlier steps

- Step 9/6 behaviour corrected: oversized request bodies now return 413 (see §2) - the 256 KiB limit itself always worked; only the error shape was wrong.
- Step 13/18 displays corrected: the three escaping holes in §2 were introduced in Steps 13 and 18 and are fixed here, in the step whose job is exactly this audit.

## 7. Next step

Step 20 - end-to-end and performance pass, with the checkpoint-review section in its report (per the run override): bundle-size pass on the web shell, search performance re-check, PTY-driven confirmation tests if feasible, `push --all`/`remote list` pagination, and any CI findings from this step's new supply-chain job.
