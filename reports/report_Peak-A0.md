# Report - Peak A0: design audit and direction

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A0: design audit and direction`
- **Status:** Done

## 1. Summary

Phase A starts from evidence, not taste. I ran the shipped app against a seeded server, captured all 12 routes at 360 / 768 / 1280 px (36 captures) plus two light-forced captures, measured overflow and touch targets, and read the current Material 3 guidance on colour, type, layout, motion and the navigation bar. The result is `docs/design/PEAK-DIRECTION.md` (palettes, type scale, shape, motion, layout, per-screen wireframes), ADR 0009 (the serif display face is dropped), and the audit screenshots in `docs/screenshots/peak-audit/`. No code changed in this WP.

## 2. What I built

- `docs/screenshots/peak-audit/` - 38 JPG captures (18-86 KB each), seeded data (user `ada`, six snippets, three public, `sys-info` at v2): landing, login, signup, home, library, editor, explore, profile, snippet, settings, device, 404 at three widths, plus `landing-1280-lightforced.jpg` and `library-1280-lightforced.jpg`.
- `docs/design/PEAK-DIRECTION.md` - audit findings (10, each tied to a capture or a measurement), the full light and dark tonal palettes from the committed generated tokens (seed #36618E, SchemeTonalSpot), the M3 type scale with ForgeHub usage, shape/elevation rules, motion tokens with the exact M3 easing curves and duration table, layout/spacing rules, text wireframes for all 11 screens, and the guidance relied on with URLs.
- `docs/adr/0009-serif-display-type.md` - decision: drop Instrument Serif everywhere including the landing hero; Roboto Flex carries display type. Alternatives (landing-only serif, a different display face) rejected with reasons.
- `docs/requests/PERMISSION-REQUESTS.md` - created; logs the five M3 guidance reads (pre-approved) and the intro-video/landing mismatch note required by plan section 8. No blocked requests.
- `PROGRESS.md` - Peak section added with all 28 WPs; A0 ticked, next is A1.

## 3. Decisions made

- **No "Classic dark" third theme.** The plan allows keeping Observe only if token-driven; it is token-driven, but it doubles the contrast/test matrix for a look the owner rejected. A1 removes the glass scope entirely; recorded in PEAK-DIRECTION section 2.
- **Serif dropped entirely** (ADR 0009), going one step past the plan's fallback of a Roboto Flex hero: even a Roboto-only exception is unnecessary - display styles are Roboto Flex by default once the serif is gone.
- **Audit method:** real server + real browser (Playwright Chromium) over seeded data, not the dev server with mocks, so the captures show production CSS and real content lengths.

## 4. Verification

Baseline gates were run on the untouched tree at the start of this WP (agent.md session checklist); this WP changes docs and screenshots only, so the code tree the gates measured is the code tree being committed.

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 438 passed, 0 failed |
| Web lint | `npm run lint` (eslint + check-tokens) | pass, check-tokens clean |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | pass |
| Web build | `npm run build` | pass |
| CI at start | `gh run list` | run 44926108115 on `8f8af7b` - success (no WP-0 needed) |
| Manual | Playwright audit captures | 38 files, all < 300 KB; overflow check: none on any route/width |

## 5. Acceptance criteria (plan A0)

- [x] Every screen audited at 360, 768 and 1280 px; screenshots in `docs/screenshots/peak-audit/`, each under 300 KB.
- [x] Visual problems listed with evidence (PEAK-DIRECTION section 1): spacing/hierarchy, contrast over glass, empty states, touch targets under 48 px (measured), serif/sans mix, hardcoded-value audit (tokens are clean per check-tokens; the problem is the Observe scope, not literals).
- [x] Material 3 guidance read for colour, type, shape/elevation (via colour + component specs), state layers, motion, adaptive layout, navigation bar/rail/drawer, search, cards, lists, chips, FAB, dialogs, snackbars, menus, empty states; the guidance relied on is cited in PEAK-DIRECTION section 8.
- [x] `docs/design/PEAK-DIRECTION.md` written with concrete palettes, type scale, fonts, shape/elevation, motion tokens, spacing grid and per-screen wireframes.
- [x] ADR on the serif written (ADR 0009).
- [x] No code changes beyond docs (plus screenshots and PROGRESS.md).

## 6. Security notes

None - docs only. The audit used a throwaway local server and a demo account (`ada@test.dev`); no real credentials exist in any committed file (the seed token lived in /tmp and is not in the repo).

## 7. Dependencies added

None.

## 8. Corrections to earlier steps

None.

## 9. Known issues / limitations

- The audit's touch-target measurement covers native elements only; Material web components keep their targets inside shadow roots, so the 48 px audit for those lands with A7's axe + manual pass.
- The intro video will not match the rebuilt landing after A5; recorded in PERMISSION-REQUESTS.md per plan section 8 (regeneration is out of scope and the generator is blocked on this account).

## 10. Deferred / ideas

- The snippet page renders the whole canonical forge file (YAML header included) in the Code tab; whether to show commands-only is a product question A5 will address in its design note.

## 11. Next step

A1: remove the `data-theme="dark"` pin and the `.observe-app` scope, add the system/light/dark preference (localStorage, pre-paint inline script), put the switch in Settings and the app bar menu, and extend the contrast test to every used pair.
