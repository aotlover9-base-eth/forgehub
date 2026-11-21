#!/usr/bin/env bash
# ptyrun.sh - run a command under a real PTY via script(1),
# answering prompts from a file.
#
# Usage: ptyrun.sh OUTFILE ANSWERS COMMAND [ARGS...]
#
# ANSWERS is a text file; each non-empty line is
#   REGEX<TAB>TEXT
# When REGEX first matches the output captured so far, TEXT
# (interpreted with printf %b, so \n works) is typed into
# the terminal. Answers fire in file order, each at most
# once. Use "-" for ANSWERS to send nothing (probe mode:
# the session is killed after TIMEOUT seconds).
#
# Environment: PTY_TIMEOUT (seconds, default 120).
# The command's exit code is printed as the last line:
#   PTYRUN_EXIT=<n>   (or PTYRUN_KILLED=1 in probe mode)
set -u
OUT="$1"; ANSWERS="$2"; shift 2
TIMEOUT="${PTY_TIMEOUT:-120}"
TMP="$(mktemp -d)"
FIFO="$TMP/in"
mkfifo "$FIFO"
# Keep a writer fd open so the reader never sees EOF early.
exec 3<>"$FIFO"
script -qefc "$*" "$OUT" < "$FIFO" &
SPID=$!
declare -a RES=() TEXTS=()
if [ "$ANSWERS" != "-" ]; then
  while IFS=$'\t' read -r re text; do
    [ -n "$re" ] || continue
    RES+=("$re"); TEXTS+=("$text")
  done < "$ANSWERS"
fi
NEXT=0
ELAPSED=0
while kill -0 "$SPID" 2>/dev/null; do
  if [ "$NEXT" -lt "${#RES[@]}" ]; then
    if grep -aq "${RES[$NEXT]}" "$OUT" 2>/dev/null; then
      printf '%b' "${TEXTS[$NEXT]}" >&3
      NEXT=$((NEXT+1))
    fi
  fi
  sleep 0.25
  ELAPSED=$(awk "BEGIN{print $ELAPSED + 0.25}")
  if awk "BEGIN{exit !($ELAPSED >= $TIMEOUT)}"; then
    kill -KILL "$SPID" 2>/dev/null
    wait "$SPID" 2>/dev/null
    echo "PTYRUN_KILLED=1 answers_sent=$NEXT"
    exec 3>&-
    rm -rf "$TMP"
    exit 0
  fi
done
wait "$SPID"
CODE=$?
echo "PTYRUN_EXIT=$CODE answers_sent=$NEXT"
exec 3>&-
rm -rf "$TMP"
