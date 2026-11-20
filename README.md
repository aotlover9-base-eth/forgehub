# ForgeHub

**Save a shell command once. Run it anywhere.**

A cloud home for shell command snippets: a server + web app to
store and share them, and a Rust CLI (`forgehub`, alias `fg`) to
run them — with the full commands, owner, version and hash shown
**before** anything executes.

| | |
|---|---|
| Stack | Rust (CLI + axum/SQLite server) · React + Material You 3 web |
| Snippets | Text only, ≤ 64 KiB. No file uploads, no binaries |
| Status | v0.1.0 · 411 Rust + 47 web tests · MIT |

![ForgeHub home — the Observe theme](docs/screenshots/home-observe.jpg)

## Quickstart — server (Docker)

```console
git clone https://github.com/aotlover9-base-eth/forgehub.git
cd forgehub
cp .env.example .env        # set FORGEHUB_SECRET_KEY and FORGEHUB_DOMAIN
docker compose up -d --build
```

Open `https://<FORGEHUB_DOMAIN>` (default `https://localhost`),
create an account. Details: [docs/deploy.md](docs/deploy.md).

## Quickstart — CLI

```console
cargo install --path crates/forge-cli   # from the clone; Rust 1.x
forgehub alias install                  # adds the `fg` alias
fg init
fg record hello             # capture shell: run `echo hello`, then `exit`
fg hello                    # shows the commands, asks [y/N], runs
```

Connect it to your server:

```console
export FORGEHUB_REGISTRY=https://localhost   # your server's URL
fg login                    # device-code flow: approve in the browser
fg push hello --public      # publish a local snippet
fg run alice/docker-setup   # run anyone's public snippet (trust flow below)
```

A checksum-verifying installer (`scripts/install.sh`) ships with
each GitHub Release.

## Screenshots

One theme — dark glass, serif headlines — from the home page
through every signed-in section.

| Library | Explore |
|---|---|
| ![Library](docs/screenshots/library-observe.jpg) | ![Explore](docs/screenshots/explore-observe.jpg) |

| Profile | A snippet |
|---|---|
| ![Profile](docs/screenshots/profile-observe.jpg) | ![Snippet page](docs/screenshots/snippet-observe.jpg) |

## Commands

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
`-y` (skip `[y/N]` on your own snippets — never skips trust
prompts) · `--offline` (run from cache) · `-v` (request log).

Full reference: [docs/cli-reference.md](docs/cli-reference.md) ·
[docs/user-guide.md](docs/user-guide.md)

## Safety rules (enforced, not optional)

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

## Configuration

| Variable | Meaning |
|---|---|
| `FORGEHUB_REGISTRY` | CLI: server URL (or `--registry`, or config) |
| `FORGEHUB_HOME` | CLI: local library location (default `~/.forgehub`) |
| `FORGEHUB_TOKEN` | CLI: token from the environment, for scripts |
| `FORGEHUB_CREDENTIAL_STORE` | CLI: `auto` (default) / `file` / `keyring` — OS keyring vs credentials file |
| `FORGEHUB_SECRET_KEY` | Server: session secret, ≥ 32 random bytes (`.env`) |
| `FORGEHUB_DOMAIN` | Server (compose/Caddy): domain to serve, default `localhost` |
| `FORGEHUB_SMTP_URL` | Server: SMTP URL for password-reset mail, e.g. `smtps://user:pass@host:465` |
| `FORGEHUB_MAIL_FROM` | Server: From address for outgoing mail |

## Documentation

[User guide](docs/user-guide.md) · [CLI reference](docs/cli-reference.md) ·
[API](docs/api.md) · [OpenAPI](docs/openapi.yaml) ·
[Self-hosting](docs/self-hosting.md) · [Deploy](docs/deploy.md) ·
[Operations](docs/operations.md) · [FAQ](docs/faq.md) ·
[Known gaps](docs/known-gaps.md) · [Forgenesis migration](docs/forgenesis-migration.md) ·
[Security policy](SECURITY.md) · [Contributing](CONTRIBUTING.md) ·
[Changelog](CHANGELOG.md) · [Demo data pack](demo-data/) — 105 starter snippets + loader

## Development

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run lint && npm run typecheck && npm test && npm run build
```

History: 22 build steps, one commit + report each —
[PROGRESS.md](PROGRESS.md), [reports/](reports/).

## License

MIT — [LICENSE](LICENSE)
