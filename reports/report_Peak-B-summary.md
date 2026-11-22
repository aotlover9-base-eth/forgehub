# Report - Peak Phase B summary (CLI improvements)

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Phase:** B (plan section 5) - B0-B9 all landed.
- **Tag:** `peak-phase-b` on the B9 head (`f373770`).

## What shipped

| WP | Commit | One line |
|---|---|---|
| B0 | audit only | `docs/cli/PEAK-AUDIT.md`: 14 findings (F1-F14), each mapped to a WP; baselines measured |
| B1 | (B1 head) + `5444184`, `18c968b` | Terminal output design system: one colour/symbol vocabulary, TTY/NO_COLOR/`--color` behaviour, golden tests |
| B2 | (B2 head) | Grouped `--help` (Workflows/Library/Cloud/Tools), `fg help <topic>`, dynamic shell completions from the live library |
| B3 | (B3 head) | Run experience: card layout, parameter prompts, quiet success line, exit-code map, golden tests |
| B4 | (B4 head) | One `ListQuery` filter set for list/search (+ sidecar visibility), `fg find` fuzzy search with `--plain` |
| B5 | (B5 head) | `fg new` editor loop with lint-repair, cloud `fg edit` download-edit-push, `fg lint` exit 0/1/2 |
| B6 | (B6 head) | `fg status` next-command hints, `push --all`/`pull --all` with `--dry-run`, diff parity local vs remote |
| B7 | (B7 head) | Trust surfaces everywhere (origin, pinned hash, age, cache state), `fg cache path`/`prune` |
| B8 | `9972275` + `602d8f3` | Network resilience: 429/5xx handling, Retry-After, cache staleness on the card, EPIPE quiet exit |
| B9 | `f373770` | `fg config get|set|list` (validated keys, wired into colour + push defaults), owed items closed (below) |

Per-WP detail lives in `reports/report_Peak-B0.md` (the audit)
through `report_Peak-B9.md`.

## Verification state at the tag

- **CI run `44974125129` for the B9 head is fully green - all
  five jobs** (Rust fmt/clippy/test, Web, E2E, Docker, Supply
  chain). Main had been Clippy-red from B4 through B8 (each WP
  carried the previous fix-forward and added its own); the
  chain was broken by the dedicated fix commit `602d8f3` (CI
  run `44973090836` fully green) and B9 landed green
  first try.
- Local gates at B9: fmt clean, clippy clean, **481 Rust tests
  passed / 0 failed** (438 at the phase's start; +43 across
  Phase B).
- Performance (release build, this machine; plan section 9
  budgets): `--version` mean 6.5 ms (budget 20), local run to
  prompt mean 8.2 ms / p95 10.5 ms over a real PTY (budget
  50), binary 9.3 MiB (budget 15 MB). Server p95 stands as
  re-checked at B0.

## Owed items - all closed in B9

- Man pages regenerated (owed since B2): 11 updated, 10 new.
- B5 cloud edit verified live: a sync integration test drives
  the real server through download-edit-push and asserts
  version 2 server-side.
- B0's release measurements re-taken (table above).
- The audit's F-findings: F1-F14 each closed by their mapped
  WP or explicitly carried: F13 (keyring decision) was closed
  in B7's design (auto/file/keyring already shipped pre-Peak);
  nothing remains open from the audit.

## Known gaps carried into Phase C

- None functional. The `scripts/install.sh` private-repo 404
  is a pre-Peak item the owner explicitly deferred (the
  `gh release download` workaround is the supported path);
  it is not a Phase B/C item.

## Next

Phase C (C1-C10) in plan order, starting with C1 collections.
