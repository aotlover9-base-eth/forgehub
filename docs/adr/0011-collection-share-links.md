# ADR 0011 - Collection share links: hashed, expiring, revocable, view-only

- **Status:** Accepted (Peak WP C6, 2026-11-22)
- **Context:** Peak C6 adds shared read-only links to a collection, including a private one: anyone holding the link can read the member snippets' code without an account. This deliberately cuts across the C1 invariant ("a collection never widens access"), so the exception needs an explicit, narrow decision before the code exists.

## Decision

Share links work under these rules, enforced in `crates/forge-server/src/shares.rs` and migration `0004_collection_share_links.sql`:

1. **The link is a bearer capability, so it is built like an API token.** 43 random base62 characters (about 256 bits) behind an `fgl_` prefix; only the SHA-256 hash and an 8-character prefix are stored. The plaintext is shown exactly once, at creation. Guessing is not a plausible attack; theft is the real one, which is what rules 2-4 are for.
2. **Every link expires.** The creator picks 1, 7 or 30 days (7 default). There is no "never expires" option. Expired, revoked and unknown tokens all render the same 404 page - the endpoint is not an oracle for which case it was.
3. **Revocation is instant and owner-only.** `revoked_at` is set once; the page lookup requires `revoked_at IS NULL AND expires_at > now`. Deleting the collection cascades its links away. At most 10 unrevoked links per collection.
4. **The grant is view-only, and it is the whole grant.** The page at `/shared/{token}` is server-rendered, works without JavaScript, and shows the collection's current members in order with their current code - including private snippets, because sharing them is the link's purpose. It offers no run, pull or copy-into-account action: executing anything still requires an account and normal snippet access. Admin-hidden and soft-deleted snippets are excluded even here; an admin action outranks an owner's share.
5. **Membership is live, not a snapshot.** The page reads the collection as it is now: removing a snippet removes it from every outstanding link, and adding one shares it. The creation UI states this in plain words ("anyone with this link can read every snippet currently in this collection"), so the owner consents to the live semantics, not a frozen copy.
6. **Management is owner-only and existence-masked.** Create/list/revoke resolve through the same owner-only path as collection mutations: another user's collection answers 404, anonymous callers 401, read-only tokens 403 on create/revoke.

## Why

- **Hashing the token** means a database leak does not leak working links - the same property API tokens already have, reused rather than reinvented (same generator and hash helpers).
- **Server-rendered page, not an API + SPA route:** link recipients are exactly the people least likely to have an account or the app loaded; the C4 public-pages pattern (self-contained HTML, full escaping, global CSP) already proves the shape.
- **Live membership over snapshot:** a snapshot silently keeps sharing code the owner removed - the dangerous direction. Live semantics fail safe: removal always wins.

## Alternatives rejected

- **Signed URLs (HMAC over collection id + expiry, no table).** Rejected: individual revocation is impossible without a denylist, which is a table again - with worse ergonomics.
- **Share by making the collection unlisted.** Rejected: unlisted collections still filter items by snippet visibility, so private snippets (the actual use case) would not be shared; and an unlisted name is guessable/enumerable in a way a 256-bit token is not.
- **Per-snippet share links.** Rejected for this WP: the plan asks for collection links; per-snippet public sharing already exists as public visibility + the C4 page.

## Consequences

- C1's invariant gains one documented exception: a collection widens access only through an explicit, expiring, revocable link its owner created.
- The server gains one table and one anonymous HTML route. The route is rate-limited by the same global layers as the rest of the app and returns no JSON, no cookies and no account data.
- Link recipients see code but get no execution path; the page says so, to avoid implying the server vouches for the content. The usual rule stands: never run code you have not read.
