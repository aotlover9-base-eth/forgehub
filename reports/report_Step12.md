# Step 12 Report - CLI push, pull, remote list, fork

## 1. Summary
The CLI can now sync with the cloud: `push` (create/new version
with real optimistic concurrency), `pull` (with provenance
sidecars and overwrite protection), `status` (ahead/behind/
diverged by hash), `diff`, the `remote` management family,
`fork`, and a `record --push` hook. The critical acceptance -
CLI hash == server hash for the same content - is asserted by
integration tests against the real server.

## 2. What was built
- **`crates/forge-cli/src/sync.rs`** (new, ~1,350 lines):
  session resolution over Step 11's credentials; an
  `api_fail` mapper implementing the roadmap error UX (401 →
  exit 4 with `forgehub login`; 404 noting private snippets are
  indistinguishable; 412 → conflict text naming `diff`/`pull`;
  422 → the secret findings listed; 429 → the `Retry-After`
  wait; network → offline note).
  - `push_one`: local `validate()` + forge-core secret scan
    first (findings refuse the push, exit 2, nothing uploaded);
    create defaults to private; updates send `If-Match` with the
    **last-synced version from the provenance sidecar** - not
    the version just read - so a remote that moved since the
    sync produces the server's real 412. No sidecar baseline +
    differing remote → local conflict error naming the recovery
    commands. Identical hash → "up to date" (visibility PATCH
    still applied when a flag was given). `push --all` prints a
    per-snippet summary and continues past failures.
  - `pull`: parses `owner/name` or bare name; fetched body is
    treated as untrusted - parsed and validated with forge-core
    before saving; overwriting an existing local snippet shows
    the escaped preview card and requires confirmation, or
    `--force`; non-TTY without either → exit 2. Writes the
    `.meta.json` sidecar (`{registry, owner, name, version,
    hash}` per spec-cli).
  - `status`: state machine over (local hash, remote hash,
    sidecar hash) → unchanged/ahead/behind/diverged/local-only/
    remote-only (unit-tested matrix); bare `status` also lists
    remote-only snippets from `/me/snippets`.
  - `diff`: unified diff (remote → local) via the `similar`
    crate.
  - `remote list [--stars]`, `remote show`, `remote delete`
    (confirmation listing what will be deleted; sidecar cleaned
    up), `remote visibility` (going public requires confirmation
    with a sensitive-content review reminder), `remote search`.
  - `fork`: server fork, then an optional TTY pull prompt.
  - `record --push`: pure decision function (not logged in →
    note + skip; non-TTY without `-y` → skip; else ask
    "Push to cloud now? [y/N]") wired through a new
    `cmd_record_inner` that also returns the saved name.
- **`cloud.rs`**: `ApiClient::request` (PUT/PATCH/DELETE with
  extra headers, e.g. `If-Match`); `ApiFailure` now carries the
  envelope `details` (boxed) and the parsed `Retry-After`;
  `active_credential` is `pub(crate)` for reuse.
- **`store.rs`**: sidecar path/read/write/remove helpers
  (atomic writes like snippets).
- **`main.rs`**: new subcommands wired; dispatch split into
  `dispatch_cloud` / `dispatch_manage` / `dispatch_record` /
  `dispatch_remote` helpers (also fixing `too_many_lines`).

## 3. Design decisions
- **If-Match baseline = sidecar version.** Using the freshly
  read remote version as the ETag would make conflicts
  impossible and silently overwrite other machines' work -
  the exact failure optimistic concurrency exists to prevent.
  The first sync test caught this (a stale push succeeded);
  the semantics were fixed and the conflict test now passes.
- Pull preview is shown only when overwriting (a fresh pull
  cannot destroy anything); JSON mode never prompts - it
  refuses instead, so scripts fail safe.
- `status` without a sidecar can only say unchanged/diverged;
  that limitation is documented in the code.

## 4. Tests
- 8 new integration tests (`crates/forge-cli/tests/sync.rs`)
  driving the real binary against the real server:
  push/pull round trip with **hash equality asserted**
  (server hash == `forge_core::hash::content_hash` of the
  local file), up-to-date re-push, conflict (another writer
  PUTs v3; stale push → exit 1 with recovery text; `diff`
  shows the change; `pull --force` recovers), status states
  (ahead/behind/diverged), secret refusal (exit 2; server has
  nothing), login-required errors (exit 4), pull overwrite
  protection, remote management + fork (fork survives source
  deletion; visibility changes), `push --all` summary.
- 4 new unit tests in sync.rs (reference parsing, sync-state
  matrix, record-push decisions, URL encoding).
- **Workspace total: 361 passed, 0 failed** (was 349).

## 5. Gates (agent.md §9) - real output
```text
$ cargo fmt --all --check            → clean
$ cargo clippy --workspace --all-targets -- -D warnings
  → 0 errors (full output read; see §8)
$ cargo test --workspace             → TOTAL: 361 passed, 0 failed
$ web: lint ✓ · typecheck ✓ · test 1 passed ✓ · build ✓ (445 ms)
$ scripts/scan-staged.sh             → clean
```

## 6. Acceptance check (roadmap Step 12)
- [x] CLI hash == server hash for the same content (tested).
- [x] Optimistic-concurrency conflicts reported and recoverable
  (tested end-to-end).
- [x] No local data overwritten without confirmation/`--force`
  (pull + record paths tested).
- [x] Error UX for 401/404/409/412/422/429 and offline.

## 7. Known gaps / risks
- `push --all` and `remote list` cap at one 100-item page
  (server caps: 500 snippets/user; cursor following is a
  Step 18 polish candidate).
- The printed web URL uses the profile-style path
  `{registry}/{owner}/{name}`; the web routes land in
  Steps 15–17.
- `record --push` is covered by unit tests for its decision
  logic and by the push integration tests for the push itself;
  the interactive record capture is not exercised end-to-end
  (it needs a live terminal session).

## 8. Corrections to earlier steps
- Process correction (no code behaviour change): this step's
  first clippy run was taken from a truncated/backgrounded
  pipeline and read as green; the full untruncated run showed
  16 errors (pedantic lints in the new code, a dead
  `cmd_record` wrapper, `result_large_err` on the enlarged
  `ApiFailure`). All were fixed at the root (boxed details,
  extracted helpers, `write!` over `push_str(&format!)`,
  `clone_from`, dispatch split) and the gates re-run in the
  foreground to a verified 0 errors before this commit - the
  AGENTS.md lesson applied to itself.

## 9. Push / CI status
- Commit `step 12: CLI push, pull, remote list, fork`, tag
  `step-12`, pushed; CI result recorded in the step summary
  given to the user (any red run is fixed inside Step 13's
  commit per the run instructions).

## 10. Next step
Step 13 - run from cloud with the full safety UX (trust store,
typed `yes`, changed-hash diffs, offline cache). Per the
user's checkpoint override for this run, Step 13's report
will carry a "Checkpoint review" section instead of stopping.
