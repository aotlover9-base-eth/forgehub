# Report - Peak A8: visual regression suite

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **Commits:** `2952c34` (`peak A8: visual regression suite (goldens via CI - sandbox write fault)`) and this commit (goldens, this report, removal of the temporary workflow)
- **Status:** Done - suite written, 72 goldens generated and verified, check run green. Generation ran on a GitHub runner because the sandbox write fault makes a local server-backed run impossible right now; the deviation and the evidence are recorded in sections 2 and 4.

## 1. Summary

The visual regression suite the plan asks for exists and is green: 12 key screens x 2 themes x 3 width classes = **72 committed goldens** under `web/e2e/visual/__screenshots__/`, produced by `web/e2e/visual/visual.spec.ts` and runnable through `scripts/visual-web.sh` (check) and `scripts/visual-web.sh --update` (regenerate). The suite is env-gated (`VISUAL_TESTS=1`) so the default e2e path and CI stay fast, exactly as the plan allows.

## 2. How the goldens were generated (and why not locally)

The suite itself was written in the previous session and sat uncommitted in the working tree (see the A8 blocker note in PROGRESS.md): the sandbox environment denies positioned writes (`pwrite64` EPERM, in long waves since 2026-11-21 ~22:22 IST), so sqlite cannot write, the dev server cannot start, and no server-backed suite can run locally. That fault also denied local git object writes for over an hour, which is why this WP lands as two commits instead of one - the same reason WP-0b's commit was created through the GitHub Git API.

Rather than leave A8 blocked indefinitely, the suite files were committed (`2952c34`, via the Git API, byte-identical to the working tree) together with a temporary, manually-dispatched workflow (`.github/workflows/visual-goldens.yml`). Run `44964200459` of that workflow, on the healthy `ubuntu-latest` runner:

1. built the server and the SPA,
2. ran `bash scripts/visual-web.sh --update --skip-build` - **goldens generated**,
3. ran `bash scripts/visual-web.sh --skip-build` - **the check run passed against the fresh goldens**,
4. uploaded the 72 PNGs as the `visual-goldens` artifact (3,309,121 bytes zipped).

This commit harvests that artifact into the repo and deletes the temporary workflow, so the tree ends exactly as the plan intends: suite + goldens + docs, no leftover scaffolding. The regular CI run for the suite commit (`44964196985`) was fully green, so the suite's presence changes nothing for the default gates (the spec skips itself without `VISUAL_TESTS=1`).

## 3. The suite (as designed in the previous session, unchanged)

- **Determinism:** fixed seed (one user per theme, three snippets with fixed bodies/tags); screens with relative times run under a browser clock frozen in the past so every relative time clamps to "just now"; the profile "joined" line (data-driven month/year) is masked; animations disabled and reduced motion emulated so A6 entrances sit at their end state.
- **Coverage:** home, library, explore, snippet, profile (signed in, frozen clock); editor-new, editor-edit, settings, device, not-found (signed in, live clock); landing, login (signed out) - each at 360 / 768 / 1280 px in light and dark.
- **Goldens location:** `web/e2e/visual/__screenshots__/` via `snapshotPathTemplate` in `web/playwright.config.ts`. Update procedure is documented in `web/e2e/visual/README.md`.

## 4. Verification of the goldens

- **Set completeness:** the artifact holds exactly the 72 expected filenames (12 screens x 2 themes x 3 widths), no extras, none missing.
- **Dimensions:** every PNG's IHDR matches its width class (360/768/1280) at 900 px height - all 72 verified programmatically.
- **Sizes:** 3,573,592 bytes total (avg ~49.6 KB); smallest `not-found-light-360.png` 22,041 bytes, largest `home-dark-1280.png` 85,792 bytes - plausible per-screen weights, no zero-byte or stub files.
- **Uniqueness:** 72 distinct SHA-256 hashes - no duplicated or blank captures.
- **Theme integrity (in lieu of part of the eyeball):** mean luminance of every dark-theme golden is 0.08-0.16 and of every light-theme golden 0.91-0.97, uniformly across all 36 screen/width pairs. An error page, a blank render or a wrong-theme capture would break that pattern; none does.
- **Check run:** green in the generating workflow (section 2, step 3).
- **Local eyeball / local check run: not run.** The same write fault blocks writing the PNGs into the local tree (one partial file in 40 verified-write attempts) and blocks the local server entirely. Contact sheets generated from the artifact are being retried against the fault; the structural and statistical checks above stand as the verification of record, together with the runner's own check run. This is stated plainly rather than rounded up to an "eyeballed" claim.

## 5. Acceptance criteria (plan A8)

- [x] Playwright screenshot suite for 12 key screens x 2 themes x 3 widths under `web/e2e/visual`.
- [x] Goldens stored under `web/e2e/visual/__screenshots__`, small (avg ~50 KB).
- [x] Update procedure documented (`web/e2e/visual/README.md`).
- [x] Suite gated behind an env var (`VISUAL_TESTS=1`).
- [x] No fake pass: the local-run gap is documented above, with the runner-based evidence named.

## 6. Decisions made

- **Two commits for one WP, and a temporary workflow, both caused by the environment fault** and both fully documented here and in PROGRESS.md. The alternative - leaving Phase A open indefinitely on an environment problem outside the repo - was worse. The temporary workflow is deleted in this commit; nothing about the suite depends on it (regeneration works locally via `scripts/visual-web.sh --update` wherever a server can run).
- **Goldens are runner-generated.** Their reference environment is `ubuntu-latest` with the pinned Playwright chromium. Check runs should be done in that environment (CI or the documented local script on a healthy machine); pixel-level font-antialiasing differences on other hosts can legitimately require a local `--update`, which the README describes.

## 7. Security notes

None. The temporary workflow ran with `contents: read`, was dispatch-only, and is removed in this commit.

## 8. Corrections to earlier steps

- The A8 blocker note in PROGRESS.md said CI verification of A5-A7 "was still to be eyeballed"; doing so uncovered the red lint gate fixed as WP-0b (`reports/report_Peak-0b.md`) before A8 was finished, per the resume order.

## 9. Known issues / limitations

- Local regeneration/check of the visual suite on this VM awaits the sandbox write fault clearing; the suite itself is proven on the runner.
- Golden drift policy: intentional UI changes update goldens via `--update` in the same WP that changes the UI (README documents this).

## 10. Next step

Phase A summary (`reports/report_Peak-A-summary.md`), tag `peak-phase-a`, then Phase B (B0).
