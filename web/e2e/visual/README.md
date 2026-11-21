# Visual regression suite (Peak A8)

Screenshot goldens for 12 key screens x 2 themes (light, dark) x 3
width classes (360, 768, 1280 px) = 72 images, stored in
`__screenshots__/` next to this file.

## Run it

```sh
scripts/visual-web.sh              # build, boot a throwaway server, compare
scripts/visual-web.sh --skip-build # reuse the current build
```

The suite is gated behind `VISUAL_TESTS=1` (the script sets it), so
the default `scripts/e2e-web.sh` run does not pay its cost.

## Update the goldens

```sh
scripts/visual-web.sh --update --skip-build
```

Then eyeball every changed image before committing - an update is a
design decision, not a formality. Commit the new PNGs with the change
that caused them.

## How determinism is achieved

- Fixed seed: one user per theme with three snippets (fixed names,
  descriptions, bodies, tags); later contexts log in instead of
  signing up again, so every width sees identical data.
- Screens that render relative times run with the browser clock
  frozen at 2020-01-01, so every relative time clamps to "just now"
  (see `relativeTime` in `src/validation.ts`). Editor screens run on
  a live clock so the validation debounce can settle.
- The profile "joined" line (month/year of the seed's creation, i.e.
  data-driven) is masked in the screenshot.
- CSS animations are disabled for captures and reduced motion is
  emulated, so the Peak A6 entrances sit at their end state.
- Screenshots are viewport captures at deviceScaleFactor 1.
