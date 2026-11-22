-- Peak C1: snippet collections. A collection is an ordered,
-- owner-curated list of snippets. It never widens access: read
-- endpoints filter items through the snippet visibility rules.
CREATE TABLE collections (
    id          INTEGER PRIMARY KEY,
    owner_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility  TEXT NOT NULL DEFAULT 'private',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE (owner_id, name)
);
CREATE INDEX idx_collections_owner ON collections(owner_id, updated_at DESC);

CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    snippet_id    INTEGER NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    added_at      TEXT NOT NULL,
    PRIMARY KEY (collection_id, snippet_id)
);
CREATE INDEX idx_collection_items_order ON collection_items(collection_id, position);
