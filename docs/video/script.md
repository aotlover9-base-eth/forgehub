# ForgeHub intro video - script

The video's voice-over was recorded on 2026-11-21
(the first two audio passes shipped without one - see
generation-log.md). One voice throughout: **Aria**
(`avocado_v2:MAI_01` - warm, friendly, neutral
English), synthesized with the tts CLI at speed 100
from the six scene lines below, word for word - the
same text as the captions. Each line was measured,
trimmed of its head/tail silence, level-matched to
-19 LUFS, and placed at its scene's first caption
start; everything else in the mix ducks about 9 dB
under the speech. Spoken spans (raw render length in
brackets): scene 1: 0.5-8.8 [8.59 s]; scene 2:
10.4-18.0 [7.99 s]; scene 3: 20.4-28.6 [8.52 s];
scene 4: 30.3-38.9 [8.93 s]; scene 5: 40.4-47.9
[7.87 s]; scene 6: 50.5-56.6 [6.05 s]. No line
needed speeding up - all six fit their scenes at a
natural pace. To re-render a line, feed the scene's
joined VO text on stdin:

    /opt/hatch/bin/tts speak --voice avocado_v2:MAI_01 \
      --language en --speed 100 \
      --output ~/workspace/.video-work/vo/line<N>.mp3 \
      --text-stdin

Under the voice sits a fully synthesized
sound-effects track - see Sound design at the end of
this file. The script below is the voice-over script
as approved in the brief: it runs as the burned-in
captions on intro-1080p-captioned.mp4 and as the caption
track (captions.vtt / captions.srt, and the landing
page's <track>) on the main file. The shorter lines are
the on-screen lower-thirds.

Total runtime 60 s. Reading pace is a calm ~2.2 words per
second; every cue sits inside its scene.

## Scene 1 - Hook (0-10 s)

- VO 0.5-5.0: "Every new machine starts the same way.
  You type the same long command, again."
- VO 5.0-9.8: "It lives in a note, a chat, a gist, or
  your shell history."
- Lower-thirds: "You've typed this before." /
  "Scattered across notes, chats and shell history."

## Scene 2 - Answer (10-20 s)

- VO 10.4-15.0: "ForgeHub is a home for your shell
  commands. Paste yours on the web, name it, save it."
- VO 15.0-19.8: "Text only, versioned, and private by
  default."
- Lower-third: "Text only. Versioned. Private by
  default."

## Scene 3 - Run anywhere (20-30 s)

- VO 20.4-25.0: "Log in once from the terminal. Then
  just type fg and the name."
- VO 25.0-29.8: "ForgeHub shows exactly what it will
  run, and asks you first."
- Lower-thirds: "fg ubuntu-setup" / "You see exactly
  what will run. Then you decide."

## Scene 4 - Safety (30-40 s)

- VO 30.3-33.3: "Safety is the product. Other people's
  snippets need a typed yes."
- VO 33.6-36.6: "If a snippet changed, you see the diff
  first."
- VO 36.9-39.8: "With no terminal, it refuses to run."
- Lower-thirds: the three sentences after "Safety is
  the product.", one per panel.

## Scene 5 - Share / discover (40-50 s)

- VO 40.4-45.0: "Keep snippets private, share a link,
  or publish them."
- VO 45.0-49.8: "Search, star and fork. Record in your
  terminal and push like git."
- Lower-thirds: "Private, unlisted or public." /
  "Record. Push. Pull. Just like git."

## Scene 6 - Close (50-60 s)

- VO 50.5-56.0: "ForgeHub. Save a command once. Run it
  anywhere."
- VO 56.0-59.5: "Save a command once. Run it anywhere
  with fg."
- On screen: wordmark, tagline, "Create your account" /
  "Install the CLI", and the install card:
  `gh release download v0.1.6`, `sha256sum -c SHA256SUMS`.

## Sound design (synthesized, added 2026-11-21)

Every sound under the voice is synthesized with
ffmpeg from sine and noise sources by
docs/video/pipeline/audio.py - no external audio
files, no recordings. (The voice-over itself is the
one recorded element; see the top of this file.)
Master measurements at the rebalance (loudnorm on
the WAV master; ebur128 agrees at -15.9 LUFS):
integrated -16.0 LUFS, true peak -2.4 dBTP. The
encoded files measure -16.0 LUFS / -2.5 dBTP (AAC)
and -16.0 LUFS / -2.3 dBTP (Opus). Voice-over pass
(later the same day): master -16.02 LUFS / -1.14
dBTP (ebur128: -15.9 LUFS, -1.1 dBFS peak); encoded
AAC -16.05 LUFS / -1.20 dBTP; Opus -16.72 LUFS /
-1.64 dBTP (the Opus encode takes a -0.7 dB trim -
it overshoots true peak further than AAC from the
same master). All times are absolute seconds.

Rebalanced later the same day at the owner's ear: the
key ticks were halved (they read as too loud) and a
real background-music bed was added under the
atmosphere - see the Background music bullet below.
The UI clicks and confirm blips were not changed.

- **Room tone** (0-60): brown noise through a 320 Hz
  lowpass, very quiet, everywhere - the only sound
  under scene 1 besides the typing.
- **Ambient pad** (10-60): six sine partials (A minor:
  A1, A2, E3, A3, C4, E4), each with a slow amplitude
  drift, lowpassed at 1400 Hz. Fades in 10.0-13.0,
  dips to 45% across scene 4 (30.0-40.5), the two
  highest partials resolve away 52-58, and the whole
  bed fades out 58-60 with the closing fade.
- **Key ticks**: short sine-ping + noise-snap
  transients, sparse and jittered (never one per
  character), over the typing only - scene 1:
  0.75-4.25 (the apt command); scene 3: 20.35-21.05
  (`forgehub login`), 24.40-24.95 (`fg ubuntu-setup`);
  scene 5: 46.35-48.87 (the record/push commands).
  Gain halved in the rebalance pass (x0.5) - the
  owner found the first mix's clicks too loud.
- **Background music** (added in the rebalance): a
  warm sine pad playing Am(add9) - Fmaj7 - Cmaj7 -
  G6 - Am(add9) - Fmaj7 - Cmaj7 - Am(add9), one
  chord every 7.5 s with 1.5 s crossfades, lowpassed
  at 1200 Hz, plus a sparse plucked-sine melody
  (A-minor pentatonic: E5, C5, G5, D5, A4 - twelve
  notes in the whole minute, each placed on a chord
  tone). Fades in 8.5-11.5, dips to 50% across
  scene 4 where the melody rests entirely, and
  fades out 58-60 resolving on Am(add9). Present
  when you listen for it; it never competes with
  the effects.
- **Soft UI clicks**: 11.05 wordmark lock (quieter),
  16.50 Save, 23.25 device Approve.
- **Confirm blips** (two sines, E5 then B5): 28.30
  the typed `y` in scene 3; 49.18 the push result
  line in scene 5.
- **Scene 4** is deliberately near-empty: no clicks,
  no blips - only very soft lowpassed ticks on the
  panel changes (33.35, 36.65) over the dipped bed.
- **Scene 5**: one soft tick at 43.40 for the Explore
  to Versions crossfade.
