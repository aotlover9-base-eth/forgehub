-- ForgeHub initial schema (docs/spec-data-model.md).
-- All timestamps are UTC ISO-8601 text supplied by the application.
-- FTS5 is maintained by application code in the write transaction
-- (spec-data-model section 3), not by triggers.

CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    email         TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    display_name  TEXT,
    bio           TEXT,
    created_at    TEXT NOT NULL,
    disabled_at   TEXT,
    deleted_at    TEXT
);

CREATE TABLE sessions (
    id_hash       TEXT PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf_token    TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    last_used_at  TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    ip_hint       TEXT,
    ua_hint       TEXT
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

CREATE TABLE api_tokens (
    id            INTEGER PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    prefix        TEXT NOT NULL,
    token_hash    TEXT NOT NULL UNIQUE,
    scopes        TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    last_used_at  TEXT,
    expires_at    TEXT,
    revoked_at    TEXT
);
CREATE INDEX idx_api_tokens_user ON api_tokens(user_id);
CREATE INDEX idx_api_tokens_hash ON api_tokens(token_hash);

CREATE TABLE device_codes (
    id               INTEGER PRIMARY KEY,
    device_code_hash TEXT NOT NULL UNIQUE,
    user_code        TEXT NOT NULL UNIQUE,
    client_name      TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending',
    user_id          INTEGER REFERENCES users(id) ON DELETE CASCADE,
    token_id         INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    created_at       TEXT NOT NULL,
    expires_at       TEXT NOT NULL,
    last_polled_at   TEXT,
    attempts         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_device_codes_user_code ON device_codes(user_code);

CREATE TABLE snippets (
    id                     INTEGER PRIMARY KEY,
    owner_id               INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                   TEXT NOT NULL,
    description            TEXT NOT NULL DEFAULT '',
    visibility             TEXT NOT NULL DEFAULT 'private',
    latest_version         INTEGER NOT NULL DEFAULT 0,
    star_count             INTEGER NOT NULL DEFAULT 0,
    fork_count             INTEGER NOT NULL DEFAULT 0,
    pull_count             INTEGER NOT NULL DEFAULT 0,
    forked_from_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL,
    forked_from_version    INTEGER,
    forked_from_hash       TEXT,
    hidden_at              TEXT,
    deleted_at             TEXT,
    created_at             TEXT NOT NULL,
    updated_at             TEXT NOT NULL,
    UNIQUE (owner_id, name)
);
CREATE INDEX idx_snippets_owner_updated ON snippets(owner_id, updated_at DESC);
CREATE INDEX idx_snippets_visibility_updated ON snippets(visibility, updated_at DESC);

CREATE TABLE snippet_versions (
    id         INTEGER PRIMARY KEY,
    snippet_id INTEGER NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
    version    INTEGER NOT NULL,
    mode       TEXT NOT NULL,
    body       TEXT NOT NULL,
    hash       TEXT NOT NULL,
    message    TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (snippet_id, version)
);
CREATE INDEX idx_versions_snippet ON snippet_versions(snippet_id, version DESC);
CREATE INDEX idx_versions_hash ON snippet_versions(snippet_id, hash);

CREATE TABLE snippet_tags (
    snippet_id INTEGER NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
    tag        TEXT NOT NULL,
    PRIMARY KEY (snippet_id, tag)
);
CREATE INDEX idx_snippet_tags_tag ON snippet_tags(tag);

CREATE TABLE stars (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    snippet_id INTEGER NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, snippet_id)
);

CREATE TABLE renames (
    owner_id   INTEGER NOT NULL,
    old_name   TEXT NOT NULL,
    snippet_id INTEGER NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (owner_id, old_name)
);

CREATE TABLE reports (
    id          INTEGER PRIMARY KEY,
    snippet_id  INTEGER NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
    reporter_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason      TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'open',
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_reports_status ON reports(status, created_at);

-- Full-text search over public snippets' latest versions.
-- Contentless FTS5: rows are managed explicitly by the repositories
-- with rowid = snippets.id (spec-data-model section 3).
CREATE VIRTUAL TABLE snippets_fts USING fts5(
    name, description, tags, owner, body,
    content = '', tokenize = 'unicode61 remove_diacritics 2'
);
