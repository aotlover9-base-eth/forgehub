#!/usr/bin/env python3
"""PTY capture driver for the ForgeHub intro video.

Runs a real CLI command under a pseudo-terminal, waits for
expected output patterns, sends scripted answers, and saves
two files per session:

  <name>.raw    the exact bytes the terminal received
  <name>.txt    the same session as clean text (ANSI escapes
                stripped, CR overwrites resolved)

Usage:
  capture.py SPEC.json

Spec format:
  {
    "name": "run-own",
    "argv": ["forgehub", "run", "ubuntu-setup"],
    "env": {"FORGEHUB_HOME": "/tmp/...", "PATH": "..."},
    "cols": 120, "rows": 40,
    "steps": [{"expect": "\\\\[y/N\\\\]", "send": "y\\n"}],
    "timeout": 120
  }

Nothing here fakes output: the transcript is whatever the
real binary printed. The scene pages re-typeset this text.
"""

import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time

ANSI_RE = re.compile(
    r"\x1b\[[0-9;?]*[ -/]*[@-~]"  # CSI
    r"|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)"  # OSC
    r"|\x1b[()][0-9A-B]"  # charset switches
    r"|\x1b[@-Z\\-_]"  # single-char escapes
)


def clean(raw: bytes) -> str:
    text = raw.decode("utf-8", errors="replace")
    text = ANSI_RE.sub("", text)
    out_lines = []
    for line in text.split("\n"):
        # Resolve carriage returns: last writer wins per segment.
        if "\r" in line:
            parts = line.split("\r")
            merged = ""
            for part in parts:
                merged = part + merged[len(part):] if len(part) < len(merged) else part
            line = merged
        out_lines.append(line.replace("\x07", "").rstrip())
    # Drop trailing blank lines; keep interior spacing intact.
    while out_lines and out_lines[-1] == "":
        out_lines.pop()
    return "\n".join(out_lines) + "\n"


def run(spec: dict, outdir: str) -> int:
    env = dict(os.environ)
    env.update(spec.get("env", {}))
    env.setdefault("TERM", "xterm-256color")
    pid, fd = pty.fork()
    if pid == 0:
        os.execvpe(spec["argv"][0], spec["argv"], env)
    # Parent: set window size.
    cols = spec.get("cols", 120)
    rows = spec.get("rows", 40)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    raw = bytearray()
    steps = list(spec.get("steps", []))
    timeout = spec.get("timeout", 60)
    deadline = time.time() + timeout
    status = None
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                chunk = b""
            if not chunk:
                break
            raw.extend(chunk)
            if steps:
                text = raw.decode("utf-8", errors="replace")
                if re.search(steps[0]["expect"], text):
                    os.write(fd, steps[0]["send"].encode())
                    steps.pop(0)
        done, status = os.waitpid(pid, os.WNOHANG)
        if done:
            # Drain anything left.
            try:
                while True:
                    chunk = os.read(fd, 65536)
                    if not chunk:
                        break
                    raw.extend(chunk)
            except OSError:
                pass
            break
    else:
        os.kill(pid, signal.SIGKILL)
        _, status = os.waitpid(pid, 0)
    os.close(fd)
    name = spec["name"]
    with open(os.path.join(outdir, name + ".raw"), "wb") as f:
        f.write(bytes(raw))
    with open(os.path.join(outdir, name + ".txt"), "w") as f:
        f.write(clean(bytes(raw)))
    exit_code = os.waitstatus_to_exitcode(status) if status is not None else -1
    print(f"{name}: exit={exit_code} bytes={len(raw)} steps_left={len(steps)}")
    return exit_code


def main() -> None:
    spec_path = sys.argv[1]
    outdir = sys.argv[2] if len(sys.argv) > 2 else "."
    with open(spec_path) as f:
        spec = json.load(f)
    code = run(spec, outdir)
    expected = spec.get("expect_exit")
    if expected is not None and code != expected:
        print(f"WARN: {spec['name']} exit {code} != expected {expected}")
    sys.exit(0)


if __name__ == "__main__":
    main()
