#!/usr/bin/env bash
# Back up a ForgeHub SQLite database (Step 20).
#
# Uses SQLite's online backup API (via python3's sqlite3
# module), so it is safe to run while the server is live: the
# copy is transactionally consistent and includes the WAL.
#
#   scripts/backup-db.sh [source.db] [dest.db]
#
# Defaults: source ./forgehub.db, dest ./forgehub-backup-<ts>.db
# Cron-friendly: quiet on success, non-zero exit on failure.
set -euo pipefail

SRC="${1:-forgehub.db}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DEST="${2:-forgehub-backup-$STAMP.db}"

if [ ! -f "$SRC" ]; then
    echo "backup-db: source database not found: $SRC" >&2
    exit 1
fi

python3 - "$SRC" "$DEST" <<'PY'
import sqlite3, sys

src_path, dest_path = sys.argv[1], sys.argv[2]
src = sqlite3.connect(f"file:{src_path}?mode=ro", uri=True)
dst = sqlite3.connect(dest_path)
with dst:
    src.backup(dst)
# Verify the copy opens and passes an integrity check.
check = dst.execute("PRAGMA integrity_check").fetchone()[0]
tables = dst.execute(
    "SELECT count(*) FROM sqlite_master WHERE type='table'"
).fetchone()[0]
dst.close()
src.close()
if check != "ok":
    print(f"backup-db: integrity check failed: {check}", file=sys.stderr)
    sys.exit(1)
print(f"backup-db: {src_path} -> {dest_path} (integrity ok, {tables} tables)")
PY
