# Report - Peak A2: type, shape, elevation and state layers

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A2: type, shape, elevation and state layers`
- **Status:** Done

## 1. Summary

The token system is now complete and every custom component consumes it the same way. tokens.css gained the full Material 3 typescale weight and tracking tokens (the colour generator never emitted them), the full M3 duration scale (short1-4, medium1-4, long1-4, extra-long1-4) and the complete easing set (standard/emphasized, each in base, decelerate and accelerate forms, with the spec-exact curves). components.css was normalised against PEAK-DIRECTION: font weights come from typescale tokens instead of ad-hoc 500/700 values, cards take the M3 large corner, the snackbar takes the extra-small corner and elevation 2, custom interactive surfaces (navigation items, toast buttons) implement real M3 state layers from the opacity tokens, all custom controls meet the 48 px target floor, and the search field shows focus on its container instead of swallowing the outline.

## 2. What I built

- `web/src/design/tokens.css` - 15 weight tokens + 15 tracking tokens (M3 baseline values: display/headline/title-large 400, title-medium/small and labels 500, body 400; tracking from the M3 scale); 16 duration tokens; 6 easing tokens (emphasized-decelerate `cubic-bezier(0.05, 0.7, 0.1, 1)`, emphasized-accelerate `cubic-bezier(0.3, 0, 0.8, 0.15)`, standard set); the three legacy duration aliases now point at the scale (short = short2, medium = medium1, long = medium4) so the landing stylesheet keeps working until A5.
- `web/src/design/components.css` -
  - Weights: top bar title, page header title, state title, status chip, toast action and the diff gutter now use typescale weight tokens; two letter-spacing tokens applied (title-large, label-medium).
  - Shape: `.fh-card` corner-medium (12) -> corner-large (16) per PEAK-DIRECTION section 4; toast corner-small -> corner-extra-small (the M3 snackbar shape).
  - Elevation: toast shadow elevation-3 -> elevation-2 (A2 rule: above level 2 belongs to menus, dialogs and the FAB, which are Material components with their own shadows). Elevated cards were already surface-container-low + elevation-1; filled cards already surface-container-highest per the M3 card spec.
  - State layers: `.fh-nav__pill` and `.fh-nav__item--beside` get a currentColor overlay pseudo-element driven by the state tokens (hover 8%, pressed 10%); toast action/close buttons get a color-mix hover layer at the hover token. Buttons, icon buttons, FAB, chips and list items wrap @material/web and already implement M3 state layers natively.
  - Targets: `.fh-nav__item` min-height 48 px; toast action/close are 48 px targets (close also 48 px wide).
  - Focus: `.fh-searchbar__input` no longer removes the outline into a void - the input's own outline is suppressed only because `.fh-searchbar:focus-within` draws the ring on the whole field (the M3 search-bar pattern); the global `:focus-visible` ring is untouched everywhere else.
- `web/scripts/check-tokens.mjs` - extended with a required-token check: all weight/tracking/duration/easing/state tokens must exist in tokens.css, with the spec-fixed values asserted (easing curves, state opacities). This runs inside `npm run lint`, so a token regeneration that dropped them fails the build.
- Evidence: `docs/screenshots/peak-audit/after-a2-home-compact-light.jpg` and `after-a2-library-dark-1280.jpg`.

## 3. Decisions made

- **Token-presence testing lives in check-tokens, not vitest.** A vitest version was written first; the project's vitest CSS pipeline stubs `tokens.css?raw` to an empty string, so the assertions could not see the file. Rather than weakening the test or fighting the pipeline, the check moved to the node script that already owns token discipline (and it runs in CI's lint step). The vitest attempt was removed, not left failing.
- **Legacy motion aliases kept** (mapped onto the new scale) because observe.css still references them; A5 deletes them with the landing rebuild.
- **Inline text links** (footer, body copy) keep their natural height: the 48 px floor is applied to every component-level control; text links follow the WCAG 2.2 AA target-size minimum (24 px) and keep the visible focus ring. Recorded here so A7 can audit against the stated rule.

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 438 passed, 0 failed |
| Supply chain | `cargo deny check` | pass |
| Web lint | `npm run lint` | pass; check-tokens clean including the new required-token check |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | 60 passed, 0 failed |
| Web build | `npm run build` | pass |
| Manual | Playwright captures on the real server | compact light Home and dark Library render correctly; the compact bottom bar is back in grid flow (the A0 overlap came from the deleted fixed glass bar) |

## 5. Acceptance criteria (plan A2)

- [x] M3 type scale as tokens (sizes existed; weights + tracking added) and components consume them; no ad-hoc font sizes or weights remain in components.css (check-tokens bans size literals; weights now reference tokens).
- [x] M3 shape scale applied: cards large, snackbar extra-small, buttons/chips/FAB full-round via Material components, CodeBlock medium (unchanged).
- [x] Elevation through tonal containers; shadows limited to elevation-1 (elevated cards) and elevation-2 (snackbar); menus/dialogs/FAB carry Material's own.
- [x] State layers (8/10/10/16 tokens) implemented on every custom interactive component (nav items, toast buttons); Material-wrapped components use their native layers. The dragged layer token exists; no custom component has a drag interaction yet (A5's bulk-select may add one).
- [x] Focus ring visible on every focusable element (global rule kept; search field rings its container).
- [x] Minimum 48 x 48 px targets on component controls (nav items, toast buttons verified in CSS; Material components are 48 by spec).

## 6. Security notes

None - presentation only. `color-mix` is used with token values only; no user data reaches these styles.

## 7. Dependencies added

None.

## 8. Corrections to earlier steps

None.

## 9. Known issues / limitations

- Two pre-existing layout faults are visible in the A2 evidence captures and belong to later WPs: Home's recent-snippets card content can overflow horizontally at 360 px (A5 Home redesign), and the compact top bar crowds its actions (A3's account menu fixes this).
- The skeleton pulse keeps its 1.4 s cycle: M3 does not tokenise ambient loading durations; A6 reviews it with the rest of the motion system.

## 10. Deferred / ideas

None.

## 11. Next step

A3: rebuild the shell - small/medium top app bars by width class, navigation bar (compact) / rail (medium) / extended rail (expanded) with the tonal indicator, content margins per class, global search entry in the app bar, an account menu (Settings, theme, Log out), and the FAB / extended FAB on Library and Profile.
