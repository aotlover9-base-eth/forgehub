# Peak C5 - Import helpers

Commit: `peak C5: import helpers` (hash recorded in PROGRESS.md after push)
Date: 2026-11-22

## What shipped

Two import paths, one shared pipeline each:

1. **History import (web).** `POST /api/v1/imports/history/analyze` takes a
   shell-history file's text (64 KiB cap, 413 over it) and runs the exact
   mining pipeline behind `fg analyze`. To make that true instead of a
   re-implementation, the pipeline moved from the CLI into forge-core as
   `history_mine` (`parse_history_text`, `filter_entries`,
   `segment_sessions`, `mine_sequences`, `mine_history_text`); the CLI's
   `analyze.rs` is now a thin shim over it, so CLI and server can never
   drift. The endpoint answers counts plus ranked candidates
   (name, commands, times seen). Secret-shaped commands are filtered by
   the marker screen and the secret scanner before anything is returned;
   an integration test seeds AWS-key and Bearer-token lines and asserts
   neither value appears anywhere in the response. The Library gains an
   Import panel: pick a history file, see the analysis note, rename and
   tick candidates, and import them as steps snippets through the normal
   create endpoint (each goes through full validation and secret scan
   again server-side).

2. **Gist import (web + CLI).** `POST /api/v1/imports/gist` imports a
   public GitHub Gist as private snippets; `fg import --from-gist <url>`
   is the CLI front end. The network surface is specified in
   **ADR 0010** before the code: the user supplies only a gist URL, the
   server extracts the id and constructs `{gist_api_base}/gists/{id}`
   itself (the base is operator config `FORGEHUB_GIST_API_BASE`, default
   `https://api.github.com`); redirects are allow-listed to the base host
   or `gist.githubusercontent.com`, at most 3; no credentials are ever
   sent (public gists only); caps are 10 s timeout, 1 MiB response,
   20 files, 64 KiB per file. Every file goes through the standard
   `parse_submission` pipeline (validation + secret scan); forge files
   keep their own name, other files are named from the filename.
   Collisions, truncated files, oversize files and secret findings are
   skipped with plain-language reasons in the response, never silently.

## Tests

- forge-core: +3 unit tests for the moved pipeline (parsing, secret
  filtering, sequence mining with suggested names). CLI analyze tests
  unchanged and green against the shim.
- Server: +2 unit tests (gist id parsing incl. hostile URLs, filename
  naming) and +3 integration tests in `tests/imports.rs`: history
  analyze mines candidates and never returns the seeded secrets, 413
  over the cap; gist import against a local mock GitHub API creates
  private snippets, honours forge-file self-naming, skips the
  secret-bearing file and the name collision with reasons; bad URLs are
  400, unknown gist 404, anonymous 401. Tests never touch the real
  GitHub.
- CLI: +1 integration test (`tests/gist_import.rs`) running
  `fg import --from-gist` against the real server with the mock API.
- Web: +2 tests (gist flow renders created/skipped; history flow
  analyzes and imports a candidate with the expected steps body).

A fixture note: the AWS documentation example key cannot be written
into this repo's test files as a contiguous literal - the authoring
pipeline redacts it (the same phenomenon behind the Step 21 incident).
The tests assemble it from pieces at runtime and say so in a comment.

## Gates (run locally, this machine)

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean (six findings fixed in-WP,
  including extracting `import_gist_files` and the config helper).
- `cargo test --workspace`: **517 passed, 0 failed** (508 at C4).
- Web: lint, typecheck, **74 tests passed** (12 files), build,
  check-tokens all green.
- OpenAPI gained both endpoints; the import man page was regenerated
  and the CLI reference updated.

## Notes

- C4's CI (run 44978917368 for `8c6f6c9`) is verified fully green; it
  was still pending when the C4 commit landed.
- The gist fetch uses reqwest with rustls native roots, promoted from
  dev-dependencies to a real server dependency for this WP.
