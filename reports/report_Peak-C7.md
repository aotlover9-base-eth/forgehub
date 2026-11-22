# Peak C7 - Per-user activity log

Commit: `peak C7: activity log` (hash recorded in PROGRESS.md after push)
Date: 2026-11-22

## What shipped

An append-only, per-user activity log (migration `0005_activity.sql`,
`ActivityRepo`), shown in the web app under **Settings - Security**,
readable from the CLI with **`fg activity`**, and exportable as JSON
(`GET /me/activity/export`, a `forgehub-activity.json` download; the
web button and the API agree on shape).

Events recorded server-side, at the point they happen:

- `login` - every session start (password login, signup, device
  flow), with `via`, IP, and a **`new_device` flag**: true when this
  user agent has never signed in before (exact-match lookup against
  past login events; the UA is stored in its own column for this).
- `snippet_created`, `snippet_pushed` (real new versions only - the
  hash-unchanged PUT path does not count), `snippet_forked` (with
  provenance), `token_created` (name/prefix/scopes), `token_revoked`
  (both the by-id and the self-revoke paths).

`snippet_trusted` is the one event the server cannot observe: trust
grants happen on the client. So the CLI reports them - `POST
/me/activity` accepts **only** that kind (allow-list), with a
validated detail (`ref` required, 512-byte cap). The CLI reports a
fresh grant after a confirmed cloud run whose content was not
already trusted at that hash, and after an explicit `fg trust`;
already-trusted re-runs report nothing (proven by test: two runs,
one event). Reporting is best-effort by design: it never fails a
run, never happens offline, and the local trust store remains the
source of truth.

Recording itself is also best-effort: a failed insert is logged and
swallowed, never allowed to fail the operation that caused the
event. The log is strictly per-user: every read is scoped to the
caller, and the tests prove one user never sees another's events.

## Tests

- Server integration (`tests/activity.rs`): +2 tests - the full
  event set with details (signup login flagged new-device, repeat
  UA not flagged, a second UA flagged; create/fork; token
  create/revoke; limit honoured) and the privacy/allow-list matrix
  (other user's log contains only their own events, anonymous 401,
  non-allow-listed kinds and missing/empty refs are 400, export is
  a named JSON download with the right user).
- CLI integration: +1 test in `tests/cloudrun.rs` - push, first
  confirmed run, then `fg activity` shows `snippet_created`,
  `snippet_trusted` (ref=anna/cloudy) and `login`; a second run of
  the same content adds no second trust event (checked via the API).
- Web: +1 test - the Security tab renders the log with
  plain-language lines ("Trusted bob/setup (version 2)", "Signed
  in via login from a new device") and the export button.

## Gates (run locally, this machine)

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean (one doc-backtick fix in-WP).
- `cargo test --workspace`: **525 passed, 0 failed** (522 at C6).
- Web: lint, typecheck (exit-code verified), **76 tests passed**,
  build, check-tokens all green.
- OpenAPI gained the three activity paths and the `ActivityEvent`
  schema; man pages regenerated (new `forgehub-activity.1`); the
  CLI reference gained the `forgehub activity` section.

## Notes and known limits

- The log has no retention pruning yet: one row per user action is
  small, the list/export are capped (200 / 1000), and pruning is a
  safe future addition. Recorded in the Phase C summary as a gap.
- `run_fetched` gained an `Option<&Session>` parameter so trust
  reporting only happens on the online path; the offline cache path
  is untouched and makes no network calls, as before.

## Also in this commit

- PROGRESS.md records C6's CI verification (run 44982188337, fully
  green, all 5 jobs) which completed after the C6 commit landed.
