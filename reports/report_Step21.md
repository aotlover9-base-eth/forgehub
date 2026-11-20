# Step 21 Report - Packaging, installer, release pipeline

## 1. Summary

ForgeHub is now installable and deployable by someone who is not its author - prepared, per the run limits, **without publishing or deploying anything**: no GitHub Release was created, no package registry was touched, no paid account exists. Everything below is in the repo, verified as far as this machine allows, with the remaining verification gaps stated plainly.

## 2. What was built

**Versioning.** Workspace version is `0.1.0`. New `build.rs` in both binaries embeds the git commit (12-char sha) and build date (honouring `SOURCE_DATE_EPOCH`): `forgehub --version` now prints `forgehub 0.1.0 (91deb5f5192c, built 2026-11-19)`, and `/api/v1/version` reports the same sha (runtime `FORGEHUB_GIT_SHA` override → build-time embed → `dev`). `CHANGELOG.md` added (Keep a Changelog; the 0.1.0 entry summarises Steps 1–20).

**Release workflow** (`.github/workflows/release.yml`, ADR 0008 records why hand-written over cargo-dist). On a `v*` tag: a five-target matrix (x86_64/aarch64 Linux musl - the aarch64 leg via `cross`, x86_64/aarch64 macOS, x86_64 Windows MSVC), strip+LTO from the workspace release profile, tarballs/zips named `forgehub-<tag>-<target>` containing the binary, LICENSE, README and the man pages, per-asset checksums combined into one `SHA256SUMS`, keyless build-provenance attestations (`actions/attest-build-provenance`), all actions pinned to **verified** commit SHAs (one SHA I initially typed from memory failed verification against the GitHub API and was corrected before commit - the check mattered). The workflow uploads assets as a workflow artifact only: **it cannot create a Release**, so a tag never becomes a public release by accident; publishing stays a deliberate human step (commands in the Step 22 report).

**Installer** (`scripts/install.sh`, POSIX sh, ~200 lines, commented, prints its plan before acting): detects OS/arch, resolves the latest tag via the GitHub API (or a pinned `FORGEHUB_VERSION`), downloads the tarball + `SHA256SUMS` over HTTPS, **verifies the SHA-256 before installing anything**, installs to `~/.local/bin` (override `FORGEHUB_INSTALL_DIR`), refuses root unless `FORGEHUB_ALLOW_ROOT=1`, never edits rc files - it *offers* `forgehub alias install` interactively and skips the offer non-interactively. `shellcheck` clean (install.sh, e2e.sh, backup-db.sh all pass shellcheck 0.11).

**Server deployment.** The Dockerfile gained a `GIT_SHA` build arg wired to `/api/v1/version`; new `docker-compose.yml` (app not published on the host, read-only root FS, `no-new-privileges`, named volume) + `Caddyfile` (automatic HTTPS, HSTS at the edge) + `.dockerignore`. `docs/deploy.md`: VPS+compose walkthrough, Fly.io `fly.toml` sketch, bare-binary path, first-run admin sequence (signup window + admin CLI), backups/upgrades/migrations/secrets/signup-policy guidance. `.env.example` was audited and is complete.

**Packaging templates** (in `packaging/`, clearly marked unpublished): Homebrew formula and AUR `PKGBUILD` tracking the release asset names with placeholder checksums to fill from `SHA256SUMS`; `cargo-deb` metadata in `crates/forge-cli/Cargo.toml`. README gained an Install section incl. `cargo install --path crates/forge-cli`.

## 3. Dry-run evidence (all local, nothing published)

- **Installer against a fake release**: built a release-shaped tarball + `SHA256SUMS`, served it over local HTTP, ran the installer with `FORGEHUB_RELEASE_BASE` pointed at it → checksum verified, binary installed, `forgehub --version` runs, PATH note + non-interactive alias note printed. **Negative test**: one corrupted hex digit in `SHA256SUMS` → `CHECKSUM MISMATCH … refusing to install`, exit 1, no install directory created. Root refusal also exercised (this shell runs as root; the documented override was used for the positive run).
- **musl release leg**: with `musl-tools` installed locally, `cargo build --release -p forge-cli --target x86_64-unknown-linux-musl` produced a static-pie ELF (9,642,816 bytes) that runs and reports the stamped version - the exact artifact shape the workflow ships for Linux.
- **Web in production shape**: Lighthouse 12 against the real server serving the production SPA build - **Performance 93, Accessibility 100, Best Practices 93** (LCP 2.6 s, FCP 2.1 s, TBT 170 ms, CLS 0, on this shared-CPU VM; the sub-100 audits are the signed-out `/me` 401 probe in the console and source-map/inspector notices).
- **Not verified here**: the Docker image build and `docker compose up` (no Docker on this machine - stated in `docs/deploy.md` and `docs/known-gaps.md`); the darwin/windows/aarch64 workflow legs (first `v*` tag run in Step 22 will exercise the whole matrix in CI and its result will be reported honestly there).

## 4. Gates (real output)

- `cargo fmt --all --check` clean; clippy `-D warnings` - **0 errors**.
- `cargo test --workspace` - **411 passed, 0 failed**. One transient failure occurred in an earlier parallel-loaded run: `session_expiry_and_logout_all` (a 1-second idle-expiry window) flaked while apt, a release build and Lighthouse competed for the same 1.5 CPUs; it passes standalone (5 s) and in the full quiet re-run. Recorded here rather than hidden.
- Web: lint clean, typecheck clean, 45 tests, production build OK (bundle unchanged from Step 20).
- `shellcheck` clean on all shipped shell scripts; both workflow files parse as valid YAML.

## 5. Corrections to earlier steps

None to code. The Step 6 Dockerfile was extended (not replaced); `docs/known-gaps.md` already carried the Docker caveat, now cross-referenced from `docs/deploy.md`.

## 6. Next step

Step 22 - docs, final audit, tag `v0.1.0` locally and push the tag (no Release), with the exact release commands for the user.
