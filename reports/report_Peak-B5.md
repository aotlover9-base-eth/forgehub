# Report - Peak B5: editing and authoring

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B5 (Phase B - CLI improvements). Fixes audit F8.
- **Status:** Done, pending CI verification of this commit's head (local cargo blocked by the sandbox write fault).

## 1. What landed

- **`fg new <name> [--script]`** - writes a valid template (steps mode by default, script mode with `--script`; both documented in the help examples) and opens it in the editor (config editor, then `$EDITOR`, `$VISUAL`, `vi` - the resolution is now one shared helper also used by `edit`). On save it validates: parse, name must stay the requested one, secret scan must be clean. A validation error **loops back into the editor** with the problem printed, instead of discarding the work; without a terminal (or when the user declines another round) the file is kept in place with a clear message and exit 1 - work is never silently lost. Refuses to overwrite an existing snippet.
- **`fg edit <owner/name>`** - cloud editing (`sync::cmd_edit_cloud`): fetches the snippet, refuses when it is not yours (with the fork fix) or when a divergent local copy would be overwritten, materialises it locally **with its sync baseline** (so the push passes the version-conflict guard), edits through the exact local `cmd_edit` flow (validation + rollback included), then asks for confirmation and pushes as a new version with the message "Edited via fg edit". No-change edits exit quietly; declined or non-TTY pushes leave the edits saved locally with the `fg push` fix line and exit 2 - the fail-safe shape used by the rest of the CLI. A rename inside the editor is detected and reported (a cloud edit cannot follow a rename).
- **`fg lint <name>`** - prints the danger lints (`forge_core::lint`, warnings only per ADR 0007) and the secret-scan findings (`forge_core::scan`) without running anything. Exit codes per the plan: **0 clean, 1 warnings, 2 blocking** (possible secrets). `--json` emits `{ "name", "lints": [{line, message}], "secrets": [{line, kind}], "verdict": "clean"|"warnings"|"blocking" }`.

## 2. Tests added

- `cmds::tests::new_templates_are_valid_and_keep_the_name`: both templates validate; a renamed name field is rejected; a private-key marker is rejected by the secret scan.
- Editor-driven flows (new/edit loop, cloud edit + push) are covered by the existing harness patterns in CI's test + e2e jobs for the parts that predate B5; the new cloud-edit path is exercised end-to-end only manually against a dev server, which the sandbox fault currently prevents - stated plainly: **cloud edit's live run is not verified in this WP** beyond compile + unit coverage of its pieces (fetch/edit/push are each covered by their own suites). It is first in line for a live pass when the environment allows.

## 3. Gates

- `cargo fmt --check` via local rustfmt: CLEAN.
- Clippy/test/build: via CI for this head (recorded in PROGRESS.md once verified).
- **This commit also carries the B4 clippy fix-forward** (six errors from CI run 44969502861 on the B4 head) - the full list and rationale are in the B4 report's addendum. The sandbox write fault prevents hunk-level commit splitting via local git, so the fixes ride here with disclosure rather than in a separate commit.

## 4. Next

B6 - sync and status quality.

## Addendum (2026-11-22, after CI run 44970331789)

The B5 head's CI failed the Rust Clippy step with two errors: `sort_by_key` (the `--sort recent` comparator, B4 code) and `bool_to_int_with_if` (`cmd_lint`'s exit code). Both fixed behaviour-preservingly in the B6 commit (disclosed in its report, section 4); the B6 CI run is the verification for both.
