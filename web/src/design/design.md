# ForgeHub design library

The component library lives in `web/src/design/`. Everything visual in the
app is built from these components plus the tokens in `tokens.css` - pages
never use raw Material elements or literal colours, sizes, radii or font
sizes (`scripts/check-tokens.mjs` enforces this in lint). The gallery at
`/dev/design` (route `/__design`, dev builds) shows every component and
state; this file is the written contract.

Foundations (Peak A1/A2): colour from seed `#36618E` (tonal spot) in light
and dark schemes, switched by the `data-theme` attribute and the
`forgehub-theme` preference (system / light / dark); the full M3 typescale
with weights and tracking; the M3 shape scale; tonal elevation (shadows
only at levels 1-2, plus Material's own for menus, dialogs and the FAB);
state layers at the spec opacities (hover 8%, focus 10%, pressed 10%,
dragged 16%); the full M3 duration and easing scale.

## Rules that apply to every component

- Tokens only. A component that needs a new value adds a token first.
- `tertiary` appears at most once per screen (the Public status chip);
  `error` is only for errors and destructive actions.
- Every interactive element is at least 48 x 48 px, shows the focus ring
  (or rings its container, like the search bar), and works by keyboard.
- Motion uses the duration/easing tokens and collapses under
  `prefers-reduced-motion`.
- Text is never truncated in lists or code; long lines wrap.

## Components

### AppShell (`components/AppShell.tsx`)
The adaptive frame: small top app bar (compact/medium) or medium top app
bar (expanded), navigation bar / rail / extended rail by width class, a
content column that scrolls internally and resets on `scrollKey` change,
and an optional FAB slot that clears the navigation bar on compact.
States: active destination carries the secondary-container indicator and
a state-layer overlay on hover/press.

### Button (`components/core.tsx`)
Filled (one primary action per view), tonal, outlined, text - over the
matching md-* buttons, so state layers, focus and disabled states are the
spec's. `icon` adds a leading Material Symbol. Never hand-roll a button.

### IconButton
md-icon-button with a mandatory text `label` (aria-label + title).

### Icon
Material Symbols (self-hosted variable font), sizes from the icon tokens.
Decorative by default (`aria-hidden`); meaning is carried by labels.

### Fab
md-fab; with `text` it renders the extended FAB (expanded layouts).
Primary-container colours come from the component - do not restyle.

### Chip
Assist chip (tags, metadata). For selectable filters use the filter-chip
pattern in pages via md-filter-chip inside md-chip-set.

### StatusChip
The one visibility treatment: Public = tertiary (the screen's single
tertiary use), Unlisted = secondary container, Private = surface variant
with a lock icon. Never recolour it locally.

### TextField (`components/inputs.tsx`)
Outlined field; errors go through `errorText` (sets the field's error
state and replaces supporting text). Multiline renders the textarea form.

### SearchBar
The M3 search bar: surface-container-highest pill, leading search icon,
clear button when non-empty, submits on Enter, rings its container on
focus-within. Used for in-page search; the app bar's search icon routes
to Explore instead of duplicating this.

### Switch / Checkbox / Radio (`components/inputs.tsx`, Peak A4)
Labelled selection rows with one anatomy: a 48 px row, token gap, the
label as a real `<label>`; Radio rows top-align for multi-line help text.
Bare Checkbox (ariaLabel only) is for dense grids like the version
picker. Pages must use these wrappers - never raw md-switch /
md-checkbox / md-radio elements.

### SelectField
Outlined select (md-outlined-select) with headline options. Use for
short closed lists (expiry, sort); longer lists belong in a Menu.

### Tabs
Primary tabs over md-tabs; keep to 2-4 destinations of one object
(Snippet: Code / Versions / Forks; Settings sections).

### Menu
Anchored md-menu behind an icon button; items carry an id, label and
optional leading icon; selection closes the menu. The app bar account
menu is this component.

### PageHeader (`components/surfaces.tsx`)
Title (headline-small), one-line supporting text, trailing actions.
One per page.

### Card
Elevated (surface-container-low + level 1), filled
(surface-container-highest), outlined (surface + outline-variant).
Large corner (16). Cards group; they are not buttons - put the action
inside.

### ListItem
md-list-item anatomy: leading (start), headline, supporting text,
trailing (end). Rows are 48 px minimum; use inside md-list.

### CodeBlock
Code on the code tokens with Roboto Mono, optional line numbers, wrap
never truncate, copy button with a "Copied" confirmation. The header
carries the snippet name and version.

### DiffView (`components/DiffView.tsx`)
Unified diff with token colours for add/remove rows and a mono gutter;
used by the snippet version diff and (Peak B3) mirrored by the CLI card.

### EmptyState / ErrorState
Centred icon, title, one-line message, one action. ErrorState is
role=alert and always offers recovery (retry) when one exists.

### Skeleton
Pulse blocks on surface-container while content loads; shape it like
the content it replaces.

### Toast (`components/feedback.tsx`, `useToast`)
The M3 snackbar: bottom-centred (above the navigation bar on compact),
one optional action, auto-dismiss, queued one at a time. Errors that
need a decision belong in a dialog, not a toast.

### ConfirmDialog / DialogActions
md-dialog with headline, content and DialogActions (text cancel +
filled confirm). `danger` makes the confirm button error-coloured and
is reserved for destructive actions; deletion flows type the name in
pages that need it.

## Adding a component

1. Check @material/web has the M3 component; wrap it, never fork it.
2. Register the element and its JSX type in `md.ts`.
3. Style only through tokens in `components.css`.
4. Add it to the gallery and to this file, with its states.
5. Add a wrapper-contract test in `components.test.tsx`.
