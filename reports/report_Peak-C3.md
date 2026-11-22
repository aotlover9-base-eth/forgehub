# Report - Peak C3: run history and rollback

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** C3 (Phase C - new features).
- **Status:** Done. `fg history` and `fg rollback` for cloud
  snippets, plus a Roll back action in the web snippet
  page's Versions tab. No server changes were needed - the
  versions, diff and PUT endpoints already existed; the WP
  is the CLI surface, the web action, and their tests.

## 1. Semantics

Versions are immutable and history is never rewritten. A
rollback does not move a pointer and does not delete
anything: it publishes the target version's content as a
NEW version (message `Rollback to v<n>`), through the same
If-Match-guarded PUT as every other publish. Rolling back
to the content you already have is a no-op (detected by
hash comparison, reported, exit 0, no version created).

## 2. CLI (new module `history_cmd.rs`)

- `fg history <name|owner/name> [--limit N]` - newest
  first: version, date, short hash, message, `(current)`
  marker; `--json` prints the raw version array. Other
  users' readable snippets work (versions are authored by
  the owner in this model, and the header says so). A
  local-only name - logged in or out - prints the honest
  explanation ("local files have no version history") with
  the push tip, exit 0, instead of a bare 404 or an auth
  error. `history` was added to `KNOWN_COMMANDS` (the B9/C1
  shorthand lesson).
- `fg rollback <name|owner/name> <version>` - own snippets
  only (a foreign ref is refused client-side with the
  reason; the server would refuse anyway). Flow: fetch
  latest + target, hash no-op check, fetch and render the
  diff (current -> target, via the server diff endpoint and
  the house `render_diff`), state plainly that a NEW
  version will be published, confirm `[y/N]` (TTY required
  without `-y`, mirroring delete), then PUT with
  `If-Match: <latest>`. A concurrent push in between lands
  on the standard 412 recovery message from `api_fail`.

## 3. Web

- New `useRollbackSnippet` hook: GET the target version,
  PUT its body with If-Match on the current latest and the
  `Rollback to v<n>` message; invalidates snippet, versions
  and library queries.
- SnippetPage Versions tab: each non-current version gains
  a **Roll back** button for the owner only, opening a
  dialog that states the semantics (new version published,
  current version kept) before confirming; success toasts
  the new version number.

## 4. Tests

- CLI integration (+4, `tests/history.rs`, real server):
  history ordering + current marker + foreign read;
  rollback publishes v3 with v1's content and the right
  message while v2 remains in the list; repeat rollback is
  a no-op (still 3 versions); missing version exits 3;
  foreign rollback refused; local-only history explains
  itself.
- Web (+1): the rollback flow in the Versions tab - the
  test initially clicked the wrong "Roll back" (the
  dialog's confirm shares the label and renders before the
  rows); it now picks the row button by ancestry, confirms
  in the dialog, and asserts the PUT's If-Match header,
  body and message.

## 5. Docs

- Man pages regenerated (`forgehub-history.1`,
  `forgehub-rollback.1`, top-level updated);
  `docs/cli-reference.md` gained the header paragraph,
  glance entries and both help sections. No OpenAPI change
  (no new endpoints).

## 6. Gates

Local: `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets -- -D warnings` clean;
`cargo test --workspace` 505 passed / 0 failed (501 at C2
+ 4). Web: typecheck clean (one narrowing fix in the new
test), lint clean, snippet suite 7/7 including the new
rollback test; the full web suite runs in CI. CI
verification of this commit's head is recorded in
PROGRESS.md.

## 7. Next

C4 - public snippet pages.
