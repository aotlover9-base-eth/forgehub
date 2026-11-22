# README refresh - deep check report

Date: 2026-11-22. Head under test: `c52f567` (Peak phase
complete). Purpose: verify ForgeHub end to end by running
everything, then rewrite the README to the post-Peak state
with fresh screenshots. Anything not run is recorded as
not run, with the reason. No product code was changed.

## CI

- Head `c52f567` ("peak C: phase summary and final
  report"): CI run 44986613767 completed **success** -
  all jobs green. (It was still in progress when the Peak
  run closed; this is its final verdict.)
- C9/C10 runs 44985571118 and 44986239180 also success.

## Rust gates (local, this machine)

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`:
  exit 0, zero errors (full output counted with
  `grep -cE '^error'`, never truncated).
- `cargo test --workspace`: exit 0, **535 passed, 0
  failed** across 37 test binaries - matches the Peak
  final report exactly.

## Web gates (local, web/)

- `npm run lint` (eslint + scripts/check-tokens.mjs):
  0 errors, 1 pre-existing warning; check-tokens clean.
- `npm run typecheck`: clean.
- `npx vitest run`: **82 passed, 15 files** - matches
  the Peak final report.
- `npm run build`: entry chunk 791.05 kB, 202.87 kB
  gzip - identical to the Peak final report's numbers.

## Supply chain

- `cargo deny check` (0.21.0): advisories ok, bans ok,
  licenses ok, sources ok.
- `cargo audit` (0.23.0): first attempt failed on a
  stale partial advisory-db cache in ~/.cargo (an
  interrupted clone, a write-fault-era leftover); after
  clearing the cache it fetched cleanly and scanned
  376 dependencies against 1555 advisories: **0
  vulnerabilities**, 5 allowed warnings (the project's
  configured allowances).

## Budgets (release binary built from head c52f567)

- Binary size: 10,317,560 bytes = 9.84 MiB (budget
  15 MB). The Peak report measured 9.6 MiB from
  `2d3710e`; both are well inside budget.
- `--version`: median 4.3 ms over 20 runs (budget
  20 ms).
- Local run-to-prompt (PTY, pulled snippet, 3 trials):
  8.5 / 8.7 / 11.4 ms (budget 50 ms).
- Server p95 at 10k snippets: not re-measured (Step 20
  proved 36.8 ms; the Peak final report records that no
  Phase C change altered those read paths).

## Live smoke (local server + real CLI)

Setup: release `forge-server` on 127.0.0.1:8791 with a
temp SQLite DB, `FORGEHUB_ALLOW_SIGNUP=true`, console
mailer; release `forgehub` CLI with scratch
FORGEHUB_HOME dirs. Health: `GET /api/v1/health` ->
`{"status":"ok","db":"ok"}`.

- Signup via API created user `demo`; session + CSRF
  flow verified (token creation without the CSRF header
  is refused, with it succeeds). API token minted
  (prefix 7Jk5mCcz, read+write).
- Device-code login, full loop: `fg login` printed code
  9ATM-5J4D, approval through the web session API, CLI
  completed with "Logged in as demo"; `fg whoami` on the
  stored session shows scopes read, write.
- Home A (token session): `fg init`, `fg import` x3,
  `fg list`, `fg lint hello-smoke` ("clean: no lints,
  no possible secrets"), `fg config list` (all five
  keys with sources), `fg push` x3 (hello-smoke public
  v1 sha256:f23faec5), `fg find hello` (match),
  `fg collection create demo-pack` + 3 adds +
  `fg collection list` (3 items), `fg activity` (login,
  token_created, snippet_created events), `forgehub
  self-check` (prints binary sha256 and the honest
  manual verification flow for the private repo).
- Home B (clean, token session): `fg pull
  demo/hello-smoke` saved locally; `fg run
  demo/hello-smoke` showed the full safety card and ran
  after typed `yes` - both echo lines, "exit code 0";
  `fg remote list` shows all 3 snippets.
- Collection run: `fg run demo-pack` in home A ran all
  3 snippets in order, each with its own card and
  confirmation, ending "collection demo/demo-pack: all
  3 snippets ran".
- Public pages: `GET /s/demo/hello-smoke` -> 200,
  server-rendered, contains the commands and title
  "hello-smoke by demo - ForgeHub"; private
  `/s/demo/disk-report` -> 404; missing snippet -> 404
  (no leak, as designed in C4).

## Observations (not breaks)

- With `FORGEHUB_TOKEN` (no stored login), a cloud run
  of the user's OWN snippet is treated as foreign:
  the card says "This snippet is not yours" and demands
  typed `yes`. With a stored device session the same
  run is correctly treated as own ([y/N]). The token
  path fails safe (stricter, never looser), but the
  ownership check evidently keys off the stored session
  username, which env-token sessions do not carry.
  Recorded here; the README makes no claim either way.
- `cargo audit`'s first failure was local cache state,
  not the project (see Supply chain).

## Screenshots

Captured with Playwright (1440x900, Chromium) against
the production web build served by the smoke server,
signed in as the seeded `demo` account (6 snippets, 2
public, hello-smoke at v2, one 3-item collection).
Every capture was read back and inspected: real
content, correct theme, no error states, no login
walls. The five Observe-era JPGs were deleted in the
same commit.

- landing-light.jpg / landing-dark.jpg - the landing
  page in both themes (Material You, Roboto Flex, no
  glass, no serif).
- landing-mobile.jpg - landing at 390px.
- home-light.jpg - signed-in Home (recent snippets,
  popular this week, recent activity).
- library-light.jpg / library-dark.jpg - the Library
  with all 6 seeded snippets and visibility chips.
- snippet-light.jpg - demo/hello-smoke at v2, with the
  C4 "Public page" link and Versions/Forks tabs.
- collection-light.jpg - demo/demo-pack with the run
  command and the C6 Share links card.
- explore-light.jpg - exactly the 2 public snippets;
  private ones correctly absent.
- editor-light.jpg - new-snippet editor with the live
  Checks panel reporting a validation problem.
- settings-light.jpg - Appearance section with the
  System/Light/Dark theme picker.
- profile-light.jpg - public profile showing only the
  2 public snippets.

Environment note: mid-capture, the sandbox service
restarted and wiped /tmp (the smoke DB and driver).
The seed was rebuilt identically from a script and all
captures were re-taken against the rebuilt account, so
the set is consistent.

## README changes

- Test counts updated to today's measured numbers
  (535 Rust + 82 web = 617) in the badge, overview and
  status table; the stale v0.1.4 status line now reads
  v0.1.6.
- New "What is inside" section covering the Peak
  features: versioning + rollback, typed parameters,
  collections + share links, public pages, imports
  (history mining + Gist), activity log, per-runner
  stats, `fg find`, `fg lint`, `fg config`, self-check,
  i18n groundwork.
- Screenshots section rebuilt on the new captures; the
  Observe "dark glass" description and the serif note
  are gone. New short Design section describes the
  Material You 3 system as shipped.
- Command tables rebuilt from the real binary's
  `--help` (all 42 commands): `find`, `new`, `lint`,
  `history`, `rollback`, `collection`, `activity`,
  `config`, `self-check`, cloud `edit` and `help <topic>`
  added; every row checked against the binary.
- Config table gained FORGEHUB_ALLOW_SIGNUP,
  FORGEHUB_BASE_URL and FORGEHUB_GIST_API_BASE (the
  real variable name, read from the server config
  source; it is the operator-set GitHub API base for
  Gist import per ADR 0010).
- Safety section gained the one bounded exception
  (collection share links, ADR 0011). The install
  section keeps the verified private-repo `gh release
  download` flow and the honest install.sh note.
- Whole file checked: zero em dashes.

## Breaks found

- No product breaks. Every gate, budget and smoke step
  above passed. The one behavioural quirk found is the
  env-token ownership note under Observations.
