# Report - Step 3: forge-core shared library

- **Date/time:** 2026-11-19T09:59:02+05:30
- **Commit:** this report is part of the Step 3 commit - see `git log --oneline -n 1` (tag `step-3`)
- **Status:** ⚠️ Done with caveats - all Step 3 acceptance criteria met and proven below; the only caveat is the carried-over push blocker (no GitHub authentication / remote URL in this environment, see Step 1 report §9 and Step 2 report §9).

## 1. Summary
`crates/forge-core` is implemented against `docs/spec-snippet-format.md`: the two-mode snippet model with Forgenesis-compatible step/condition/retry/param normalisation, a strict non-panicking parser (UTF-8, LF normalisation, size/control-char/YAML-safety checks, line+column errors), name validators with the shared reserved-word list, the §9 canonical serialisation with SHA-256 hashing, variable resolution in the specified order with all-missing-at-once reporting, the ADR 0007 secret scanner with redaction, the danger lints, and the Forgenesis import layer. 196 forge-core tests pass (151 unit + 25 corpus + 5 integration + 5 proptest + 10 doctests), plus `scripts/scan-staged.sh`, which was proven in both directions: it exits 1 on a planted secret and 0 on this step's real diff.

## 2. What I built
- `src/model.rs` - `Snippet`/`StepSpec`/`Step`/`Condition`/`Retry`/`Param` types (bare strings and maps, command aliases `run`/`cmd`/`exec`, `- true` bool-step handling, top-level `parallel:` shorthand, workflow-default retry), `resolved_steps()`, `referenced_variables()` (including condition fields - see §8), and `validate()` enforcing every limit (64 KiB, 256 steps, 20 tags, 280-char description, 8,192-byte commands/lines, 64 variables, 128 env entries, retry 1–10, durations ≤ 24 h).
- `src/parse.rs` - `Snippet::from_bytes`/`from_yaml`: UTF-8 check, LF normalisation, control-character rejection (only `\n`/`\t` allowed), YAML anchor/alias/merge-key/tag rejection (structural pre-scan) plus a value-level depth (≤ 16) and tagged-value check, tag folding at the input boundary, strict serde parse with `deny_unknown_fields`, errors carrying parser line/column.
- `src/names.rs` - snippet-name/username/tag validators implementing the spec regexes rule-by-rule, tag folding + dedupe + limit, and `RESERVED_WORDS` (52 words) as the single source, with a test asserting every CLI subcommand is in it.
- `src/hash.rs` - canonical serialisation exactly per spec §9 (fixed key order via insertion-ordered JSON, structural-string trailing-whitespace trimming, script body preserved internally with exactly one trailing newline, `created`/`source` excluded, tags sorted/deduped, bare and map forms canonicalising identically) and SHA-256 + 8-char short hash.
- `src/vars.rs` - placeholder discovery, substitution (strict and lossy), built-ins (`HOME PWD DIRNAME DATE TIME TIMESTAMP USER`), `resolve()` in the spec order (CLI > defaults > built-ins) returning `MissingVariables` with **all** missing names, `KEY=value` parsing, `$VAR` path expansion, and `parse_duration` (ported from Forgenesis, plus the 24 h cap).
- `src/scan.rs` - secret scanner: private keys, AWS, GitHub (classic + fine-grained), GitLab, Slack, Stripe live secrets, bearer tokens, password literals, secret-named assignments, and high-entropy values **only** in assignment context and only with a token charset (see §3). Findings are `{kind, line}` - values are never stored or printed. `redact()` replaces detected values with `REDACTED`. Corpus tests live in `tests/scan_corpus.rs` (25 tests: every kind positively; placeholders, checksums, git SHAs, publishable/test Stripe keys, ordinary commands negatively).
- `src/lint.rs` - danger lints (warnings only): `rm -rf` at root/home, `dd` to block devices, `mkfs`, fork bomb, `chmod 777 /`, `curl|sh`, writes to `/dev/sd*`, `sudo`, `eval` of downloads; structured `{kind, line}` with plain-language messages.
- `src/import.rs` - Forgenesis conversion over YAML values: drops unknown top-level/step fields **with a note per drop**, lowercases names with a note, then re-parses through the strict ForgeHub path. Fixtures in `tests/fixtures/` (a full-featured file importing with zero notes; a legacy file with `packs`/`sync`/`plugin` dropped and noted).
- `examples/secret_scan.rs` (stdin scanner used by tooling), `benches/parse_hash.rs` (std-only timing harness, no criterion dependency), `scripts/scan-staged.sh` (scans added lines of the staged diff, excluding the `crates/*/tests/` corpora - the standard allowlist pattern, documented in the script).

## 3. Decisions made
- **Scanner false-positive rules were driven by dogfooding.** Running `scan-staged.sh` on this step's own diff initially produced ~20 findings; each was investigated and fixed in the scanner (not by allowlisting source files): (a) entropy now requires a token charset - prose in a `Cargo.toml` description and code expressions like `vec!["Ubuntu".to_string()]` are not tokens; (b) a variable literally named `KEY` is no longer "secret-named" (`KEY=value` in an error message is not a secret); (c) code-shaped password values (`password: Regex::new(...)` - the scanner's own field declarations) are not literals. What remains genuinely flagged is the test corpus, which moved to `tests/scan_corpus.rs` and is excluded by path in `scan-staged.sh`, with the reason written in both places.
- **YAML anchor defence is layered**: a structural pre-scan rejects anchors/aliases/merge keys/tags, and even a hypothetical miss is bounded by post-parse limits (input ≤ 64 KiB, depth ≤ 16, steps ≤ 256). The pre-scan is a heuristic over structural positions; that limitation is documented in `parse.rs`.
- **Tag folding happens in `from_yaml`** (the input boundary), while `validate()` on a programmatically built snippet rejects unfolded tags - parsing a document and building one are different boundaries, and the spec folds at input.
- **Canonical form hashes resolved semantics** (normalised steps with workflow retry applied, bare param = `{default}`), which is what makes the spec §9 invariance properties hold; the §9 byte definition itself is unchanged.
- **Benchmark harness is std-only** (the roadmap allowed "criterion or simple timing"); criterion would add ~30 crates to compile for one number.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass, zero warnings (pedantic set; all findings fixed in code - see below) |
| Tests | `cargo test --workspace` | **198 passed, 0 failed**: forge-core 151 unit + 5 integration + 5 proptest + 25 corpus + 10 doctests, forge-cli 1, forge-server 1 |
| Web | `npm run lint && npm run typecheck && npm test -- --run && npm run build` | pass (1 web test) |
| Benchmark | `cargo bench -p forge-core --bench parse_hash` | input 60,031 bytes: **parse+hash mean 2.62 ms, p95 3.66 ms** over 200 iterations (debug of the number: release bench build) |
| scan-staged (negative) | plant `export TOKEN=Zx9Qw8Er5Ty2Ui7Op4` in a staged file, run `scripts/scan-staged.sh` | exit 1, finding `line 1: secret_assignment` (kind + line only, no value) |
| scan-staged (positive) | run on this step's full staged diff | exit 0, "scan-staged: clean" |
| Fuzz-style | `tests/api.rs::garbage_never_panics` + proptest `parser_never_panics_on_arbitrary_text` | pass: empty/NUL/emoji/70 KB/alias-bomb inputs all return errors, never panic |

Clippy fixes worth naming (all in code, no lint suppression except one justified `#[allow]`): a temporary-borrow in canonicalisation, `clone_from` for retry defaults, integer p95 index math in the bench, a `# Panics` section on the two public functions containing an internal `expect`, an entropy-cast allow with the bound (≤ 8,192-byte lines) written next to it, and `assert_eq!(…, 0)` conversions in tests - one of which my own scripted conversion initially broke (`!x.len()`), caught by the test suite and fixed.

## 5. Acceptance criteria
- [x] At least 100 unit tests covering model, parsing, variables and hashing. - 151 unit tests in the library alone; 196 forge-core tests total.
- [x] Property tests prove hash determinism and invariance to formatting changes. - proptest: determinism, key-order invariance, command-change sensitivity, parser-never-panics, tag-fold idempotence; plus 19 targeted hash unit tests (quoting, CRLF, tag order/dupes, `created`/`source` exclusion, trailing spaces, script-body rules).
- [x] Secret scanner corpus tests pass. - 25 corpus tests (positive per kind, negative per placeholder/hash/ordinary case), all green; redaction covered.
- [x] `scripts/scan-staged.sh` runs the scanner over the git staged diff. - Proven both directions in §4.

## 6. Security notes
The scanner never carries secret values in findings (asserted by test). The parser rejects the YAML features that enable expansion bombs, caps every collection, and has no panicking path on hostile input (fuzz-style + proptest evidence). forge-core has no network or filesystem code; its dependency set is serde/serde_yaml/serde_json/sha2/regex/chrono/thiserror (plus proptest as a dev-dependency).

## 7. Dependencies added
Runtime (forge-core): `serde` 1 (derive), `serde_yaml` 0.9, `serde_json` 1 (preserve_order - required by the §9 fixed key order), `sha2` 0.10, `regex` 1 (linear-time engine; no catastrophic backtracking on hostile input), `chrono` 0.4 (clock only, for `DATE`/`TIME` built-ins - the same crate Forgenesis used), `thiserror` 2. Dev: `proptest` 1.

## 8. Corrections to earlier steps
- `referenced_variables()` initially missed variables referenced **only in step conditions** (e.g. an `equals:` guard); the full-fixture integration test caught it (`RESTART` missing), the function now scans condition fields too, and the test passes. This was a Step 3-internal fix, recorded here because it changes behaviour relative to the first Step 3 draft.
- No Step 1–2 files required correction. (The Step 2 spec's promise that a Step 3 test would diff the reserved-word list against the spec is implemented as a subcommand-coverage test in `names.rs`.)

## 9. Known issues / limitations
- **Push blocker (carried over):** Steps 1–3 are committed and tagged locally; `git push --follow-tags` still needs a GitHub remote (no `gh` auth, no URL supplied). One user action unblocks all three.
- The YAML anchor pre-scan is a structural heuristic (documented in §3); the layered limits bound any miss.
- The scanner's entropy rule will miss a high-entropy secret written with characters outside the token charset, and its password rule ignores values containing `( ) ,` or `::` - deliberate false-negative trades for near-zero false positives, consistent with ADR 0007's honesty note.
- `USER` built-in is absent when the process environment has no `USER` variable (documented in `vars::builtins`); a snippet referencing `{{USER}}` then reports it missing rather than guessing.

## 10. Deferred / ideas
- A real `cargo fuzz` target (libfuzzer) could replace the fuzz-style tests if the project later wants nightly fuzzing in CI (Step 21 hardening candidate).
- The scanner could grow a JSON output mode in the example when the server needs it (Step 9 returns findings through forge-core directly, so this is not needed yet).

## 11. Next step
Step 4 builds the local CLI core on this library: storage under `$FORGEHUB_HOME`, `init`, the recorder (with secret refusal using this scanner), the runner (conditions/retries/timeouts/background/parallel, script mode via ADR 0004's private temp file), `run` + shorthand with the confirmation card, and `list`/`show`/`search`/`tag`/`rename`/`delete`/`edit`.
