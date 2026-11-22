# Report - Peak C1: snippet collections

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** C1 (Phase C - new features).
- **Status:** Done. Collections end to end: server (migration
  0003, repo, 8 endpoints, authorisation matrix tests), CLI
  (`fg collection ...` + run integration), web (Library
  section, collection page, add-from-menu), OpenAPI.

## 1. The invariant

A collection NEVER widens access. It is an ordered list of
references, not a container: the read endpoint filters every
item through the snippet read rules for the specific viewer
(public/unlisted for all, private for the snippet's owner
only, admin-hidden and soft-deleted hidden from non-owners /
everyone respectively). Consequences, all test-proven:

- A private collection is 404 to everyone but its owner -
  indistinguishable from a missing one.
- A public collection containing the owner's private snippet
  shows that snippet only to its owner; other viewers do not
  even learn it exists (the visible `item_count` excludes it
  and `total_items` is sent only to the owner).
- If a member snippet goes private later, it vanishes from
  the collection for everyone but its owner at the next read.
- You can only ADD a snippet you can read (the add endpoint
  resolves through the same 404-masking read path), so a
  collection cannot be used to probe private names.

## 2. Server

- Migration `0003_collections.sql` (append-only): `collections`
  (owner, name, description, visibility, timestamps; unique
  per owner) and `collection_items` (collection, snippet,
  position, added_at; PK on the pair; both FKs cascade, so
  deleting a snippet or a collection cleans up).
- `CollectionRepo` trait + `SqliteStore` impl in `repo.rs`,
  wired into `Repos`; handlers in new `collections.rs`
  following the snippets module's patterns (`RequireUser` /
  `CurrentUser`, `check_write_scope_and_csrf`,
  `resolve_readable` reuse for adds).
- Endpoints: `POST /collections` (201, 409 on duplicate),
  `GET /me/collections`, `GET /collections/{owner}/{name}`
  (optional auth), `PATCH` + `DELETE` on the same path
  (owner-only, 404-masked), `PUT`/`DELETE
  .../items/{sowner}/{sname}`, `POST .../reorder` (the body
  must list exactly the current items, each once - 400
  otherwise).
- Limits (module constants, plan-sized): 100 collections per
  user, 200 items per collection, description <= 280 chars.
  Collection names follow the snippet-name rules.
- Removal resolves the snippet WITHOUT the read filter, so an
  owner can always remove an item that became unreadable.
- Tests: `crates/forge-server/tests/collections.rs` - 4
  integration tests over real HTTP: the visibility matrix,
  the never-widens-access suite (including visibility flip
  after add), reorder/remove/delete authorisation, and token
  scopes (anonymous 401, read token 403 on create, write
  token works).

## 3. CLI

New module `collections_cmd.rs` + `fg collection` (Cloud
heading, `--registry` option):

- `list` (own, with counts), `create` (--description,
  --visibility), `show` (numbered items; owner's view notes
  how many items are hidden from them), `add` (409 becomes a
  friendly "already in"), `remove`, `order` (full list),
  `visibility`, `delete` (TTY confirmation or `-y`; foreign
  collections refused client-side too). All support `--json`
  where they print data.
- `config`-style routing catch, again: `collection` was added
  to `KNOWN_COMMANDS` so `fg collection` is not rewritten to
  a snippet run.
- **Run integration** (`cmds::cmd_run` + `try_run_collection`):
  when a snippet reference 404s - both the explicit
  `owner/name` path and the bare-name own-cloud fallback -
  the CLI tries a collection of that name. Members run in
  order through the UNCHANGED cloud-run trust flow (cards,
  typed yes for others' snippets, hash pins). The run stops
  at the first member that fails, is declined, or exits
  non-zero, says exactly where it stopped, and returns that
  member's code - a half-run runbook never looks complete.
  Snippets keep precedence over same-named collections;
  offline runs skip collections (they are not cached).
- Tests: `crates/forge-cli/tests/collections.rs` - 3
  integration tests against the real server: full lifecycle,
  ordered run with stop-on-failure (exit 3 surfaces, later
  member's marker absent), and the foreign view (private item
  name absent from `show`; running someone else's collection
  without a TTY refuses, and `-y` does not bypass it).

## 4. Web

- Types + hooks in `src/api` (`useMyCollections`,
  `useCollection`, create/patch/delete, add/remove/reorder
  mutations with cache invalidation).
- Library (Dashboard): a Collections section
  (`CollectionsPanel`) listing own collections with counts
  and a create form; each snippet's action menu gains
  "Add to <collection>" entries.
- New page `CollectionPage` at `/collections/:owner/:name`
  (route registered before `:owner/:name`; document title
  added): ordered items with versions and visibility chips,
  the `fg run owner/name` command with copy button, and for
  the owner: add-by-reference form, visibility chips,
  move up/down reorder buttons, per-item remove, delete with
  confirmation. Non-owners get the read view only; the
  hidden-items note shows when the server reports unreadable
  items.
- Tests: `src/pages/collections.test.tsx` - 5 tests (visitor
  vs owner views, reorder posts the swapped order, 404
  state, panel list + create).

## 5. Docs

- `docs/openapi.yaml`: 6 new paths + `Collection`,
  `CollectionItem`, `CollectionDetail`, `CollectionInput`,
  `CollectionPatch` schemas.
- `docs/cli-reference.md`: header paragraph, glance entry,
  full `forgehub collection` help sections from the binary.
- `docs/man/`: regenerated (9 new collection pages).

## 6. Gates

Local, this session: `cargo fmt --all --check` clean;
`cargo clippy --workspace --all-targets -- -D warnings`
clean (the first pass caught four lints in the new code -
two `too_many_lines`, two doc lints - fixed in this WP by
extracting helpers and splitting tests, not carried forward);
`cargo test --workspace` 491 passed / 0 failed (481 at B9 +
7 server collection tests + 3 CLI collection tests). Web:
lint clean (one pre-existing warning in ExplorePage),
typecheck clean, full vitest suite green (5 new collection
tests), production build green. CI verification of this
commit's head is recorded in PROGRESS.md.

## 7. Next

C2 - typed parameters.
