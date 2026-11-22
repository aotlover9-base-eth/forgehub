# Report - Peak C2: typed parameters

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** C2 (Phase C - new features).
- **Status:** Done. Parameters gain types end to end:
  declaration + validation in forge-core, enforcement at
  resolve time, typed prompts in the CLI, secret masking on
  every display surface, and a parameters panel in the web
  editor fed by an extended server summary.

## 1. The format (docs/spec-snippet-format.md section 7)

`variables:` map form accepts `type:` - one of `text`
(default), `choice`, `number`, `path`, `yesno`, `secret` -
plus `choices:` for choice parameters. Declaration rules,
enforced by `Snippet::validate` (hence at parse, save, push
and import alike):

- unknown type is an error naming the valid set;
- `choice` requires a `choices:` list (max 50, no empties)
  and its `default` must be one of the choices;
- `choices:` on any other type is an error;
- a `number`/`yesno` default must itself satisfy the type;
- **a `secret` must not declare a `default`** - it would be
  stored in the file, defeating the point.

Hash stability: the new fields serialize only when present
(`skip_serializing_if`), so pre-C2 documents keep their
canonical form and content hashes.

## 2. Enforcement (forge-core `vars::resolve` + `coerce_value`)

After the classic resolution (CLI > default > built-ins) and
the missing check, every declared parameter's resolved value
is type-checked, whatever its source:

- `choice`: must equal a listed choice; the error lists them.
- `number`: must parse as a finite f64 (`inf`/`NaN` rejected).
- `yesno`: accepts yes/y/true/1 and no/n/false/0 in any case
  and is **normalised to `yes`/`no`** before substitution.
- `path`: must not be empty.
- `text`/`secret`: any string (a secret's value is never
  quoted in an error).

## 3. Typed prompts (CLI)

`resolve_or_prompt` now dispatches on the declared type:
`choice` prints a numbered menu and accepts a number or the
exact value; `number` re-asks on bad input (3 attempts);
`yesno` asks a `[y/N]` question and stores `yes`/`no`;
`secret` uses the new `prompt::ask_secret` - crossterm raw
mode, key-event loop, no echo, Ctrl-C/Esc cancels, non-TTY
returns no answer (so the standard missing-values error
fires instead of a hanging or leaking prompt). Undeclared
variables keep the plain line prompt.

## 4. Secret hygiene

- `vars::display_values` returns a copy of the value map
  with secret values replaced by `••••••`. Display surfaces
  use it: the local run plan (`render_plan`, both `--dry`
  and the confirm card), the runner's step-label fallback,
  and the step-through (`--step`) display. Execution always
  uses the unmasked map.
- The cloud card never sees values (it renders before
  prompting, from the raw body) and the run history records
  only name/timing/counts (verified in `history.rs`), so
  secrets reach neither the server, the history DB, nor any
  log. The spec now advises the prompt over `KEY=value` for
  secrets (shell history is the user's own).
- Proven by integration tests: the dry plan contains the
  mask and not the value; a real run executes with the
  value; a steps run prints the value from the step itself
  but the stderr step label shows only the mask.

## 5. Web

- Server `summary_json` gains `parameters` (name, type,
  required, default - never for secrets - choices,
  description), typed as `ParamInfo[]` in the web client.
- The editor's live panel gains a **Parameters** section
  listing each declared parameter with its type chip,
  required marker, choices, default, and the secret note,
  plus a short guide to declaring types. Editing stays in
  the YAML (the panel reflects the live validation), which
  keeps one source of truth; the panel makes types visible
  without a second editor to keep in sync.

## 6. Tests

- forge-core (+5): declaration refusal matrix (7 invalid
  documents), resolve type-checks + yesno normalisation,
  masking, pre-C2 documents unchanged.
- CLI integration (+5, `tests/params.rs`): choice/number
  enforcement with parameter-naming errors, yesno
  normalisation both ways, secret masked in plan + step
  labels but executed, secret without value/TTY is a clean
  missing error, secret-with-default refused by `fg lint`.
- Web (+1): the parameters panel renders types, choices,
  defaults and the secret note from a validate response.

## 7. Gates

Local: `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets -- -D warnings` clean (first pass
caught five lints in the new code - collapsible match,
redundant closures, a missing `# Errors` doc section, an
unused import, a match-for-one-pattern - all fixed in this
WP); `cargo test --workspace` 501 passed / 0 failed (491 at
C1 + 5 core + 5 CLI). Web: typecheck clean, lint clean, the
editor test suite green including the new panel test; the
full web suite runs in CI. CI verification of this commit's
head is recorded in PROGRESS.md.

## 8. Next

C3 - run history and rollback.
