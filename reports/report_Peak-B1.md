# Report - Peak B1: output design system for the terminal

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B1 (Phase B - CLI improvements). Fixes audit F3, F12, F13 and F15 (found during this WP).
- **Status:** Done, pending CI verification of this commit's head (local cargo is blocked by the sandbox write fault; see Gates).

## 1. What landed

- **New module `crates/forge-cli/src/output.rs`** - the one output system for the CLI (agent.md section 8.4, spec-cli section 3):
  - `Style::resolve` (pure, fully unit-tested): colour from the `--color` mode, `NO_COLOR`, `TERM=dumb` and TTY-ness; box-drawing from UTF-8 locale + `TERM`. Non-TTY output is always plain, so pipes and captured output stay byte-stable.
  - Paint helpers with the section 8.4 discipline: `accent` (blue, headings/selection only), `dim` (secondary), `success`/`warn`/`error` (green/amber/red, semantic only).
  - Components: `heading`, aligned `kv` blocks, and `card` - the rounded "surface" card from spec-cli section 3 with an ASCII fallback. Character-count widths (wide glyphs are not width-compensated; noted in the doc comment).
  - Process-wide `output::init` / `output::style()` following the existing `cloud::VERBOSE` global pattern; `Style::PLAIN` before init.
- **`--color auto|always|never`** global flag (clap `ValueEnum`), documented in spec-cli (section 3 already specified it; now implemented) and `docs/cli-reference.md`.
- **Error standard completed at the single choke point** (`exit::render_error`, called from `main`):
  - Human form: the existing what/why chain, plus a `fix:` line when the chain bottoms out in an `io::Error` with a known remedy (PermissionDenied, NotFound, generic) and the message does not already carry one. This is the F12 fix - the B0 audit observed `writing .../config.yaml: Operation not permitted (os error 1)` reaching the user bare.
  - `--json` form: the spec-cli section 3 envelope `{ "error": { "code", "message" } }` on stderr, exit code unchanged. This was promised by the spec but not implemented (new audit finding F15, recorded in `docs/cli/PEAK-AUDIT.md`).
  - Exit codes are untouched: `code_of` still owns them; tests pin 1/2/3/4 through the renderer.
- **TUI palette unified**: `Palette::detect` now reads `output::style()` instead of re-reading `NO_COLOR`/`TERM` itself, so `fg ui` honours `--color` too. Behaviour on the default path is unchanged; non-UTF-8 locales now also get the ASCII fallback in the TUI (the spec's intent).
- **F3 spec fix**: spec-cli's `run` row no longer lists a `[--version N]` flag that clap never defined; it now states the `name@version` target syntax is the version pin.

## 2. Tests added

- `output.rs`: resolution matrix (auto/always/never x NO_COLOR x TERM x TTY x locale), paint wrap/reset bytes, plain-style identity, kv alignment, card in Unicode and ASCII modes.
- `exit.rs`: exit-code passthrough for all four codes through `render_error`, fix-line insertion for PermissionDenied/NotFound io chains, no duplicate fix when one exists, JSON envelope shape and code per spec.

## 3. Adoption boundary (deliberate)

The module is adopted at the two universal choke points (all error output; the TUI palette) in this WP. The `show`/`whoami` cards and the run confirmation card keep their current bytes for now: the run card is safety content that B3 pins with a golden test, and its card treatment lands there; `show`/`whoami` adopt `card`/`kv` with B6's status/whoami work. This is stated plainly rather than implied - "one module for all commands" is the direction, and the remaining adoption is scheduled, not skipped.

## 4. Gates

- `cargo fmt --check`: run locally via rustfmt over the crate - CLEAN (rustfmt needs no target-dir writes).
- `cargo clippy` / `cargo test` / build: NOT RUN locally - the sandbox write fault denies cargo's fingerprint writes (EPERM, reproduced at this WP). CI for this commit's head runs the full gate suite on a healthy runner; per the WP-0b lesson this WP is not treated as done until that run is eyeballed green, and the result is recorded in PROGRESS.md / the B2 report.

## 5. Next

B2 - help, discoverability and completions.
