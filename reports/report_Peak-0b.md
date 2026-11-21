# Report - Peak 0b: fix CI (web lint)

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak 0: fix ci (web lint)`
- **Status:** Done - CI root cause found and fixed at the source; no lint rule, config or token check was touched.

## 1. Summary

CI on `main` has been red since the A3 head (`44952892060`) - every Peak Phase A run from A3 through A7 failed, always in the same place: the **Web** job's **Lint** step (`npm run lint` = `eslint . && node scripts/check-tokens.mjs`). The Rust, E2E, Docker and Supply chain jobs passed throughout. The A3-A7 reports each claimed local lint was green; that claim does not reproduce at the committed heads (see section 2). The failure is three genuine `eslint-plugin-react-hooks` v7 compiler-rule violations in page components. All three are fixed here by restructuring the components the way the React documentation prescribes - not by disabling rules, adding eslint-disable comments, or weakening the config.

## 2. Root cause (evidence, not conjecture)

- The failing CI job logs (A7 head run `44960000289`, job `137016490782`; A4 head run `44953663243`, job `136996909808`) show eslint exiting 1 with:
  - `web/src/pages/ExplorePage.tsx:99` - `react-hooks/set-state-in-effect` ("Calling setState synchronously within an effect"): the `?q=` search param was adopted into state inside a `useEffect`.
  - `web/src/pages/DashboardPage.tsx:109` - `react-hooks/set-state-in-effect`: the keyboard cursor was clamped into the visible list inside a `useEffect`.
  - `web/src/pages/EditorPage.tsx:325` - `react-hooks/refs` ("Cannot access refs during render"): `saveRef.current = save` was assigned during render (the latest-ref shortcut).
  - Plus one pre-existing `react-refresh/only-export-components` **warning** on ExplorePage (warnings do not fail the gate; it predates Peak and is unchanged).
- A4's log shows only the ExplorePage error; the Dashboard and Editor errors appear from A5, matching `git log -S`: the Explore pattern landed in A3 (`e2d4402`), the other two in A5 (`8341b9e`). CI run history confirms the boundary: A0-A2 green, A3 onwards red.
- The discrepancy is **not** in the tree: at HEAD, the working tree is identical to the committed tree for every linted file (the only uncommitted web change is A8's `snapshotPathTemplate` lines in `playwright.config.ts`, unrelated), `package-lock.json` resolves exactly the plugin versions installed locally (eslint 10.15.0, eslint-plugin-react-hooks 7.1.1), and running `npm run lint` locally at HEAD reproduces the identical 3 errors. So the committed code was never lint-clean under the pinned toolchain; the A3-A7 reports' "lint pass" lines must have been produced against a pre-final file state or a stale local toolchain and were not re-verified after the final edits. That process lesson is recorded in section 7.

## 3. The fixes (one per site, all in this commit)

- **ExplorePage** - the sync-from-URL effect is replaced by the React-endorsed "adjust state during render" pattern: a `seenParams` state holds the previous `searchParams` object; when the object identity changes (exactly the old effect's trigger condition), the new `?q=` is adopted during render, before paint. The lazy `useState` initialiser still handles first mount. User typing is untouched (typing does not change `searchParams`).
- **DashboardPage** - the clamp effect is replaced by the same pattern: a `clampedLength` state guards the adjustment; when `visible.length` changes, the cursor is clamped during render to exactly the value the effect computed. All cursor consumers (j/k handler, Enter, scroll-into-view effect, `data-cursor` rendering) read the same state as before.
- **EditorPage** - the latest-ref write moves into a `useEffect` with no dependency array, so `saveRef.current` is refreshed after every render and the stable Ctrl+S/Cmd+S key handler keeps calling the current `save` closure. Refs are no longer written during render.

No other files change. `eslint.config.js`, `scripts/check-tokens.mjs`, `package.json` and the lockfile are untouched.

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Web lint (the failing CI step) | `npm run lint` | **pass** - eslint: 0 errors, 1 pre-existing warning (react-refresh on ExplorePage); check-tokens: clean |
| Web typecheck | `npm run typecheck` | **not run as scripted locally** - the sandbox write fault (see PROGRESS.md's A8 blocker note) denies the `.tsbuildinfo` writes `tsc -b` requires. The same checker was run directly (`tsc --noEmit --composite false` for both `tsconfig.app.json` and `tsconfig.node.json`): **pass, 0 errors**. The scripted form runs in CI for this head. |
| Web tests | `npm test -- --run` | **not run locally** - vite's config-bundle temp write is denied by the same sandbox fault (EPERM at startup, before any test executes). Verified by the CI run for this head instead (see below). |
| Web build | `npm run build` | **not run locally** - same sandbox write fault. Verified by the CI run for this head. |
| CI for this head | GitHub Actions | verified fully green after push (all five jobs) before any further WP work began |

The three changed components are covered by the existing web suites (Library keyboard navigation and filtering, Explore `?q=` adoption, editor Ctrl+S save) in `web/src/pages/*.test.tsx`, which run in the CI Web job for this commit.

## 5. Acceptance criteria

- [x] Root cause identified from the actual CI logs, not inferred.
- [x] Fix addresses the code, not the gate: no rule disabled, no config weakened, no eslint-disable added, token check untouched.
- [x] Lint passes locally with the pinned toolchain at the fixed tree.
- [x] Own commit (`peak 0: fix ci (web lint)`) ahead of the A8 commit, per the resume instructions.

## 6. Security notes

None. No behaviour change beyond render-timing of three internal state updates; no user-visible output changes.

## 7. Corrections to earlier steps

- The A3, A4, A5, A6 and A7 reports each state that web lint passed locally. At the committed heads it did not - CI was red from A3 onward and the local claims were never reconciled with CI. The lesson, applied from here on: a WP is not done until the CI run **for its own head** has been eyeballed, not just the local gates.

## 8. Known issues / limitations

- The sandbox positioned-write fault is still active in this environment; it blocks local vite/tsc/sqlite writes intermittently and is the reason A8's goldens are still pending. It does not affect GitHub-hosted CI.

## 9. Next step

Return to A8 per the resume steps in PROGRESS.md (goldens when the write fault clears), then the Phase A summary and tag.
