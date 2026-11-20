# Report - Step 2: specs, ADRs, threat model, API contract

- **Date/time:** 2026-11-19T09:43:43+05:30
- **Commit:** this report is part of the Step 2 commit - see `git log --oneline -n 1` (tag `step-2`)
- **Status:** ⚠️ Done with caveats - all documents written, cross-checked and validated; the only caveat is the carried-over push blocker (no GitHub authentication / remote URL in this environment, see §9), which affects Steps 1–2 equally.

## 1. Summary
Roadmap Appendices A–E are now precise, normative specifications. `docs/spec-snippet-format.md` defines the forge file (both modes, all limits, canonical serialisation and hashing byte-for-byte, Forgenesis compatibility, 9 worked valid examples plus invalid ones); `docs/spec-data-model.md` defines every table, the FTS5 design and migration policy with an ER diagram; `docs/openapi.yaml` is a complete OpenAPI 3.1 contract for all 31 paths that passes the real `openapi-spec-validator`; `docs/spec-cli.md` defines every command, exit code, `--json` shape, the name-resolution algorithm and the confirm/trust model; `docs/threat-model.md` maps 17 threats to mitigations and implementing steps; `docs/design-spec.md` expands Material 3 into a page inventory with per-width layouts and states. Six ADRs (0002–0007) record the mandated decisions. No code was written in this step, per its prompt.

## 2. What I built
- `docs/spec-snippet-format.md` - fields/types/limits table (64 KiB body, 256 steps, 20 tags, 280-char description, 24 h duration cap, retry 1–10); script and steps modes; step/condition/retry grammars; variable syntax and the 5-stage resolution order; name/username/tag regexes and the single reserved-word list; §9 canonical serialisation (fixed key orders, LF normalisation, trailing-whitespace policy for structural strings vs byte-preserved script body, `created`/`source` excluded, explicit-`shell` rule) and SHA-256/short-hash definition; YAML safety rules (no anchors/aliases/tags, depth ≤ 16, duplicate keys rejected); §11 examples 11.1–11.9 including the Ubuntu setup one-liner in both forge-file and raw-web-input form; §12 Forgenesis compatibility table (imports cleanly / converted / rejected); §13 differences from Appendix A.
- `docs/spec-data-model.md` - mermaid ER diagram; full column/constraint definitions for `users`, `sessions` (hashed PK), `api_tokens`, `device_codes`, `snippets`, `snippet_versions` (immutable, unique `(snippet_id, version)`), `snippet_tags`, `stars`, `renames` (30-day redirects), `reports`; FTS5 virtual table definition with the "public latest version only" rule; index list; forward-only migration policy and the two-step rule for destructive changes.
- `docs/openapi.yaml` - OpenAPI 3.1; 31 paths covering every Appendix B endpoint plus `DELETE /me` (needed by Step 17) and `GET …/versions/{version}`; 15 component schemas (error envelope, `Snippet`, `SnippetInput`, `ValidationResult` with secret findings that never carry values, pagination pages, highlight ranges as text offsets); `cookieSession` and `bearerToken` security schemes; CSRF header parameters (required for session-only mutations, optional-marked where bearer is an alternative); rate-limit behaviour in the API description; `ETag`/`If-None-Match` and `If-Match` optimistic concurrency on the snippet endpoints.
- `docs/spec-cli.md` - global flags and environment variables; exit codes 0/1/2/3/4/130; human and `--json` output shapes per command; all local commands (Steps 4–5) and cloud commands (Steps 11–13, 18) with flags; the 4-stage name-resolution algorithm (subcommand > local > own cloud > explicit `owner/name`) with the never-auto-run-a-suggestion rule; offline/cache behaviour; the normative confirm/trust table (own = `[y/N]`, third-party first time = typed `yes`, hash change = diff + reconfirm, `-y` limits, non-TTY fail-safe exit 2); display-escaping rules for untrusted text (ANSI, CR, bidi, zero-width) at every display site; execution semantics and startup budgets.
- `docs/threat-model.md` - 17 STRIDE threats (T-EXEC-1…4, T-ACCT-1…4, T-WEB-1…4, T-DOS-1…3, T-SECRET-1, T-SUPPLY-1…2, T-LOCAL-1, T-ABUSE-1, T-PRIV-1), each with scenario, mitigation and implementing step; plus an honest accepted-risks section (a user who confirms a bad snippet runs it; no account recovery in v1; the server operator can read the database).
- `docs/design-spec.md` - colour-restraint rules in reviewable form (primary only for actions/selection/focus; tertiary only on the Public chip; neutral tag chips; diff by gutter symbol + tone, not colour alone); navigation map; per-screen layouts for compact/medium/expanded with the loading/empty/error states each screen must design; the full token list (colour roles, type, shape, elevation, state layers, motion, spacing); component inventory for Step 14; accessibility requirements; microcopy tone.
- ADRs: `0002` SQLite vs Postgres, `0003` script vs steps modes, `0004` script execution via a private temp file (with the `read`/stdin and `ps`-exposure analysis), `0005` cookie sessions for web vs bearer tokens for CLI, `0006` hash/pinning, `0007` secret scanning.
- `docs/README.md` updated to index all of the above.

## 3. Decisions made
- **FTS rows are maintained by application code in the write transaction, not triggers** (data-model §3, ADR 0002). Appendix C said "maintained by triggers/app code"; triggers would duplicate the public-latest-only rule in SQL on every write path.
- **Canonical hash serialisation is a fixed-key-order JSON byte definition** (format §9), chosen over "hash the YAML" (quoting/CRLF churn would false-positive the trust diff) and over an unspecified AST hash (not independently implementable). See ADR 0006.
- **Mixed-case handling:** tags fold to lowercase; snippet names and usernames reject mixed case (format §8). Appendix A was silent; folding names would create silent collisions.
- **YAML anchors/aliases and tags are rejected** (format §10) as a parser-bomb defence (T-DOS-2); Appendix A was silent.
- **Rename is a `PATCH` field** on the snippet (openapi), not a separate endpoint; Appendix B listed renaming under Step 9's prompt but not in its endpoint table. The 30-day redirect semantics are defined in data-model (`renames`).
- **`DELETE /me` is specified now** (openapi) because Step 17 implements account deletion; its typed-username + password confirmation matches design-spec §3.
- **Retry and duration bounds** (1–10, ≤ 24 h) and a **256-step cap** were added where Appendix A left values unbounded (DoS hygiene, T-DOS-1).
- **`ETag` = content hash** on snippet GETs, specified now so Step 9 implements it and Step 20 can measure 304 revalidation.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass (no Rust code changed) |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| Tests | `cargo test --workspace` | 4 passed, 0 failed |
| Web | `npm run lint && npm run typecheck && npm test -- --run && npm run build` | pass (1 web test) |
| OpenAPI structure | python `yaml.safe_load` + endpoint/ref audit | 31 paths, 0 missing Appendix B endpoints, 23/23 `$ref`s resolve, 15 schemas |
| OpenAPI validity | `openapi-spec-validator` (venv install) `validate(doc)` | **pass** - after it caught and I fixed a real defect: the `ETag` header description was unquoted, so YAML parsed `for If-None-Match` as a key. First two install attempts (system pip, PEP 668 / a Debian `typing_extensions` conflict) failed; the venv install succeeded. |
| Consistency | scripted cross-check of limits/regexes/exit codes/seed across the docs | 12/12 checks pass (64 KiB, 280 chars, 20 tags, password ≥ 10, device 600 s / interval 5, exit codes, reserved words, both regexes byte-identical between format spec and openapi, ≥ 17 threat rows, `#36618E`, ≥ 8 examples) |
| Manual | read-through of all seven documents against each other and against Appendices A–E | inconsistencies resolved and listed in §8 |

## 5. Acceptance criteria
- [x] The seven documents exist, are mutually consistent, and cite exact limits. (Six files plus the ADR set; consistency proven by the scripted check in §4, not just asserted.)
- [x] OpenAPI file validates (or the validation method is documented). - Validated with `openapi_spec_validator` in a venv; method and the defect it found are in §4.
- [x] Threat model maps each threat to a step number. - Every row's last column is a step or step range; Step 19 re-verifies all rows.

## 6. Security notes
This step is where the security model is fixed: execution safety (T-EXEC-1…4), hashed sessions/tokens/device codes, CSRF design, 404-not-403 for private content, parser-bomb rules, and the shared secret scanner (ADR 0007). Two honest limits are recorded in the threat model itself: confirmation cannot save a user who confirms anyway, and private snippet bodies are readable by the server operator (no E2E encryption in v1).

## 7. Dependencies added
None. (`openapi-spec-validator` was installed into a throwaway venv under `/tmp` for validation only; it is not a project dependency.)

## 8. Corrections to earlier steps
- Step 1's report said SHA-pinning of CI actions is a Step 19 item; the Step 2 design/threat documents now also assign it to Steps 19/21 (T-SUPPLY-1) - consistent, no change needed to Step 1's files.
- No Step 1 code or document required correction.

Inconsistencies between the roadmap appendices that this step resolved (also in the specs): FTS maintenance mechanism (triggers vs app code); rename having no endpoint in Appendix B; account deletion having no endpoint in Appendix B; unbounded retry/durations; silent mixed-case policy; silent YAML anchor policy. Each resolution is listed in §3.

## 9. Known issues / limitations
- **Push blocker (carried over):** `git push origin main --follow-tags` still cannot run - there is no `origin` (`gh` is not authenticated on this machine and no remote URL has been supplied). Steps 1 and 2 are committed and tagged locally with clean trees. One user action (authenticate `gh`, or paste a private-repo remote URL) unblocks both pushes at once.
- The OpenAPI document is a contract, not yet an implementation: the route-vs-spec comparison test belongs to Step 9, per its prompt.
- `docs/known-gaps.md` does not exist yet; it is created in Step 7 per its prompt (account recovery is already flagged in the threat model).

## 10. Deferred / ideas
- Consider generating TypeScript types from `openapi.yaml` in Step 15 if a generator proves available offline (the Step 15 prompt already allows hand-written types kept in sync).
- A machine-readable copy of the reserved-word list will live in `forge-core` (Step 3); a test there should diff it against spec-snippet-format §8.

## 11. Next step
Step 3 implements `forge-core` against spec-snippet-format: the model for both modes, strict parsing/validation with line/column errors, the §9 canonical serialisation + SHA-256 with property tests, variable resolution, the secret scanner and danger lints (ADR 0007), the Forgenesis import layer, `scripts/scan-staged.sh`, ≥ 100 tests, and parse+hash benchmarks for a 64 KiB snippet.
