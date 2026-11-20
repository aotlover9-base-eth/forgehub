# ADR 0001 - Product name, repository name and binary name

- **Status:** Accepted (Step 1, 2026-11-19)
- **Deciders:** ForgeHub author / coding agent, per `roadmap.md` Step 1 §3

## Context

The working name is ForgeHub, binary `forgehub`, alias `fg`. Step 1 requires a
collision check on crates.io, npm and GitHub before locking the name. A binary
crate name collision on crates.io would not block the repository, but would
affect a future `cargo install` story (Step 21), so it must be recorded.

## Checks performed (2026-11-19, live API responses, not guesses)

| Registry | Query | Result |
|----------|-------|--------|
| crates.io API `GET /api/v1/crates/forgehub` | `forgehub` | `{"errors":[{"detail":"crate `forgehub` does not exist"}]}` - name is free |
| crates.io API `GET /api/v1/crates/forge-hub` | `forge-hub` | `{"errors":[{"detail":"crate `forge-hub` does not exist"}]}` - name is free |
| npm registry `GET /forgehub` | `forgehub` | `{"error":"Not found"}` - name is free |
| npm registry `GET /forge-hub` | `forge-hub` | `{"error":"Not found"}` - name is free |
| GitHub API `GET /repos/forgehub/forgehub` | org/repo | `404 Not Found` - no such org repo |
| GitHub API `GET /repos/aotlover9-base-eth/forgehub` | author's account | `404 Not Found` - the author's `forgehub` repo name is available |
| GitHub search `forgehub in:name` | general | 94 unrelated repositories by other owners use the name in some form; none is the author's, and GitHub names are namespaced by owner, so this does not block `aotlover9-base-eth/forgehub` |

## Decision

- Product name: **ForgeHub**
- Repository: **`forgehub`** under the author's GitHub account (private, created
  once `gh` authentication or a remote URL is supplied - see the Step 1 report)
- Binary name: **`forgehub`**
- Alias: **`fg`**, installed by `forgehub alias install` (Step 5). `fg` is a
  shell job-control builtin; aliases are expanded before builtins in
  interactive shells, which is why the alias works and a binary named `fg`
  alone would not. See `roadmap.md` §6.
- Crate names: `forge-core`, `forge-cli`, `forge-server` in a Cargo workspace.
  The crates.io names `forgehub` and `forge-hub` were free at check time; no
  crate is published in Step 1, and availability must be rechecked before any
  future publication.

## Alternatives rejected

- Rename the product because unrelated GitHub repositories use "ForgeHub":
  rejected - GitHub repository names are per-owner, crates.io and npm names
  are free, and a rename would churn every locked decision in `roadmap.md`
  for no blocking collision.
- Name the binary `fg`: rejected - shadowed by the shell builtin in
  interactive shells (documented in Forgenesis's README as well).

## Consequences

One find-and-replace rename remains possible if a blocking collision appears
before publication. No rename is needed now.
