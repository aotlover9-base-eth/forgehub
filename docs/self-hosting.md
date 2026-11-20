# Self-hosting ForgeHub

ForgeHub is designed to be run by one person on one small
server: a single binary, a single SQLite file, no other
services to babysit.

**Start here:** [deploy.md](deploy.md) walks through the
three supported shapes -

1. **Docker Compose + Caddy** on a VPS (recommended):
   automatic HTTPS, the app container unpublished on the host,
   data in a named volume.
2. **Fly.io** with a persistent volume.
3. **Bare binary** behind your own TLS terminator.

**Then read:** [operations.md](operations.md) - backups
(`scripts/backup-db.sh`, safe against the live database),
the restore drill, upgrades and migrations, retention purges,
and the operator admin CLI for abuse handling
(`forge-server admin hide-snippet|ban-user|list-reports …`).

## Decisions you own as the operator

- **Signup policy** - `FORGEHUB_ALLOW_SIGNUP=false` for a
  private instance (open it briefly to create your account);
  `true` for a public one, with the report queue reviewed.
- **The secret key** - `FORGEHUB_SECRET_KEY` (32+ random
  bytes) from your environment or an untracked `.env`. It is
  never in the database or the repo.
- **The database file is a credential store** - it holds
  password hashes, token hashes and session hashes. Protect
  the volume/directory accordingly; back it up encrypted if
  it leaves the host.
- **First admin** - there is no built-in admin account. The
  operator is whoever controls the host; moderation runs
  through the admin CLI against the same database.

## Honest limits

SQLite suits a single-node instance (the Step 20 measurements:
read p95 ≈ 1.6 ms at 50k snippets). The repository layer is
trait-based so a Postgres backend is a contained future
change, not a rewrite - see `docs/known-gaps.md`. The Docker
image is maintained with the code but was not build-verified
on the development machine (no Docker there); the first
`docker compose up --build` on your host is the smoke test,
and `deploy.md` lists exactly what to check.
