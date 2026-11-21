# Report - Peak A7: accessibility and quality pass

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A7: accessibility and quality pass`
- **Status:** Done

## 1. Summary

Axe now runs as a test, not a hope: `web/e2e/a11y.spec.ts` sweeps every route in both themes at all three M3 width classes (360/768/1280), signed in and signed out - 102 route/theme/width combinations - and fails on any violation. The first sweep found five real defect classes; four were fixed in the product (a disabled-field contrast failure, a link with no accessible name, missing h1s on standalone empty pages, the FAB living outside any landmark) and one is an upstream @material/web shadow-DOM defect, disabled surgically in the sweep with the reason recorded at the call site. The pass also adds per-route document titles, and the keyboard walkthrough for the five core flows is documented from the actual UI.

## 2. What I built

- `web/e2e/a11y.spec.ts` - the sweep. It seeds its own user + public snippet per test (the harness server is shared across the file; per-theme usernames keep tests isolated), sets the theme via localStorage before first paint, and injects axe through a same-origin route interception because the app CSP (`script-src 'self'`) rightly blocks inline injection. It runs in the existing harness, so CI executes it with `scripts/e2e-web.sh`.
- Fixes from the first sweep:
  - **Contrast (serious):** the editor's disabled Name field rendered its label and supporting text at 2.34:1 (the Material disabled colour). The field is now `readonly` instead of `disabled` (a `readOnly` prop on TextField) - full contrast, text stays selectable, which is also better UX for a value you may want to copy. WCAG exempts disabled text, but readonly removes the question entirely.
  - **Link name (serious):** the snippet page's Raw link (an anchor wrapping a Material button) had no computable name; it now carries `aria-label="Open the raw snippet in a new tab"`.
  - **Heading order (moderate):** standalone empty pages (404, missing profile) had no h1 - EmptyState's title was always an h2. EmptyState gained `titleLevel`; standalone pages pass 1.
  - **Landmarks (moderate):** the FAB rendered outside every landmark. AppShell now renders it inside `<main>` (it is position:fixed, so nothing moves visually).
  - **Upstream (minor, not fixable here):** interactive `md-list-item` renders `<button role="listitem">` in its shadow DOM, which axe's aria-allowed-role flags. The control remains focusable, labelled and keyboard-operable; the markup is the vendored component's. The sweep disables that single rule with a comment explaining exactly this; everything else stays on.
- Per-route titles (`App.tsx` `usePageTitle` + the landing's own effect): static routes map to names ("Library - ForgeHub", "Settings - ForgeHub", ...); dynamic routes derive from the path (`<name> by <owner> - ForgeHub`, `Edit <name> - ForgeHub`, `<username> - ForgeHub`, "Not found - ForgeHub"). The stale router comment describing the Observe landing was corrected in passing.
- `docs/keyboard-walkthrough.md` - tab order and keys for signup, create-a-snippet, run instructions, the settings token flow and device approval, plus the global shortcuts and Library extras (j/k, bulk select), all matching the shipped UI (including the Esc-then-Tab editor escape and Deny-before-Approve tab order on the device card).
- Already in place and re-verified, not rebuilt: `lang="en"` on `<html>`; toasts are `role="status"` live regions (feedback.tsx); every icon-only control goes through IconButton/Menu, which require labels; the A1 contrast suite covers 21 token pairs at AA in both schemes (design.test.ts).

## 3. Decisions made

- **One axe rule is disabled, with the evidence attached.** "Axe clean" achieved by silently switching rules off would be worthless; the single disabled rule is an upstream shadow-DOM defect we cannot patch from outside the component, the control's operability was verified by hand, and the justification lives in the spec next to the switch. If @material/web fixes list-item roles upstream, the disable can come out.
- **Readonly over disabled for the name field** - see section 2; it fixes the contrast finding at the root instead of exempting it.
- **The sweep seeds through the real UI signup** (then creates its snippet through the API with that session), so it also smoke-tests the real auth path in both themes. (The seed originally posted the signup payload directly; the repo's secret scanner flags any password-assignment-shaped line in a diff - including public test fixtures - so the seed follows the codebase's own convention, the same one app.spec.ts uses: credentials only ever flow through form fills and variables.)

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 439 passed, 0 failed |
| Web lint | `npm run lint` | pass; check-tokens clean |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | 65 passed, 0 failed |
| Web build | `npm run build` | pass |
| E2E + axe | `bash scripts/e2e-web.sh --skip-build` | 5 passed: the web story + axe sweep signed-in/out x light/dark (102 route/theme/width combos, 0 violations with the one documented rule exception) |

## 5. Acceptance criteria (plan A7)

- [x] axe-core clean on every route in both themes at all three widths - as a test in the e2e harness (one upstream rule exception, documented).
- [x] Keyboard-only walkthrough documented for signup, create, run instructions, settings token flow, device approval.
- [x] Contrast tests for every token pair used (A1 suite, 21 pairs, both schemes).
- [x] Screen-reader labels on every icon-only control (library-enforced; the sweep's link-name/button-name rules now guard it).
- [x] Live region for snackbars (role=status) and form errors (field error text + generic banners were already announced patterns; verified in the walkthrough).
- [x] Language attribute (en) and per-route page titles.

## 6. Security notes

None. The axe injection in tests uses a same-origin route fulfilment inside the test browser only; the shipped CSP is untouched (and is what forced the technique).

## 7. Dependencies added

None (axe-core was already a devDependency; the sweep uses its browser build).

## 8. Corrections to earlier steps

- The A5 report said the Raw link and FAB placements were fine as built; the sweep proved otherwise for assistive tech - both fixed here.
- EmptyState's heading level was wrong for standalone pages since its introduction; now configurable with h1 where the state IS the page.

## 9. Known issues / limitations

- The `aria-allowed-role` exception stands until @material/web fixes interactive list-item roles upstream; it is the only non-clean rule in the sweep.
- The sweep covers the routes reachable in the seeded state; dialog-open states (menus, the token dialog) are covered by the jsdom axe checks in the component tests and step17, not by this sweep.

## 10. Deferred / ideas

None.

## 11. Next step

A8: visual regression - Playwright screenshot suite across screens x themes x widths with committed baselines.
