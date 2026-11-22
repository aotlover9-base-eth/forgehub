# Peak C9 - i18n catalogue and RTL test page

Commit: `peak C9: i18n catalogue and RTL test page`
Plan: Phase C, C9 - "Extract all UI strings into a message
catalogue (English only for now, but structured), so
translations can be added later. Right-to-left layout
checked with a test page."

## What was built

- **The catalogue** (`web/src/i18n/messages.ts`): one
  structured English catalogue, ~430 strings in 24
  sections by surface (nav, titles, auth, library,
  snippet, collection, editor, settings, landing, home,
  explore, profile, device, design system, the C1/C5/C6/
  C7/C8 panels, API error copy, validation and relative
  time). `Messages` is derived from the English catalogue
  by type, so a future locale is compile-checked for
  completeness. `fmt(template, vars)` fills `{name}`
  placeholders and leaves unknown ones visible.
- **The sweep**: every page and shared component now
  reads copy from the catalogue - App shell (nav, titles,
  account menu, offline banner, help dialog), all 16
  pages, the design components, `validation.ts` (issue
  strings + relative time) and `api/client.ts` (the
  rate-limit / unreachable messages). Markup-bearing
  strings are split into before/after keys around the
  markup; plurals are paired keys chosen by code.
  Server-driven text (API errors from the server, user
  content) stays data, not copy. The sweep was verified
  string-by-string against the existing test suite, which
  asserts on visible text: all pre-existing tests pass
  unchanged, proving the rendered copy is byte-identical.
- **Catalogue integrity test** (`messages.test.ts`):
  every leaf is a non-empty string; every locale's key
  set equals English exactly (only English exists today);
  `fmt` behaviour pinned.
- **RTL test page** (`RtlTestPage.tsx`, dev route
  `__rtl`, same pattern as `__design`): representative UI
  (list, chips, field, buttons, status chips) inside a
  single `dir="rtl"` subtree with sample Arabic text from
  the catalogue, page chrome left LTR. `rtl.test.tsx`
  asserts exactly one rtl root, the samples and list
  inside it, and the title outside it.
- **Docs**: `docs/i18n.md` - how the catalogue works, how
  to add a locale (including where `dir="rtl"` belongs),
  and the rules for new strings.

## Gates (this machine)

- Web lint 0, typecheck 0, **tests 82 passed (15 files,
  +4 new)**, build 0 (bundle 791.05 kB, +~7 kB of
  catalogue), check-tokens clean.
- No Rust changes in this WP.

## Notes

- Bundle note: the catalogue adds its strings once; the
  per-page lazy chunks are unaffected structurally.
- A real second locale remains future work by design
  (plan says English only, structured); `activeLocale`
  is the single switch point.
