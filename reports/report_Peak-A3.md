# Report - Peak A3: adaptive shell and navigation

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A3: adaptive shell and navigation`
- **Status:** Done

## 1. Summary

The shell is now a true adaptive app frame. It is a fixed-height grid (100dvh) whose chrome never scrolls - the content column scrolls internally and resets to the top on route change - so the navigation bar, rail and top app bar are always reachable, and the audit's "bar overlaps content" failure class is structurally gone. The top app bar gains its M3 variants (small on compact/medium, medium with a headline-small title on expanded), a global search entry, and an account menu holding Profile, Settings, the theme cycle and Log out (the signed-out bar keeps the theme button and Log in). Library and the owner's own profile get the FAB (extended on expanded), and the Explore page adopts the `?q=` the app bar search hands it.

## 2. What I built

- `web/src/design/components/AppShell.tsx` - new optional `fab` and `scrollKey` props; the main element is ref'd and scrolled to top when `scrollKey` changes (optional-chained: jsdom has no `Element.scrollTo`); the top bar takes the `fh-topbar--medium` variant on the expanded class.
- `web/src/design/components.css` - `.fh-shell` is `height: 100dvh` (was min-height); `.fh-shell__main` scrolls (`overflow-y: auto`, `min-height: 0`) with 16 px margins on compact and 24 px on medium/expanded; medium top bar styles (headline-small title from tokens); `.fh-fab` fixed placement, lifted by nav-bar height on compact.
- `web/src/design/components/core.tsx` - `Fab` accepts optional `text` and renders the extended FAB via md-fab's `label`.
- `web/src/App.tsx` - the app bar now carries: a search icon button (routes to Explore), and for signed-in users an account menu (Profile (username), Settings, `Theme: <current>` cycling in place, Log out); signed-out keeps the A1 theme button plus Log in. The FAB (add icon; extended with the "New snippet" label on expanded) renders on `/dashboard` and the owner's own profile route only, and navigates to the editor. `scrollKey` is the pathname.
- `web/src/pages/ExplorePage.tsx` - the search query initialises from and follows the `q` search param.
- Evidence: `docs/screenshots/peak-audit/after-a3-library-{compact,medium,expanded}.jpg` - tonal active indicator on Library in all three, extended rail and medium top bar on expanded, FAB clear of the bottom bar on compact, extended FAB on expanded.

## 3. Decisions made

- **Internal content scroll over fixed/sticky chrome.** Sticky positioning inside a page-scrolling grid could not keep all three chrome pieces (top bar, rail, bottom bar) simultaneously pinned without fragile offsets. The app-frame pattern (fixed shell, scrolling main) is the standard M3 web shell and also fixes the Editor's sticky Checks panel context (it now sticks within the content column, as intended).
- **The account menu replaces the inline username + Settings + Log out buttons** (A3's brief). The theme cycle lives in the menu for signed-in users; the standalone theme button remains for signed-out users, where there is no account menu. The Settings page Appearance tab remains the full control.
- **App bar search is an entry, not an overlay**: it routes to Explore, whose own full search bar is the search view (M3's search pattern is a dedicated view; building a second overlay search would duplicate Explore). The `?q=` hand-off keeps a future inline field lossless.
- **FAB scope**: exactly the two screens the plan names. Other profiles (someone else's) get no FAB - creating is not the primary action there.

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 438 passed, 0 failed |
| Web lint | `npm run lint` | pass; check-tokens clean |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | 60 passed, 0 failed (two failures during development were the jsdom scrollTo gap; fixed by optional-chaining, no test changed) |
| Web build | `npm run build` | pass |
| E2E | `bash scripts/e2e-web.sh --skip-build` | pass (full signup -> create -> run -> logout flow in Chromium) |
| Manual | Playwright captures, 3 width classes | shell verified visually (section 2) |

## 5. Acceptance criteria (plan A3)

- [x] Shell follows the layout grid: navigation bar (compact), rail (medium), extended rail (expanded) - all pre-existing forms kept, now pinned by the app frame; active destination uses the tonal secondary-container indicator (restored in A1).
- [x] Top app bar: small and medium variants by width class.
- [x] Content max width 1200 px, margins 16 (compact) / 24 (medium, expanded).
- [x] All four destinations reachable at every size; Settings (and Profile, theme, Log out) from the account menu.
- [x] Global search entry in the app bar.
- [x] FAB on Library and own Profile (compact/medium), extended FAB on expanded.

## 6. Security notes

The account menu renders the username as React text (no HTML injection path). No new data flows; the FAB and search entry are pure navigation.

## 7. Dependencies added

None.

## 8. Corrections to earlier steps

None. (A1's standalone theme button remains for signed-out users only; the A1 report's "app-bar switch" is now the account-menu item for signed-in users - the Settings control is unchanged.)

## 9. Known issues / limitations

- Library list rows at 360 px still squeeze their text into a narrow column (the row layout is the page's, not the shell's) - A5's Library redesign owns it.
- Window-level scrolling is gone inside the app (the content column scrolls); browser find-in-page and Playwright both work against scrollable containers, verified in e2e.

## 10. Deferred / ideas

None.

## 11. Next step

A4: component library refresh - every design component audited against M3 (tonal/elevated/outlined cards, list-item anatomy, chips, menus, dialogs, snackbar, progress, search bar), each with states, a design.md note, and gallery coverage in /dev/design; delete components nothing uses.
