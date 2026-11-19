#!/usr/bin/env python3
"""Run a command under a pseudo-terminal, answer its prompt
once, and relay output + exit code (Step 20 e2e helper).

Usage: ptyrun.py <input-line> <command...>

ForgeHub's trust prompts require a real TTY (by design — a
pipe must never be able to confirm running someone else's
code). This driver gives the e2e script a TTY: it feeds
<input-line> to the child as soon as a confirmation prompt
appears in the output, or after a short grace period.
"""
import os
import pty
import select
import subprocess
import sys
import time


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: ptyrun.py <input-line> <command...>", file=sys.stderr)
        return 2
    data_in = (sys.argv[1] + "\n").encode()
    cmd = sys.argv[2:]

    master, slave = pty.openpty()
    proc = subprocess.Popen(
        cmd, stdin=slave, stdout=slave, stderr=slave, close_fds=True
    )
    os.close(slave)

    out = b""
    sent = False
    started = time.time()
    deadline = started + 60
    while time.time() < deadline:
        ready, _, _ = select.select([master], [], [], 0.2)
        if ready:
            try:
                chunk = os.read(master, 4096)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
            if not sent and (b"[y/N]" in out or b"yes" in out.lower()):
                os.write(master, data_in)
                sent = True
        if not sent and time.time() - started > 3:
            # No recognisable prompt; feed anyway (harmless if
            # the child never reads stdin).
            try:
                os.write(master, data_in)
            except OSError:
                pass
            sent = True
        if proc.poll() is not None and not ready:
            break
    try:
        while True:
            chunk = os.read(master, 4096)
            if not chunk:
                break
            out += chunk
    except OSError:
        pass
    os.close(master)
    rc = proc.wait()
    sys.stdout.buffer.write(out)
    sys.stdout.flush()
    return rc


if __name__ == "__main__":
    sys.exit(main())
