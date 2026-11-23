# Report - UI restore: the pre-Peak "Observe" design (2026-11-23)

Owner order (2026-11-23): the Peak Phase A Material You
redesign is reverted. The web UI goes back to how it looked
before the Peak plan (the Observe design, last shipped at
`8f8af7b`), while the CLI, the server logic and every Peak
Phase B + C feature stay exactly as they are. This commit
is that restoration: presentation only.

## What was restored, and how

The old design was recovered from git history (commits
`b08c106`, `1f93478`, `82e45ff`, `5adbd4f`, `051f8ef`,
`51600e8`; tree at `8f8af7b`), not reinvented:

- **Design tokens** (`web/src/design/tokens.css`): the
  `--fh-observe-*` token block is back in `:root`, and the
  old `.observe-app` scope is appended at the end of the
  file: it re-points every `--md-sys-color-*` role to the
  monochrome glass values (white is the only accent; error
  red deliberately not remapped). Because every component
  and page is token-driven, this one scope restyles the
  whole current app - including all Peak-era screens -
  without touching their code. The Peak A2 token additions
  (type scale, durations) stay; the generated light/dark
  schemes stay in the file as the generator's contract but
  only the dark one is ever applied.
- **The Observe stylesheet** (`web/src/design/observe.css`)
  is byte-identical to the pre-Peak original (verified with
  `diff` against `8f8af7b`): the landing scene, liquid
  glass, the signed-in app backdrop and glass bars.
- **Assets**: `web/public/media/observe-bg.jpg` and the two
  Instrument Serif woff2 files restored from `8f8af7b`;
  the serif `@font-face` blocks are back in `fonts.css`.
- **One theme, pinned**: `web/index.html` pins
  `<html data-theme="dark">`; `theme-init.js` is replaced
  by `density-init.js` (density only). The Peak theme
  system is deleted outright: `design/theme.ts`, the app
  bar theme button, the account-menu theme item, and the
  Settings Appearance theme radio group (the density
  switch stays). Dead theme/landing keys were pruned from
  the i18n catalogue; the C9 string layer otherwise works
  exactly as before.
- **Landing page** (`LandingPage.tsx`): rebuilt to the old
  standalone Observe scene outside the app shell - glass
  nav pill, serif display headline, glass command pill,
  the three glass circles, and the intro section with the
  intro video and hero loop - but wired to the current
  i18n catalogue (new keys: `heroTitleStart`,
  `heroTitleWord`, `getStarted`, the circle aria labels,
  loop caption/aria/alt) and the Peak docTitle behaviour.
  One deliberate deviation from the original markup: the
  circles strip is a `<nav>` landmark instead of a bare
  `<div>` (same class, same look), because the Peak A7 axe
  sweep - current functionality - flags content outside
  landmarks, a check the original design never faced.
- **App shell** (`AppShell.tsx`): the shell root carries
  `.observe-app` again and renders the fixed
  `.observe-app-bg` smoke backdrop as its first child,
  exactly as the old shell did. Route transitions and card
  hover lift come back with the stylesheets.
- **Server-rendered pages**: `pages.rs` gains a shared
  `PAGE_CSS` constant (dark glass monochrome: smoke
  backdrop, glass panels, white pill buttons, serif h1)
  used by the public snippet page and the not-found page;
  `shares.rs` uses the same constant for the
  shared-collection page and its not-found page. No Rust
  logic changed anywhere; the CLI crate is untouched.

## Screens restyled

Every current route wears the restored design through the
token remap: Landing, Home, Library (incl. the stats card
and imports panel), Explore, Snippet page (incl. versions,
diff and the rollback UI), Editor (incl. the Parameters
panel), Collections list, Collection page (incl.
ShareLinksCard), Profile, Settings (incl. the Activity
card), Device, Login/Signup, 404 - plus the two
server-rendered pages (public snippet, shared collection).

## Tests and gates (all run locally, 2026-11-23)

- Web unit: **84 passed / 15 files** (`npx vitest run`).
  `theme.test.ts` was replaced by `design/observe.test.ts`,
  which pins the restored design (dark pin in index.html,
  no theme API exported, observe tokens present, white
  accent values, error role not remapped, WCAG AA pairs
  over black). `landing-video.test.tsx` was rewritten for
  the Observe scene (5 tests). Every feature test is
  unchanged in intent.
- Web lint: **0 errors** (1 pre-existing warning in
  ExplorePage), `check-tokens` clean. Typecheck clean.
  Build clean (index chunk 792.7 kB, gzip 203.2 kB - the
  Material runtime was already the bulk of it).
- Rust: `cargo fmt --all --check` clean;
  `cargo test -p forge-server` **119 passed, 0 failed**
  (one timing-sensitive auth test,
  `login_failures_are_generic_and_throttled`, failed once
  while the machine was also running npm builds in
  parallel and passes standalone and in the quiet full
  re-run; it does not touch the changed code).
- E2E (`scripts/e2e-web.sh`): **PASS** - the full web
  story plus both axe sweeps (signed-in and signed-out
  routes at 360/768/1280) are clean in the restored
  design. The axe sweep now runs once (the product has
  one theme) instead of twice.
- Visual regression (`scripts/visual-web.sh`): the suite
  was rewritten from 12 screens x 2 themes to 12 x 1 (the
  light golden set is deleted; the 36 dark goldens were
  regenerated for the restored design) and now **passes
  twice in a row** locally. Two harness determinism fixes
  were needed, both in the spec, neither weakening the
  pixel-exact assertions:
  1. The seed signs up through the API instead of the
     form. The form lands on /home, whose first Explore
     fetch filled the server's deliberate 60-second
     explore cache (Step 10) with the pre-seed empty
     feed, so the "Popular this week" card captured the
     stale empty state. Root-caused with the Playwright
     trace (three 34-byte `{"snippets":[]}` responses)
     and an API-level repro; the cache itself is intended
     behaviour and is untouched.
  2. The home capture waits (bounded, one reload retry)
     for the Popular card's loaded state, because under
     the frozen clock a failed first query attempt never
     retries (its backoff timers are frozen).

## Visual verification

Booted the real server with the production web build and
a seeded demo account (4 snippets, 1 public collection)
and captured at 1440x900: landing, home, library, snippet,
collection, explore, editor, settings, profile, the
server-rendered public page, the shared-collection page
(via a real share link), and the landing at 390px. Every
capture was read back: the family resemblance to the old
Observe screenshots is direct - smoke backdrop, glass
cards, white pill nav/buttons, serif landing headline.
The captures replace the Peak set in `docs/screenshots/`
(new names without light/dark suffixes: `landing.jpg`,
`library.jpg`, `home.jpg`, `snippet.jpg`, `collection.jpg`,
`explore.jpg`, `editor.jpg`, `profile.jpg`, `settings.jpg`,
`public-page.jpg`, `landing-mobile.jpg`), and the README
hero image, Screenshots section and Design section were
rewritten to describe the restored design truthfully.

## Not matched / deviations

- The circles strip landmark change (above) is the only
  markup deviation from the original landing, and it is
  invisible.
- The server-rendered pages had no Observe-era equivalent
  styling to copy (they were plain in both eras); they
  were styled fresh in the same language (glass panels,
  serif title, white pills).
- Peak-era screens (collections, share links, imports,
  parameters, activity, stats, rollback) never existed in
  the Observe era; they take the design from the token
  remap rather than from any historical screenshot.
- The Peak visual goldens were originally generated on a
  CI runner; the new goldens were generated on this
  machine, so the opt-in visual suite's baseline is now
  this machine's rendering (the suite is not a CI gate).

No tags, no release, no deployment.
