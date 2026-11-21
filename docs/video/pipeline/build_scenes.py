#!/usr/bin/env python3
"""Build the six intro-video scene pages (+ hero loop).

Every scene is a 1920x1080 HTML page whose entire visual
state is a pure function of time: the renderer steps
`window.__setTime(t)` frame by frame and screenshots each
state, so the render is deterministic - no wall-clock
animation anywhere.

All terminal text is VERBATIM from real captures of the
real CLI against a throwaway dev server (see
docs/video/generation-log.md for the capture inventory and
the two staged bits: shell prompts, which are the demo
user's own prompt, and the `record` Saved line, whose path
is shown in the product's default form ~/.forgehub/... -
the capture used a scratch FORGEHUB_HOME).

Usage:
  build_scenes.py --repo ~/workspace/forgehub \
                  --work ~/workspace/.video-work \
                  --out  ~/workspace/.video-work/scenes
"""

import argparse
import html
import os

# ----------------------------------------------------------------
# Verbatim capture text (source file noted per block)
# ----------------------------------------------------------------

APT_CMD = (
    "sudo apt update && sudo apt -y upgrade && sudo apt -y install "
    "curl wget tar htop tmux ca-certificates"
)

# captures/login3.txt - device-code login, registry on
# localhost (the product is self-hosted; the capture server
# ran on this machine). The CLI's token line is never shown.
LOGIN_LINES = [
    "Log in to ForgeHub",
    "  registry: http://localhost:18086",
    "  open:     http://localhost:18086/device?code=TQH5-W8KS",
    "  code:     TQH5-W8KS",
    "Waiting for approval… (the code expires in 10 minutes)",
    "Logged in as alice on http://localhost:18086.",
]

# captures/run-own.txt lines 1-9 - the confirmation card for
# alice/ubuntu-setup v1, exactly as the CLI printed it
# (120-column wrap preserved).
CARD_OWN = [
    "About to run `ubuntu-setup` (cloud: alice/ubuntu-setup)",
    "  Update an Ubuntu machine and install the basics",
    "  version 1 · sha256:5b790bf8 · private · mode: script",
    "  Warnings:",
    "    line 1: runs with sudo (administrator privileges)",
    "  Commands:",
    "   1. sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux",
    "            ca-certificates",
]
CARD_OWN_PROMPT = "Run this on this machine? [y/N]"

# captures/run-own.txt - real apt output of the run (update
# phase Hit lines, then the install simulation's tail) and
# the CLI's own summary line.
RUN_OUT_SCROLL = [
    "Hit:1 http://azure.archive.ubuntu.com/ubuntu noble InRelease",
    "Hit:2 http://azure.archive.ubuntu.com/ubuntu noble-updates InRelease",
    "Hit:3 http://azure.archive.ubuntu.com/ubuntu noble-backports InRelease",
    "Hit:4 http://azure.archive.ubuntu.com/ubuntu noble-security InRelease",
    "Reading package lists... Done",
    "Building dependency tree... Done",
    "Reading state information... Done",
    "42 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.",
    "Inst ca-certificates [20260816ubuntu0.24.04.1] (20260816ubuntu0.24.04.2 Ubuntu:24.04/noble-updates, Ubuntu:24.04/noble-security [all])",
    "Inst libnl-3-200 (3.7.0-0.3build1.1 Ubuntu:24.04/noble-updates [amd64])",
    "Inst libnl-genl-3-200 (3.7.0-0.3build1.1 Ubuntu:24.04/noble-updates [amd64])",
    "Inst htop (3.3.0-4build1 Ubuntu:24.04/noble [amd64])",
    "Conf ca-certificates (20260816ubuntu0.24.04.2 Ubuntu:24.04/noble-updates, Ubuntu:24.04/noble-security [all])",
    "Conf htop (3.3.0-4build1 Ubuntu:24.04/noble [amd64])",
]
RUN_SUMMARY = "exit code 0 · 1 executed, 0 skipped · 10.2s"

# captures/run-other.txt - bob's snippet, typed-yes prompt.
CARD_OTHER = [
    "About to run `docker-setup` (cloud: bob/docker-setup)",
    "  Check the machine is ready for Docker and print the install steps",
    "  version 1 · sha256:9d791112 · public · mode: script",
    "  ⚠ This snippet is not yours. Owner: bob · ★ 0 · 2 pulls",
    "  Commands:",
    '   1. echo "Checking this machine for Docker..."',
    '   2. echo "1. Install Docker Engine from docs.docker.com"',
    '   3. echo "2. Add your user to the docker group"',
    '   4. echo "3. Log out and back in, then run: docker run hello-world"',
]
OTHER_PROMPT = "Run this on this machine?"
OTHER_OUT = [
    "Checking this machine for Docker...",
    "1. Install Docker Engine from docs.docker.com",
]

# captures/diff2.txt - bob/docker-setup changed v1 -> v2.
DIFF_HEAD = [
    "About to run `docker-setup` (cloud: bob/docker-setup)",
    "  Check the machine is ready for Docker and print the install steps",
    "  version 2 · sha256:5084e5ef · public · mode: script",
    "  ⚠ This snippet is not yours. Owner: bob · ★ 0 · 4 pulls",
    "  ⚠ This snippet CHANGED since you last ran it. Review the diff below carefully.",
]
DIFF_BLOCK = [
    ("ctx", "  --- bob/docker-setup v1 (last run)"),
    ("ctx", "  +++ bob/docker-setup v2 (now)"),
    ("ctx", "  @@ -8,6 +8,6 @@"),
    ("ctx", "   mode: script"),
    ("ctx", "   script: |"),
    ("ctx", '     echo "Checking this machine for Docker..."'),
    ("del", '  -  echo "1. Install Docker Engine from docs.docker.com"'),
    ("add", '  +  echo "1. Install Docker Engine and Docker Compose from docs.docker.com"'),
    ("ctx", '     echo "2. Add your user to the docker group"'),
    ("ctx", '     echo "3. Log out and back in, then run: docker run hello-world"'),
]
DIFF_TAIL = [
    "  Commands:",
    '   1. echo "Checking this machine for Docker..."',
    '   2. echo "1. Install Docker Engine and Docker Compose from docs.docker.com"',
    '   3. echo "2. Add your user to the docker group"',
    '   4. echo "3. Log out and back in, then run: docker run hello-world"',
]
DECLINED = "forgehub: not run (declined)"

# captures/notty.txt - piped run refusal (exit code 2).
NOTTY_TAIL = [
    "forgehub: not confirmed",
    "  what: `bob/docker-setup` was not run",
    "  why: there is no terminal to confirm on and someone else's snippet needs a typed confirmation the first time",
    "  fix: run it once interactively to approve this exact content",
]

# captures/record.txt + captures/push.txt - record/push.
RECORD_LINES = [
    ("cmd", "forgehub record deploy"),
    ("out", "forgehub: recording with bash. Run your commands, then type `exit` to finish."),
    ("cmd", "echo building the app"),
    ("out", "building the app"),
    ("cmd", "echo deploying to production"),
    ("out", "deploying to production"),
    ("cmd", "exit"),
    ("out", "Saved `deploy` (2 commands) to ~/.forgehub/snippets/deploy.yaml"),
    ("cmd", "forgehub push deploy"),
    ("out", "alice/deploy v1 sha256:bd02407b"),
]

PROMPT = "alice@new-server:~$"


def esc(s: str) -> str:
    return html.escape(s, quote=False)


def lines_html(lines, cls="ln", start_id=0):
    out = []
    for i, ln in enumerate(lines):
        out.append(f'<div class="{cls}" id="ln{start_id + i}">{esc(ln) or "&nbsp;"}</div>')
    return "".join(out)


SHARED_CSS = """
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
body {
  width: 1920px; height: 1080px; overflow: hidden; position: relative;
  background: #000; color: #f2f3f5;
  font-family: 'Roboto Flex', sans-serif;
}
.bg { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.scrim { position: absolute; inset: 0;
  background: linear-gradient(180deg, rgba(0,0,0,.40), rgba(0,0,0,.58)); }
.term {
  position: absolute; left: 50%; top: 50%; width: 1700px;
  transform: translate(-50%, -50%);
  background: rgba(7,9,11,.80);
  border: 1px solid rgba(255,255,255,.14); border-radius: 18px;
  box-shadow: 0 34px 90px rgba(0,0,0,.55);
}
.term-head { display: flex; align-items: center; gap: 10px;
  padding: 17px 24px; border-bottom: 1px solid rgba(255,255,255,.09); }
.dot { width: 14px; height: 14px; border-radius: 50%; background: rgba(255,255,255,.16); }
.dot.on { background: rgba(255,255,255,.38); }
.term-title { margin-left: 14px; font-size: 22px; color: rgba(255,255,255,.55); }
.term-body {
  padding: 32px 42px 38px; font-family: 'Roboto Mono', monospace;
  font-size: 22px; line-height: 1.55;
}
.col { display: flex; flex-direction: column; justify-content: flex-end;
  overflow: hidden; }
.ln { white-space: pre-wrap; word-break: break-all; }
.ln.keep { word-break: normal; }
.dim { color: rgba(235,238,240,.58); }
.prompt { color: #fff; font-weight: 700; }
.cursor { display: inline-block; width: 13px; height: 26px;
  background: #fff; vertical-align: -4px; margin-left: 2px; }
.warn { color: #ffd9d9; }
.lt { position: absolute; left: 118px; bottom: 80px; max-width: 1350px;
  font-size: 41px; font-weight: 600; line-height: 1.3; color: #fff;
  text-shadow: 0 2px 22px rgba(0,0,0,.8); opacity: 0; }
.lt::before { content: ''; position: absolute; left: -34px; top: 6px;
  width: 7px; height: 78%; background: #fff; border-radius: 4px; }
.lt.mono { font-family: 'Roboto Mono', monospace; font-weight: 500; }
.chip { display: inline-block; padding: 10px 22px; border-radius: 999px;
  border: 1px solid rgba(255,255,255,.25); background: rgba(10,12,14,.55);
  font-family: 'Roboto Mono', monospace; font-size: 24px; color: #e8eaed; }
.browser { position: absolute; width: 1560px; left: 180px; top: 96px;
  border-radius: 16px; overflow: hidden;
  border: 1px solid rgba(255,255,255,.16);
  box-shadow: 0 34px 90px rgba(0,0,0,.55); background: #0a0c0e; }
.browser-bar { display: flex; align-items: center; gap: 10px;
  padding: 13px 20px; background: rgba(255,255,255,.05);
  border-bottom: 1px solid rgba(255,255,255,.08); }
.url { margin-left: 14px; font-size: 20px; color: rgba(255,255,255,.55);
  font-family: 'Roboto Mono', monospace; }
.browser img { display: block; width: 100%; }
.ring { position: absolute; border: 3px solid rgba(255,255,255,.9);
  border-radius: 14px; box-shadow: 0 0 0 6px rgba(255,255,255,.12); opacity: 0; }
.mark { font-family: 'Instrument Serif', serif; }
.diff-del { background: rgba(217,138,138,.13); color: #f0c4c4; }
.diff-add { background: rgba(147,199,154,.13); color: #cdeeda; }
.pchip { margin-left: auto; font-size: 20px; padding: 6px 18px; }
"""

SHARED_JS = """
const clamp01 = x => Math.max(0, Math.min(1, x));
const seg = (t, a, b) => clamp01((t - a) / (b - a));
const E = id => document.getElementById(id);
function op(id, v) { const e = E(id); if (e) e.style.opacity = v; }
function type(id, full, t, a, b) {
  const e = E(id); if (!e) return;
  const n = Math.floor(full.length * seg(t, a, b));
  e.textContent = full.slice(0, n);
}
function blink(t, period) { return ((t % period) < period * 0.55) ? 1 : 0; }
function fadeSlide(id, t, a, b, dy) {
  const e = E(id); if (!e) return;
  const p = seg(t, a, b);
  e.style.opacity = p;
  e.style.transform = 'translateY(' + ((1 - p) * dy) + 'px)';
}
"""


def page(title: str, body: str, js: str, repo: str) -> str:
    fonts = f"""
@font-face {{ font-family: 'Roboto Flex'; src: url('file://{repo}/web/public/fonts/roboto-flex.woff2'); }}
@font-face {{ font-family: 'Roboto Mono'; src: url('file://{repo}/web/public/fonts/roboto-mono.woff2'); }}
@font-face {{ font-family: 'Instrument Serif'; src: url('file://{repo}/web/public/fonts/instrument-serif.woff2'); }}
"""
    return f"""<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>{title}</title>
<style>{fonts}{SHARED_CSS}</style></head>
<body>
<img class="bg" src="file://{repo}/web/public/media/observe-bg.jpg">
<div class="scrim"></div>
{body}
<script>{SHARED_JS}
{js}
</script>
</body></html>"""


def term_head(title: str, extra: str = "") -> str:
    return (
        '<div class="term-head"><div class="dot on"></div>'
        '<div class="dot"></div><div class="dot"></div>'
        f'<div class="term-title">{esc(title)}</div>{extra}</div>'
    )


def scene1(repo: str, work: str) -> str:
    out_lines = [
        "Hit:1 http://azure.archive.ubuntu.com/ubuntu noble InRelease",
        "Hit:2 http://azure.archive.ubuntu.com/ubuntu noble-updates InRelease",
        "Hit:3 http://azure.archive.ubuntu.com/ubuntu noble-backports InRelease",
        "Hit:4 http://azure.archive.ubuntu.com/ubuntu noble-security InRelease",
        "Reading package lists... Done",
    ]
    body = f"""
<div class="term" id="term" style="opacity:0">
  {term_head('alice@new-server: ~')}
  <div class="term-body">
    <div class="ln keep"><span class="prompt">{PROMPT} </span><span id="typed"></span><span class="cursor" id="cur"></span></div>
    <div id="s1out" style="margin-top:10px">
      {lines_html(out_lines, 'ln dim keep', 100)}
    </div>
  </div>
</div>
<div class="chip" id="chip1" style="position:absolute; left:150px; top:140px; transform:rotate(-3deg); opacity:0">notes.txt</div>
<div class="chip" id="chip2" style="position:absolute; right:160px; top:170px; transform:rotate(2deg); opacity:0">chat</div>
<div class="chip" id="chip3" style="position:absolute; left:170px; bottom:200px; transform:rotate(2deg); opacity:0">~/.bash_history</div>
<div class="chip" id="chip4" style="position:absolute; right:150px; bottom:230px; transform:rotate(-2deg); opacity:0">old gist</div>
<div class="lt" id="lt1">You've typed this before.</div>
<div class="lt" id="lt2">Scattered across notes, chats and shell history.</div>
"""
    js = """
const OUT_T = [4.75, 4.9, 5.05, 5.2, 5.4];
window.__setTime = function(t) {
  op('term', seg(t, 0.0, 0.5));
  type('typed', %r, t, 0.7, 4.3);
  op('cur', t < 4.55 ? blink(t, 1.0) : 0);
  for (let i = 0; i < 5; i++) op('ln' + (100 + i), seg(t, OUT_T[i], OUT_T[i] + 0.25));
  op('chip1', seg(t, 5.5, 6.0)); op('chip2', seg(t, 6.0, 6.5));
  op('chip3', seg(t, 6.5, 7.0)); op('chip4', seg(t, 7.0, 7.5));
  op('lt1', seg(t, 0.9, 1.3) * (1 - seg(t, 4.7, 5.1)));
  op('lt2', seg(t, 5.3, 5.7));
};
""" % APT_CMD
    return page("Scene 1 - Hook", body, js, repo)


def scene2(repo: str, work: str) -> str:
    body = f"""
<div id="mark" style="position:absolute; inset:0; display:flex; flex-direction:column; align-items:center; justify-content:center; opacity:0">
  <div class="mark" style="font-size:170px; line-height:1">ForgeHub</div>
  <div style="font-size:47px; color:rgba(255,255,255,.78); margin-top:26px">Save a command once. Run it anywhere.</div>
</div>
<div class="browser" id="browser" style="opacity:0">
  <div class="browser-bar"><div class="dot on"></div><div class="dot"></div><div class="dot"></div>
    <div class="url">forgehub / new snippet</div></div>
  <div style="overflow:hidden">
    <div id="imgwrap" style="position:relative; width:1560px; height:878px">
      <img id="shotA" src="file://{work}/shots/editor-filled.png" style="position:absolute; width:1560px; height:878px">
      <img id="shotB" src="file://{work}/shots/snippet-saved.png" style="position:absolute; width:1560px; height:878px; opacity:0">
    </div>
  </div>
  <div class="ring" id="ring" style="left:1068px; top:184px; width:278px; height:274px"></div>
</div>
<div class="lt" id="lt1">Text only. Versioned. Private by default.</div>
"""
    js = """
window.__setTime = function(t) {
  const m = E('mark');
  m.style.opacity = seg(t, 0.2, 1.1) * (1 - seg(t, 1.9, 2.5));
  m.style.transform = 'translateY(' + ((1 - seg(t, 0.2, 1.1)) * 26) + 'px)';
  op('browser', seg(t, 2.3, 3.0));
  op('ring', seg(t, 4.4, 4.9) * (1 - seg(t, 6.0, 6.4)));
  op('shotB', seg(t, 6.5, 7.2));
  op('lt1', seg(t, 3.1, 3.5) * (1 - seg(t, 9.6, 10.0)));
};
"""
    return page("Scene 2 - Answer", body, js, repo)


def scene3(repo: str, work: str) -> str:
    login_first = lines_html(LOGIN_LINES[:-1], "ln keep", 0)
    card = lines_html(CARD_OWN, "ln keep", 20)
    out = lines_html(RUN_OUT_SCROLL, "ln dim keep", 40)
    body = f"""
<div class="term" id="term" style="opacity:0">
  {term_head('alice@new-server: ~')}
  <div class="term-body">
    <div class="col" style="height:600px; justify-content:flex-start">
      <div id="colInner">
        <div class="ln keep"><span class="prompt">{PROMPT} </span><span id="typedLogin"></span><span class="cursor" id="curL"></span></div>
        <div id="loginBlock" style="opacity:0">{login_first}</div>
        <div class="ln keep" id="loggedIn" style="opacity:0">{esc(LOGIN_LINES[-1])}</div>
        <div class="ln keep" style="margin-top:14px"><span class="prompt">{PROMPT} </span><span id="typedFg"></span><span class="cursor" id="curF"></span></div>
        <div id="cardBlock" style="opacity:0">{card}</div>
        <div class="ln keep" id="cardPrompt" style="opacity:0">{esc(CARD_OWN_PROMPT)} <span id="ansY"></span><span class="cursor" id="curC"></span></div>
        <div id="outBlock">{out}</div>
        <div class="ln keep" id="summary" style="opacity:0; font-weight:700">{esc(RUN_SUMMARY)}</div>
      </div>
    </div>
  </div>
</div>
<div class="browser" id="device" style="width:920px; left:500px; top:150px; opacity:0">
  <div class="browser-bar"><div class="dot on"></div><div class="dot"></div><div class="dot"></div>
    <div class="url">localhost:18086/device</div></div>
  <img src="file://{work}/shots/device-crop.png" style="width:920px">
  <div class="ring" id="approveRing" style="left:74px; top:474px; width:158px; height:62px"></div>
</div>
<div class="lt mono" id="lt1">fg ubuntu-setup</div>
<div class="lt" id="lt2">You see exactly what will run. Then you decide.</div>
"""
    js = """
const OUT_N = %d;
window.__setTime = function(t) {
  op('term', seg(t, 0.0, 0.4));
  type('typedLogin', 'forgehub login', t, 0.3, 1.1);
  op('curL', t < 1.25 ? blink(t, 1.0) : 0);
  op('loginBlock', seg(t, 1.35, 1.7));
  const dev = seg(t, 2.3, 2.7) * (1 - seg(t, 3.55, 3.9));
  op('device', dev);
  E('term').style.opacity = seg(t, 0.0, 0.4) * (1 - 0.45 * dev);
  op('approveRing', seg(t, 3.05, 3.3) * (1 - seg(t, 3.5, 3.7)));
  op('loggedIn', seg(t, 3.95, 4.2));
  type('typedFg', 'fg ubuntu-setup', t, 4.35, 5.0);
  op('curF', (t > 4.3 && t < 5.15) ? blink(t, 1.0) : 0);
  op('cardBlock', seg(t, 5.15, 5.55));
  op('cardPrompt', seg(t, 5.55, 5.8));
  E('ansY').textContent = t >= 8.3 ? 'y' : '';
  op('curC', (t > 5.7 && t < 8.3) ? blink(t, 1.0) : 0);
  for (let i = 0; i < OUT_N; i++) {
    const at = 8.55 + i * 0.075;
    op('ln' + (40 + i), seg(t, at, at + 0.1));
  }
  op('summary', seg(t, 9.62, 9.8));
  const shift = seg(t, 8.5, 9.8) * 546;
  E('colInner').style.transform = 'translateY(' + (-shift) + 'px)';
  op('lt1', seg(t, 4.35, 4.7) * (1 - seg(t, 6.15, 6.45)));
  op('lt2', seg(t, 6.5, 6.85));
};
""" % len(RUN_OUT_SCROLL)
    return page("Scene 3 - Run anywhere", body, js, repo)


def scene4(repo: str, work: str) -> str:
    notty_card = [l.replace("2 pulls", "3 pulls") for l in CARD_OTHER]
    p1_lines = lines_html(CARD_OTHER, "ln keep", 0)
    p1_out = lines_html(OTHER_OUT, "ln dim keep", 20)
    diff_rows = []
    for i, (kind, text) in enumerate(DIFF_BLOCK):
        cls = {"ctx": "ln keep", "del": "ln keep diff-del", "add": "ln keep diff-add"}[kind]
        diff_rows.append(f'<div class="{cls}" id="d{i}">{esc(text)}</div>')
    p2 = (
        lines_html(DIFF_HEAD, "ln keep", 30)
        + "\n"
        + "\n".join(diff_rows)
        + "\n"
        + lines_html(DIFF_TAIL, "ln keep", 50)
    )
    p3_lines = lines_html(notty_card, "ln keep", 70)
    p3_tail = lines_html(NOTTY_TAIL, "ln keep", 90)
    body = f"""
<div class="term" id="p1" style="opacity:0">
  {term_head('alice@new-server: ~', '<span class="chip pchip">Someone else\'s snippet</span>')}
  <div class="term-body">
    {p1_lines}
    <div class="ln keep" id="p1prompt">{esc(OTHER_PROMPT)} <span id="yesTyped"></span><span class="cursor" id="curY"></span></div>
    {p1_out}
  </div>
</div>
<div class="term" id="p2" style="opacity:0">
  {term_head('alice@new-server: ~', '<span class="chip pchip">Changed since you last ran it</span>')}
  <div class="term-body">
    {p2}
    <div class="ln keep" id="p2prompt">{esc(OTHER_PROMPT)} <span id="noTyped"></span></div>
    <div class="ln keep" id="declined" style="opacity:0">{esc(DECLINED)}</div>
  </div>
</div>
<div class="term" id="p3" style="opacity:0">
  {term_head('alice@new-server: ~', '<span class="chip pchip">No terminal</span>')}
  <div class="term-body">
    <div class="ln keep"><span class="prompt">{PROMPT} </span><span id="typedPipe"></span><span class="cursor" id="curP"></span></div>
    {p3_lines}
    {p3_tail}
    <div style="margin-top:16px"><span class="chip" id="exitChip" style="opacity:0">exit code 2</span></div>
  </div>
</div>
<div class="lt" id="lt1">Other people's snippets need a typed yes.</div>
<div class="lt" id="lt2">Changed since last time? You see the diff.</div>
<div class="lt" id="lt3">No terminal? It won't run.</div>
"""
    js = """
function stag(prefix, count, t, a, step) {
  for (let i = 0; i < count; i++) op(prefix + i, seg(t, a + i * step, a + i * step + 0.18));
}
window.__setTime = function(t) {
  // Panel 1: typed yes.
  op('p1', seg(t, 0.0, 0.25) * (1 - seg(t, 3.12, 3.35)));
  stag('ln', 9, t, 0.15, 0.07);
  op('p1prompt', seg(t, 0.95, 1.15));
  const yn = Math.floor(3 * seg(t, 1.9, 2.5));
  E('yesTyped').textContent = 'yes'.slice(0, yn);
  op('curY', (t > 1.0 && t < 2.6) ? blink(t, 1.0) : 0);
  op('ln20', seg(t, 2.7, 2.9)); op('ln21', seg(t, 2.88, 3.05));
  // Panel 2: changed snippet diff.
  op('p2', seg(t, 3.35, 3.6) * (1 - seg(t, 6.42, 6.65)));
  for (let i = 0; i < 5; i++) op('ln' + (30 + i), seg(t, 3.62 + i * 0.05, 3.8 + i * 0.05));
  for (let i = 0; i < 10; i++) op('d' + i, seg(t, 3.95 + i * 0.055, 4.13 + i * 0.055));
  for (let i = 0; i < 5; i++) op('ln' + (50 + i), seg(t, 4.62 + i * 0.05, 4.8 + i * 0.05));
  op('p2prompt', seg(t, 5.15, 5.35));
  E('noTyped').textContent = t >= 5.5 ? 'no' : '';
  op('declined', seg(t, 5.8, 6.0));
  // Panel 3: no TTY refusal.
  op('p3', seg(t, 6.65, 6.9));
  type('typedPipe', 'fg bob/docker-setup | tee run.log', t, 6.95, 7.55);
  op('curP', (t > 6.9 && t < 7.7) ? blink(t, 1.0) : 0);
  for (let i = 0; i < 9; i++) op('ln' + (70 + i), seg(t, 7.7 + i * 0.05, 7.88 + i * 0.05));
  for (let i = 0; i < 4; i++) op('ln' + (90 + i), seg(t, 8.35 + i * 0.13, 8.55 + i * 0.13));
  op('exitChip', seg(t, 9.15, 9.4));
  // Captions.
  op('lt1', seg(t, 0.35, 0.65) * (1 - seg(t, 3.05, 3.3)));
  op('lt2', seg(t, 3.75, 4.05) * (1 - seg(t, 6.35, 6.6)));
  op('lt3', seg(t, 7.0, 7.3));
};
"""
    return page("Scene 4 - Safety", body, js, repo)


def scene5(repo: str, work: str) -> str:
    rows = []
    for i, (kind, text) in enumerate(RECORD_LINES):
        if kind == "cmd":
            rows.append(
                f'<div class="ln keep"><span class="prompt">{PROMPT} </span>'
                f'<span id="rc{i}"></span></div>'
            )
        else:
            rows.append(f'<div class="ln keep" id="ro{i}" style="opacity:0">{esc(text)}</div>')
    body = f"""
<div class="browser" id="browser">
  <div class="browser-bar"><div class="dot on"></div><div class="dot"></div><div class="dot"></div>
    <div class="url">forgehub / explore</div></div>
  <div style="overflow:hidden">
    <div id="imgwrap" style="position:relative; width:1560px; height:878px">
      <img id="shotA" src="file://{work}/shots/explore-docker.png" style="position:absolute; width:1560px; height:878px">
      <img id="shotB" src="file://{work}/shots/versions-diff.png" style="position:absolute; width:1560px; height:878px; opacity:0">
    </div>
  </div>
</div>
<div class="term" id="term" style="opacity:0">
  {term_head('alice@new-server: ~')}
  <div class="term-body">
    {"".join(rows)}
  </div>
</div>
<div class="lt" id="lt1">Private, unlisted or public.</div>
<div class="lt" id="lt2">Record. Push. Pull. Just like git.</div>
"""
    js = """
const CMD_T = [[0,'forgehub record deploy',6.35,6.62],[2,'echo building the app',6.92,7.17],[4,'echo deploying to production',7.47,7.77],[6,'exit',8.07,8.27],[8,'forgehub push deploy',8.57,8.87]];
const OUT_T = {1:6.78, 3:7.32, 5:7.92, 7:8.42, 9:9.18};
window.__setTime = function(t) {
  op('shotB', seg(t, 3.35, 3.95));
  op('browser', 1 - seg(t, 6.0, 6.4));
  op('term', seg(t, 6.05, 6.45));
  for (const [i, txt, a, b] of CMD_T) type('rc' + i, txt, t, a, b);
  for (const i in OUT_T) op('ro' + i, seg(t, OUT_T[i], OUT_T[i] + 0.18));
  op('lt1', seg(t, 0.4, 0.7) * (1 - seg(t, 3.1, 3.4)));
  op('lt2', seg(t, 6.55, 6.85));
};
"""
    return page("Scene 5 - Share and discover", body, js, repo)


def scene6(repo: str, work: str) -> str:
    body = """
<div id="closeWrap" style="position:absolute; inset:0; display:flex; flex-direction:column; align-items:center; justify-content:center">
  <div class="mark" id="mark" style="font-size:190px; line-height:1; opacity:0">ForgeHub</div>
  <div id="tag" style="font-size:48px; color:rgba(255,255,255,.82); margin-top:30px; opacity:0">Save a command once. Run it anywhere.</div>
  <div id="btns" style="display:flex; gap:26px; margin-top:52px; opacity:0">
    <div style="background:#fff; color:#000; font-size:28px; font-weight:600; padding:19px 44px; border-radius:999px">Create your account</div>
    <div style="border:1px solid rgba(255,255,255,.4); color:#fff; font-size:28px; padding:19px 44px; border-radius:999px">Install the CLI</div>
  </div>
  <div id="install" style="margin-top:56px; opacity:0; background:rgba(7,9,11,.72); border:1px solid rgba(255,255,255,.14); border-radius:16px; padding:30px 40px; font-family:'Roboto Mono',monospace; font-size:26px; line-height:1.6">
    <div style="font-family:'Roboto Flex',sans-serif; font-size:20px; letter-spacing:.14em; color:rgba(255,255,255,.55); margin-bottom:12px">INSTALL THE CLI - LINUX, FROM THE RELEASES PAGE</div>
    <div>$ gh release download v0.1.6</div>
    <div>$ sha256sum -c SHA256SUMS<span class="cursor" id="curI"></span></div>
  </div>
  <div id="linux" style="margin-top:30px; font-size:23px; color:rgba(255,255,255,.6); opacity:0">Linux-only - on other systems, use the web app in a browser.</div>
</div>
<div id="fadeout" style="position:absolute; inset:0; background:#000; opacity:0"></div>
"""
    js = """
window.__setTime = function(t) {
  fadeSlide('mark', t, 0.3, 1.2, 30);
  fadeSlide('tag', t, 1.1, 1.9, 22);
  fadeSlide('btns', t, 2.1, 2.8, 18);
  fadeSlide('install', t, 3.1, 3.9, 18);
  op('curI', t > 3.9 ? blink(t, 1.0) : 0);
  op('linux', seg(t, 4.2, 4.8));
  op('fadeout', seg(t, 9.6, 10.0));
};
"""
    return page("Scene 6 - Close", body, js, repo)


def hero(repo: str, work: str) -> str:
    card = lines_html(CARD_OWN, "ln keep", 0)
    body = f"""
<div class="term" style="width:1560px">
  {term_head('alice@new-server: ~')}
  <div class="term-body">
    <div class="ln keep"><span class="prompt">{PROMPT} </span><span id="typedFg"></span><span class="cursor" id="curF"></span></div>
    <div id="cardBlock" style="opacity:0">{card}</div>
    <div class="ln keep" id="cardPrompt" style="opacity:0">{esc(CARD_OWN_PROMPT)} <span class="cursor" id="curC"></span></div>
  </div>
</div>
"""
    js = """
window.__setTime = function(t) {
  E('typedFg').textContent = t < 7.5 ? 'fg ubuntu-setup'.slice(0, Math.floor(15 * seg(t, 0.2, 1.3))) : '';
  op('curF', t < 1.5 ? blink(t, 1.0) : 0);
  const card = seg(t, 1.5, 2.1) * (1 - seg(t, 6.7, 7.5));
  op('cardBlock', card);
  op('cardPrompt', seg(t, 1.9, 2.3) * (1 - seg(t, 6.7, 7.5)));
  op('curC', (t > 2.3 && t < 6.7) ? blink(t, 1.0) : 0);
};
"""
    return page("Hero loop", body, js, repo)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=os.path.expanduser("~/workspace/forgehub"))
    ap.add_argument("--work", default=os.path.expanduser("~/workspace/.video-work"))
    ap.add_argument("--out", default=os.path.expanduser("~/workspace/.video-work/scenes"))
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    scenes = {
        "scene1.html": scene1(args.repo, args.work),
        "scene2.html": scene2(args.repo, args.work),
        "scene3.html": scene3(args.repo, args.work),
        "scene4.html": scene4(args.repo, args.work),
        "scene5.html": scene5(args.repo, args.work),
        "scene6.html": scene6(args.repo, args.work),
        "hero.html": hero(args.repo, args.work),
    }
    for name, doc in scenes.items():
        with open(os.path.join(args.out, name), "w") as f:
            f.write(doc)
        print("wrote", name, len(doc), "bytes")


if __name__ == "__main__":
    main()
