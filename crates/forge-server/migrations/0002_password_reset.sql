-- Password reset tokens (post-v0.1.0 increment, 2026-11-20).
-- Only the SHA-256 hash of a token is stored, mirroring web
-- sessions and API tokens: a database leak does not leak a
-- usable reset link. One live token per user - issuing a new
-- one deletes the previous rows (enforced in the repository).
CREATE TABLE password_reset_tokens (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT
);

CREATE INDEX idx_password_reset_tokens_user ON password_reset_tokens (user_id);
