#!/usr/bin/env python3
"""Synthesized sound design for the intro video.

Every sound in the track is synthesized with ffmpeg from
sine and noise sources - no external audio files, no
downloads, no recordings. This script only computes cue
times (from the scene timings in build_scenes.py) and
drives ffmpeg; all DSP happens inside ffmpeg filters.

    python3 docs/video/pipeline/audio.py [--work DIR]

Writes $WORK/audio/mix.wav: a 60.000 s stereo master at
48 kHz, loudness-adjusted to about -16 LUFS integrated
with true peak at or below -1 dBTP (measured with
loudnorm; the script prints the real numbers).

Cue map (absolute seconds; scene starts at 0/10/20/30/
40/50, in-scene moments from build_scenes.py):

- Key-tick patter during typing: scene 1 (0.7-4.3, the
  apt command), scene 3 (20.3-21.1 login, 24.35-25.0
  fg ubuntu-setup), scene 5 (46.35-48.87 record/push
  commands). Sparse and jittered, never one per character.
  Halved in gain (TICK_GAIN) in the 2026-11-21 rebalance
  pass - the owner found the clicks too loud. The soft
  UI clicks and confirm blips were left as they were.
- Soft UI clicks: 11.05 wordmark lock (scene 2, quieter),
  16.50 Save crossfade (scene 2), 23.25 device Approve
  ring (scene 3).
- Two-tone confirm blips: 28.30 the typed "y" (scene 3),
  49.18 the push result line (scene 5).
- Scene 4 stays serious: no blips, only very soft
  lowpassed ticks on the panel changes (33.35, 36.65),
  and the bed dips. One soft tick at 43.40 for the
  scene 5 Explore-to-Versions crossfade.
- Ambient bed: brown-noise room tone throughout; a pad
  of six detuned-drift sines (A minor: A1 A2 E3 A3 C4 E4)
  fading in over 10-13 s, dipping to 45% across scene 4
  (30-40.5 s), its two highest partials resolving away
  over 52-58 s, and the whole bed fading out over the
  last 2 s while scene 6 fades to black.
- Background music (added in the rebalance pass, at the
  owner's request for an actual music bed under the
  atmosphere): a warm sine pad playing Am(add9) -
  Fmaj7 - Cmaj7 - G6 - Am(add9) - Fmaj7 - Cmaj7 -
  Am(add9), one chord every 7.5 s with 1.5 s crossfades
  between chords, plus a sparse plucked-sine melody
  (A-minor pentatonic, long decays, no notes at all
  across scene 4). The music fades in over 8.5-11.5 s,
  dips to 50% across scene 4 (30-40.5 s) with the rest
  of the bed, and fades out over the last 2 s. Still
  100% synthesized - chords and melody are sine
  partials with envelopes, nothing recorded.
"""

import argparse
import json
import random
import re
import subprocess
import sys
from pathlib import Path

SR = 48000
DUR = 60.0

# Measured on this ffmpeg (8.1.2): the sine lavfi source
# peaks at about -18.1 dBFS (amplitude ~0.124), not full
# scale. Gains written for sine sources below are the
# intended absolute amplitudes multiplied by this
# compensation, so the design levels are what is heard.
SINE_GAIN = 8.0

# Typing windows (start, end, step_min, step_max) in
# absolute seconds, from build_scenes.py timings.
TYPING_WINDOWS = [
    (0.75, 4.25, 0.09, 0.17),    # scene 1: the apt command
    (20.35, 21.05, 0.08, 0.13),  # scene 3: forgehub login
    (24.40, 24.95, 0.08, 0.13),  # scene 3: fg ubuntu-setup
    (46.35, 46.62, 0.07, 0.11),  # scene 5: record deploy
    (46.92, 47.17, 0.07, 0.11),  # scene 5: echo building
    (47.47, 47.77, 0.07, 0.11),  # scene 5: echo deploying
    (48.07, 48.27, 0.07, 0.11),  # scene 5: exit
    (48.57, 48.87, 0.07, 0.11),  # scene 5: push deploy
]

# (time, gain) - soft UI clicks.
CLICKS = [
    (11.05, 0.65),  # scene 2: wordmark lock (quieter)
    (16.50, 1.00),  # scene 2: Save
    (23.25, 1.00),  # scene 3: device Approve
]

# Two-tone confirm blips.
BLIPS = [28.30, 49.18]

# (time, gain) - very soft lowpassed ticks, scene 4
# panel changes only. Scene 4 gets no blips.
PANEL_TICKS = [(33.35, 0.32), (36.65, 0.32)]

# (time, gain) - one soft tick for the scene 5
# Explore-to-Versions crossfade.
SOFT_TICKS = [(43.40, 0.50)]

# Pad partials: (frequency, gain, drift Hz, drift phase).
# Gains are pre-normalization design levels; the final
# loudness pass sets the absolute level.
PAD_PARTIALS = [
    (55.00, 0.055, 0.050, 0.0),   # A1
    (110.00, 0.095, 0.070, 1.3),  # A2
    (164.81, 0.060, 0.060, 2.6),  # E3
    (220.00, 0.055, 0.090, 0.7),  # A3
    (261.63, 0.033, 0.080, 1.9),  # C4 (high group)
    (329.63, 0.024, 0.110, 3.4),  # E4 (high group)
]
HIGH_GROUP = {261.63, 329.63}

# Scene envelope for the bed: in over 10-13 s, dip to
# 45% for scene 4 (30-40.5 s), out over 58-60 s.
ENV_MAIN = (
    "if(lt(t,10),0,"
    "if(lt(t,13),(t-10)/3,"
    "if(lt(t,30),1,"
    "if(lt(t,31.5),1-0.3667*(t-30),"
    "if(lt(t,39),0.45,"
    "if(lt(t,40.5),0.45+0.3667*(t-39),"
    "if(lt(t,58),1,(60-t)/2)))))))"
)
# The high partials resolve away over 52-58 s.
ENV_RESOLVE = "if(lt(t,52),1,if(lt(t,58),(58-t)/6,0))"

# Owner feedback (rebalance pass): the typing clicks
# were too loud. The key-tick patter - and only the
# patter; the panel ticks, the crossfade tick, the UI
# clicks and the blips keep their levels - is scaled
# by this factor relative to the first mix.
TICK_GAIN = 0.5

# Background music: one chord every 7.5 s, two
# statements of Am(add9) - Fmaj7 - Cmaj7 - G6, the
# second ending back on Am(add9) so the piece resolves
# at home under the closing fade. Partials are
# (frequency, gain) sine pairs, voiced low and warm;
# adjacent chords crossfade over 1.5 s.
CHORD_DUR = 7.5
CHORD_XFADE = 1.5
CHORDS = [
    ("Am(add9)", [(55.00, 0.050), (110.00, 0.085), (164.81, 0.055),
                  (246.94, 0.036), (261.63, 0.033)]),
    ("Fmaj7", [(87.31, 0.078), (174.61, 0.046), (220.00, 0.050),
               (261.63, 0.037), (329.63, 0.029)]),
    ("Cmaj7", [(65.41, 0.068), (130.81, 0.058), (164.81, 0.050),
               (196.00, 0.044), (246.94, 0.033)]),
    ("G6", [(98.00, 0.076), (146.83, 0.048), (196.00, 0.047),
            (246.94, 0.038), (329.63, 0.030)]),
    ("Am(add9)", [(55.00, 0.050), (110.00, 0.085), (164.81, 0.055),
                  (246.94, 0.036), (261.63, 0.033)]),
    ("Fmaj7", [(87.31, 0.078), (174.61, 0.046), (220.00, 0.050),
               (261.63, 0.037), (329.63, 0.029)]),
    ("Cmaj7", [(65.41, 0.068), (130.81, 0.058), (164.81, 0.050),
               (196.00, 0.044), (246.94, 0.033)]),
    ("Am(add9)", [(55.00, 0.050), (110.00, 0.085), (164.81, 0.055),
                  (246.94, 0.036), (261.63, 0.033)]),
]

# Sparse plucked-sine melody (time, frequency, gain).
# A-minor pentatonic, each note a chord tone or a
# gentle extension of the chord under it. Nothing
# between 30 and 40.5 s: the melody rests across
# scene 4 with the dipped bed.
MELODY = [
    (12.00, 659.25, 0.050),  # E5 - the maj7 of Fmaj7
    (14.20, 523.25, 0.044),  # C5
    (18.50, 659.25, 0.050),  # E5 over Cmaj7
    (20.80, 783.99, 0.040),  # G5
    (24.00, 587.33, 0.046),  # D5 over G6
    (27.50, 659.25, 0.048),  # E5 - the 6 of G6
    (42.00, 523.25, 0.044),  # C5 over Fmaj7
    (47.00, 659.25, 0.048),  # E5 over Cmaj7
    (49.50, 587.33, 0.042),  # D5
    (53.50, 659.25, 0.048),  # E5 over Am(add9)
    (55.50, 523.25, 0.044),  # C5
    (57.20, 440.00, 0.040),  # A4 - home, under the fade
]

# Global envelope for the music: silent until 8.5 s,
# in by 11.5 s, dipped to 50% across scene 4
# (30-40.5 s), out over the last 2 s.
BGM_ENV = (
    "if(lt(t,8.5),0,"
    "if(lt(t,11.5),(t-8.5)/3,"
    "if(lt(t,30),1,"
    "if(lt(t,31.5),1-0.3333*(t-30),"
    "if(lt(t,39),0.5,"
    "if(lt(t,40.5),0.5+0.3333*(t-39),"
    "if(lt(t,58),1,(60-t)/2)))))))"
)


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True)


def ffmpeg_filter(out: Path, inputs: list[str], graph: str,
                  extra: list[str] | None = None) -> None:
    """Run ffmpeg with a filter_complex script file."""
    script = out.with_suffix(".filter.txt")
    script.write_text(graph)
    cmd = ["ffmpeg", "-y", "-loglevel", "error"]
    for i in inputs:
        cmd += ["-f", "lavfi", "-i", i] if i.startswith(
            ("sine=", "anoisesrc=")) else ["-i", i]
    cmd += ["-filter_complex_script", str(script)]
    if extra:
        cmd += extra
    cmd += [str(out)]
    r = run(cmd)
    script.unlink(missing_ok=True)
    if r.returncode != 0:
        sys.exit(f"ffmpeg failed for {out.name}:\n{r.stderr}")


def measure(path: Path) -> tuple[float, float]:
    """Integrated LUFS and true peak (dBTP) via loudnorm."""
    r = run(["ffmpeg", "-i", str(path), "-af",
             "loudnorm=I=-16:TP=-1:LRA=11:print_format=json",
             "-f", "null", "-"])
    m = re.search(r"\{\s*\"input_i\".*?\}", r.stderr, re.S)
    if not m:
        sys.exit(f"loudnorm measurement failed:\n{r.stderr[-800:]}")
    d = json.loads(m.group(0))
    return float(d["input_i"]), float(d["input_tp"])


def build_prototypes(d: Path) -> None:
    # Key tick: a short sine ping plus a noise snap, both
    # under fast exponential decays.
    ffmpeg_filter(
        d / "tick.wav",
        ["sine=frequency=2300:sample_rate=48000:duration=0.03",
         "anoisesrc=color=white:sample_rate=48000:duration=0.03:amplitude=0.9"],
        f"[0:a]volume='{0.16 * SINE_GAIN}*exp(-90*t)':eval=frame[s];"
        "[1:a]highpass=f=5000,volume='0.10*exp(-160*t)':eval=frame[n];"
        "[s][n]amix=inputs=2:normalize=0,"
        "pan=stereo|c0=c0|c1=c0,atrim=0:0.03[out]",
        ["-map", "[out]", "-ar", str(SR)])
    # UI click: a soft low thock - sine body + band noise.
    ffmpeg_filter(
        d / "click.wav",
        ["sine=frequency=950:sample_rate=48000:duration=0.06",
         "anoisesrc=color=white:sample_rate=48000:duration=0.03:amplitude=0.9"],
        f"[0:a]volume='{0.30 * SINE_GAIN}*exp(-55*t)':eval=frame[s];"
        "[1:a]bandpass=f=1100:width_type=h:w=600,"
        "volume='0.12*exp(-90*t)':eval=frame[n];"
        "[s][n]amix=inputs=2:normalize=0,"
        "pan=stereo|c0=c0|c1=c0,atrim=0:0.06[out]",
        ["-map", "[out]", "-ar", str(SR)])
    # Confirm blip: two ascending sine tones (E5 then B5),
    # softly windowed.
    ffmpeg_filter(
        d / "blip.wav",
        ["sine=frequency=659.25:sample_rate=48000:duration=0.10",
         "sine=frequency=987.77:sample_rate=48000:duration=0.16"],
        f"[0:a]volume='{0.16 * SINE_GAIN}*sin(PI*min(t/0.10,1))':eval=frame[a];"
        f"[1:a]volume='{0.15 * SINE_GAIN}*sin(PI*min(t/0.16,1))':eval=frame,adelay=85[b];"
        "[a][b]amix=inputs=2:normalize=0,"
        "pan=stereo|c0=c0|c1=c0,atrim=0:0.26[out]",
        ["-map", "[out]", "-ar", str(SR)])


def build_bed(d: Path) -> None:
    inputs, chains, low_labels, high_labels = [], [], [], []
    for i, (f, g, drift, phase) in enumerate(PAD_PARTIALS):
        inputs.append(f"sine=frequency={f}:sample_rate=48000:duration=60")
        chains.append(
            f"[{i}:a]volume='{g * SINE_GAIN}*(0.82+0.18*sin(2*PI*{drift}*t+{phase}))'"
            f":eval=frame[p{i}]")
        (high_labels if f in HIGH_GROUP else low_labels).append(f"[p{i}]")
    n = len(PAD_PARTIALS)
    inputs.append(
        "anoisesrc=color=brown:sample_rate=48000:duration=60:amplitude=0.6")
    chains.append(
        f"[{n}:a]lowpass=f=320,volume=0.014[room]")
    chains.append(
        f"{''.join(low_labels)}amix=inputs={len(low_labels)}:normalize=0,"
        f"lowpass=f=1400,volume='{ENV_MAIN}':eval=frame[low]")
    chains.append(
        f"{''.join(high_labels)}amix=inputs={len(high_labels)}:normalize=0,"
        f"lowpass=f=1400,volume='{ENV_MAIN}':eval=frame,"
        f"volume='{ENV_RESOLVE}':eval=frame[high]")
    chains.append(
        "[low][high][room]amix=inputs=3:normalize=0,"
        "pan=stereo|c0=c0|c1=c0,atrim=0:60[out]")
    ffmpeg_filter(d / "bed.wav", inputs, ";".join(chains),
                  ["-map", "[out]", "-ar", str(SR)])


def build_bgm(d: Path) -> None:
    """The music bed: chord pad segments + pluck melody.

    Each chord is rendered as its own short file whose
    envelope ramps over the crossfade at both ends, so
    simply delaying the segments onto the timeline and
    summing them crossfades the progression. The first
    segment starts at 0 (its window opens the piece,
    silent under the global envelope until 8.5 s) and
    the last ends exactly at 60 s.
    """
    half = CHORD_XFADE / 2
    n = len(CHORDS)
    segment_files: list[Path] = []
    for k, (_name, partials) in enumerate(CHORDS):
        start = k * CHORD_DUR
        if k == 0:
            seg_dur, env = CHORD_DUR + half, (
                f"if(lt(t,{CHORD_DUR - half}),1,"
                f"({CHORD_DUR + half}-t)/{CHORD_XFADE})")
        elif k == n - 1:
            seg_dur, env = CHORD_DUR + half, (
                f"if(lt(t,{CHORD_XFADE}),t/{CHORD_XFADE},1)")
        else:
            seg_dur, env = CHORD_DUR + CHORD_XFADE, (
                f"if(lt(t,{CHORD_XFADE}),t/{CHORD_XFADE},"
                f"if(lt(t,{CHORD_DUR}),1,"
                f"({CHORD_DUR + CHORD_XFADE}-t)/{CHORD_XFADE}))")
        inputs, chains, labels = [], [], []
        for i, (f, g) in enumerate(partials):
            inputs.append(
                f"sine=frequency={f}:sample_rate=48000:duration={seg_dur}")
            drift = 0.05 + 0.013 * i
            phase = 0.7 * i + 1.1 * k
            chains.append(
                f"[{i}:a]volume='{g * SINE_GAIN}"
                f"*(0.85+0.15*sin(2*PI*{drift}*t+{phase}))'"
                f":eval=frame,volume='{env}':eval=frame[p{i}]")
            labels.append(f"[p{i}]")
        chains.append(
            f"{''.join(labels)}amix=inputs={len(labels)}:normalize=0,"
            f"lowpass=f=1200,pan=stereo|c0=c0|c1=c0,"
            f"atrim=0:{seg_dur}[out]")
        seg = d / f"bgm_chord{k}.wav"
        ffmpeg_filter(seg, inputs, ";".join(chains),
                      ["-map", "[out]", "-ar", str(SR)])
        segment_files.append(seg)
    # Assembly: delayed chord segments plus the melody.
    # Each melody note is a sine fundamental with a
    # quiet octave above it, both under exponential
    # decays (the octave decays faster, like a pluck).
    inputs = [str(p) for p in segment_files]
    chains, labels = [], []
    for k in range(n):
        ms = int(round((k * CHORD_DUR - (half if k else 0.0)) * 1000))
        chains.append(f"[{k}:a]adelay={ms}|{ms}[ch{k}]")
        labels.append(f"[ch{k}]")
    for j, (t, f, g) in enumerate(MELODY):
        base = n + 2 * j
        inputs.append(f"sine=frequency={f}:sample_rate=48000:duration=2.4")
        inputs.append(
            f"sine=frequency={2 * f}:sample_rate=48000:duration=2.4")
        ms = int(round(t * 1000))
        chains.append(
            f"[{base}:a]volume='{g * SINE_GAIN}*exp(-1.7*t)':eval=frame,"
            f"adelay={ms}|{ms}[mf{j}]")
        chains.append(
            f"[{base + 1}:a]volume='{g * 0.32 * SINE_GAIN}*exp(-3.0*t)'"
            f":eval=frame,adelay={ms}|{ms}[mh{j}]")
        labels += [f"[mf{j}]", f"[mh{j}]"]
    chains.append(
        f"{''.join(labels)}amix=inputs={len(labels)}:normalize=0,"
        f"volume='{BGM_ENV}':eval=frame,"
        "pan=stereo|c0=c0|c1=c0,atrim=0:60[out]")
    ffmpeg_filter(d / "bgm.wav", inputs, ";".join(chains),
                  ["-map", "[out]", "-ar", str(SR)])


def tick_times() -> list[tuple[float, float]]:
    """Deterministic sparse patter over the typing windows."""
    rng = random.Random(20261121)
    out: list[tuple[float, float]] = []
    for start, end, lo, hi in TYPING_WINDOWS:
        t = start + rng.uniform(0.0, 0.03)
        while t <= end - 0.04:
            if rng.random() > 0.15:
                out.append((t, rng.uniform(0.7, 1.0)))
            t += rng.uniform(lo, hi)
    return out


def delay(ms_time: float) -> str:
    ms = int(round(ms_time * 1000))
    return f"adelay={ms}|{ms}"


def build_raw_mix(d: Path) -> None:
    ticks = tick_times()
    parts: list[str] = []
    labels: list[str] = ["[0:a]", "[4:a]"]
    # All tick uses share one asplit of the tick prototype:
    # the typing patter, scene 4's lowpassed panel ticks,
    # and scene 5's soft crossfade tick.
    n_tick_uses = len(ticks) + len(PANEL_TICKS) + len(SOFT_TICKS)
    parts.append(f"[1:a]asplit={n_tick_uses}"
                 + "".join(f"[s{i}]" for i in range(n_tick_uses)))
    branch = 0
    for t, g in ticks:
        parts.append(
            f"[s{branch}]{delay(t)},volume={g * TICK_GAIN:.3f}[e{branch}]")
        labels.append(f"[e{branch}]")
        branch += 1
    for t, g in PANEL_TICKS:
        parts.append(
            f"[s{branch}]lowpass=f=1100,{delay(t)},volume={g:.2f}[e{branch}]")
        labels.append(f"[e{branch}]")
        branch += 1
    for t, g in SOFT_TICKS:
        parts.append(f"[s{branch}]{delay(t)},volume={g:.2f}[e{branch}]")
        labels.append(f"[e{branch}]")
        branch += 1
    # Clicks.
    parts.append(f"[2:a]asplit={len(CLICKS)}"
                 + "".join(f"[c{i}]" for i in range(len(CLICKS))))
    for i, (t, g) in enumerate(CLICKS):
        parts.append(f"[c{i}]{delay(t)},volume={g:.2f}[ce{i}]")
        labels.append(f"[ce{i}]")
    # Confirm blips.
    parts.append(f"[3:a]asplit={len(BLIPS)}"
                 + "".join(f"[b{i}]" for i in range(len(BLIPS))))
    for i, t in enumerate(BLIPS):
        parts.append(f"[b{i}]{delay(t)}[be{i}]")
        labels.append(f"[be{i}]")
    parts.append(
        f"{''.join(labels)}amix=inputs={len(labels)}:normalize=0,"
        "atrim=0:60,asetpts=N/SR/TB[out]")
    ffmpeg_filter(
        d / "mix_raw.wav",
        [str(d / "bed.wav"), str(d / "tick.wav"),
         str(d / "click.wav"), str(d / "blip.wav"),
         str(d / "bgm.wav")],
        ";".join(parts), ["-map", "[out]", "-ar", str(SR)])


def master(d: Path) -> tuple[float, float]:
    """Static gain to about -16 LUFS, verified under -1 dBTP.

    alimiter caps the sample peak at -1 dB, but broadband
    transients can still overshoot between samples, so the
    true peak is measured after every pass and the gain is
    backed off until the measurement complies. The numbers
    returned are the final measured ones, not targets.
    """
    src = d / "mix_raw.wav"
    out = d / "mix.wav"
    i0, _ = measure(src)
    gain = -16.0 - i0
    i1 = tp1 = 0.0
    for attempt in range(3):
        r = run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(src),
                 "-af", f"volume={gain:.2f}dB,"
                        "alimiter=limit=0.891:attack=5:release=100:level=0,"
                        "atrim=0:60,asetpts=N/SR/TB",
                 "-ar", str(SR), str(out)])
        if r.returncode != 0:
            sys.exit(f"mastering failed:\n{r.stderr}")
        i1, tp1 = measure(out)
        print(f"master pass {attempt + 1}: gain {gain:+.2f} dB -> "
              f"integrated {i1:.2f} LUFS, true peak {tp1:.2f} dBTP")
        if tp1 > -1.0:
            gain -= (tp1 + 1.0) + 0.15
            continue
        if i1 < -16.8 and attempt < 2:
            gain += (-16.0 - i1)
            continue
        break
    return i1, tp1


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--work", default=str(
        Path.home() / "workspace" / ".video-work"),
        help="VIDEO_WORK directory (default ~/workspace/.video-work)")
    args = ap.parse_args()
    d = Path(args.work) / "audio"
    d.mkdir(parents=True, exist_ok=True)
    build_prototypes(d)
    build_bed(d)
    build_bgm(d)
    build_raw_mix(d)
    i1, tp1 = master(d)
    print(f"mix: {d / 'mix.wav'}")
    print(f"final: integrated {i1:.2f} LUFS, true peak {tp1:.2f} dBTP, "
          f"duration {DUR:.3f} s, {SR} Hz stereo")


if __name__ == "__main__":
    main()
