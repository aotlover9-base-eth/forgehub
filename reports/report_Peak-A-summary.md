# Report - Peak Phase A summary: the Material You rebuild

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Scope:** Work packages A0-A8 plus WP-0b (CI fix). Tag: `peak-phase-a`.
- **Status:** Done - Phase A exit gates green (see section 4).

## 1. What Phase A set out to do

The pre-Peak web app was a single pinned dark "Observe" glass theme: no light theme, no theme switch, serif display type mixed into product UI, and a shell that predated the design spec's adaptive requirements. Phase A rebuilt the visual system as faithful Material You 3 from the seed `#36618E`: real light, dark and system themes, the M3 type/shape/elevation/state-layer system, an adaptive shell (navigation bar, rail, drawer/list-detail by width class), a refreshed component library, every route redesigned, restrained M3 motion, an axe-enforced accessibility pass, and a committed visual-regression suite.

## 2. The work packages

| WP | Commit | What landed |
|----|--------|-------------|
| A0 | `1d38245` | Design audit (every screen at 360/768/1280 in both themes, screenshots in `docs/screenshots/peak-audit/`), `docs/design/PEAK-DIRECTION.md`, ADR 0009 (serif dropped from the app; Roboto Flex is the display face) |
| A1 | `15abfe9` | Theme system: light, dark, system; stored preference applied before first paint; switch in Settings and the app bar; generated tonal schemes from the seed; 21 token-pair contrast tests at WCAG AA in both schemes |
| A2 | `a17de86` | M3 type scale as tokens, shape scale, tonal elevation, state layers (hover 8 / focus 10 / pressed 10 / dragged 16), visible focus rings, 48 px touch targets |
| A3 | `e2d4402` | Adaptive shell: top app bar, navigation bar (compact), rail (medium), rail/drawer with list-detail (expanded), global search entry, FAB / extended FAB |
| A4 | `0799c96` | Component library refresh against the new tokens: buttons, fields, chips, cards, lists, dialogs, snackbars, menus, tabs, segmented controls, progress, skeletons, CodeBlock (Roboto Mono), DiffView (+/- gutter plus tonal shift) |
| A5 | `8341b9e` | Every route redesigned: landing, home dashboard, Library (density, sort/filter chips, bulk select, j/k keyboard nav, expanded side sheet), two-pane editor with validation panel and conflict dialog, snippet page with Run card and version diffing, Explore facets, profile, settings sections, device card, auth pages, 404 |
| A6 | `e6ec4ee` | M3 emphasized-easing transitions (shared axis, 200-400 ms), copy/save micro-feedback, reduced-motion honoured everywhere; no decorative loops beyond the hero video |
| A7 | `46722ba` | Axe sweep as a test: 102 route/theme/width combinations, signed in and out, zero violations (one documented upstream @material/web rule exception); four real defects fixed (disabled-field contrast, unnamed Raw link, missing h1s, FAB landmark); per-route document titles; `docs/keyboard-walkthrough.md` |
| A8 | `2952c34` + `3bb18b0` | Playwright visual regression: 12 screens x 2 themes x 3 widths = 72 committed goldens under `web/e2e/visual/__screenshots__/`, env-gated (`VISUAL_TESTS=1`), runner `scripts/visual-web.sh`, deterministic seed + frozen clock. Goldens generated on a CI runner - the sandbox write fault blocks the local server; see `reports/report_Peak-A8.md` |
| 0b | `0ab22f8` | CI fix: main had been red since A3 (Web lint only). Three react-hooks v7 violations fixed in code; no rule weakened. See `reports/report_Peak-0b.md` |

## 3. Before and after

Before (Observe dark glass, pinned dark, serif display): `docs/screenshots/peak-audit/landing-1280.jpg`, `library-1280.jpg`, `editor-1280.jpg`, `settings-1280.jpg` (38 audit captures in total, three widths each).

After (Material You, both themes): `docs/screenshots/peak-audit/after-a1-library-light-1280.jpg`, `after-a2-library-dark-1280.jpg`, `after-a5-landing-light.jpg`, `after-a5-landing-dark.jpg`, `after-a5-editor.jpg`, `after-a5-settings-rail.jpg`, `after-a3-library-compact.jpg` / `after-a3-library-expanded.jpg`. The A8 goldens are the durable after-record: every key screen at all three widths in both themes.

## 4. Phase A exit gates

| Gate | Result |
|------|--------|
| `check-tokens` | green |
| Web lint / typecheck / tests / build | green (CI for the A8 head `3bb18b0`, run `44965399856`, all five jobs) |
| Axe sweep (A7) | green - 102 combinations, one documented upstream exception |
| E2E suite | green in CI |
| Visual suite (A8) | goldens generated and the check run passed in workflow run `44964200459`; env-gated in CI by design. Local check run not run - sandbox write fault (see the A8 report) |
| CI on `main` | green |

## 5. Decisions and honest notes

- The Observe look is fully retired, per the plan; nothing of the glass/serif system remains in the app. The intro video on the landing page still shows the pre-Peak UI and the old `exit` wording - regenerating video is out of scope for this phase (plan section 8); the mismatch is recorded in `docs/requests/PERMISSION-REQUESTS.md`.
- The A3-A7 CI redness (WP-0b) is the phase's process scar: local gate claims were not reconciled with CI for five consecutive WPs. From Phase B on, a WP is done only when the CI run for its own head has been eyeballed.
- The sandbox positioned-write fault (see PROGRESS.md) slowed A8 and forced the 0b and A8 commits through the GitHub Git API; both are documented in their reports. No gate was faked around it: what could not run locally says "not run" and was verified in CI instead.

## 6. Next

Phase B - CLI improvements (B0 audit first), per the plan.
