# Deploying ForgeHub

One server, one SQLite file, one port. This guide covers the
Docker path (recommended), a Fly.io variant, and the operating
basics. Configuration reference: `docs/running.md`. Day-2
operations (backup, abuse, upgrades): `docs/operations.md`.

## Option A - single VPS with Docker + Caddy (recommended)

You need: a VPS with Docker + the compose plugin, a domain
pointed at it (for HTTPS), and 5 minutes.

```sh
git clone https://github.com/aotlover9-base-eth/forgehub.git
cd forgehub
cp .env.example .env
```

Edit `.env` - at minimum:

```ini
FORGEHUB_DOMAIN=hub.example.com
FORGEHUB_SECRET_KEY=<48+ random bytes, e.g. `openssl rand -hex 32`>
FORGEHUB_ALLOW_SIGNUP=false        # see "First run" below
```

```sh
docker compose up -d --build
curl https://hub.example.com/api/v1/health   # {"status":"ok","db":"ok"}
```

Caddy obtains a Let's Encrypt certificate automatically and
terminates TLS; the app container is not published on the host
at all. The database lives in the `forgehub-data` volume.

### First run / first admin

There is no built-in admin account - the operator is whoever
controls the host and the database. Recommended sequence:

1. Start with `FORGEHUB_ALLOW_SIGNUP=false`.
2. Flip it to `true` briefly (`docker compose up -d` to apply),
   create your own account in the web UI, then set it back to
   `false`. On a private instance, keep signups closed; on a
   public one, leave them open and watch `admin list-reports`.
3. Moderation is the server binary's admin subcommand against
   the same database:
   `docker compose exec forgehub forge-server admin list-reports`
   (plus `hide-snippet`, `ban-user` - see `docs/operations.md`).

## Option B - Fly.io

```toml
# fly.toml (sketch - adjust app/region)
app = "your-forgehub"
primary_region = "iad"

[build]
  dockerfile = "Dockerfile"

[env]
  FORGEHUB_ENV = "prod"
  FORGEHUB_BIND = "0.0.0.0:8080"
  FORGEHUB_BASE_URL = "https://your-forgehub.fly.dev"
  FORGEHUB_DATABASE_URL = "sqlite:///data/forgehub.db"
  FORGEHUB_WEB_DIR = "/srv/web"

[mounts]
  source = "forgehub_data"
  destination = "/data"

[[services]]
  internal_port = 8080
  protocol = "tcp"
  [[services.ports]]
    port = 443
    handlers = ["tls", "http"]
```

```sh
fly volumes create forgehub_data --size 1
fly secrets set FORGEHUB_SECRET_KEY=$(openssl rand -hex 32)
fly deploy
```

Fly terminates TLS at its edge; the same `Secure`-cookie and
signup guidance as Option A applies.

## Without Docker

Build on the host (Rust 1.x + Node 24):

```sh
cargo build --release -p forge-server
(cd web && npm ci --legacy-peer-deps && npm run build)
FORGEHUB_ENV=prod FORGEHUB_SECRET_KEY=... \
FORGEHUB_DATABASE_URL=sqlite:///var/lib/forgehub/forgehub.db \
FORGEHUB_WEB_DIR=$PWD/web/dist ./target/release/forge-server
```

Put any TLS terminator (Caddy/nginx) in front; the app listens
on `FORGEHUB_BIND` (default `127.0.0.1:8080`).

## Backups, upgrades, migrations, secrets

- **Backups**: `scripts/backup-db.sh` against the database
  file (safe while running) - cron example and the restore
  drill are in `docs/operations.md`. Back up before every
  upgrade.
- **Upgrades**: pull the new image/source, back up, restart.
  The CLI is independent: users upgrade it via
  `scripts/install.sh` or their package manager.
- **Migrations**: run automatically at startup, forward-only.
  There is no down-migration; the pre-upgrade backup is the
  rollback.
- **Secrets**: `FORGEHUB_SECRET_KEY` comes from the
  environment/compose `.env` (never commit `.env`). It is not
  used to encrypt the database - protect the volume like any
  credential store (see `docs/security.md`, operator section).
- **Signup policy**: closed (`false`) for private instances;
  open for public ones, with the report queue reviewed via the
  admin CLI.

## Verification status

The image and compose file are maintained with the code and
statically reviewed, but they have **not** been built on the
development machine (no Docker available there) - the first
`docker compose up --build` on a real host is the smoke test:
check `/api/v1/health`, sign up in the web UI, and run the CLI
against it (`fg login` with `FORGEHUB_REGISTRY` set to the
deployment URL). The same binaries are covered locally by
`scripts/e2e.sh`, which runs the complete story against the
real server on every CI run. This is recorded in
`docs/known-gaps.md`.
