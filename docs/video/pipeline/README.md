# Intro video pipeline

The intro video is rendered, not generated: every scene
is an HTML page whose visual state is a pure function of
time, rendered frame by frame with Playwright and encoded
with ffmpeg. All terminal text is verbatim from real CLI
captures and all app views are real screenshots - see
../generation-log.md for the full inventory.

## Pieces

- `capture.py` - PTY driver used to capture the real CLI
  sessions (device login, run cards, typed-yes prompt,
  changed-snippet diff, no-TTY refusal, record/push).
  `ptyrun.sh` is the script(1)-based variant used for the
  captures that need a human-paced answer file.
- `build_scenes.py` - writes the six scene pages and the
  hero-loop page (1920x1080) into $VIDEO_WORK/scenes.
  The on-screen terminal text lives here as constants,
  each annotated with the capture it came from.
- `render.mjs` - steps `window.__setTime(t)` at 30 fps,
  screenshots every frame, encodes one MP4 segment per
  scene into $VIDEO_WORK/segments, plus the poster source
  frame. Playwright is resolved from web/node_modules.
- `audio.py` - the sound design and the voice-over.
  Every sound under the voice is synthesized with
  ffmpeg from sine and noise sources (no external
  audio files): key-tick patter over the typing
  (halved in the rebalance pass), soft clicks on Save
  and Approve, two-tone confirm blips, a six-partial
  sine pad under brown room tone, and a synthesized
  music bed (a chord-pad progression with a sparse
  plucked-sine melody), all timed from the scene
  timings in build_scenes.py. The voice-over is the
  one recorded element: the six script lines,
  synthesized with the tts CLI (voice Aria) into
  $VIDEO_WORK/vo/line1..6.mp3 - audio.py stops with
  the exact commands if they are missing (they are
  also in ../script.md). It trims and level-matches
  the lines, ducks everything else about 9 dB under
  the speech, and writes $VIDEO_WORK/audio/mix.wav,
  loudness-adjusted to about -16 LUFS with true peak
  at or below -1 dBTP (measured with loudnorm; the
  script prints the real numbers). The cue list
  lives in ../script.md.
- `build.sh` - builds the audio mix via audio.py,
  concatenates the segments (hard cuts), lays the mix
  under them, and writes every final deliverable into
  web/public/video/ plus the contact sheet into
  docs/video/.

$VIDEO_WORK defaults to ~/workspace/.video-work and holds
the captures, screenshots, scene pages and segments.
None of it is committed; the committed sources are this
directory plus the screenshots' *provenance* in the
generation log.

## Rebuild

From a checkout with the capture fixtures in place
(captures + screenshots under $VIDEO_WORK - the fixtures
are reproducible by re-running the captures against a
dev server seeded per generation-log.md):

```sh
python3 docs/video/pipeline/build_scenes.py
node docs/video/pipeline/render.mjs        # ~8 min
docs/video/pipeline/build.sh
```

`build.sh` alone rebuilds all final files from existing
scene segments in under a minute.
