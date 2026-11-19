# ADR 0008 — Release pipeline: hand-written matrix over cargo-dist

Date: 2026-11-19 · Step 21

## Context

The release workflow must produce CLI binaries for five targets
(x86_64/aarch64 Linux musl, x86_64/aarch64 macOS, x86_64
Windows MSVC) with SHA-256 checksums and build provenance,
from a `v*` tag, with every GitHub Action pinned to a commit
SHA (Step 19 supply-chain rule).

## Decision

Hand-write the workflow (`.github/workflows/release.yml`) with
an explicit build matrix, `taiki-e/install-action` for the
`cross` tool on the aarch64-musl leg, plain `cargo build` +
`rustup target add` elsewhere, and
`actions/attest-build-provenance` for keyless attestations.

The workflow **does not create a GitHub Release**: it uploads
the finished assets as a workflow artifact. Publishing a
release is a separate, deliberate, human step (the exact
commands are in the Step 22 report), which keeps "tag exists"
and "release published" from ever being accidentally the
same event.

## Alternatives considered

- **cargo-dist**: generates the workflow, installers and
  release metadata. Rejected for v1: it takes over release
  creation (against the deliberate-publish rule above), its
  generated workflow changes shape between cargo-dist
  versions (harder to keep SHA-pinned and reviewable), and
  ForgeHub needs no per-platform installer generation — one
  tarball/zip per target plus `scripts/install.sh` covers it.
  Revisit if the target matrix or installer formats grow.
- **Fully manual releases**: rejected — unrepeatable and
  error-prone; the matrix build is exactly what CI is for.

## Consequences

- The workflow file is longer but every step is visible and
  reviewable in-repo.
- Asset naming (`forgehub-<tag>-<target>.tar.gz`, plus
  `SHA256SUMS`) is a public contract that `scripts/install.sh`
  and the packaging templates depend on; changing it means
  changing all three together.
