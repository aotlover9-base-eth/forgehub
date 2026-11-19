# ForgeHub

**Work in progress — Step 1 of 22 (bootstrap). Nothing user-facing works yet.**

ForgeHub will be a cloud home for shell command snippets: a web app where you
save commands and small shell scripts as text, and a fast Rust CLI (`forgehub`,
aliased to `fg`) that fetches a snippet, shows exactly what it will run, asks
`[y/N]`, and only then runs it.

```text
fg ubuntu-setup
```

## Status

- [x] Step 1: monorepo skeleton (Cargo workspace + Vite/React/TypeScript web), CI skeleton, Forgenesis analysis
- [ ] Everything else: see `PROGRESS.md` and `roadmap.md`

## Layout

```text
crates/forge-core     shared library (Step 3)
crates/forge-cli      the `forgehub` binary (Steps 4-5, 11-13, 18)
crates/forge-server   axum API + static web hosting + SQLite (Step 6 onward)
web/                  React + TypeScript + Vite (design system in Step 14)
docs/                 ADRs, specs, analysis
reports/              report_Step1.md … report_Step22.md
```

## Building the skeleton

```bash
cargo test --workspace
cargo run -p forge-cli -- --version
cd web && npm install --legacy-peer-deps && npm run build
```

## Safety note

ForgeHub runs remote text on your machine by design. That is why the product
requires explicit confirmation, shows the full commands, owner, version and
hash, requires typed `yes` for other people's snippets, shows a diff when a
trusted snippet changes, and fails safe without a TTY. Those behaviours are
specified in `agent.md` §10 and `roadmap.md`, and land in Step 13.

## Licence

MIT — see `LICENSE`. Forgenesis, whose local features ForgeHub ports, is also
MIT by the same author; see `docs/forgenesis-analysis.md`.
