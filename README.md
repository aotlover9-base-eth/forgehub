![ForgeHub - save a command once, run it anywhere](docs/banner.jpg)

<div align="center">

# ForgeHub

**Save a shell command once. Run it anywhere.** ⚒️

![Release](https://img.shields.io/badge/release-v0.1.6-FFFFFF?style=flat-square&labelColor=0D1117)
![Tests](https://img.shields.io/badge/tests-617%20passing-2EA043?style=flat-square&labelColor=0D1117)
![License](https://img.shields.io/badge/license-MIT-9E9E9E?style=flat-square&labelColor=0D1117)
![Platform](https://img.shields.io/badge/platform-Linux-6E7F8D?style=flat-square&labelColor=0D1117)

![Rust](https://img.shields.io/badge/Rust-DEA584?style=flat-square&logo=rust&logoColor=black&labelColor=0D1117)
![axum](https://img.shields.io/badge/axum-0.8-5B8DEF?style=flat-square&labelColor=0D1117)
![React](https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black&labelColor=0D1117)
![TypeScript](https://img.shields.io/badge/TypeScript-5.9-3178C6?style=flat-square&logo=typescript&logoColor=white&labelColor=0D1117)
![SQLite](https://img.shields.io/badge/SQLite-003B57?style=flat-square&logo=sqlite&logoColor=white&labelColor=0D1117)
![Docker](https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=0D1117)

</div>

You know that one perfect command - the one with six flags you
can never quite remember? ForgeHub is its home. 🏠 Save it once
in the web app or the terminal, and it is one short `fg` away on
every machine you own - with the full commands, owner, version
and hash shown **before** anything executes. No more digging
through shell history, no more "it worked on my old laptop".

## ⚡ Overview

- **Total cost: $0** - self-hosted, one `docker compose up` and
  the whole thing (server + web app) is yours
- **One command, every machine** - `fg ubuntu-setup` fetches,
  shows, asks, runs. That is the whole trick, done properly
- **Safety is the product** - other people's snippets need a
  typed `yes`, a changed snippet shows a diff and asks again,
  and nothing runs silently, ever
- **Runbooks, not just snippets** - group snippets into
  ordered collections and run the whole set with one command,
  each step confirmed in turn
- **Tested like it matters** - 535 Rust + 82 web tests, CI on
  every push, secret scanning on every save
- **Featherweight** - the CLI binary is about 10 MB, answers
  `--version` in about 5 ms, the server is a single SQLite
  file, and snippets are text only (≤ 64 KiB - no uploads,
  no binaries)

Built with Rust (CLI + axum/SQLite server) and a React 19 web app.

**Jump to:** [Quickstart](#-quickstart---server-docker) ·
[CLI](#%EF%B8%8F-quickstart---cli) · [Screenshots](#-screenshots) ·
[Commands](#-commands) · [Safety](#%EF%B8%8F-safety-rules-enforced-not-optional) ·
[Configuration](#%EF%B8%8F-configuration) · [Docs](#-documentation)

![ForgeHub landing page](docs/screenshots/landing.jpg)

## ✨ What is inside

- **Snippets with versions** - every push is a new immutable
  version; `fg history` shows them and `fg rollback` republishes
  an old version's content as a new one (history is never
  rewritten)
- **Typed parameters** - declare `{{name:type}}` prompts
  (text, choice, number, path, yes/no, secret); secrets are
  asked for hidden and never shown in the run plan
- **Collections** - ordered runbooks in the CLI and on the web;
  run one end to end, or share it with an expiring, revocable
  read-only link
- **Public pages** - every public snippet gets a shareable,
  read-only page at `/s/<owner>/<name>` that works without
  JavaScript; private snippets answer 404 there, always
- **Imports** - mine your shell history for repeated commands,
  or import a GitHub Gist; likely secrets are filtered before
  anything is stored
- **Activity and stats that are yours** - a per-account
  activity log (sign-ins, pushes, forks, token changes, trust
  grants; exportable as JSON) and usage stats counted per
  runner, never per owner
- **A CLI that respects you** - fuzzy `fg find`, `fg lint`
  before you run, `fg config` for settings, grouped help,
  dynamic completions, network retries that honour
  `Retry-After`, and `forgehub self-check` to verify your
  binary against the published checksums
- **Internationalisation groundwork** - every web string
  lives in one message catalogue (about 430 strings), ready
  for the first real translation; English ships today

## 🚀 Quickstart - server (Docker)

```console
git clone https://github.com/aotlover9-base-eth/forgehub.git
cd forgehub
cp .env.example .env        # set FORGEHUB_SECRET_KEY and FORGEHUB_DOMAIN
docker compose up -d --build
```

Open `https://<FORGEHUB_DOMAIN>` (default `https://localhost`),
create an account. Details: [docs/deploy.md](docs/deploy.md).

## ⌨️ Quickstart - CLI

```console
cargo install --path crates/forge-cli   # from the clone; toolchain pinned in rust-toolchain.toml
forgehub alias install                  # adds the `fg` alias
fg init
fg record hello             # capture shell: run `echo hello`, then `end`
fg hello                    # shows the commands, asks [y/N], runs
```

Connect it to your server:

```console
export FORGEHUB_REGISTRY=https://localhost   # your server's URL
fg login                    # device-code flow: approve in the browser
fg push hello --public      # publish a local snippet
fg run alice/docker-setup   # run anyone's public snippet (trust flow below)
```

Prebuilt binaries (Linux x86_64 and aarch64 tarballs) are on
the [Releases page](https://github.com/aotlover9-base-eth/forgehub/releases).
ForgeHub is Linux-only - on other systems, use the web app in
a browser or build the CLI from source. This repository is
private, so download signed in - in the browser, or with
`gh release download` - and verify against the release's
`SHA256SUMS`. (`scripts/install.sh` uses unauthenticated
downloads and only works once a repository is public.)

## 📸 Screenshots

The design is the "Observe" look: one dark theme,
monochrome glass over a drifting smoke backdrop, white as
the only accent. The landing page is a standalone glass
scene with an Instrument Serif headline; the signed-in app
is Roboto Flex. Every capture below is the real app, taken
from a local server with a seeded demo account.

| Landing | Library |
|---|---|
| ![Landing](docs/screenshots/landing.jpg) | ![Library](docs/screenshots/library.jpg) |

| Home | A snippet |
|---|---|
| ![Home](docs/screenshots/home.jpg) | ![Snippet page](docs/screenshots/snippet.jpg) |

| A collection | Explore |
|---|---|
| ![Collection page](docs/screenshots/collection.jpg) | ![Explore](docs/screenshots/explore.jpg) |

| New snippet editor | Profile |
|---|---|
| ![Editor](docs/screenshots/editor.jpg) | ![Profile](docs/screenshots/profile.jpg) |

Settings and the public snippet page (server-rendered, no
JavaScript):

| Settings | Public page |
|---|---|
| ![Settings](docs/screenshots/settings.jpg) | ![Public page](docs/screenshots/public-page.jpg) |

## 🎨 Design

- **One dark theme** - monochrome "liquid glass" surfaces
  over a fixed smoke backdrop; white is the only accent
  (the active nav pill, filled buttons), error red kept.
  There is no theme switch: the design is the theme
- **Instrument Serif on the landing page only**; the
  signed-in app is Roboto Flex throughout. Both fonts are
  self-hosted like every other asset - no CDNs, no
  third-party fonts or scripts
- **Colour comes from tokens, never hard-coded** - the
  `.observe-app` scope re-points the Material colour roles
  to the monochrome glass values, so every screen,
  including the Peak-era collections, imports and activity
  screens, wears the same design
- **Responsive shell** - navigation rail on desktop, pinned
  glass bar on small screens; entrance and route
  animations are reduced-motion safe; WCAG 2.2 AA checked
  by an axe sweep over every route, and a 36-screenshot
  visual regression suite guards the look
  (`scripts/visual-web.sh`)

## 🧰 Commands

**Local** (offline, in `~/.forgehub`):

| Command | Does |
|---|---|
| `fg <name>` / `fg run <name>` | Show a snippet's commands, ask, run it |
| `fg record <name>` | Record a shell session as a snippet |
| `fg new` | Create from a template in `$EDITOR`; validation errors loop back in |
| `fg list` · `fg show <name>` · `fg search <text>` | Browse the local library |
| `fg find <text>` | Fuzzy-find by name across the local library and cached cloud names |
| `fg lint <name>` | Danger lints + secret scan without running anything (exit 0 clean, 1 warnings, 2 blocking) |
| `fg edit` · `fg rename` · `fg delete` · `fg tag` · `fg clone` | Manage snippets |
| `fg parameterize <name>` | Turn repeated literals into prompted variables |
| `fg import` · `fg export` | Move snippets as YAML (Forgenesis files included) |
| `fg analyze` · `fg stats` | Mine shell history; show run statistics |
| `fg backup` · `fg restore` | Archive / restore the whole library |

**Cloud** (needs `fg login`):

| Command | Does |
|---|---|
| `fg login` · `fg logout` · `fg whoami` | Browser device-code login; session info |
| `fg push <name> [--public]` | Upload; re-push creates a new version |
| `fg pull <name>` | Download a cloud snippet into the local library |
| `fg status` · `fg diff <name>` | Local vs cloud: ahead / behind / diverged |
| `fg history <name>` · `fg rollback <name>` | Version history; republish an old version as a new one |
| `fg fork <owner>/<name>` | Copy a public snippet into your account |
| `fg edit <owner>/<name>` | Edit your cloud snippet; pushes the result as a new version after confirmation |
| `fg collection` | Ordered runbooks: create, add, reorder, share, run end to end |
| `fg remote list` | List your snippets on the server |
| `fg trust list` · `fg trust revoke <owner>/<name>` | Manage run approvals |
| `fg activity` | Your account activity log (also in the web app, exportable) |

**Utility:** `fg ui` (terminal browser) · `fg doctor` (health
check) · `fg config` (read/write local settings) ·
`fg cache list|clear|prune` · `fg update --check` ·
`forgehub self-check` (verify the binary against the
published checksums) · `fg completions <shell>` ·
`fg alias show` · `fg help <topic>`

**Flags that matter:** `--json` (machine-readable output) ·
`-y` (skip `[y/N]` on your own snippets - never skips trust
prompts) · `--offline` (run from cache, cloud fails fast) ·
`--color auto|always|never` and `NO_COLOR` · `-v` (request
log).

Full reference: [docs/cli-reference.md](docs/cli-reference.md) ·
[docs/user-guide.md](docs/user-guide.md)

## 🛡️ Safety rules (enforced, not optional)

- Nothing runs unseen: every run prints the full command list first.
- Your snippet: `[y/N]`. Someone else's, first time: type `yes`.
- Approval is pinned to the content hash. Changed content: you
  see a diff and approve again. `-y` never skips this.
- No interactive terminal: unskippable prompts fail closed (exit 2).
- A secret scanner blocks likely credentials at record, push and
  web save. Findings name the kind and line, never the value.
- Passwords argon2id-hashed; tokens SHA-256 at rest, shown once.
  No telemetry. No third-party scripts or fonts.
- One bounded exception, documented in ADR 0011: collection
  share links are read-only bearer grants - expiring,
  revocable, and they never grant running anything.

Detail: [docs/security.md](docs/security.md) ·
[docs/threat-model.md](docs/threat-model.md)

## ⚙️ Configuration

| Variable | Meaning |
|---|---|
| `FORGEHUB_REGISTRY` | CLI: server URL (or `--registry`, or `fg config`) |
| `FORGEHUB_HOME` | CLI: local library location (default `~/.forgehub`) |
| `FORGEHUB_TOKEN` | CLI: token from the environment, for scripts |
| `FORGEHUB_CREDENTIAL_STORE` | CLI: `auto` (default) / `file` / `keyring` - OS keyring vs credentials file |
| `FORGEHUB_SECRET_KEY` | Server: session secret, ≥ 32 random bytes (`.env`) |
| `FORGEHUB_DOMAIN` | Server (compose/Caddy): domain to serve, default `localhost` |
| `FORGEHUB_ALLOW_SIGNUP` | Server: allow public account creation (`true`/`false`) |
| `FORGEHUB_BASE_URL` | Server: public base URL, used for links and public-page metadata |
| `FORGEHUB_GIST_API_BASE` | Server: GitHub API base for Gist import (default `https://api.github.com`; point it somewhere unreachable to disable outbound fetches) |
| `FORGEHUB_SMTP_URL` | Server: SMTP URL for password-reset mail, e.g. `smtps://user:pass@host:465` |
| `FORGEHUB_MAIL_FROM` | Server: From address for outgoing mail |

## 📚 Documentation

[User guide](docs/user-guide.md) · [CLI reference](docs/cli-reference.md) ·
[API](docs/api.md) · [OpenAPI](docs/openapi.yaml) ·
[Self-hosting](docs/self-hosting.md) · [Deploy](docs/deploy.md) ·
[Operations](docs/operations.md) · [FAQ](docs/faq.md) ·
[Known gaps](docs/known-gaps.md) · [Forgenesis migration](docs/forgenesis-migration.md) ·
[Security policy](SECURITY.md) · [Contributing](CONTRIBUTING.md) ·
[Changelog](CHANGELOG.md) · [Demo data pack](demo-data/) - 105 starter snippets + loader

## 🔧 Development

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run lint && npm run typecheck && npm test && npm run build
```

History: 22 build steps, then the Peak phase (29 work
packages: UI overhaul, CLI overhaul, new features) - one
commit + report each - [PROGRESS.md](PROGRESS.md),
[reports/](reports/).

## License

MIT - [LICENSE](LICENSE)
