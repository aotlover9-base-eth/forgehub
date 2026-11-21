# Report - Peak A4: component library refresh

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commit:** (this commit) `peak A4: component library refresh`
- **Status:** Done

## 1. Summary

The audit found the library's real gap: it had no selection controls, so four pages reached past it and dropped raw `md-switch` / `md-checkbox` / `md-radio` elements (and one raw HTML `<select>`) inline, each with its own ad-hoc row styling. A4 adds the missing wrappers - `Switch`, `Checkbox`, `Radio`, `SelectField` - with one labelled-row anatomy, migrates every raw call site to them, registers `md-outlined-select` / `md-select-option`, exports `DiffView` from the library index (SnippetPage imported it by path), extends the gallery with the new components and a Diff example, adds wrapper-contract tests, and writes the library's contract down in `web/src/design/design.md`. A usage sweep found no dead components: every export has at least one consumer, so nothing was deleted.

## 2. What I built

- `web/src/design/components/inputs.tsx` - four new wrappers:
  - `Switch` / `Checkbox` / `Radio`: a `<label class="fh-select-row">` (48 px row, token gap, pointer cursor; Radio top-aligns for multi-line help text) around the Material element; the change handler receives the control's new value read from the event target, not a blind toggle. Checkbox without a label renders the bare control for dense grids (the version picker).
  - `SelectField`: md-outlined-select with headline options.
- `web/src/design/md.ts` - registers `@material/web/select/outlined-select` and `select-option` with JSX types.
- `web/src/design/components.css` - `.fh-select-row` anatomy.
- Migrations (no raw selection elements remain in any page - verified by grep):
  - EditorPage: visibility radio group -> `Radio` (rich label kept); Soft wrap / Line numbers -> `Switch`.
  - SettingsPage: theme radio group -> `Radio`; token scope checkboxes -> `Checkbox`; the expiry `<select>` -> `SelectField` (label "Expires").
  - SnippetPage: version-picker checkboxes -> bare `Checkbox`.
- `web/src/design/index.ts` - `DiffView` exported with the rest of the library.
- `web/src/pages/DesignPage.tsx` - gallery gains a Selection controls section (all four, live state) and a Diff section; the stale header comment (it described the deleted Observe theme) is corrected.
- `web/src/design/components/components.test.tsx` - 4 new wrapper-contract tests (attribute mirroring + value-reporting change events for each control). Web tests 60 -> 64.
- `web/src/design/design.md` - the library contract: foundation rules, every component with its M3 counterpart, variants, states and usage rules, and the steps for adding a component.
- Evidence: `docs/screenshots/peak-audit/after-a4-token-dialog.jpg` and `after-a4-token-dialog-select-open.jpg` - the production Settings token dialog with the new Checkbox rows and the SelectField menu open (Never / 30 / 90 / 365 days).

## 3. Decisions made

- **No component was deleted.** The plan's deletion rule targets dead code; the sweep (every export counted against all of `src/`) shows each component has a consumer. `ListItem`'s only consumer is the gallery today; it is the M3 list primitive A5's Library redesign adopts, and design.md now documents it as such. Deleting it days before its planned use would be churn, not hygiene.
- **The wrappers read the new value from the event** (`selected` / `checked` / `value`) instead of the pages' old blind `!state` toggles - identical behaviour for real clicks, correct for keyboard and programmatic changes.
- **SelectField wraps md-outlined-select rather than styling the native select**: the one raw `<select>` was the last non-Material control in the app; it now matches the text fields it sits beside (proven in the evidence capture).

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 438 passed, 0 failed |
| Web lint | `npm run lint` | pass; check-tokens clean |
| Web typecheck | `npm run typecheck` | pass |
| Web tests | `npm test -- --run` | 64 passed, 0 failed |
| Web build | `npm run build` | pass |
| E2E | `bash scripts/e2e-web.sh --skip-build` | pass |
| Manual | Chromium on the production build | token dialog: checkboxes render checked with labels; the select opens its M3 menu with all options (screenshot) |

## 5. Acceptance criteria (plan A4)

- [x] Every component audited against its M3 counterpart (design.md records counterpart, variants and states for all 24 exports).
- [x] Components shown in the /dev/design gallery - the four new ones and DiffView added; the rest were already covered.
- [x] Each component documented in web/src/design/design.md.
- [x] Unused components deleted - none were unused (sweep evidence in section 1); the rule is recorded in design.md for future additions.
- [x] Pages consume the library instead of raw elements: no `md-switch`, `md-checkbox`, `md-radio` or `<select>` remains in any page.

## 6. Security notes

None - presentation components only. Labels render as React text.

## 7. Dependencies added

None (the select element ships inside the already-vendored @material/web).

## 8. Corrections to earlier steps

- The DesignPage header comment still described the removed Observe theme; corrected.
- `DiffView` was previously imported by direct file path from SnippetPage; it is a normal library export now (the page's import is unchanged and still resolves - index re-export added, path import kept valid).

## 9. Known issues / limitations

- The gallery route exists only in dev builds (`import.meta.env.DEV`), so gallery verification in this WP was done on the production pages that consume the same wrappers (Settings, Editor, Snippet) plus the jsdom contract tests; A8's visual suite runs against the dev server for the gallery if needed.

## 10. Deferred / ideas

None.

## 11. Next step

A5: screen redesign - every route rebuilt to the PEAK-DIRECTION section 7 wireframes: Landing on tokens with the serif and the Observe leftovers removed, Home, Library (bulk select, j/k, side sheet), Explore, Snippet (tabs + two-version diff), Editor (two-pane, Ctrl+S, conflict dialog), Profile, Settings (density option), Auth.
