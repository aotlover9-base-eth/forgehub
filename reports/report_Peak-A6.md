# Report - Peak A6: motion and micro-interactions

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A6: motion and micro-interactions`
- **Status:** Done

## 1. Summary

The app had silently lost its route transition and hover motion when observe.css was deleted in A5 (they lived there). A6 rebuilds motion as a small, token-driven system in components.css: shared-axis route entrances, staggered list entrances, a sliding side sheet and rising toasts - all on the M3 emphasized curve at token durations (300 ms routes, 250 ms lists), and all collapsing to a short fade under `prefers-reduced-motion`. The plan's micro-interactions are implemented: copy buttons morph to a check (a shared `CopyIconButton`, adopted by the snippet run card, the Library sheet and the Home CLI card; CodeBlock already had the morph), and saving in the editor flashes an inline check on the save button for 500 ms before navigating.

## 2. What I built

- `web/src/design/components.css` - the motion block:
  - `.fh-route` (the App's route wrapper, keyed by pathname): `fh-page-in`, fade + 8 px rise, medium2 (300 ms) emphasized-decelerate - the shared-axis Y entrance.
  - `.fh-list-enter > *`: `fh-item-in` (fade + 4 px), medium1, with nth-child stagger delays 25 ms apart capped at 175 ms. Applied to the Library list and Home's three lists.
  - `.fh-sheet-enter`: the Library side sheet fades and slides 16 px from the trailing edge, medium2.
  - `.fh-toast`: `fh-toast-in` rise, medium1.
  - `@media (prefers-reduced-motion: reduce)`: all four switch to `fh-fade-in` at short2 with no delay and no travel - fade only, per the plan.
  - No looping decorative animation exists anywhere (the skeleton pulse is a loading indicator and already stops under reduced motion).
- `web/src/design/components/core.tsx` - `CopyIconButton`: clipboard write, icon morphs content_copy -> check for 1.5 s on success, optional `onCopied` (pages pair it with their toast). Failure leaves the icon unchanged. Contract test added (11 tests in components.test.tsx).
- Migrations: SnippetPage run card, Dashboard side sheet, Home CLI card now use CopyIconButton (their ad-hoc copy helpers were removed where they became unused).
- `web/src/pages/EditorPage.tsx` - save success routes through `succeedSave`: the button shows a check + "Saved", disables, and navigation happens 500 ms later. All four success paths (create, meta-only, update, conflict overwrite) use it; error paths are untouched.
- `web/src/design/design.md` - a Motion section recording the system and the reduced-motion rule.

## 3. Decisions made

- **Entrances only.** Shared-axis *exits* need a presence/router-animation layer; React Router's plain Outlet remount unmounts the old route instantly, and vendoring an animation library for exit frames is not justified. The entrance half (the half users perceive as the transition) is implemented on the exact M3 pattern, and the limitation is stated here rather than faked.
- **No hover lift.** The Observe-era card lift (translateY) was deleted with its stylesheet and is not restored: M3 expresses hover as a state layer (colour), which the Material components already do. Motion stays restrained, as the plan orders.
- **The 500 ms save check delays navigation deliberately** - it is the feedback. Error paths still surface instantly, and the unsaved-changes guard stands down via the existing allowLeave path.

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 439 passed, 0 failed |
| Web lint | `npm run lint` | pass; check-tokens clean |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | 65 passed, 0 failed (64 + CopyIconButton morph test) |
| Web build | `npm run build` | pass |
| E2E | `bash scripts/e2e-web.sh --skip-build` | pass (includes the editor save flow with the new check delay) |

## 5. Acceptance criteria (plan A6)

- [x] M3 emphasized easing for shared-axis route and list-to-detail transitions, 200-400 ms (300/250 ms from the token scale).
- [x] Reduced motion = fade only (short2), no travel, no stagger.
- [x] Copy feedback: icon morph plus a snackbar (CopyIconButton + the pages' toasts; CodeBlock morphs in place).
- [x] Saving: short inline success check (500 ms check + "Saved" on the button).
- [x] No looping decorative animation; no parallax.

## 6. Security notes

None - presentation only. Clipboard writes carry only the run command / code text the user asked to copy.

## 7. Dependencies added

None.

## 8. Corrections to earlier steps

- Noted for the record: A5's observe.css deletion also removed the route transition and card hover motion from the Observe era (51600e8). A6 replaces the route motion properly; the hover lift is intentionally not restored (section 3).

## 9. Known issues / limitations

- Route exits are instant (see Decisions). If a future WP wants true two-way shared-axis, it needs a router-level transition layer.
- The stagger applies to the first eight rows; deeper rows share the cap delay.

## 10. Deferred / ideas

None.

## 11. Next step

A7: accessibility and quality pass - axe-core across routes x themes x widths as a test, keyboard walkthrough documentation, per-route page titles, live regions, and the contrast suite already started in A1.
