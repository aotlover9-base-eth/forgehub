# Report - Peak A5: screen redesign

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A5: screen redesign`
- **Status:** Done

## 1. Summary

Every route now implements its PEAK-DIRECTION section 7 wireframe. The landing page was rebuilt on the shared tokens and components and the Observe era was deleted end to end (observe.css, the serif fonts, the smoke background, the ambient hero-loop videos, the `--fh-observe-*` tokens). Library gained M3 list rows that actually navigate, bulk-select with bulk delete / visibility / add-tag, j/k keyboard navigation, a density toggle, and an expanded-width detail side sheet. The editor gained Ctrl+S, a three-way version-conflict dialog with a real "Reload theirs", and segmented visibility. The snippet page gained its Forks tab, backed by a new read endpoint (`GET /snippets/{owner}/{name}/forks`) with visibility-safe semantics and an integration test. Settings is vertical-tabbed on expanded, stacked on compact, and hosts the new density preference. Home, Explore and Profile were brought to their wireframes, and the error boundary now shows a short reference id.

## 2. What I built

**Landing** - `LandingPage.tsx` rewritten: token top bar (wordmark, Explore/GitHub/Guide, Log in / Create account or the library CTA), display-large headline in Roboto Flex, the real `fg ubuntu-setup` confirmation exchange in a CodeBlock, three filled feature cards, the intro-video section on tokens (poster, controls, default captions, skip link, transcript - unchanged contract), and a plain footer. Deleted: `observe.css`, `public/media/observe-bg.jpg`, `public/video/hero-loop.{mp4,webm}`, both Instrument Serif woff2 files and their @font-face rules, the `--fh-observe-*` token block, and the A2 legacy motion aliases (observe.css was their only consumer). Styles live in components.css as `.fh-landing*` (tokens only; check-tokens clean).

**Forks endpoint** - `repo.rs`: `forks_of` (public forks + the viewer's own at any visibility; unlisted forks are not listed - their existence stays with link-knowers; hidden/deleted excluded). `snippets.rs`: `list_forks` handler (parent must be readable first, so a private parent's fork list 404s) + route; OpenAPI entry added. Integration test `forks_list_visibility` covers anonymous / owner / after-publish / private-parent cases. Web: `useForks` hook + the Snippet page's third tab with provenance-aware empty states and ListItem rows.

**Library** - rows are the library `ListItem` (leading mode icon, name, description - version - relative time, tags below, StatusChip + actions menu trailing) and clicking a row opens the snippet (previously rows did nothing). Bulk mode: a Select toggle swaps rows to checkboxes and shows a contextual bar (count, visibility menu, Add tag dialog, Delete with a count confirm, Close); bulk operations run per snippet and report done/failed counts honestly. Keyboard: j/k move a visible cursor (row highlights and scrolls into view), Enter opens; the global handler still owns n, / and ?. Density: new `design/density.ts` preference (localStorage `forgehub-density`, `data-density` on `<html>`, applied pre-paint by theme-init.js) driving `--fh-row-pad-block`; toggled from the Library toolbar and from Settings. Side sheet (expanded only): clicking a row previews it in a 360 px sticky sheet (code, run command with copy, Open full page) instead of navigating. Subtitle now carries the count.

**Editor** - Ctrl+S / Cmd+S saves from anywhere (save ref pattern, preventDefault). Conflict dialog rebuilt as a three-way md-dialog: Reload theirs (fetches the latest and resets both the fields and the dirty-base - a new `base` state replaced mount-time `initial` in the dirty check), Keep editing, Save mine as new version (the existing fresh-version retry). Visibility is a segmented control (`SegmentedControl` wrapper over the labs segmented buttons, registered in md.ts) with the selected option's help text under it.

**Settings** - layout by width class: compact stacks all six sections under headline-small heads; expanded shows a vertical tab rail (secondary-container active pill, aria-current); medium keeps the horizontal tabs. Appearance gains the density switch. The stale "app bar, under Theme" copy now points at the account menu (A3).

**Home / Explore / Profile / errors** - Home: quick actions (New snippet + Run-from-CLI card with copy), recent snippets and popular as ListItem rows, a Recent activity list (version events from the same data), and the two-step empty state (fg login / fg record with the `end` wording, plus the web create action) for new accounts. Explore: sort is the segmented control (Relevance / Most stars / Recent). Profile: avatar is primary-container, rows are ListItem. ErrorBoundary: shows `Reference: <6-char id>` (selectable, never a stack trace).

## 3. Decisions made

- **The forks endpoint is the one server change in a web WP.** The wireframe's Forks tab cannot be faked; a small read-only endpoint with the app's existing visibility rules is the honest way to build the screen. Unlisted forks are excluded from listing (see section 2) - the conservative reading of "unlisted".
- **Home "Recent activity" is derived, labelled as version events.** There is no activity API until C7; the list renders each recent snippet's latest version and update time, which is exactly what the data says - no invented events.
- **Test adaptations (all to intentional, reported UI changes; assertions and flows preserved):** landing-video.test.tsx rewritten to the new landing contract (the ambient loop it tested was removed by the wireframe; the intro-video guarantees - poster, controls, captions, no autoplay - are kept and strengthened with a page-wide no-autoplay assertion); App/AppAuthed tests use findAllByText for CTAs that now appear in both the top bar and the hero; editor.test.tsx clicks the renamed conflict action "Save mine as new version" (same handler, same flow); step17.test.tsx drives the Settings section rail by button name instead of md-tabs indices (jsdom width = expanded); pages.test.tsx expects the new Home empty-state copy; e2e/app.spec.ts selects Public on the segmented control by role (exact name) instead of the removed radio label.
- **Icon subset incident (found in screenshots, fixed here):** the self-hosted Material Symbols font is a 96-icon subset; four Peak icon names (cloud_upload, density_medium/large, checklist, open_in_new) are not in it and rendered as raw text. Replaced with subset icons (file_upload, check, chevron_right) and a text density button; the full inventory is now documented in design.md so the constraint is discoverable before the next icon is typed.

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass (after fmt of the new test) |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 439 passed, 0 failed (438 + forks_list_visibility) |
| Supply chain | `cargo deny check` | pass |
| Web lint | `npm run lint` | pass; check-tokens clean |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | 64 passed, 0 failed |
| Web build | `npm run build` | pass; entry bundle 187 KB gz (budget: landing initial JS < 250 KB gz) |
| E2E | `bash scripts/e2e-web.sh --skip-build` | pass |
| Manual | Chromium captures on the production build | landing light (fixed icons), Library with side sheet open, Editor with segmented visibility - `docs/screenshots/peak-audit/after-a5-{landing-light,landing-dark,library-sheet,editor,settings-rail}.jpg` |

## 5. Acceptance criteria (plan A5)

- [x] Every screen matches its section 7 wireframe (landing, home, library, editor, snippet with tabs + two-version diff, explore, profile, settings, device - unchanged, already to spec - auth, 404).
- [x] Landing uses tokens and library components; Instrument Serif removed everywhere including the hero (ADR-0009 executed); observe.css and the `--fh-observe-*` tokens deleted.
- [x] Settings includes the density option (and it works - it drives list row padding app-wide).
- [x] Library: bulk-select, j/k shortcuts, detail side sheet on expanded.
- [x] Editor: two-pane layout (pre-existing), Ctrl+S, conflict dialog with the three resolutions.
- [x] No screen keeps Observe styling; no serif remains in the product.

## 6. Security notes

- The forks endpoint resolves the parent with the standard readable check first (private parent = 404, no existence leak) and lists only forks the viewer may open; a new integration test pins all four visibility cases.
- Bulk actions reuse the existing per-snippet mutations, so every item passes the server's own authorization; partial failures are reported, not hidden.
- The landing demo card is static text rendered through CodeBlock (React text, no HTML injection).

## 7. Dependencies added

None. (The labs segmented-button modules ship inside the already-vendored @material/web.)

## 8. Corrections to earlier steps

- The A2 report kept the legacy motion aliases "for observe.css until A5"; A5 removed them as promised.
- Library rows previously did not navigate anywhere on click (only the overflow menu worked) - a pre-existing UX defect the redesign fixes; recorded here because it was not in the A0 audit's top findings.

## 9. Known issues / limitations

- The icon subset font remains a hard constraint (documented in design.md); new icons need a font regeneration, which is a tooling task of its own.
- Home's activity list is version events derived from the snippet list; the real activity log arrives with C7.
- The gallery route remains dev-only, so A5's visual evidence comes from production routes.

## 10. Deferred / ideas

None.

## 11. Next step

A6: motion - route transitions on the shared-axis pattern, list enter animations, dialog/snackbar motion from the token scale, all collapsing under prefers-reduced-motion.
