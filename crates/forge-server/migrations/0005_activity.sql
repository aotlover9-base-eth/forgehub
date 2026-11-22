-- Peak C7: per-user activity log. Append-only; `detail` is a
-- small JSON object whose shape depends on `kind`. Login rows
-- also carry the user agent in its own column so "have we
-- seen this device before" is an exact-match lookup.
CREATE TABLE activity_events (
    id         INTEGER PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,
    detail     TEXT NOT NULL DEFAULT '{}',
    user_agent TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE INDEX idx_activity_user_time ON activity_events(user_id, created_at DESC, id DESC);
