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

## Addendum - Audio pass (2026-11-21)

Added at the user's request, the same day: a fully
synthesized sound-effects track for the intro video.
No external audio files, no downloads, no recordings,
no AI generation - every sound is synthesized with
ffmpeg from sine and noise sources by the new pipeline
step `docs/video/pipeline/audio.py` (build.sh runs it,
so the one-command rebuild reproduces the track).

- **Method:** audio.py computes cue times from the
  scene timings in build_scenes.py and drives ffmpeg;
  all DSP (envelopes, filters, delays, mixing) is
  ffmpeg's. Mastering is a static gain plus a
  sample-peak limiter safety, with the true peak
  measured after the pass and the gain backed off if
  the measurement exceeds -1 dBTP.
- **Cues (absolute seconds):** brown-noise room tone
  throughout; a six-partial sine pad (A minor) fading
  in 10-13 s, dipping to 45% across scene 4, its top
  partials resolving 52-58 s, bed out 58-60 s; sparse
  jittered key-tick patter over the typing (scene 1:
  0.75-4.25; scene 3: 20.35-21.05 and 24.40-24.95;
  scene 5: 46.35-48.87); soft UI clicks at 11.05
  (wordmark lock), 16.50 (Save), 23.25 (Approve);
  two-tone confirm blips (E5-B5) at 28.30 (typed `y`)
  and 49.18 (push result); scene 4 kept serious - no
  clicks or blips, only soft lowpassed ticks on the
  panel changes (33.35, 36.65) over the dipped bed;
  one soft tick at 43.40 for scene 5's crossfade.
  Full cue list in docs/video/script.md.
- **Measured loudness / peak:** WAV master -16.00 LUFS
  integrated, -1.71 dBTP true peak (loudnorm; ebur128
  agrees: -15.9 LUFS, -1.7 dBFS peak). Encoded tracks:
  AAC -16.04 LUFS / -1.63 dBTP; Opus -16.02 LUFS /
  -1.75 dBTP.
- **Files changed:** web/public/video/intro-1080p.mp4
  (9,734,723 B), intro-1080p-captioned.mp4
  (8,484,743 B), intro-1080p.webm (4,657,233 B) - in
  each, the video stream was copied, not re-encoded
  (streamhash MD5 verified identical), durations stay
  60.000 s (the WebM container reports 60.008 s, it
  was 60.018 s before). Hero loops untouched and
  still silent. No application code changed.
- **No voice-over in this pass** - captions,
  script.md and the landing transcript carried the
  words. (Superseded later the same day - see the
  voice-over entry at the end of this addendum.)
- **Rebalance (same day, owner feedback):** the
  typing clicks were halved (a `TICK_GAIN` of 0.5 on
  the patter only - clicks, blips and scene 4's panel
  ticks unchanged) and a subtle background-music bed
  was added: a warm sine pad playing Am(add9) -
  Fmaj7 - Cmaj7 - G6 - Am(add9) - Fmaj7 - Cmaj7 -
  Am(add9) at 7.5 s per chord with 1.5 s crossfades,
  plus a sparse plucked-sine pentatonic melody that
  rests across scene 4; the music fades in
  8.5-11.5 s, dips to 50% across scene 4 and resolves
  out 58-60 s. New measurements: master -15.99 LUFS /
  -2.36 dBTP (ebur128 -15.9 LUFS); AAC -16.01 /
  -2.51 dBTP; Opus -16.00 / -2.33 dBTP. Video streams
  again copied bit-for-bit (streamhash MD5s
  unchanged), durations still 60.000 s; new sizes
  9,736,063 B / 8,486,083 B / 4,733,710 B. Hero loops
  untouched.
- **Voice-over (same day, later; owner request):**
  the six script lines are now actually spoken.
  Voice: **Aria** (`avocado_v2:MAI_01` - warm,
  friendly, neutral English), one voice throughout,
  synthesized verbatim from script.md at natural
  speed (tts speed 100; no atempo - every line fit
  its scene as rendered). Raw -> trimmed durations:
  8.59 -> 8.31 s, 7.99 -> 7.63 s, 8.52 -> 8.17 s,
  8.93 -> 8.56 s, 7.87 -> 7.52 s, 6.05 -> 6.05 s;
  placed at the caption starts 0.5 / 10.4 / 20.4 /
  30.3 / 40.4 / 50.5, ending at 8.81 / 18.03 /
  28.57 / 38.86 / 47.92 / 56.55 - inside every
  scene, in sync with the captions. audio.py's new
  build_vo step trims, level-matches the lines to
  -19 LUFS each, and refuses an overrunning line;
  the rest of the mix (bed, music, ticks, clicks,
  blips) ducks about 9 dB under each spoken window
  with 0.3 s ramps. Mastering: the limiter ceiling
  moved from exactly -1 dB to 0.87 (about -1.2 dB)
  after the -1 dB ceiling measured back as
  -0.99 dBTP, and the pass now hard-fails if the
  true peak exceeds -1 dBTP. Measured: master
  -16.02 LUFS / -1.14 dBTP (ebur128 -15.9 LUFS);
  AAC -16.05 / -1.20 dBTP; Opus -16.72 / -1.64 dBTP
  (Opus took a -0.7 dB encode trim for true peak).
  Video streams copied bit-for-bit (streamhash MD5s
  unchanged), durations 60.000 s; new sizes
  9,736,711 B / 8,486,731 B / 4,729,733 B. The
  intro-video release assets were updated and
  size-verified. Hero loops untouched, still silent.
