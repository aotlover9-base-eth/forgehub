![ForgeHub - save a command once, run it anywhere](docs/banner.jpg)

<div align="center">

# ForgeHub

**Save a shell command once. Run it anywhere.** ⚒️

![Release](https://img.shields.io/badge/release-v0.1.6-FFFFFF?style=flat-square&labelColor=0D1117)
![Tests](https://img.shields.io/badge/tests-478%20passing-2EA043?style=flat-square&labelColor=0D1117)
![License](https://img.shields.io/badge/license-MIT-9E9E9E?style=flat-square&labelColor=0D1117)
![Platform](https://img.shields.io/badge/platform-Linux-6E7F8D?style=flat-square&labelColor=0D1117)

![Rust](https://img.shields.io/badge/Rust-DEA584?style=flat-square&logo=rust&logoColor=black&labelColor=0D1117)
![axum](https://img.shields.io/badge/axum-0.8-5B8DEF?style=flat-square&labelColor=0D1117)
![React](https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black&labelColor=0D1117)
![TypeScript](https://img.shields.io/badge/TypeScript-5.9-3178C6?style=flat-square&logo=typescript&logoColor=white&labelColor=0D1117)
![SQLite](https://img.shields.io/badge/SQLite-003B57?style=flat-square&logo=sqlite&logoColor=white&labelColor=0D1117)
![Docker](https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white&labelColor=0D1117)

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
- **Tested like it matters** - 428 Rust + 50 web tests, CI on
  every push, secret scanning on every save
- **Featherweight** - the CLI download is ~4.5 MB, the server
  is a single SQLite file, the snippets are text only
  (≤ 64 KiB - no uploads, no binaries)

| | |
|---|---|
| Stack | Rust (CLI + axum/SQLite server) · React 19 web |
| Status | v0.1.4 · 478 tests green · MIT |

![ForgeHub home - the Observe theme](docs/screenshots/home-observe.jpg)

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
cargo install --path crates/forge-cli   # from the clone; Rust 1.x
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

One theme - dark glass - from the home page through every
signed-in section. (The serif display type lives on the
public landing page only.)

| Library | Explore |
|---|---|
| ![Library](docs/screenshots/library-observe.jpg) | ![Explore](docs/screenshots/explore-observe.jpg) |

| Profile | A snippet |
|---|---|
| ![Profile](docs/screenshots/profile-observe.jpg) | ![Snippet page](docs/screenshots/snippet-observe.jpg) |

## 🧰 Commands

**Local** (offline, in `~/.forgehub`):

| Command | Does |
|---|---|
| `fg <name>` / `fg run <name>` | Show a snippet's commands, ask, run it |
| `fg record <name>` | Record a shell session as a snippet |
| `fg list` · `fg show <name>` · `fg search <text>` | Browse the local library |
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
| `fg fork <owner>/<name>` | Copy a public snippet into your account |
| `fg remote list` | List your snippets on the server |
| `fg trust list` · `fg trust revoke <owner>/<name>` | Manage run approvals |

**Utility:** `fg ui` (terminal browser) · `fg doctor` (health
check) · `fg cache list|clear` · `fg update --check` ·
`fg completions <shell>` · `fg alias show`

**Flags that matter:** `--json` (list/show/stats/doctor) ·
`-y` (skip `[y/N]` on your own snippets - never skips trust
prompts) · `--offline` (run from cache) · `-v` (request log).

Full reference: [docs/cli-reference.md](docs/cli-reference.md) ·
[docs/user-guide.md](docs/user-guide.md)

## 🛡️ Safety rules (enforced, not optional)

- Nothing runs unseen: every run prints the full command list first.
- Your snippet: `[y/N]`. Someone else's, first time: type `yes`.
- Approval is pinned to the content hash. Changed content → you
  see a diff and approve again. `-y` never skips this.
- No interactive terminal → unskippable prompts fail closed (exit 2).
- A secret scanner blocks likely credentials at record, push and
  web save. Findings name the kind and line, never the value.
- Passwords argon2id-hashed; tokens SHA-256 at rest, shown once.
  No telemetry. No third-party scripts or fonts.

Detail: [docs/security.md](docs/security.md) ·
[docs/threat-model.md](docs/threat-model.md)

## ⚙️ Configuration

| Variable | Meaning |
|---|---|
| `FORGEHUB_REGISTRY` | CLI: server URL (or `--registry`, or config) |
| `FORGEHUB_HOME` | CLI: local library location (default `~/.forgehub`) |
| `FORGEHUB_TOKEN` | CLI: token from the environment, for scripts |
| `FORGEHUB_CREDENTIAL_STORE` | CLI: `auto` (default) / `file` / `keyring` - OS keyring vs credentials file |
| `FORGEHUB_SECRET_KEY` | Server: session secret, ≥ 32 random bytes (`.env`) |
| `FORGEHUB_DOMAIN` | Server (compose/Caddy): domain to serve, default `localhost` |
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

History: 22 build steps, one commit + report each -
[PROGRESS.md](PROGRESS.md), [reports/](reports/).

## License

MIT - [LICENSE](LICENSE)
