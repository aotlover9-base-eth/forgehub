# Spec — Design (Material You / Material Design 3)

- **Status:** Normative for Steps 14–17. **Version:** 1 (Step 2, 2026-11-19)
- **Law:** `agent.md` §8 wins over this document on any conflict. This spec expands roadmap §4 into screens and states.
- **Seed colour:** `#36618E`. One generated light + dark scheme (`@material/material-color-utilities`), committed as tokens. No design work happens before Step 14; this document is the plan Step 14 implements.

## 1. Colour restraint (the reviewable rule)

- Surfaces and text are neutral tonal values from the generated scheme.
- `primary` appears only on: the primary action of a screen, the selected navigation destination, focus rings, links, and progress indicators.
- `tertiary` appears at most once per screen, only on the small "Public" status chip.
- `error` appears only on error text, error states and destructive confirmations.
- Tags are always neutral outlined chips. Visibility chips: Private = `secondary-container`, Unlisted = `secondary-container`, Public = `tertiary-container` (the one tertiary use).
- Diff additions/deletions are distinguished by a `+`/`-` gutter symbol **and** a tonal background shift, never by green/red alone.
- No gradients anywhere. No colour is hardcoded: components use `--md-sys-color-*` and the token variables of §4 only (enforced by `web/scripts/check-tokens.mjs` from Step 14).

## 2. Navigation map

```text
/                     landing (logged out) · dashboard redirect (logged in)
/login                login
/signup               signup
/dashboard            my snippets (logged-in home)
/new                  new snippet
/explore              explore + search (public)
/device               device-code approval (CLI login)
/settings             profile · security · tokens · CLI · danger zone (tabs)
/{username}           public profile
/{owner}/{name}       snippet detail (tabs: Code · Versions)
/{owner}/{name}/edit  edit snippet
/__design            dev-only design-system gallery (Step 14)
*                     404 page
```

App shell: top app bar (small) with wordmark, search entry (from Step 17), theme menu and account menu; navigation by width class (§5): bar (compact), rail (medium), rail or drawer + list-detail (expanded). A FAB ("New snippet") appears on dashboard and profile when logged in.

## 3. Page inventory — layout and states per width class

For every screen: **compact** < 600 px, **medium** 600–839 px, **expanded** ≥ 840 px (content max ~1200 px, centred).

| Screen | Compact | Medium | Expanded | States that must be designed |
|--------|---------|--------|----------|------------------------------|
| Landing `/` | Single column: what it is, the `fg ubuntu-setup` CodeBlock example, primary "Create account", text "Explore" | Same, wider measure | Two-column hero (text / example card) | Logged-in visitors redirect to `/dashboard` |
| Login / Signup | Single centred card column | Same, card max-width | Same | Field errors inline; generic auth error banner; rate-limit message with retry time; signup shows password guidance (length + denylist, no composition rules) |
| Dashboard | Snippet list, filter chips row (All/Private/Unlisted/Public), FAB | Rail + list | Rail + list, wider rows | Loading: skeleton rows. Empty: explanation + single "New snippet" action + install-the-CLI card. Error: message + Retry |
| New / Edit | Single-column form; preview panel below the editor | Same | Two panes: form/editor left, live preview/validation right | Validation errors with line numbers; secret findings block Save; lint warnings do not; unsaved-changes guard; version-conflict dialog (theirs / overwrite / copy mine) |
| Snippet detail | Header, "Run it" card, CodeBlock, tabs stacked | Same | Header + two-column (code / side metadata) where useful; Versions tab supports two-select diff | Private-to-others = the shared 404 page (never a "private" reveal); hostile content renders as inert text; changed-visibility confirmations |
| Explore | Search bar, filter chips, result cards list | Rail + list | Rail + list with popular-tags row | No-results state (query echoed as text, one action: clear filters); loading skeletons; "Load more" with stable cursors |
| Profile `/{username}` | Avatar-initial, name, bio, public snippet list | Same | Same, wider list | Unknown user = 404 page; user with no public snippets = empty state (no private counts leaked) |
| Settings | Tabs become a stacked list of sections | Tabs (vertical) | Tabs (vertical) + content pane | Token plaintext shown **once** in a CodeBlock with an explicit "I've copied it" acknowledgement; session revoke confirmations; account deletion requires typing the username |
| Device `/device` | Single centred card: code entry/confirmation, client name, what approval grants, Approve / Deny | Same | Same | Not-logged-in → login → return here; invalid/expired code error with rate-limit messaging; success and denied end states; never auto-approves |
| 404 / error boundary / offline | Centred message + one primary action ("Go home") | Same | Same | Offline banner is global, dismissible, and non-colour-only (icon + text) |

## 4. Tokens (implemented in Step 14 in `web/src/design/tokens.css` + `tokens.ts`)

- **Colour roles:** `primary, on-primary, primary-container, on-primary-container, secondary, on-secondary, secondary-container, on-secondary-container, tertiary, on-tertiary, tertiary-container, on-tertiary-container, error, on-error, error-container, on-error-container, surface, surface-dim, surface-bright, surface-container-lowest, surface-container-low, surface-container, surface-container-high, surface-container-highest, on-surface, on-surface-variant, outline, outline-variant, inverse-surface, inverse-on-surface, inverse-primary, scrim, shadow`.
- **Type:** Roboto Flex (fallback Roboto), self-hosted; Roboto Mono for code. Scale: display L/M/S, headline L/M/S, title L/M/S, body L/M/S, label L/M/S — values from the M3 type scale, as tokens.
- **Shape:** extra-small 4, small 8, medium 12, large 16, extra-large 28, full 9999 (px, in tokens only).
- **Elevation:** tonal containers first; shadow levels 0–3 only for menus/dialogs.
- **State layers:** hover 8%, focus 10%, pressed 10%, dragged 16%; disabled content 38%, container 12%.
- **Motion:** emphasized `cubic-bezier(0.2, 0, 0, 1)`; exit `cubic-bezier(0.3, 0, 1, 1)`; durations short1–4, medium1–4, long1–4 tokens (100–500 ms); `prefers-reduced-motion` disables non-essential motion.
- **Spacing:** 4-point tokens: 4, 8, 12, 16, 24, 32, 48.
- **Theme:** `prefers-color-scheme` by default with a manual system/light/dark override persisted in `localStorage` (guarded try/catch), applied before first paint.

## 5. Components (Step 14 builds these; Steps 15–17 use only these)

From `@material/web` where available: filled / filled-tonal / outlined / text buttons, filled text fields, chips, menus, dialogs, tabs, switch, checkbox, linear/circular progress, lists, icon buttons.
In-house (M3-spec, token-only): TopAppBar, NavigationBar, NavigationRail, NavigationDrawer, AppShell, PageHeader, Card (filled/outlined), SnippetCard, CodeBlock (surface-container-high, Roboto Mono, copy icon-button, optional line numbers, wrapped long lines), EmptyState, ErrorState, Skeleton, Snackbar (via a ToastProvider), ConfirmDialog, SearchBar, Fab, StatusChip (visibility), ThemeMenu.

**Icons:** Material Symbols Outlined, self-hosted subset only. **Touch targets** ≥ 48 px. **Focus:** visible focus ring on every interactive element; full keyboard navigation; `aria-*` only where native semantics are insufficient.

## 6. Accessibility requirements

WCAG 2.2 AA: contrast verified programmatically for the key role pairs in both schemes (Step 14 test); no information by colour alone (diff §1, offline banner, lint warnings carry icons/text); form errors are text associated with their fields; dialogs trap focus and return it; the editor's Tab handling must not trap keyboard users (Esc, then Tab leaves).

## 7. Content design (microcopy)

Calm, direct, plain. No jokes in error or security contexts. Errors say what failed, why, and how to fix it. Security copy is informative, not frightening: e.g. on a third-party snippet — "Review commands before you run them. `fg` will ask you to confirm." Destructive actions name the thing being destroyed ("Delete `ubuntu-setup`?"). The device page states plainly what approval grants: "This will let the CLI on `<client_name>` read and write your snippets."
