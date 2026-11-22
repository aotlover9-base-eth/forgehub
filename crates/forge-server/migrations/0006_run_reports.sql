-- Peak C8: cloud run reports. The CLI reports each completed
-- cloud run (best-effort); rows belong to the RUNNER, so stats
-- only ever count a user's own runs. Local runs are never
-- reported - they stay in the CLI's local history database.
CREATE TABLE run_reports (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ref         TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 0,
    commands    INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    success     INTEGER NOT NULL DEFAULT 1,
    saved_ms    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_run_reports_user ON run_reports(user_id, created_at DESC);
