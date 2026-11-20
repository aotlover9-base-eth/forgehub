# User guide

ForgeHub has two faces: the **web app** (accounts, editing,
discovery) and the **CLI** (`forgehub`, alias `fg`) for
recording and running snippets. They share one account and one
library. The format both speak is the *forge file*
(`docs/spec-snippet-format.md`).

## Web

### Accounts

Sign up with a username (2–39 chars, lowercase letters,
digits, hyphens), an email, and a password of 10+ characters.
There is **no email verification and no password reset** in
v0.1.0 - no mail is ever sent (see `docs/known-gaps.md`); if
you lose your password on a self-hosted instance, the operator
is your recovery path.

Sessions last 7 days idle / 30 days absolute. **Settings →
Security** lists every session (device hints included) and
lets you revoke any of them, or sign out everywhere.

### Creating and editing snippets

**New snippet** (or `/:owner/:name/edit`): name (immutable
except via rename in the editor), description (≤ 280 chars),
visibility (**Private** by default; Public means anyone can
read, star, fork and run it), up to 20 tags, and the body -
**Script** mode (one shell script) or **Steps** mode (named
steps with per-step retry/timeout, written as YAML).

While you type, the editor validates against the server:
secret findings (kind + line) **block saving**; danger lints
(`sudo`, `rm -rf`, pipe-to-shell, …) warn but do not block.
Saving an edit creates a new **version** - versions are
immutable, and the Versions tab diffs any two of them.

### Sharing and discovery

Public snippets appear in **Explore** and full-text **Search**
(matches in name, description, tags and body are highlighted).
Profiles (`/:username`) list a user's public snippets. On
someone else's snippet you can **star** it, **fork** it into
your account (provenance is kept), or **report** it to the
operator. The snippet page shows the exact `fg` command to run
it, and a safety banner when it isn't yours.

### Tokens and device login

**Settings → Tokens** creates API tokens (`fgh_…`, scopes
read / read+write, optional expiry). A token is **shown once**;
store it in a password manager. The CLI normally doesn't need
one: `fg login` uses the **device flow** - the CLI shows a
short code, you open the server's `/device` page, sign in, and
approve. **Settings → CLI** explains both paths.

## CLI

Full reference: `docs/cli-reference.md`. The essentials:

### Local library

```console
fg init                     # create ~/.forgehub
fg record <name>            # guided capture (or record a live session)
fg <name>                   # run (shorthand for `fg run <name>`)
fg list / show / search     # browse the library
fg edit <name>              # $EDITOR, validated, rolled back if invalid
fg parameterize <name>      # turn repeated literals into {{variables}}
fg analyze                  # mine your shell history for repeat sequences
fg stats                    # time saved, most-run snippets
fg backup / restore         # checksummed tarball of the library
fg doctor                   # environment + library health checks
fg ui                       # terminal UI over Local · Mine · Explore
```

Snippets may declare `{{variables}}`; you are prompted for
values, or pass them: `fg deploy --env prod` style assignments
are positional `name=value` arguments to `run`.

### Cloud

```console
fg login                    # device-code flow (or --token, stdin only)
fg whoami                   # username, registry, scopes
fg push <name> [--public]   # create or publish a new version
fg pull <name>              # download into the local library
fg status                   # ahead / behind / diverged per snippet
fg diff <name>              # local vs cloud latest
fg fork alice/tool          # server-side fork into your account
fg remote list              # your cloud snippets (and starred, with flags)
fg remote search <text>     # search public snippets on the server
```

### Trust: running other people's snippets

```console
$ fg run alice/docker-setup
```

You always see the card first: owner, version, short hash,
visibility, lint warnings, and **every command, numbered**.
Then:

- **Your own** snippet: `[y/N]`; `-y` may skip.
- **Someone else's, first time**: type the word `yes`; `-y`
  never skips this.
- **Someone else's, unchanged** since you trusted that exact
  hash: `[y/N]`; `-y` may skip.
- **Changed** since you trusted it: the diff is shown and you
  confirm again; `-y` never skips a change.

`fg trust list` shows what you've approved (owner, name, hash,
runs); `fg trust revoke owner/name` forgets an approval.
Without an interactive terminal, unskippable confirmations
refuse with exit code 2 - scripts should run their own or
pre-trusted snippets.

### Offline

Local commands never need the network. Cloud commands fail
with a clear message when the server is unreachable and keep
your local library untouched; `--offline` makes the CLI refuse
network use up front. Previously run cloud snippets are cached
under `~/.forgehub/cache` (LRU, 100 MiB default cap;
`fg cache list|clear`), and an offline run can use the cache
only for a hash you already trusted.

## Migrating from Forgenesis

`fg import --from-forgenesis <dir>` converts a Forgenesis
library; see `docs/forgenesis-migration.md`.
