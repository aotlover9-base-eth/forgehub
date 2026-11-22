# Peak Phase C - summary

Phase C (new features, C1-C10) is complete: ten work
packages, each one commit with its own report, each
verified green in CI on its own head before the next
began. Tag: `peak-phase-c`.

## The work packages

| WP | Commit | What shipped | CI run |
|----|--------|--------------|--------|
| C1 | `b30647d` | Collections: ordered snippet runbooks (migration 0003, 8 endpoints, `fg collection`, `fg run <collection>`, web panel + page). A collection never widens access. | 44976274680 green |
| C2 | `5c32aa8` | Typed parameters: `type:` on `variables:` (text/choice/number/path/yesno/secret), validated at run time, secrets hidden and defaultless. | 44977381414 green |
| C3 | `710ecc3` | History + rollback: `fg history`, `fg rollback` publishes a NEW version (history never rewritten), web rollback UI. | 44978258087 green |
| C4 | `8c6f6c9` | Public snippet pages: server-rendered `/s/{owner}/{name}` for public snippets, one shared 404 for everything else. | 44978917368 green |
| C5 | `1fb9e89` | Import helpers: shell-history mining moved to forge-core, `POST /imports/history/analyze` (secrets filtered server-side), Gist import under ADR 0010 (operator-set API base, strict caps, mock-tested). | 44980899065 green |
| C6 | `90ea9e6` | Shared collection links under ADR 0011: `fgl_` tokens (hashed, shown once), 1/7/30-day expiry, instant revocation, anonymous read page at `/shared/{token}`. | 44982188337 green |
| C7 | `18a4c33` | Activity log (migration 0005): sign-ins with new-device flags, snippet/token events, client-reported trust grants, `fg activity`, Settings card, JSON export. Strictly per-user. | 44983374639 green |
| C8 | `2c31225` | Usage stats: cloud runs reported per runner (migration 0006), `GET /me/stats` counts only your own runs, Library "Your library" card; `fg stats` documented local-only. | 44984362296 green |
| C9 | `0224421` | i18n catalogue (~430 strings, 24 sections, compile-checked structure), full UI sweep with byte-identical rendered copy, RTL test page at `__rtl`, docs/i18n.md. | 44985571118 green (head `ed946c4`, includes `0224421`) |
| C10 | `2d3710e` | `forgehub self-check` (binary hash always; archive verification against published SHA256SUMS online or `--sums` offline; honest manual flow when unreachable) + the Peak CHANGELOG. | 44986239180 green |

## Test counts

- Rust workspace: 481 (Phase B exit) -> **535 passed,
  0 failed** at C10.
- Web: 74 (C5) -> **82 passed (15 files)** at C9/C10.
- Every Phase C feature carries authorisation tests
  (cross-user 404/401/403 cases), failure-path tests and
  an OpenAPI + docs update, per the phase exit gates.

## Design decisions recorded

- ADR 0010 (C5): server-side Gist fetching - operator-set
  API base, redirect allow-list, no credentials, hard
  caps; the alternative (client-side fetch) was rejected
  for SSRF and consistency reasons.
- ADR 0011 (C6): share links are bearer grants by design
  - hashed at rest, expiring, revocable, live membership;
  the one documented exception to C1's never-widen rule,
  bounded to reading.

## Known gaps carried forward

- Activity log has no retention pruning yet (reads are
  capped at 200, export at 1000). The table grows slowly
  (events, not content), but a prune policy belongs in a
  future WP.
- Run reports (C8) are best-effort by design: a run made
  while offline is never reported, so cloud stats are a
  floor, not an exact count. Stated in the UI footnote
  and the C8 report.
- The i18n catalogue is English-only; a second locale is
  now a translation task, not an engineering task.
- `scripts/install.sh` still cannot install from the
  private repository (no token handling) - deferred by
  the owner's explicit choice of the `gh release
  download` workaround; unchanged by this phase.
