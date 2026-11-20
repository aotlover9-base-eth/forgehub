# ForgeHub operations

Running, backing up, and restoring a ForgeHub server. Configuration
itself lives in `docs/running.md`; the security posture for operators
is in `docs/security.md`.

## Backup

ForgeHub's whole state is one SQLite database (plus nothing else -
snippet bodies, versions, users, sessions and tokens are all rows).
Back it up with the online backup API, which is safe while the
server is live and produces a transactionally consistent copy
including the WAL:

```sh
scripts/backup-db.sh /path/to/forgehub.db /path/to/backups/forgehub-2026-11-19.db
```

With no arguments it backs up `./forgehub.db` to
`./forgehub-backup-<UTC timestamp>.db`. The script verifies the
copy (`PRAGMA integrity_check`) and fails loudly if it is not ok.

Cron example (daily at 03:17, keep 14 days):

```cron
17 3 * * *  cd /srv/forgehub && scripts/backup-db.sh forgehub.db backups/forgehub-$(date -u +\%F).db && find backups -name 'forgehub-*.db' -mtime +14 -delete
```

Do **not** copy the `.db` file with plain `cp` while the server is
running: with WAL mode you can capture a torn state. Either use the
backup script or stop the server first.

## Restore drill (tested in Step 20)

1. Stop the server.
2. Move the damaged database aside (`forgehub.db` plus any
   `forgehub.db-wal` / `forgehub.db-shm`).
3. Copy the backup into place as `forgehub.db`.
4. Start the server; check `GET /api/v1/health` → `{"status":"ok","db":"ok"}`
   and read a known snippet.

`scripts/e2e.sh` performs exactly this drill on every run: it backs
up a live database, replaces it with the backup, restarts the
server, and verifies the data is served.

## Routine operations

- **Abuse**: `forge-server admin list-reports`,
  `admin hide-snippet <owner>/<name>`, `admin unhide-snippet <owner>/<name>`,
  `admin ban-user <username>` / `admin unban-user <username>` (same
  `FORGEHUB_DATABASE_URL`). Banning revokes the user's sessions and
  API tokens immediately.
- **Retention**: soft-deleted snippets and expired rename redirects
  are hard-deleted after 30 days, at startup and every 6 hours.
- **Registration**: set `FORGEHUB_ALLOW_SIGNUP=false` to close
  signups without downtime (existing users are unaffected).
- **Upgrades**: stop the server, back up, deploy the new binary,
  start it. Migrations run automatically at startup and are
  forward-only; the backup is your way back.

## Failure behaviour (verified in Step 20)

- Database locked by another writer: reads continue (WAL); writes
  wait up to the 5 s busy timeout and then return a generic
  `500 {"error":{"code":"internal_error",...}}` - no storage
  details leak, and writes succeed again once the lock clears.
- Server restart: in-flight requests fail, everything else is in
  the database; sessions survive (they are rows, not memory).
  Rate limits and login throttles are per-process and reset -
  see `docs/known-gaps.md`.
- Disk full: writes fail as generic 500s; the server keeps
  serving reads. Free space or restore onto a larger volume.
