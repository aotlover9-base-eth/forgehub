# ADR 0006 - Content hash and trust pinning design

- **Status:** Accepted (Step 2, 2026-11-19)

## Context
Running remote text safely requires answering "is this exactly what I ran
last time?" Versions alone cannot answer it (a version number is a claim by
the server; two different bodies must never share an identity). The hash
must be computable identically by `forge-core` on the CLI, the server, and
(in principle) the web, from the same document.

## Decision
- Identity of a version's content is `SHA-256` over the canonical
  serialisation defined normatively in spec-snippet-format §9 (fixed key
  orders, LF normalisation, tags sorted, `created`/`source` excluded,
  script body byte-preserved apart from LF + one trailing newline).
- The server stores the hash it computed; the CLI **recomputes it locally**
  from the fetched body and refuses to run on a mismatch (hard error).
- Trust is pinned to `(owner, name, hash)` - not to a name, not to a
  version number (spec-cli §7). A version pin (`@N`) selects *which*
  version to fetch; the hash check and trust rules still apply to it.
- `GET` responses carry the hash as an `ETag`, enabling `If-None-Match`
  revalidation (Step 20 measures this).

## Alternatives rejected
- **Hash the raw submitted bytes:** YAML re-quoting, CRLF, or key reordering
  by an editor would change the hash of a semantically identical snippet,
  training users to click through "changed" warnings - the worst outcome
  for a trust system.
- **Hash a "semantic AST" without a serialisation spec:** not independently
  implementable or testable; §9's byte-level definition is.
- **Author signatures in v1:** real, but a key-management product of its
  own; listed under Future (`fg verify`). Hash + transport + trust-on-first-use
  with diff-on-change is the honest v1 model, and the docs say so.

## Consequences
Step 3 must ship property tests proving §9's consequences (key order,
quoting and CRLF invariance; content sensitivity). Any future change to §9
is a hash-breaking change and requires a new ADR and a format version.
