# Report - Intro video: deterministic build, storyboard and landing section

- **Date/time:** 2026-11-21T16:37:07+05:30
- **Commit:** (this commit) `feat(web): intro video, storyboard and landing section`
- **Status:** ✅ Done

## 1. Summary

ForgeHub has a 60-second introduction video on its landing page, built
deterministically because AI video generation is blocked on this account
(a lasting regional 403; the generator was never called). Every scene is
assembled from real captures - the actual CLI run against a throwaway dev
server, and Playwright screenshots of the real web app - typeset into
HTML scene pages, rendered frame-by-frame at 30 fps, and encoded with
ffmpeg. The landing page gained an "Intro" section in the current
Observe style with the full video (poster, controls, default captions),
an 8-second hero loop that autoplays muted only when reduced motion is
not requested, a skip link, and the full transcript. The server CSP now
names `media-src 'self'` explicitly, with a test asserting it.

## 2. What I built

- **Video deliverables** in `web/public/video/`: `intro-1080p.mp4`
  (H.264, silent AAC, faststart), `intro-1080p.webm` (VP9/Opus),
  `intro-1080p-captioned.mp4` (VO captions burned in with libass),
  `hero-loop.mp4` / `hero-loop.webm` (8 s, seamless), `poster.png`
  (scene 3 at the card hold), `captions.vtt`.
- **Docs** in `docs/video/`: `storyboard.md`, `script.md` (the full VO
  script - the video is silent, so the script runs as captions),
  `captions.vtt`, `captions.srt`, `generation-log.md` (a render log:
  capture inventory, per-scene method, every deviation),
  `contact-sheet.png` (1 fps, 10x6), and `pipeline/` (`capture.py`,
  `ptyrun.sh`, `build_scenes.py`, `render.mjs`, `build.sh`, `README.md`
  - one command rebuilds the finals from the scene segments).
- **Captures** (working files, not committed): real device-code login,
  the `fg ubuntu-setup` confirmation card and its full run ending in
  the real summary line, a typed-`yes` run of bob's snippet, the
  changed-snippet diff run (answered no), the no-TTY refusal (exit 2),
  `record`/`push`, and web screenshots (editor with the live Checks
  panel, saved snippet page, Explore searched for "docker", the
  Versions compare diff, the /device approval card).
- **Landing integration**: `web/src/pages/LandingPage.tsx` (Intro
  section + `usePrefersReducedMotion` hook), styles in
  `web/src/design/observe.css` (+ one token,
  `--fh-observe-section-size`, in `tokens.css`), and
  `web/src/pages/landing-video.test.tsx` (4 tests: poster/controls/
  captions-track/no-autoplay on the main video; muted autoplay loop;
  poster still under reduced motion; skip link + full transcript).
- **Server**: `crates/forge-server/src/http.rs` CSP gains
  `media-src 'self'` (nothing else changed); `crates/forge-server/
  tests/http.rs` asserts the directive is present.

## 3. Decisions made

- **Deterministic render instead of a generator** - the generator is
  blocked on this account; the user approved the fallback. Method:
  HTML scene pages whose state is a pure function of time, stepped by
  Playwright. All terminal text is verbatim from captures (the text
  lives in `build_scenes.py` as constants annotated with their source
  capture), so no frame can contain garbled or invented output.
- **Observe theme, not the brief's Material You light framing** - the
  product's UI was re-themed after the brief was written; the video
  matches the product as it exists today. Logged as a deviation.
- **Silent audio** per the brief's own fallback; the VO script is the
  caption track (VTT/SRT + burned-in variant) and the landing
  transcript.
- **Hard cuts** between scenes (the brief allowed cuts or dissolves);
  cuts keep every capture frame-exact.
- **Scene 6 install text follows today's README**: the repo is
  private, so the card shows `gh release download v0.1.6` +
  `sha256sum -c SHA256SUMS`. `scripts/install.sh` is not shown as
  working (the README itself says it needs a public repo).
- **A second demo account (bob)** exists only as the other snippet
  owner; the typed-yes and changed-diff stories are impossible with
  one account. Deviation logged.
- **Capture shims, disclosed in the generation log**: `apt-get update`
  ran for real but restricted to the Ubuntu Azure mirror (the
  sandbox's second mirror stalled for minutes and killed three
  attempts); `upgrade`/`install` ran as apt's own `-s` simulation so
  the build VM was never modified. The card, prompt and summary line
  are the CLI's own output, untouched.
- **JPEG frames for rendering** (q92): PNG encoding a photographic
  1080p frame cost ~1.5 s/frame in software rendering here vs ~0.2 s;
  segments are re-encoded to H.264/VP9 immediately, so the
  intermediate is invisible. The poster is a PNG.
- **Two existing App tests re-scoped, not weakened**: they located the
  landing headline with a bare text query that now also matches the
  transcript's closing line (the tagline, quoted verbatim). They now
  query the level-1 heading by role - the same assertion, precisely
  targeted.

## 4. Verification

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 errors) |
| Tests | `cargo test --workspace` | 430 passed, 0 failed (2 ignored) |
| Web lint | `npm run lint` (incl. `scripts/check-tokens.mjs`) | pass, tokens clean (1 pre-existing warning in ExplorePage) |
| Web types | `npx tsc --noEmit` | pass |
| Web tests | `npm test` | 54 passed (50 existing + 4 new), 0 failed |
| Web build | `npm run build` | pass; `dist/video/` holds all 7 media files |
| E2E | `bash scripts/e2e-web.sh` | PASS (signup, create, edit, diff, token, device) |
| Video spec | `ffprobe` on every deliverable | see below |
| Manual | Live server + browser checks | see below |

ffprobe (final files): all 1920x1080, 30 fps, yuv420p limited range,
Rec.709 tags. `intro-1080p.mp4` H.264 + silent AAC, 60.000 s,
faststart (moov before mdat), audio measured at -91 dB (digital
silence). `intro-1080p.webm` VP9 + Opus, 60.018 s.
`intro-1080p-captioned.mp4` 60.000 s. Hero loops 8.000 s, no audio
track. Range requests against the real server return 206; the CSP
header served live contains `media-src 'self'`; in a real browser
the main video sits paused on its poster with controls, and the
hero loop plays muted (readyState 4). Frames were inspected
individually (14 check frames during the build) and as the 1 fps
contact sheet after assembly; the burned-in caption variant was
frame-checked at 34 s.

File sizes (bytes): intro-1080p.mp4 8,796,521 (cap ~25 MB);
intro-1080p.webm 4,130,653; intro-1080p-captioned.mp4 7,546,541;
hero-loop.mp4 531,191; hero-loop.webm 355,525; poster.png 956,463;
captions.vtt 1,087; captions.srt 1,187; contact-sheet.png 1,515,302.

## 5. Acceptance criteria

- [x] 56-60 s, 6 scenes x 10 s, 1920x1080, 30 fps, H.264 MP4
      (yuv420p, CRF 18, faststart) + WebM VP9, Rec.709
- [x] 8 s muted seamless hero loop (MP4 + WebM), poster.png,
      captions .vtt + .srt, one captioned MP4 variant
- [x] Silent audio; full VO script in docs/video/script.md
- [x] Every scene built from real CLI captures / real screenshots;
      the confirmation card is held readable for 3 s in scene 3
- [x] Storyboard, generation (render) log, pipeline with one-command
      rebuild, contact sheet
- [x] Landing Intro section: poster, play affordance via controls,
      captions track default, preload="metadata", playsInline, no
      autoplay with sound; hero loop autoplays muted only without
      prefers-reduced-motion, poster otherwise; transcript; skip
      link; 16:9 box reserved; tokens only (checker clean)
- [x] CSP: explicit `media-src 'self'` added, nothing weakened,
      server test asserts it
- [x] All repo gates green; exactly one commit; no raw captures or
      rejected renders committed

## 6. Security notes

No credentials appear in any frame: the login's token line is never
shown, the demo passwords exist only in the throwaway fixture DB
(deleted with the scratch server), and the device codes shown are
expired single-use codes for a local fixture. The CSP change adds an
explicit directive equal to the existing default - it widens nothing.
The video files are static assets served like every other public
file. The apt capture shims ran only inside this sandbox (real
update from the Ubuntu archive; simulations for upgrade/install).

## 7. Dependencies added

None. The pipeline uses the repo's existing Playwright install,
Python 3, and the system ffmpeg 8.1.2.
