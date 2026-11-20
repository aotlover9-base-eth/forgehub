# ADR 0002 - SQLite (WAL + FTS5) for v1, behind repository traits

- **Status:** Accepted (Step 2, 2026-11-19)

## Context
The server needs users, sessions, tokens, snippets, immutable versions and
full-text search, on a laptop-class single host, cheap to self-host (one
container, roadmap §2). The performance budget is p95 < 50 ms for snippet
read/list with 10k snippets.

## Decision
SQLite in WAL mode via `sqlx`, foreign keys on, with FTS5 for search
(spec-data-model §3). All SQL lives behind repository traits
(`UserRepo`, `SnippetRepo`, `TokenRepo`, …) with SQLite implementations, so
handlers never see SQL and a Postgres implementation can be added later
without handler changes.

## Alternatives rejected
- **Postgres now:** an extra service to run, back up and secure; no v1
  requirement (multi-writer scale, row-level security) needs it, and SQLite
  WAL comfortably meets the stated budget for a single-host deployment.
- **Embedded KV / files on disk:** snippets need relational constraints
  (unique owner/name, immutable version numbering, transactional counters)
  and ranked full-text search; reimplementing those on files is more code
  and more risk.
- **Triggers to maintain FTS:** rejected for the reason in spec-data-model
  §3 - the "public, latest version only" rule would be duplicated in SQL.

## Consequences
Single-writer semantics are accepted; write paths must be short
transactions. The 10k-snippet p95 is measured in Step 10 and the 50k-snippet
load test in Step 20 against this decision.
