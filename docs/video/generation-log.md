# Intro video - generation log

**Nothing in this video was AI-generated.** This is a
render log, not a generation log: the account's AI video
generation is blocked (a lasting regional 403 from the
media service), so the video was built deterministically
instead - real CLI captures and real app screenshots,
typeset into HTML scene pages, rendered frame by frame
with Playwright, and encoded with ffmpeg. Every word of
on-screen terminal text is verbatim from a real run of
the real product (sources below); the only staged items
are the demo user's shell prompt, and they are marked.

## Fixtures (all throwaway, nothing real)

- Server: the debug build of forge-server on
  127.0.0.1:18086, FORGEHUB_ENV=dev, a scratch SQLite DB,
  serving the real web build. Registry shown on screen:
  http://localhost:18086 - the product is self-hosted, and
  the server's base URL was set to match so every printed
  line is consistent.
- Accounts: alice (the demo user) and bob. Bob exists
  only because the trust prompts need a snippet owned by
  somebody else; the brief's "alice only" rule is about
  whose story is told.
- Seeded snippets: alice/ubuntu-setup (the apt command
  from scene 1; v1 sha256:5b790bf8), alice's public
  docker-cleanup / docker-image-sizes /
  docker-container-ips / sys-info from demo-data, and
  bob/docker-setup (harmless echo lines; v1
  sha256:9d791112, v2 sha256:5084e5ef after one line
  changed - the change that produces the diff).

## Capture inventory (pipeline/capture.py + ptyrun.sh)

| Capture | How | Used in |
|---|---|---|
| login3.txt | Real `forgehub login` device flow under a PTY; the code (TQH5-W8KS) approved on the real /device page | Scene 3 login block |
| device-approve.png | Real /device page screenshot (Playwright, 1920x1080), cropped to the card | Scene 3 overlay |
| run-own.txt | Real `fg ubuntu-setup`: card, [y/N] answered y, full run, summary "exit code 0 · 1 executed, 0 skipped · 10.2s" | Scene 3 card + scroll; hero loop; poster |
| run-other.txt | Real `fg bob/docker-setup`, prompt answered with the typed word `yes` | Scene 4 panel 1 |
| diff2.txt | Same snippet after bob pushed v2: the CLI printed the CHANGED warning and the unified diff first; answered `no` ("not run (declined)") | Scene 4 panel 2 |
| notty.txt | Same run with stdin closed: the card, then "forgehub: not confirmed" (what/why/fix), exit code 2 | Scene 4 panel 3 |
| record.txt | Real `forgehub record deploy`: two echo commands, `exit`, "Saved `deploy` (2 commands)..." | Scene 5 |
| push.txt | Real `forgehub push deploy` → "alice/deploy v1 sha256:bd02407b" (its second line, the fixture's URL, is not shown) | Scene 5 |
| editor-filled.png | The real editor driven by Playwright: name, description, tags, body typed; Checks panel live ("Valid - ready to save.", "Uses sudo", "Size: 101 / 65,536 bytes") | Scene 2 |
| snippet-saved.png | The snippet page after the real Save ("ubuntu-setup saved" toast, v1, Private) | Scene 2 |
| explore-docker.png | Real Explore with "docker" searched: 4 cards, tag chips, star/fork counts | Scene 5 |
| versions-diff.png | Real Versions tab on bob/docker-setup: v1 + v2 selected, Compare pressed, the real diff view | Scene 5 |

Capture notes:

- The run output in scene 3 is real apt output. Two
  shims kept the capture honest and finite: `apt-get
  update` ran for real but restricted to the Ubuntu
  Azure mirror (the sandbox's second mirror stalled for
  minutes at a time and killed three capture attempts),
  and `upgrade`/`install` ran as apt's own simulation
  (`apt-get -s`) so the build VM was never actually
  upgraded. The confirmation card, the prompt and the
  summary line are the CLI's own, untouched.
- The `record` Saved line is shown with the product's
  default store path (~/.forgehub/snippets/deploy.yaml,
  crates/forge-cli/src/store.rs: "default ~/.forgehub");
  the capture itself ran under a scratch FORGEHUB_HOME.
- Shell prompts (alice@new-server:~$) are the demo
  user's own prompt, staged; everything the programs
  printed is verbatim.
- No token or credential appears anywhere: the login
  capture's token line is never shown (the CLI stores
  the token; the video ends the login beat at
  "Logged in as alice").

## Per-scene render record

All scenes: HTML pages (pipeline/build_scenes.py) whose
state is a pure function of t; render.mjs steps
__setTime(t) at 30 fps and screenshots each frame
(JPEG q92 frames - PNG encoding a photographic 1080p
frame cost ~1.5 s/frame in this environment vs ~0.2 s;
segments are re-encoded immediately, so the intermediate
format is invisible downstream). Segments: libx264,
CRF 18, yuv420p, Rec.709. Assembly (pipeline/build.sh):
hard cuts (the brief allowed cuts or dissolves; cuts
keep every capture frame-exact), silent stereo AAC from
anullsrc (measured -91 dB, digital silence), faststart.

| Scene | Method | Iterations |
|---|---|---|
| 1 Hook | Typeset terminal; typewriter of the verbatim apt command; real Hit lines from run-own.txt; label chips | Accepted as built |
| 2 Answer | Real editor + snippet screenshots in a browser frame; ring highlight on the Checks panel | Rejected: a slow zoom on the screenshots cropped the app's own header text - zoom removed. Ring repositioned once (it had double-counted the frame offset) |
| 3 Run anywhere | Typeset login + card from captures; device card overlay with Approve ring; output scroll of real apt lines ending on the real summary line | Rejected: an early layout bug (preserved whitespace between line divs) doubled every line's spacing - fixed at the CSS source. Scroll distance tuned twice so the summary line lands fully in frame |
| 4 Safety | Three typeset panels from run-other / diff2 / notty verbatim | Accepted as built |
| 5 Share | Real Explore + Versions screenshots; typeset record/push from captures | Accepted as built (zoom removed with scene 2's) |
| 6 Close | Typeset wordmark/buttons; install card with the README's real private-repo flow | Accepted as built |
| Hero | Scene 3's card segment as its own page; 1 s cursor blink divides the 8 s loop exactly | Accepted as built |
| Poster | Scene 3 rendered once at t = 6.8 s (PNG) | Accepted as built |

Assembly fix worth recording: the first assembly came
out yuvj420p (full range) because the source frames are
JPEG; build.sh now converts to limited-range yuv420p
(format filter + color_range tv) for every MP4/WebM.

## Deviations from the original brief

1. No AI generation at all (account block) - the
   deterministic pipeline above replaces the generator,
   as approved by the user before work started.
2. Silent audio with the VO script as captions and in
   docs/video/script.md - the brief's own fallback.
3. The Observe theme (dark glass, current product
   design) instead of the brief's Material You light
   framing - the product's UI changed after the brief
   was written; the video matches the product.
4. Scene 6's install text follows the README as it
   reads today: the repo is private, so the verified
   flow is `gh release download` + SHA256SUMS.
   scripts/install.sh is NOT shown as working (the
   README itself says it needs a public repo).
5. Bob appears as the second snippet owner (see
   Fixtures) - the typed-yes and diff stories are
   impossible with a single account.
6. The demo registry is localhost:18086, a real
   self-hosted setup, instead of a production domain
   (none exists yet; deployment is undecided).

## Final deliverables (web/public/video/)

Verified with ffprobe after assembly: 1920x1080,
30 fps, H.264 (MP4) / VP9 (WebM), yuv420p limited range,
Rec.709 tags, faststart on the MP4s, main duration
60.000 s, hero loop 8.000 s. File sizes are listed in
reports/report_IntroVideo.md.

## Audio pass (2026-11-21)

A fully synthesized sound-effects track was added the
day after the render, by docs/video/pipeline/audio.py
(user request). Method, honestly stated:

- Nothing was recorded, downloaded or AI-generated.
  Every sound is an ffmpeg lavfi source - `sine` and
  `anoisesrc` - shaped by volume envelopes, bandpass /
  lowpass filters and delays, mixed with amix. This
  script only computes cue times (taken from the scene
  timings in build_scenes.py); all DSP is ffmpeg's.
- Sounds: key-tick patter (sine ping + noise snap under
  exponential decays) over the typing windows; soft UI
  clicks (low sine thock + band noise) on the wordmark
  lock, Save and device Approve; two-tone confirm blips
  (E5, B5) on the typed `y` and the push result; an
  ambient pad of six drifting sine partials (A minor)
  over brown-noise room tone, dipping for scene 4 and
  resolving/fading out across the close. Scene 4 gets
  no clicks or blips, only two very soft lowpassed
  ticks on its panel changes. The full cue list with
  timestamps is in script.md.
- One measurement worth logging: this ffmpeg's `sine`
  source peaks at -18.1 dBFS, not full scale (verified
  with volumedetect on a raw source). audio.py carries
  a documented SINE_GAIN compensation so the design
  levels are the heard levels.
- Mastering: static gain plus a sample-peak limiter
  safety, with the true peak *measured* after the pass
  (broadband ticks overshoot between samples, so the
  limiter alone was not trusted). WAV master, measured
  with loudnorm: integrated -16.00 LUFS, true peak
  -1.71 dBTP; ebur128 independently reads -15.9 LUFS /
  -1.7 dBFS peak. Window checks confirm the design:
  scene 4's window (34-38 s) reads -23.1 LUFS against
  scene 2's -15.0.
- Delivery: the audio was replaced in the three intro
  files with the video stream copied bit-for-bit
  (streamhash MD5s identical before and after:
  a9ad7ad1... main, c41d8eb0... captioned,
  9445b87a... webm). AAC at 128k for the MP4s, Opus at
  96k for the WebM; the encoded tracks measure
  -16.04 LUFS / -1.63 dBTP and -16.02 LUFS /
  -1.75 dBTP. Durations: 60.000 s (MP4s); the WebM
  container reports 60.008 s (was 60.018 s before this
  pass - Opus timestamp rounding; its video is exactly
  1800 frames). New sizes: intro-1080p.mp4 9,734,723 B,
  intro-1080p-captioned.mp4 8,484,743 B,
  intro-1080p.webm 4,657,233 B. The hero loops are
  untouched and remain silent by design.
- build.sh now runs audio.py and lays mix.wav under
  the concat, so the one-command rebuild reproduces
  this track. Still no voice-over.
- Rebalance pass (2026-11-21, later; owner feedback
  after listening): two changes, both in audio.py.
  (1) The key-tick patter was halved - a TICK_GAIN of
  0.5 applied to the typing branches only; the UI
  clicks, confirm blips, scene 4 panel ticks and the
  43.40 crossfade tick keep their first-mix levels.
  (2) A background-music bed was added (new build_bgm
  step): a warm sine pad playing Am(add9) - Fmaj7 -
  Cmaj7 - G6 - Am(add9) - Fmaj7 - Cmaj7 - Am(add9),
  one chord per 7.5 s, chords rendered as overlapping
  segments so neighbours crossfade over 1.5 s,
  lowpassed at 1200 Hz; plus a sparse plucked-sine
  melody (A-minor pentatonic, fundamental + faster-
  decaying octave, twelve notes, none across scene
  4). The music fades in 8.5-11.5 s, dips to 50%
  across scene 4 (30-40.5 s), and fades out 58-60 s.
  Still 100% synthesized - no audio files were added.
- Rebalance measurements: WAV master -15.99 LUFS
  integrated, -2.36 dBTP true peak (loudnorm; ebur128
  agrees: -15.9 LUFS, -2.4 dBFS peak) - one mastering
  pass, more peak headroom than the first mix because
  the loudest transients (the ticks) were halved.
  Window checks: 0-8 s -25.2 LUFS (room tone + halved
  ticks), 12-20 s -15.0 and 24-30 s -14.6 (music
  present), 34-38 s -23.9 (scene 4 still the
  calmest), 53-58 s -16.8. Delivery as before: audio
  replaced in the three intro files, video copied
  bit-for-bit (streamhash MD5s unchanged: a9ad7ad1...
  main, c41d8eb0... captioned, 9445b87a... webm);
  encoded tracks measure AAC -16.01 LUFS / -2.51
  dBTP and Opus -16.00 LUFS / -2.33 dBTP; durations
  60.000 s (MP4s), WebM container 60.008 s as before.
  New sizes: intro-1080p.mp4 9,736,063 B,
  intro-1080p-captioned.mp4 8,486,083 B,
  intro-1080p.webm 4,733,710 B. Hero loops untouched
  and still silent.
