# Step 14 Report - Web design system (Material You 3)

## 1. Summary
The complete Material You 3 design system: tokens generated from the locked seed `#36618E` and committed, self-hosted fonts, a themed `@material/web` component layer behind typed React wrappers, an adaptive app shell (bottom bar / rail / extended rail), a hidden dev-only `/__design` review page, and a script-enforced token discipline wired into lint.

## 2. What was built
- **Token generation** (`web/scripts/generate-tokens.mjs`, `npm run tokens`): `@material/material-color-utilities` **0.2.7** (pinned - 0.3.x's ESM build is broken in Node: extensionless internal import) with `SchemeTonalSpot`, emitting committed `src/design/tokens.css` + `tokens.ts`: all 46 colour roles × light/dark, the M3 typescale, shape corners, a 4-pt spacing scale, motion, state-layer opacities, elevations, plus semantic aliases (focus ring, code background, top-bar, nav-active). The `--md-sys-color-*` / `--md-*` variables are set from the same values so `@material/web` components theme from the tokens.
- **Dark scheme mechanism**: `:root[data-theme='dark']` overrides; with no attribute, `prefers-color-scheme` decides; light is the default block.
- **Fonts** (self-hosted WOFF2 in `web/public/fonts/`, no CDN): Roboto Flex variable (latin subset, 85 KB), Roboto Mono (33 KB), Material Symbols Outlined subset (52 KB, 60 icons via the css2 `icon_names` API).
- **Theme** (`theme.ts`): system/light/dark preference in `localStorage` (try/catch everywhere), applied before first paint by an inline script in `index.html` (kept in sync with `PREPAINT_SCRIPT`), re-applied at boot; the shell's app-bar button cycles system → light → dark.
- **Components** (`src/design/components/`, typed wrappers over the md elements registered in `md.ts`): `Button` (filled/tonal/outlined/text), `IconButton`, `Fab`, `Chip`, `StatusChip`, `TextField` (with error/supporting text), `SearchBar` (M3 pill), `Tabs`, `Menu`, `PageHeader`, `Card` (3 variants), `ListItem`, `CodeBlock` (copy button, wrapped lines, optional numbers), `EmptyState`, `ErrorState`, `Skeleton`, `ToastProvider`/`useToast`, `ConfirmDialog` (with a danger variant), `AppShell` + `useWidthClass` (compact <600 / medium 600–839 / expanded ≥840).
- **`/__design`** (dev builds only): every component and state on one page.
- **Token discipline** (`web/scripts/check-tokens.mjs`, part of `npm run lint`): fails on hex colours, raw `rgb()/hsl()`, px border-radius, and font-size literals anywhere in `src/` outside the token files (tests exempt - they assert token values; documented in the script).

## 3. Colour-restraint review (design-spec §1)
- **Primary** is used only by filled buttons, the active-nav pill/extended-rail highlight, the focus ring, and links/actions - never as decoration or large fills.
- **Tertiary** appears in exactly one place: the **Public** `StatusChip` (tertiary-container), as the spec mandates. Nothing else in the system references tertiary except its `on-tertiary-container` pair in the contrast test.
- **Error** appears only in `ErrorState`, text-field error text (via the md field's error state), and the danger confirm button in `ConfirmDialog`. No other component uses it.
- **Tags/chips** are neutral: assist chips use the md default (surface/outline), tag chips are never coloured.
- Surfaces are the neutral tonal ladder (`surface-container*`); the code block uses `surface-container` via the `--fh-code-bg` alias.
- Diff colours arrive with the diff view in Step 16 and will follow the spec (gutter +/− and tone, never red/green alone).

## 4. Tests and gates (real output)
- **Contrast, programmatic**: a Vitest suite computes WCAG ratios from the generated `tokens.ts` for 9 foreground/background pairs × both schemes - **all ≥ 4.5:1** (AA). Also asserts `light.primary === #36618e` (the seed survives Tonal Spot) and the width-class boundaries and theme round-trip/corrupt-value handling.
- **Component tests**: StatusChip tertiary-only-for-Public, Chip wrapper contract, CodeBlock lines + copy control, Empty/Error states, and an **axe-core run with zero violations** over a composition. (md-* internals render into shadow DOM in real browsers; jsdom tests assert the wrapper contract - noted honestly.)
- **Token check proven**: a planted `.x { color: #ff0000; }` file made `check-tokens.mjs` exit 1 naming the file/line; removed, it prints `check-tokens: clean`.
- `npm run lint` (eslint + check-tokens): clean, 0 errors 0 warnings. `npm run typecheck`: clean. `npm test`: **13 passed (3 files)**. `npm run build`: built (one chunk-size warning for the @material/web bundle; code-splitting is future polish, recorded below).
- Serving smoke: `dist/` over a static server - index, Roboto Flex and Material Symbols all HTTP 200; the built CSS contains the token variables.

## 5. Deviations and problems
- `@material/material-color-utilities` pinned to 0.2.7 (0.3.x ESM broken under Node) and `axe-core` added as a devDependency; both recorded in `package.json`. In 0.2.7 colours come from `MaterialDynamicColors.<role>.getArgb(scheme)`; the naive `scheme[role]` access silently yields black - caught by asserting the generated values against the seed before writing files.
- `md.ts` props typing is deliberately permissive (`Record<string, unknown>` over HTMLAttributes): the elements' property surfaces don't map onto React props; type safety is enforced at the wrapper APIs instead. One targeted eslint-disable for the JSX namespace declaration (declaration merging has no ES-module form), with the reason in a comment.
- Bundle size: the single JS chunk exceeds Vite's 500 kB warning threshold because @material/web is imported wholesale for the design page; route-level code-splitting is the fix and is deferred to when real routes land (Steps 15–17), where it will actually pay off.

## 6. Known gaps
- The `/__design` page is verified by tests/build/serving, not by a real browser screenshot in this environment.
- Placeholder routes (Home/Explore/Library/Profile) show an honest "arrives in a later step" empty state until Steps 15–17.

## 7. Next step
Step 15 - web auth screens and dashboard.
