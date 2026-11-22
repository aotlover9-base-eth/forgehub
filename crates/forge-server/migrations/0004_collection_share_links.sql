-- Peak C6: share links for collections (ADR 0011). Only the
-- SHA-256 hash and a display prefix of each token are stored;
-- links always expire and can be revoked by the owner.
CREATE TABLE collection_share_links (
    id            INTEGER PRIMARY KEY,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    token_hash    TEXT NOT NULL UNIQUE,
    token_prefix  TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    revoked_at    TEXT
);
CREATE INDEX idx_share_links_collection ON collection_share_links(collection_id, created_at DESC);
