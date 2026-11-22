# Peak phase - final report

The Peak phase is complete: Phase A (UI/UX overhaul),
Phase B (CLI improvements) and Phase C (new features),
executed in plan order from
`MUSE_PEAK_PHASE_0_a4i0.md`, one commit and one report
per work package, tags `peak-phase-a`, `peak-phase-b`,
`peak-phase-c` on the final heads of each phase.

## Every work package

### Phase A - UI/UX overhaul (tag peak-phase-a)

A0-A8: the web app rebuilt as faithful Material You 3
(seed #36618E, tokens only) with light + dark + system
themes and a real switch, Roboto Flex throughout, the
Observe glass theme removed, responsive navigation,
Library bulk actions and previews, and a 72-golden
visual regression suite (generated on CI; see the A8
report for the sandbox write-fault story). Summary:
`reports/report_Peak-A-summary.md`.

### Phase B - CLI improvements (tag peak-phase-b)

B0 audit (F1-F14 in docs/cli/PEAK-AUDIT.md), B1 output
design system, B2 grouped help + dynamic completions,
B3 run experience, B4 ListQuery + `fg find`, B5 `fg new`
/ cloud edit / `fg lint`, B6 status + push/pull --all,
B7 trust surfaces + cache prune, B8 network resilience,
B9 `fg config` (`f373770`). Summary:
`reports/report_Peak-B-summary.md`. The B4-B8 clippy
chain (each WP's CI Rust job red on its own new lints)
was broken at B9: CI has been fully green on every head
since.

### Phase C - new features (tag peak-phase-c)

| WP | Commit | Headline |
|----|--------|----------|
| C1 | `b30647d` | Collections - ordered runbooks, never widen access |
| C2 | `5c32aa8` | Typed parameters (text/choice/number/path/yesno/secret) |
| C3 | `710ecc3` | History + rollback (rollback publishes a new version) |
| C4 | `8c6f6c9` | Public snippet pages at /s/owner/name |
| C5 | `1fb9e89` | Import helpers - history mining + Gist import (ADR 0010) |
| C6 | `90ea9e6` | Shared collection links (ADR 0011) |
| C7 | `18a4c33` | Activity log - per-user, exportable |
| C8 | `2c31225` | Usage stats - per-runner cloud run counts |
| C9 | `0224421` | i18n catalogue (~430 strings) + RTL test page |
| C10 | `2d3710e` | `forgehub self-check` + the Peak CHANGELOG |

Summary: `reports/report_Peak-C-summary.md`, with the
per-WP CI run ids for C1-C8. C9 verified green via CI
run 44985571118 (head `ed946c4`, which includes C9's
`0224421`); C10 via run 44986239180 (see PROGRESS.md for
the recorded result).

## Numbers

- Tests at phase exit: **535 Rust workspace tests, 0
  failed** (481 at Phase B exit); **82 web tests, 15
  files** (was 44 at the Observe-era end); Playwright
  E2E and the visual goldens run in CI.
- Web bundle: 791.05 kB JS (202.87 kB gzip) at C9 -
  includes the full message catalogue; per-page chunks
  stay lazy.
- CLI budgets (plan section 9), measured on this machine
  with the release binary built from `2d3710e`:
  `--version` 6 ms (budget 20 ms); binary 10,016,760
  bytes = 9.6 MiB (budget 15 MB). Server p95 read/list
  under 50 ms at 10k snippets was proven in Step 20
  (36.8 ms) and no Phase C change altered the read paths
  it measured; collections and stats add their own
  indexed queries.
- Migrations are append-only throughout: 0003
  collections, 0004 share links, 0005 activity, 0006 run
  reports.

## Standing rules honoured

- No release published, nothing deployed, no paid
  accounts - the phase scope excluded them and they
  still need fresh approval.
- Safety model untouched: full command/owner/version/
  hash cards, [y/N], typed yes for others' snippets,
  diff on change, fail-safe without a TTY. C6's share
  links are the one documented, bounded exception
  (read-only bearer grants, ADR 0011).
- No em dashes in the repo; one commit + report per WP;
  history never rewritten; tags never moved.

## Known gaps

1. Activity log retention: no pruning yet (reads capped
   at 200, export at 1000). Slow-growing, but a policy
   is owed.
2. Cloud run stats (C8) are best-effort: offline runs
   are never reported, so the numbers are a floor.
   Documented in the UI and the C8 report.
3. `scripts/install.sh` cannot install from the private
   repository (no token handling). Deferred by the
   owner's explicit choice of the `gh release download`
   workaround - not silently forgotten, recorded here
   and in the C summary.
4. The i18n catalogue is English-only by design; the
   first real translation is now a translation task.
5. B5's cloud edit live run against a real registry was
   never verified end-to-end (owed since Phase B; the
   code paths are covered by integration tests against
   the in-process server).

## The three things to fix first

1. **Activity retention pruning** (gap 1) - the only
   unbounded growth the phase introduced.
2. **B5 cloud-edit live verification** (gap 5) - run it
   once against the owner's own registry on their PC and
   record the result; it is the phase's one unverified
   live path.
3. **A first real locale** (gap 4) - the catalogue made
   it cheap; even a partial Hindi catalogue would prove
   the switch end to end, including the document-title
   and validation paths.
