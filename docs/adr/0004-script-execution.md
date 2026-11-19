# ADR 0004 — Script mode executes from a private temp file, not stdin

- **Status:** Accepted (Step 2, 2026-11-19)

## Context
Script mode must run a multi-line body with the declared shell. The two
candidate mechanisms are `shell -s` with the body on stdin, and writing the
body to a temporary file and running `shell <file>`.

Known gotchas of the stdin mechanism: any command inside the script that
reads stdin (`read`, `ssh`, `cat`, interactive installers) consumes the
*script itself* as its input; the script cannot also receive piped input
from the user; and some shells behave differently for non-file input
(`$0`, `BASH_SOURCE`, job control warnings).

## Decision
Write the body to a file created with `O_EXCL` inside a fresh private temp
directory (directory mode `0700`, file mode `0700`), run `<shell> <file>`
with the terminal inherited, and delete the file and directory on **every**
exit path — success, failure, and signal handling (SIGINT/SIGTERM set a
cleanup flag; a panic hook performs the same cleanup). The directory lives
under the system temp dir or `$FORGEHUB_HOME/tmp` when the system temp dir
is world-writable without a sticky bit — `doctor` reports a bad temp dir.

## Alternatives rejected
- **`shell -s` / `shell -c <body>`:** the stdin-consumption bug above for
  `-s`; for `-c`, very large bodies hit argv limits and the body appears in
  the process list (`ps`) for the whole run — a confidentiality regression
  for private snippets.
- **Named pipe:** same stdin semantics as `-s`, plus writer-blocking
  failure modes.

## Consequences
Steps mode does not use this mechanism (each step is `shell -c`, Forgenesis
behaviour, where a step is a single command and `ps` exposure is the
user's own typed command). Temp-file handling is a named threat
(T-LOCAL-1) and is re-verified in Step 19.
