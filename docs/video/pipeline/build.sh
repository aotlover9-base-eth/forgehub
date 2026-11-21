#!/usr/bin/env bash
# Assemble the final intro-video deliverables from the
# per-scene segments produced by render.mjs.
#
#   docs/video/pipeline/build.sh
#
# Inputs (in $VIDEO_WORK, default ~/workspace/.video-work):
#   segments/scene1..6.mp4, segments/hero.mp4, poster-src.png
# Outputs:
#   web/public/video/{intro-1080p.mp4, intro-1080p.webm,
#     intro-1080p-captioned.mp4, hero-loop.mp4,
#     hero-loop.webm, poster.png, captions.vtt}
#   docs/video/contact-sheet.png
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../../.." && pwd)"
WORK="${VIDEO_WORK:-$HOME/workspace/.video-work}"
SEG="$WORK/segments"
OUT="$WORK/final"
PUB="$REPO/web/public/video"
mkdir -p "$OUT" "$PUB"

for n in 1 2 3 4 5 6; do
  test -f "$SEG/scene$n.mp4" || { echo "missing segment scene$n.mp4 - run render.mjs first" >&2; exit 1; }
done
test -f "$SEG/hero.mp4" || { echo "missing hero.mp4 - run render.mjs first" >&2; exit 1; }

# The synthesized sound-design track (audio.py - every
# sound is synthesized with ffmpeg from sines and noise;
# see docs/video/script.md for the cue list). Builds
# $WORK/audio/mix.wav, loudness-adjusted to about
# -16 LUFS with true peak at or below -1 dBTP.
python3 "$SCRIPT_DIR/audio.py" --work "$WORK"

# Concat the six scenes with hard cuts, lay the sound
# design under them, faststart for the web.
LIST="$WORK/concat.txt"
: > "$LIST"
for n in 1 2 3 4 5 6; do echo "file '$SEG/scene$n.mp4'" >> "$LIST"; done
ffmpeg -y -loglevel error -f concat -safe 0 -i "$LIST" \
  -i "$WORK/audio/mix.wav" \
  -map 0:v:0 -map 1:a:0 \
  -vf format=yuv420p \
  -c:v libx264 -pix_fmt yuv420p -crf 18 -preset slow -r 30 \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv \
  -c:a aac -b:a 128k -movflags +faststart \
  "$OUT/intro-1080p.mp4"

# WebM (VP9) variant.
ffmpeg -y -loglevel error -i "$OUT/intro-1080p.mp4" \
  -c:v libvpx-vp9 -b:v 0 -crf 32 -pix_fmt yuv420p \
  -c:a libopus -b:a 96k "$OUT/intro-1080p.webm"

# Captioned variant: the VO captions burned in with libass,
# so they are visible in any player.
ffmpeg -y -loglevel error -i "$OUT/intro-1080p.mp4" \
  -vf "subtitles=$REPO/docs/video/captions.vtt:force_style='FontSize=20,PrimaryColour=&HFFFFFF&,OutlineColour=&H80000000&,BorderStyle=1,Outline=2,Shadow=0,Alignment=2,MarginV=36',format=yuv420p" \
  -c:v libx264 -pix_fmt yuv420p -crf 20 -preset medium \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv \
  -c:a copy -movflags +faststart \
  "$OUT/intro-1080p-captioned.mp4"

# Hero loop (8 s, silent, no audio track - it autoplays
# muted). Re-encoded (not copied) for the same range fix.
ffmpeg -y -loglevel error -i "$SEG/hero.mp4" \
  -vf format=yuv420p \
  -c:v libx264 -pix_fmt yuv420p -crf 18 -preset medium \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv \
  -an -movflags +faststart "$OUT/hero-loop.mp4"
ffmpeg -y -loglevel error -i "$SEG/hero.mp4" \
  -vf format=yuv420p \
  -c:v libvpx-vp9 -b:v 0 -crf 34 -pix_fmt yuv420p -an \
  "$OUT/hero-loop.webm"

# Poster + the files the landing page serves.
cp "$WORK/poster-src.png" "$OUT/poster.png"
cp "$OUT/intro-1080p.mp4" "$OUT/intro-1080p.webm" \
   "$OUT/intro-1080p-captioned.mp4" "$OUT/hero-loop.mp4" \
   "$OUT/hero-loop.webm" "$OUT/poster.png" "$PUB/"
cp "$REPO/docs/video/captions.vtt" "$PUB/captions.vtt"

# Contact sheet: one frame per second, 10x6 tiles.
ffmpeg -y -loglevel error -i "$OUT/intro-1080p.mp4" \
  -vf "fps=1,scale=320:180,tile=10x6" -frames:v 1 \
  "$REPO/docs/video/contact-sheet.png"

echo "built:"
ls -la "$PUB"
