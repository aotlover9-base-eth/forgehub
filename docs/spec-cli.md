# Spec - CLI (`forgehub`, aliased to `fg`)

- **Status:** Normative for Steps 4, 5, 11, 12, 13, 18. **Version:** 1 (Step 2, 2026-11-19)
- **Binary:** `forgehub`. `fg` is a shell alias/function installed by `alias install` (roadmap §6); non-interactive scripts must call `forgehub`.
- **Local state:** `$FORGEHUB_HOME` or `~/.forgehub/` - `config.yaml`, `snippets/`, `cache/`, `trust.db`, `history.db`, `backups/`. (Step 5 correction: the Step 2 draft said `config.toml`/`workflows/`; Steps 4–5 shipped YAML config and `snippets/` - the config *keys* are unchanged. See `reports/report_Step5.md`, Corrections.) **Credentials:** `$XDG_CONFIG_HOME/forgehub/credentials.toml` or `~/.config/forgehub/credentials.toml`, directory `0700`, file `0600`.

## 1. Global flags

`--registry <url>` · `--json` · `--color auto|always|never` · `-v/--verbose` (redacted) · `-q/--quiet` · `-y/--yes` (semantics restricted by §7) · `--offline` · `-h/--help` · `-V/--version`.

Environment: `FORGEHUB_HOME`, `FORGEHUB_REGISTRY`, `FORGEHUB_TOKEN` (CI override, never printed), `FORGEHUB_ASCII=1`, `NO_COLOR`.

## 2. Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Failure (a step failed, a server error, invalid input) |
| 2 | Refused / not confirmed - the fail-safe path: prompt declined, non-TTY without permission to run, trust not established |
| 3 | Not found (name resolution found nothing, §6) |
| 4 | Authentication required or expired (`forgehub login` is the fix, and the error says so) |
| 130 | Interrupted (SIGINT) |

## 3. Output formats

- **Human (default):** plain aligned columns for lists; rounded "surface" cards (box-drawing, ASCII fallback) for `show`, `whoami` and the run confirmation (§7). Colour per agent.md §8.4: one accent, dim secondary, red/amber/green only for error/warning/success; no colour when non-TTY, `NO_COLOR`, or `--color=never`.
- **`--json`:** one JSON object on stdout, no other stdout output. Stable keys per command:
  - `list`: `{ "workflows": [{ "name", "description", "tags", "mode", "origin" }] }` (local) - cloud lists add `"owner"`, `"version"`, `"hash"`, `"visibility"`.
  - `show`: `{ "name", "description", "tags", "mode", "shell", "variables", "commands" }` where `commands` is the ordered list of command strings that would run.
  - `stats`: `{ "runs", "commands", "total_ms", "failures", "estimated_saved_ms", "workflows": [{ "name", "runs", "commands", "total_ms", "failures", "last_run" }] }`.
  - `doctor`: `{ "checks": [{ "name", "level": "ok|warn|fail", "detail" }], "ok": bool }`.
  - `whoami`: `{ "username", "registry", "token_name", "token_prefix", "scopes", "expires_at" }` - never the token itself.
  - `push`/`pull`/`fork`: `{ "owner", "name", "version", "hash", "url" }`.
  - `status`: `{ "snippets": [{ "name", "owner", "state", "local_version", "remote_version" }] }` (states per §5).
  - `cache list`: `{ "entries": [{ "owner", "name", "version", "bytes", "modified" }], "total_bytes", "cap_bytes" }`; `cache clear`: `{ "cleared_entries", "freed_bytes", "files_removed" }`.
  - `trust list`: `{ "trusted": [{ "owner", "name", "hash", "version", "runs" }] }`.
  - `update --check`: `{ "current", "latest", "update_available", "source" }` (`source` only when an update is available).
  - Errors with `--json`: `{ "error": { "code", "message" } }` on stderr, exit code unchanged.
- Errors (human) follow **what failed → why → how to fix**, one line each, on stderr.

## 4. Commands - local (Steps 4–5)

| Command | Behaviour (summary; Steps 4–5 are normative for detail) |
|---------|----------------------------------------------------------|
| `init` | Create home, `config.yaml` (`default_shell`, `editor`, `confirm_before_run`, `ignore_commands`, `history_file`, `human_overhead_seconds`, analyze thresholds), `snippets/`, `backups/`. Idempotent |
| `record <name> [--simple] [--force] [-d desc] [--push]` | Bash/zsh live capture with a `● rec` prompt marker; `--simple` reads stdin. Drops noise (`ls`, `cd`, `clear`, config `ignore_commands`) and consecutive duplicates; a leading `cd` becomes `cwd`; secret scanner **refuses** to save secrets and offers a variable instead. Saves steps-mode YAML |
| `run <target> [KEY=value…] [--dry] [--step] [-k/--keep-going] [-q] [-y] [--version N] [--no-cache] [--offline]` | §6–§8. `forgehub <name>` is shorthand for `run <name>` when `<name>` is not a subcommand |
| `list [--tag t] [--long]` | Local workflows; `--json` per §3 |
| `show <name>` | Surface card + numbered commands |
| `search <query>` | Local search over name, description, tags, commands |
| `tag <name> +a -b …` | Add tags with `+`, remove with `-`; a bare tag means add |
| `rename <from> <to>` · `clone <from> <to>` · `delete <name>` (confirms) · `edit <name>` (`$EDITOR`, re-validate on save; invalid save keeps the original and reports the error) | Local management |
| `parameterize <name> [--literal TEXT --as NAME …]` | Rank repeated literals, whole-word replacement to `{{VAR}}`, preview + confirmation |
| `import <path\|dir> [--from-forgenesis] [--force]` | YAML file(s); collisions reported, never silently overwritten; Forgenesis conversion per snippet-format §12 |
| `export <name> [-o file]` | YAML to stdout or file |
| `analyze [--min-count n] [--limit n] [--max-len n] [--dry] [-y] [--file path]` | Mine bash/zsh/fish history: session grouping by time gaps, non-overlapping counts, fragment/rotation suppression, secrets scrubbed **before** anything is written |
| `stats [--limit n] [--explain]` | From `history.db`; saved = (commands × `human_overhead_seconds`) − actual runtime; interrupted runs excluded |
| `backup [name] [-o file]` / `restore [archive] [--force] [--config]` | Timestamped `.tar` archives (SHA-256 manifest, verified before any write) in `~/.forgehub/backups/`; restore defaults to the newest archive, confirms, and never overwrites without confirmation. Never credentials: archives hold config + snippets only |
| `doctor [topic]` | Home perms, config validity, shell, alias presence, workflow parse, ecosystem topics, (Step 11+) registry reachability, token validity, credential perms, clock skew. Non-zero exit on real breakage |
| `alias install\|uninstall\|show [--shell bash\|zsh\|fish] [-y]` | Idempotent marked block in the rc file (`alias fg='forgehub'` + `alias jobs-fg='builtin fg'`; fish: a `fg` function + `jobs-fg`); backs up the rc file first; asks unless `-y` |
| `completions <bash\|zsh\|fish\|powershell\|elvish>` | Prints the registration snippet for `clap_complete`'s dynamic env protocol (`COMPLETE=<shell> forgehub`); completion of local workflow names is dynamic, served by the binary from the live library |
| `config` | **Not a command in v1** (see forgenesis-analysis §5). Edit `config.yaml` directly |

## 5. Commands - cloud (Steps 11–13, 18)

| Command | Behaviour |
|---------|-----------|
| `login [--registry url] [--token]` | Device-code flow (openapi: device group): print verification URL + user code in a card, try to open a browser, poll honouring `interval`/`slow_down`, store the token. `--token` reads a token from **stdin only** |
| `logout [--revoke]` | Delete local credentials; `--revoke` also revokes server-side |
| `whoami` | Card or `--json` (§3) |
| `push <name> [--public\|--unlisted\|--private] [-m msg] [--as remote-name] [--all]` | Local validate + secret scan first (refuse on findings); create or new version with `If-Match`; prints `owner/name vN sha256:xxxxxxxx` + URL; identical hash prints "up to date". Default visibility: private for a new snippet, unchanged for an existing one |
| `pull <owner/name\|name> [--as local] [--version N] [--force]` | Fetch, preview (untrusted text, escaped per §8), validate, save locally with a `.meta` provenance sidecar (`{registry, owner, name, version, hash}`); never overwrites without `--force` or confirmation |
| `status [name]` | Local vs cloud by hash/version: `ahead`, `behind`, `diverged`, `unchanged`, `local-only`, `remote-only` |
| `diff <name>` | Unified diff, local vs cloud latest |
| `remote list [--mine\|--stars]` · `remote show <owner/name>` · `remote delete <name>` (confirms) · `remote visibility <name> <v>` (confirms when going public, with a review reminder) · `remote search <q>` | Cloud management |
| `fork <owner/name> [--as name]` | Server fork (Step 10 semantics), then optionally pull |
| `trust list` · `trust revoke <owner/name>` · `trust <owner/name@hash>` | Manage `trust.db` (§7) |
| `cache list` · `cache clear` | Immutable version cache in `~/.forgehub/cache`, size-capped with LRU eviction (config `cache_max_mb`, default 100) |
| `ui` | TUI (Step 18): Local · Mine · Explore tabs; running from the TUI goes through exactly §7–§8 |
| `update --check` | Report the latest known CLI version and the upgrade command; never self-replaces |

## 6. Name resolution for `forgehub <target>`

Target forms: `name`, `owner/name`, either with `@<version>` (cloud only).

Order (first match wins, and the match source is always shown in the confirm card):
1. **Built-in subcommand** - if the first argument is a subcommand name, it is never treated as a snippet. (`forgehub run <name>` bypasses this for a snippet whose name collides - impossible for cloud names, which are reserved-word-checked, but possible for legacy local files; `run` is the escape hatch.)
2. **Local workflow** - `~/.forgehub/snippets/<name>` (project `.forgenesis/` shadowing is a dropped Forgenesis feature and does not apply).
3. **Own cloud snippet** - if logged in, `me/<name>` at the latest version (or `@N`).
4. **Explicit `owner/name`** - that owner's snippet (only reachable in this explicit form for other people's snippets).

If nothing matches: exit 3, print "not found" plus up to 3 suggestions (similar local names by edit distance; top `remote search` hits when logged in and online). A suggestion is **never** auto-run.

Offline (`--offline` or network failure): steps 3–4 resolve only from the cache, and the card says "from cache, may be outdated" with the cached version and hash.

## 7. Confirmation and trust (normative; Step 13 implements)

Before any cloud snippet runs, and before any local snippet runs unless `-y`
is given, the CLI prints the confirm card (§8) and prompts.

| Case | Prompt | `-y` effect |
|------|--------|-------------|
| Local workflow | `Run this on this machine? [y/N]` | Skips |
| Own cloud snippet | `[y/N]` | Skips |
| Third-party snippet, first time at this hash | Type `yes` in full | **Does not skip** |
| Any snippet whose hash changed since last trusted run | Diff shown first (§8), then the prompt for its case above | **Does not skip** |
| Third-party snippet, previously trusted at this exact `(owner, name, hash)` | `[y/N]` | Skips |

Trust is recorded in `trust.db` as `(owner, name, hash, first_trusted_at, last_run_at)` on an explicit `yes`/run confirmation - never silently. `forgehub trust <owner/name@hash>` records trust without running. Anything other than the exact expected answer is a **No**: exit 2, nothing runs.

**Fail-safe without a TTY:** if stdin is not a TTY and the case above does not permit `-y` skipping (or `-y` was not given), the CLI prints the card, does **not** run, and exits 2.

## 8. Display safety for untrusted text

Snippet bodies are untrusted text. Everywhere the CLI displays them (confirm
card, `show`, `diff`, `pull` preview, TUI, `remote show`):

- Lines are numbered and wrapped, never truncated.
- ANSI/CSI/OSC escape sequences, carriage returns, backspaces, and other C0/C1 control characters (except `\n`, `\t` shown as spaces to the tab stop) are rendered visibly (e.g. `␛[31m`, `␍`) - a snippet must not be able to hide or rewrite a command line on screen.
- Unicode bidirectional controls (U+202A–U+202E, U+2066–U+2069) and zero-width characters are rendered visibly in the same way.
- The card header always shows: `owner/name`, version, `sha256:<short>`, visibility, mode, shell, description, and lint warnings (sudo, `rm -rf`, `curl|sh`, … from `forge-core` - warnings, never blocks).

Reference card (roadmap §4):

```text
╭─ alice/ubuntu-setup · v3 · sha256:9f2c1a7e ───────────────────────────╮
│ Update an Ubuntu machine and install the basics                       │
│ owner: you · visibility: private · mode: script · shell: bash         │
╰───────────────────────────────────────────────────────────────────────╯
   1 │ sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux ca-certificates

  ! uses sudo (will ask for your password)
Run this on this machine? [y/N]
```

## 9. Execution semantics

- Steps mode: each step in its own shell (`<shell> -c <command>`), honouring condition, `wait`, `retry`, `timeout`, `env`, `cwd`, `background`, `quiet`, `continue_on_error`, then `always` steps. First failure stops the run unless `-k` or the step sets `continue_on_error`.
- Script mode: per ADR 0004 - written to a file with mode `0700` inside a private (`0700`) temp directory, executed with the declared shell, deleted on every exit path including signals.
- `--dry`: print the numbered commands (variables substituted where values are known) and exit 0 without running or prompting.
- `--step`: confirm before each step in addition to §7.
- Run summary on completion: exit status of the run, duration, and (for failures) which step failed. Runs are recorded in `history.db` (interrupted runs marked as such).
- Startup budgets (agent.md §9): `--version` ≤ 20 ms; local run to first prompt ≤ 50 ms. Cloud commands may start a minimal async runtime; local commands must not.
