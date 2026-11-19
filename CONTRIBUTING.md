# Contributing to ForgeHub

Contributions are welcome — issues, discussions and pull
requests. ForgeHub was built step by step under strict rules;
keeping them is what makes the project trustworthy.

## Ground rules

- **No stubs, ever.** No placeholder functions, `todo!()`,
  mocked-out core logic, or fake data. If something can't be
  done, say so in the PR and in `docs/known-gaps.md`.
- **Prove it by running it.** Every change ships with tests
  (unit, plus integration where behaviour crosses a boundary)
  and the gates below must pass locally before you push.
- **The safety model is the product.** Anything touching
  snippet display, confirmations, trust, secrets or auth
  needs failure-path tests and a note in the PR about which
  threat-model row (`docs/threat-model.md`) it affects.
- **Design discipline (web).** Material You 3 tokens only —
  no hardcoded colours, radii or font sizes outside the token
  files; the token check runs in lint and will fail the PR.

## Development setup

```sh
git clone https://github.com/aotlover9-base-eth/forgehub.git
cd forgehub
cargo build --workspace
(cd web && npm ci --legacy-peer-deps && npm run build)
```

Run the server: `cargo run -p forge-server` (configuration in
`docs/running.md`). Run the CLI: `cargo run -p forge-cli -- …`.

## Gates (all must be green)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check && cargo audit
(cd web && npm run lint && npm run typecheck && npm test && npm run build)
bash scripts/e2e.sh        # the full story, real binaries
bash scripts/e2e-web.sh    # the browser story (Playwright)
shellcheck scripts/*.sh
```

Do not truncate gate output when checking it, and never
weaken a test to make it pass — fix the code or, if the test
is wrong, explain why in the PR.

## Pull requests

- One topic per PR, with a summary of behaviour changes and
  the test evidence (commands + results).
- Update the docs your change touches (`docs/`), the
  changelog's Unreleased section, and `docs/known-gaps.md`
  if you close or open a gap.
- Security-sensitive reports do **not** go in public issues —
  see `SECURITY.md`.
