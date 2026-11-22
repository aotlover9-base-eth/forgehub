# CLI reference

Generated from the binary's own `--help` output (Step 22); the normative contract is `docs/spec-cli.md`. Global flags: `--json` (machine-readable output with stable keys), `-y/--yes` (answer confirmations where the trust model allows it), `--offline` (refuse all network use), `-v/--verbose` (request logging to stderr), `--color auto|always|never` (Peak B1: `auto` colours only on a capable terminal and honours `NO_COLOR`; non-TTY output is always plain).

Help layout (Peak B2): `forgehub --help` groups commands under Library, Cloud, Safety and Tools. Concept explainers live behind `fg help <topic>`: `alias`, `trust`, `hash-pinning`, `yes`, `offline`. Shell completions complete local snippet names and cached `owner/name` references for name- and reference-taking arguments.

Listing (Peak B4): `fg list` and `fg search` share `--tag`, `--visibility` (last-synced visibility from the snippet's `.meta` sidecar), `--sort name|recent`, `--since 30m|24h|7d|2w` and `--long` (aligned table). `fg find <query>` fuzzy-matches local and cached cloud names, ranked; `--plain` prints bare names for piping (e.g. into fzf), `--limit` caps the results. Push and pull record the snippet's visibility in the sidecar so the filter works offline. Exit codes: 0 ok · 1 failure · 2 refused/not confirmed · 3 not found · 4 auth required · 130 interrupted.

## Commands at a glance

- `forgehub init` - Create the `ForgeHub` home and config
- `forgehub record` - Record a shell session as a snippet
- `forgehub run` - Run a snippet by name
- `forgehub list` - List local snippets
- `forgehub show` - Show a snippet (card + numbered commands)
- `forgehub search` - Search local snippets (name, description, tags, commands)
- `forgehub tag` - Add or remove tags: fg tag <name> add|remove <tag>…
- `forgehub rename` - Rename a local snippet
- `forgehub delete` - Delete a local snippet
- `forgehub edit` - Edit a snippet in $EDITOR; invalid edits are rolled back
- `forgehub parameterize` - Turn repeated literals into variables
- `forgehub clone` - Duplicate a local snippet under a new name
- `forgehub import` - Import snippet file(s) (`ForgeHub` or Forgenesis format)
- `forgehub export` - Export a snippet as YAML (to stdout or a file)
- `forgehub analyze` - Mine shell history for repeated command sequences
- `forgehub stats` - Show run statistics from history.db
- `forgehub backup` - Write a backup archive (default: backups/forgehub-backup-<timestamp>.tar)
- `forgehub restore` - Restore snippets from a backup archive
- `forgehub login` - Log in to a `ForgeHub` server (device-code flow in the browser)
- `forgehub logout` - Log out: delete the stored credential for a registry
- `forgehub whoami` - Show the current login (username, registry, token, scopes)
- `forgehub push` - Push a local snippet to the cloud (create or new version)
- `forgehub pull` - Pull a cloud snippet into the local library
- `forgehub status` - Compare local snippets with the cloud (ahead/behind/diverged)
- `forgehub diff` - Unified diff of a local snippet against the cloud latest
- `forgehub fork` - Fork someone's snippet into your account (server-side)
- `forgehub remote` - Inspect and manage your cloud snippets
- `forgehub trust` - Manage execution trust for cloud snippets (Step 13): `trust list`, `trust revoke <owner/name>`, or approve an exact snippet directly: `trust <owner/name@hash>`
- `forgehub doctor` - Check the environment and library health
- `forgehub alias` - Manage the `fg` shell alias (install|uninstall|show)
- `forgehub completions` - Print shell completions (dynamic: local snippet names included)
- `forgehub ui` - Browse snippets in a terminal UI (Local · Mine · Explore)
- `forgehub cache` - Inspect and manage the cloud run cache
- `forgehub update` - Check whether a newer `ForgeHub` is available (never downloads or replaces anything)

## Per-command help

### `forgehub init`

```text
Create the `ForgeHub` home and config

Usage: forgehub init [OPTIONS]

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg init
```

### `forgehub record`

```text
Record a shell session as a snippet

Usage: forgehub record [OPTIONS] [NAME]

Arguments:
  [NAME]  Name for the snippet (prompted when omitted)

Options:
      --description <DESCRIPTION>  Short description
      --json                       Print machine-readable JSON instead of human output
      --tags <TAGS>                Comma-separated tags
  -y, --yes                        Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
      --force                      Overwrite an existing snippet with the same name
  -v, --verbose...                 Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline                    Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
      --push                       After saving, offer to push to the cloud (when logged in)
  -h, --help                       Print help

Examples:
  fg record ubuntu-setup --description 'Base packages' --tags ubuntu,setup
```

### `forgehub run`

```text
Run a snippet by name

Usage: forgehub run [OPTIONS] <NAME> [KEY=value]...

Arguments:
  <NAME>          Snippet name, or `owner/name` / `name@version` for a cloud snippet (Step 13)
  [KEY=value]...  KEY=value variable assignments

Options:
      --dry-run     Print the plan and execute nothing (alias: --dry)
      --json        Print machine-readable JSON instead of human output
      --step        Confirm each step before running it (steps mode)
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
      --no-cache    Do not write fetched snippets to the run cache
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Run from the local cache only; never fetch
  -h, --help        Print help

Examples:
  fg run ubuntu-setup
  fg ubuntu-setup            (shorthand)
  fg alice/deploy --dry-run
```

### `forgehub list`

```text
List local snippets

Usage: forgehub list [OPTIONS]

Options:
      --json        Print machine-readable JSON instead of human output
      --tag <TAG>   Only show snippets with this tag
      --long        Show mode, shell and hash for each snippet
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg list --long
  fg list --tag ubuntu
```

### `forgehub show`

```text
Show a snippet (card + numbered commands)

Usage: forgehub show [OPTIONS] <NAME>

Arguments:
  <NAME>  Snippet name

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg show ubuntu-setup
```

### `forgehub search`

```text
Search local snippets (name, description, tags, commands)

Usage: forgehub search [OPTIONS] <QUERY>

Arguments:
  <QUERY>  Search text

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg search docker
```

### `forgehub tag`

```text
Add or remove tags: fg tag <name> add|remove <tag>…

Usage: forgehub tag [OPTIONS] <NAME> <ACTION> [TAGS]...

Arguments:
  <NAME>     Snippet name
  <ACTION>   `add` or `remove`
  [TAGS]...  Tags to add or remove

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg tag ubuntu-setup add ubuntu base
```

### `forgehub rename`

```text
Rename a local snippet

Usage: forgehub rename [OPTIONS] <OLD> <NEW>

Arguments:
  <OLD>  Current name
  <NEW>  New name

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg rename old-name new-name
```

### `forgehub delete`

```text
Delete a local snippet

Usage: forgehub delete [OPTIONS] <NAME>

Arguments:
  <NAME>  Snippet name

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg delete ubuntu-setup
```

### `forgehub edit`

```text
Edit a snippet in $EDITOR; invalid edits are rolled back

Usage: forgehub edit [OPTIONS] <NAME>

Arguments:
  <NAME>  Snippet name

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg edit ubuntu-setup
```

### `forgehub parameterize`

```text
Turn repeated literals into variables

Usage: forgehub parameterize [OPTIONS] <NAME>

Arguments:
  <NAME>  Snippet name

Options:
      --json            Print machine-readable JSON instead of human output
      --literal <TEXT>  A literal to parameterize (repeat with --as; pairs apply directly)
      --as <NAME>       Variable name for the preceding --literal
  -y, --yes             Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...      Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline         Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help            Print help

Examples:
  fg parameterize deploy --literal eu-west-1 --as region
```

### `forgehub clone`

```text
Duplicate a local snippet under a new name

Usage: forgehub clone [OPTIONS] <NAME> <NEW>

Arguments:
  <NAME>  Existing name
  <NEW>   New name

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg clone ubuntu-setup laptop-setup
```

### `forgehub import`

```text
Import snippet file(s) (`ForgeHub` or Forgenesis format)

Usage: forgehub import [OPTIONS] [FILE]

Arguments:
  [FILE]  A snippet file, or a directory of snippet files. Optional with --from-forgenesis

Options:
      --from-forgenesis  Import from the Forgenesis workflows directory instead ($`FORGENESIS_HOME/workflows` or ~/.forgenesis/workflows)
      --json             Print machine-readable JSON instead of human output
      --name <NAME>      Save under a different name (single file only)
  -y, --yes              Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
      --force            Overwrite existing snippets with the same name
  -v, --verbose...       Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline          Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help             Print help

Examples:
  fg import snippets.yaml
  fg import --from-forgenesis
```

### `forgehub export`

```text
Export a snippet as YAML (to stdout or a file)

Usage: forgehub export [OPTIONS] <NAME>

Arguments:
  <NAME>  Snippet name

Options:
      --json        Print machine-readable JSON instead of human output
  -o, --out <OUT>   Write to this file instead of stdout
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg export ubuntu-setup --out ubuntu-setup.yaml
```

### `forgehub analyze`

```text
Mine shell history for repeated command sequences

Usage: forgehub analyze [OPTIONS]

Options:
      --json                   Print machine-readable JSON instead of human output
      --min-count <MIN_COUNT>  Minimum repetitions before a sequence is reported
      --limit <LIMIT>          Maximum number of candidates to show [default: 10]
  -y, --yes                    Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
      --max-len <MAX_LEN>      Longest command sequence to look for
  -v, --verbose...             Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --dry                    Print candidates only; create nothing
      --offline                Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
      --file <FILE>            Read this history file instead of auto-detecting one
  -h, --help                   Print help

Examples:
  fg analyze --dry
```

### `forgehub stats`

```text
Show run statistics from history.db

Usage: forgehub stats [OPTIONS]

Options:
      --json           Print machine-readable JSON instead of human output
      --limit <LIMIT>  Maximum number of snippets in the ranking [default: 10]
      --explain        Explain the saved-time formula
  -y, --yes            Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...     Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline        Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help           Print help

Examples:
  fg stats
```

### `forgehub backup`

```text
Write a backup archive (default: backups/forgehub-backup-<timestamp>.tar)

Usage: forgehub backup [OPTIONS] [NAME]

Arguments:
  [NAME]  Back up only this snippet (plus the config)

Options:
      --json        Print machine-readable JSON instead of human output
  -o, --out <OUT>   Output file instead of the default location
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg backup
  fg backup --out ~/forgehub-backup.tar
```

### `forgehub restore`

```text
Restore snippets from a backup archive

Usage: forgehub restore [OPTIONS] [FILE]

Arguments:
  [FILE]  Backup file (default: the newest in the backups directory)

Options:
      --config      Also restore config.yaml from the archive
      --json        Print machine-readable JSON instead of human output
      --force       Overwrite without asking (same as -y)
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg restore ~/forgehub-backup.tar
```

### `forgehub login`

```text
Log in to a `ForgeHub` server (device-code flow in the browser)

Usage: forgehub login [OPTIONS]

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
      --token                Read an API token from stdin instead of the device flow (for CI)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg login
  fg login --registry https://forgehub.example.com
```

### `forgehub logout`

```text
Log out: delete the stored credential for a registry

Usage: forgehub logout [OPTIONS]

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
      --revoke               Also revoke the token server-side
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg logout --revoke
```

### `forgehub whoami`

```text
Show the current login (username, registry, token, scopes)

Usage: forgehub whoami [OPTIONS]

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg whoami
```

### `forgehub push`

```text
Push a local snippet to the cloud (create or new version)

Usage: forgehub push [OPTIONS] [NAME]

Arguments:
  [NAME]  Local snippet name (omit with --all)

Options:
      --all                  Push every local snippet and print a summary
      --json                 Print machine-readable JSON instead of human output
      --public               Make the snippet public
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
      --unlisted             Make the snippet unlisted
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
      --private              Make the snippet private (the default for new snippets)
  -m, --message <MESSAGE>    Version message for the push
      --as <AS_NAME>         Push under a different remote name
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -h, --help                 Print help

Examples:
  fg push ubuntu-setup --public
  fg push --all
```

### `forgehub pull`

```text
Pull a cloud snippet into the local library

Usage: forgehub pull [OPTIONS] <REFERENCE>

Arguments:
  <REFERENCE>  `owner/name`, or a bare name for your own snippet

Options:
      --as <AS_NAME>         Save under a different local name
      --json                 Print machine-readable JSON instead of human output
      --version <VERSION>    Pull a specific version instead of the latest
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
      --force                Overwrite an existing local snippet without asking
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -h, --help                 Print help

Examples:
  fg pull alice/deploy
  fg pull ubuntu-setup --force
```

### `forgehub status`

```text
Compare local snippets with the cloud (ahead/behind/diverged)

Usage: forgehub status [OPTIONS] [NAME]

Arguments:
  [NAME]  Only this snippet (default: everything tracked)

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg status
  fg status ubuntu-setup
```

### `forgehub diff`

```text
Unified diff of a local snippet against the cloud latest

Usage: forgehub diff [OPTIONS] <NAME>

Arguments:
  <NAME>  Local snippet name

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg diff ubuntu-setup
```

### `forgehub fork`

```text
Fork someone's snippet into your account (server-side)

Usage: forgehub fork [OPTIONS] <REFERENCE>

Arguments:
  <REFERENCE>  `owner/name` of the snippet to fork

Options:
      --as <AS_NAME>         Name for the fork (default: the source name)
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg fork alice/deploy --as my-deploy
```

### `forgehub remote`

```text
Inspect and manage your cloud snippets

Usage: forgehub remote [OPTIONS] <COMMAND>

Commands:
  list        List your cloud snippets (or your stars with --stars)
  show        Show one cloud snippet in full
  delete      Delete one of your cloud snippets (soft-delete, 30-day undo)
  visibility  Change a snippet's visibility: public|unlisted|private
  search      Search public snippets on the registry
  help        Print this message or the help of the given subcommand(s)

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg remote list
  fg remote search docker
  fg remote list --stars
```

#### `forgehub remote list`

```text
List your cloud snippets (or your stars with --stars)

Usage: forgehub remote list [OPTIONS]

Options:
      --json                 Print machine-readable JSON instead of human output
      --stars                List snippets you have starred instead
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help
```

#### `forgehub remote show`

```text
Show one cloud snippet in full

Usage: forgehub remote show [OPTIONS] <REFERENCE>

Arguments:
  <REFERENCE>  `owner/name`, or a bare name for your own snippet

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help
```

#### `forgehub remote delete`

```text
Delete one of your cloud snippets (soft-delete, 30-day undo)

Usage: forgehub remote delete [OPTIONS] <NAME>

Arguments:
  <NAME>  Snippet name

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help
```

#### `forgehub remote visibility`

```text
Change a snippet's visibility: public|unlisted|private

Usage: forgehub remote visibility [OPTIONS] <NAME> <VISIBILITY>

Arguments:
  <NAME>        Snippet name
  <VISIBILITY>  The new visibility

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help
```

#### `forgehub remote search`

```text
Search public snippets on the registry

Usage: forgehub remote search [OPTIONS] <QUERY>

Arguments:
  <QUERY>  Search text

Options:
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help
```

### `forgehub trust`

```text
Manage execution trust for cloud snippets (Step 13): `trust list`, `trust revoke <owner/name>`, or approve an exact snippet directly: `trust <owner/name@hash>`

Usage: forgehub trust [OPTIONS] [REFERENCE]
       forgehub trust <COMMAND>

Commands:
  list    List trusted cloud snippets
  revoke  Revoke trust for one snippet
  add     Approve an exact snippet without running it
  help    Print this message or the help of the given subcommand(s)

Arguments:
  [REFERENCE]  Shorthand for `trust add`: `owner/name@hash` or `owner/name@version`

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg trust list
  fg trust alice/deploy@3
```

#### `forgehub trust list`

```text
List trusted cloud snippets

Usage: forgehub trust list [OPTIONS]

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help
```

#### `forgehub trust revoke`

```text
Revoke trust for one snippet

Usage: forgehub trust revoke [OPTIONS] <REFERENCE>

Arguments:
  <REFERENCE>  `owner/name` whose trust should be removed

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help
```

#### `forgehub trust add`

```text
Approve an exact snippet without running it

Usage: forgehub trust add [OPTIONS] <REFERENCE>

Arguments:
  <REFERENCE>  `owner/name@hash` or `owner/name@version`

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help
```

### `forgehub doctor`

```text
Check the environment and library health

Usage: forgehub doctor [OPTIONS] [TOPIC]

Arguments:
  [TOPIC]  Only run checks for this topic (e.g. alias, rust, python)

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg doctor
  fg doctor alias
```

### `forgehub alias`

```text
Manage the `fg` shell alias (install|uninstall|show)

Usage: forgehub alias [OPTIONS] <ACTION>

Arguments:
  <ACTION>  install, uninstall or show

Options:
      --json           Print machine-readable JSON instead of human output
      --shell <SHELL>  Shell to manage (bash, zsh or fish; default: $SHELL or bash)
  -y, --yes            Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...     Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline        Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help           Print help

Examples:
  fg alias install
  fg alias show
```

### `forgehub completions`

```text
Print shell completions (dynamic: local snippet names included)

Usage: forgehub completions [OPTIONS] <SHELL>

Arguments:
  <SHELL>  Shell: bash, zsh, fish, powershell or elvish [possible values: bash, elvish, fish, powershell, zsh]

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg completions bash
```

### `forgehub ui`

```text
Browse snippets in a terminal UI (Local · Mine · Explore)

Usage: forgehub ui [OPTIONS]

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg ui
```

### `forgehub cache`

```text
Inspect and manage the cloud run cache

Usage: forgehub cache [OPTIONS] <COMMAND>

Commands:
  list   List cached cloud snippets with sizes
  clear  Remove every cached snippet
  help   Print this message or the help of the given subcommand(s)

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help

Examples:
  fg cache list
  fg cache clear
```

#### `forgehub cache list`

```text
List cached cloud snippets with sizes

Usage: forgehub cache list [OPTIONS]

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help
```

#### `forgehub cache clear`

```text
Remove every cached snippet

Usage: forgehub cache clear [OPTIONS]

Options:
      --json        Print machine-readable JSON instead of human output
  -y, --yes         Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...  Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline     Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help        Print help
```

### `forgehub update`

```text
Check whether a newer `ForgeHub` is available (never downloads or replaces anything)

Usage: forgehub update [OPTIONS]

Options:
      --check                Only check; there is no self-update mode
      --json                 Print machine-readable JSON instead of human output
      --registry <REGISTRY>  Registry base URL (default: `FORGEHUB_REGISTRY` or config)
  -y, --yes                  Skip confirmation prompts (never bypasses hash-change or third-party trust prompts; see spec-cli §7)
  -v, --verbose...           Verbose request logging to stderr (method, status, timing; tokens are never printed)
      --offline              Offline mode: cloud commands fail fast with an offline message; local commands and `run --offline` still work
  -h, --help                 Print help

Examples:
  fg update --check
```
