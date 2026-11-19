# Spec — Snippet format (the "forge file")

- **Status:** Normative for Steps 3, 4, 9, 12, 13. **Version:** 1 (Step 2, 2026-11-19)
- **Source:** `roadmap.md` Appendix A, refined. Where this spec and Appendix A differ, this spec wins and the difference is recorded in §13 and in the Step 2 report.

A snippet is a single YAML document (the *forge file*) describing either one
shell **script** or a list of **steps**. Everything is UTF-8 text. There is no
file, binary or attachment form.

## 1. Limits (hard, enforced by `forge-core` and re-checked by the server)

| Item | Limit |
|------|-------|
| Serialised forge file / raw script body | ≤ 65,536 bytes (64 KiB), measured after LF normalisation (§9) |
| `name` | regex §8, ≤ 64 chars |
| `description` | ≤ 280 chars, single line (no `\n`) |
| `tags` | ≤ 20, each regex §8, ≤ 30 chars |
| Steps (`steps` + `always` + `parallel` branches) | ≤ 256 steps total |
| Single step command / single script line | ≤ 8,192 bytes |
| Variables declared | ≤ 64 |
| Variable name | `^[A-Za-z_][A-Za-z0-9_]*$`, ≤ 64 chars |
| `env` entries (workflow + per-step combined view) | ≤ 128 keys, key regex as variable name, value ≤ 1,024 chars |
| Nesting depth of YAML values | ≤ 16 (aliases/anchors: §10) |
| `wait` / `timeout` durations | `500ms`, `5s`, `2m`, `1h` grammar; value ≤ 24h |
| `retry.times` | 1–10 |

Violations are validation errors with the field path and, where the YAML
parser provides it, line/column. Nothing is silently truncated.

## 2. Top-level fields

| Field | Type | Required | Default | Notes |
|-------|------|----------|---------|-------|
| `name` | string | yes | — | §8. In a stored file it must equal the address name; on import a filename stem may supply it (Forgenesis behaviour) only when `name` is absent — see §12 |
| `description` | string | no | empty | ≤ 280 chars |
| `tags` | list of string | no | `[]` | §8; duplicates removed in canonical form (§9) |
| `mode` | `script` \| `steps` | no | `script` if `script:` present, else `steps` | Explicit `mode` conflicting with the body present is an error |
| `script` | string (block scalar) | in script mode | — | The shell text, §3 |
| `steps` | list of step (§5) | in steps mode | — | At least 1 step unless `parallel`/`always` present |
| `parallel` | list of string | no | — | Top-level shorthand: appended as one parallel step after `steps` (Forgenesis) |
| `shell` | string | no | `bash` | One of `bash`, `sh`, `zsh`, `fish` for script mode; steps run via the configured shell (§6 of `spec-cli.md`). The executable must exist locally at run time |
| `variables` | map name → param (§7) | no | `{}` | Aliases accepted on parse: `vars`, `params` |
| `env` | map string → string | no | `{}` | Applied to every step / the script process |
| `cwd` | string | no | caller's cwd | Supports `~` and `{{VAR}}`; relative paths resolve against the caller's cwd |
| `retry` | retry spec (§5) | no | none | Default retry for steps without their own |
| `always` | list of step | no | `[]` | Run at the end even after a failure; failures in `always` are reported but do not mask the original failure |
| `created` | string (RFC 3339) | no | — | Informational; excluded from the content hash |
| `source` | string | no | — | Informational provenance (e.g. `record`, import path); excluded from the content hash |

Unknown top-level fields are an **error** (`deny_unknown_fields` semantics),
except in the Forgenesis import path (§12), which drops the fields it maps to
nothing and reports each drop.

## 3. Script mode

```yaml
mode: script
script: |
  <any multi-line shell text>
```

- The body is one shell invocation: `cd`, `export` and functions persist
  across lines. Execution mechanics (temp file, permissions) are ADR 0004.
- The web "Script" tab accepts raw pasted text; the server wraps it as
  `{name, description, tags, mode: script, shell: bash, script: <text>}`.
- An empty or whitespace-only script is an error.
- The body is preserved byte-exact apart from LF normalisation and exactly
  one trailing newline (§9).

## 4. Steps mode

Each step runs in its **own** shell process. A bare `cd` step therefore has
no effect on later steps; recorders convert a leading `cd` into `cwd`
(Forgenesis behaviour, kept). State that must persist belongs in script mode.

## 5. Step fields

A step is a bare string, a boolean (YAML reads unquoted `true`/`false` as a
command — accepted, as in Forgenesis), or a map:

| Field | Type | Aliases | Notes |
|-------|------|---------|-------|
| `command` | string | `run`, `cmd`, `exec` | Exactly one of `command` / `parallel` per step |
| `parallel` | list of string | — | Commands run concurrently, chunked by `parallel_limit` (config, default 4) |
| `name` | string | — | Display label, ≤ 120 chars |
| `if` | condition (§6) | — | Step is skipped when the condition is false |
| `wait` | duration | — | Sleep **after** the step succeeds |
| `retry` | integer 1–10, or `{times, delay}` | `attempts`, `count` for `times` | Retries the step on non-zero exit |
| `cwd` | string | `workdir` | Relative to the workflow `cwd` |
| `env` | map string → string | — | Step-only environment |
| `continue_on_error` | bool | `ignore_errors` | Default `false` |
| `background` | bool | — | Detach; exit status is not collected |
| `quiet` | bool | — | Suppress the step's stdout/stderr |
| `timeout` | duration | — | Kill the step after this long; counts as a failure |

Unknown step fields are an error, as at the top level.

## 6. Conditions (`if`)

A bare string is a path-existence shorthand: `if: package.json` means "run
only if `package.json` exists (relative to the step's cwd)". A map may set
exactly the applicable keys; all set keys must hold (logical AND):

| Key | Type | Meaning |
|-----|------|---------|
| `exists` | path | Path must exist |
| `missing` | path | Path must not exist |
| `which` | string | Executable must be on `PATH` |
| `env` | string | Env var (after workflow `env`) is set and non-empty |
| `equals` | `[string, string]` | The two values are equal **after** variable substitution |
| `shell` | string | Shell snippet exits 0 |

## 7. Variables

Syntax: `{{NAME}}` (optional inner spaces: `{{ NAME }}`), matched by
`\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}`. Substitution is **whole-placeholder
only** — there is no partial or nested expansion, and `{{` inside a value is
never re-expanded.

Declaration forms:

```yaml
variables:
  env: staging                      # bare string = default value
  target:                           # map form
    default: prod
    description: Target environment
    required: true
```

**Resolution order** (first hit wins):

1. CLI arguments `KEY=value` (and, for the web, nothing — the web never runs snippets)
2. Declared `default`
3. Built-ins: `HOME`, `PWD`, `DIRNAME` (basename of `PWD`), `DATE` (`YYYY-MM-DD`), `TIME` (`HH:MM:SS`), `TIMESTAMP` (`YYYYMMDD-HHMMSS`), `USER`
4. Interactive prompt (TTY only, Step 5)
5. Error listing **all** missing names at once, with a copy-pasteable `KEY=value` hint

A variable declared `required: true` with no default and no value is missing
under rule 5. Declared-but-unreferenced variables are allowed (they document
intent). Referenced-but-undeclared variables resolve by the same order.

Substitution applies to: step commands and parallel branches, workflow and
step `cwd`, workflow and step `env` values, condition `equals` operands, and
the script body in script mode.

`$VAR` / `${VAR}` are **not** substituted by ForgeHub — they are left for the
shell, which expands them at run time from the process environment (including
workflow `env`). Path expansion of a leading `~` applies only to `cwd`
fields, never to command text.

## 8. Names

| Kind | Regex | Extra rules |
|------|-------|-------------|
| Snippet name | `^[a-z0-9][a-z0-9._-]{0,63}$` | No `..`; does not end with `.`; not a reserved word (below) |
| Username | `^[a-z0-9][a-z0-9-]{1,38}$` | ASCII only; no consecutive hyphens (`--`); does not end with `-`; not reserved |
| Tag | `^[a-z0-9][a-z0-9-]{0,29}$` | Normalised to lowercase before validation; uppercase input is folded, not rejected, **only** for tags |

Names are case-sensitive storage keys that are only ever valid in lowercase:
mixed-case snippet names and usernames are **rejected**, not folded (folding
would create silent collisions — see the Step 2 consistency notes).

**Reserved words** (cannot be snippet names; usernames additionally cannot be
any of these): every CLI subcommand — `run`, `init`, `record`, `login`,
`logout`, `whoami`, `push`, `pull`, `fork`, `status`, `diff`, `list`, `show`,
`search`, `tag`, `rename`, `clone`, `delete`, `edit`, `import`, `export`,
`analyze`, `parameterize`, `stats`, `backup`, `restore`, `doctor`, `alias`,
`completions`, `ui`, `cache`, `update`, `trust`, `remote`, `help`, `version`,
`config` — plus `me`, `api`, `admin`, `settings`, `explore`, `new`, `device`,
`signup`, `static`, `assets`, `health`, `tokens`, `users`, `snippets`.

This list is the single source: `forge-core` owns it (Step 3), the CLI's
subcommand table and the server's route table are tested against it.

## 9. Canonical serialisation and content hash (normative)

The content hash is `SHA-256` over the **canonical bytes**, defined exactly:

1. Parse the document; validation must pass first.
2. Normalise all line endings (`\r\n`, `\r`) to `\n`. Reject NUL and any
   control character other than `\n` and `\t` anywhere in the document.
3. Build a JSON object (this is a serialisation for hashing, not a storage
   format) with keys in this fixed order, omitting absent/empty fields
   (`description` when empty, `tags` when empty, `env`/`variables` when
   empty, `cwd`/`shell` when at their defaults `null`/`bash` — `shell` is
   **included** when explicitly set, even to `bash`):
   `name, description, tags, mode, shell, variables, env, cwd, retry, steps, parallel, script, always`.
   `created` and `source` are **excluded**.
4. `tags`: sorted lexicographically (byte order), deduplicated.
   `variables` and `env`: keys sorted (byte order). Steps keep their order
   (order is semantics). Step objects use this fixed key order, omitting
   defaults: `name, command, parallel, if, wait, retry, cwd, env, continue_on_error, background, quiet, timeout`; condition objects: `exists, missing, which, env, equals, shell`; retry objects: `times, delay`; param objects: `default, description, required` (omitting `required` when `false`).
5. Strings: structural strings (everything except the script body) have
   trailing spaces/tabs on each line trimmed and leading/trailing blank
   lines removed. The **script body** is preserved byte-exact after step 2,
   then forced to end with exactly one `\n`.
6. Numbers serialise as bare integers. Booleans as `true`/`false`.
7. Serialise the JSON with no insignificant whitespace, UTF-8, keys in the
   orders above (not alphabetical — the lists above are the order).
8. `hash = lowercase_hex(SHA-256(canonical_bytes))`. Short hash = first 8
   hex characters, displayed as `sha256:<short>`.

Consequences that Step 3 must prove with property tests: reordering map keys
in the YAML, changing YAML quoting style, CRLF vs LF, and adding/removing
`created`/`source` do not change the hash; changing any command character,
step order, tag set, or variable default does.

## 10. YAML parsing rules

- YAML 1.2 core schema via `serde_yaml` (Step 3). Duplicate mapping keys are
  an error. Anchors/aliases are **rejected** (they are a billion-laughs /
  expansion vector and never needed in a forge file — see threat model T-DOS-2).
- Tags/directives (`!!str`, `%YAML`, …) are rejected.
- The document must be a single mapping at the top level.

## 11. Worked examples

### 11.1 The Ubuntu setup one-liner (script mode, as pasted on the web)

```yaml
name: ubuntu-setup
description: Update an Ubuntu machine and install the basics
tags: [ubuntu, setup]
mode: script
shell: bash
script: |
  sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux ca-certificates
```

Raw web input that produces exactly this file: name `ubuntu-setup`,
description as above, tags `ubuntu, setup`, body
`sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux ca-certificates`.

### 11.2 Multi-line script with state

```yaml
name: bootstrap-node
description: Install Node dependencies in a fresh clone
mode: script
script: |
  set -e
  cd ~/projects/app
  export NODE_ENV=development
  npm ci
  npm run build
```

(`cd` and `export` persist because this is one shell invocation.)

### 11.3 Steps mode with variables, condition, retry, always

```yaml
name: deploy-site
description: Build and publish the static site
mode: steps
variables:
  env: { default: staging, description: "Target environment" }
cwd: ~/projects/site
steps:
  - npm ci
  - { name: build, command: "npm run build -- --mode {{env}}", retry: 2, timeout: 5m }
  - { command: "rsync -a dist/ server:/var/www/{{env}}/", if: { which: rsync } }
always:
  - echo "done at $(date)"
```

(`retry: 2` is the integer form; `timeout: 5m` is a duration string.)

### 11.4 Bare-string and map steps mixed, path condition shorthand

```yaml
name: dev-up
mode: steps
steps:
  - git pull origin main
  - { run: pnpm install, if: package.json }
  - { command: docker compose up -d, wait: 5s, retry: { times: 3, delay: 2s } }
```

### 11.5 Parallel step and top-level parallel shorthand

```yaml
name: watch-all
mode: steps
steps:
  - { parallel: ["pnpm dev", "pnpm test --watch"] }
parallel:
  - echo "started"
```

### 11.6 Per-step env, cwd and background

```yaml
name: serve-docs
mode: steps
env: { PORT: "4000" }
steps:
  - { command: npm run docs:serve, cwd: docs, background: true, quiet: true }
  - { command: "curl -sf http://127.0.0.1:4000", retry: { times: 5, delay: 1s } }
```

### 11.7 `always` cleanup and `continue_on_error`

```yaml
name: test-with-db
mode: steps
steps:
  - { command: docker compose up -d db }
  - { command: npm test, continue_on_error: true }
always:
  - docker compose down
```

### 11.8 Recorded workflow (what `fg record` writes)

```yaml
name: release-check
description: Recorded with fg record
mode: steps
cwd: ~/projects/app
created: 2026-11-19T09:00:00+05:30
source: record
steps:
  - cargo fmt --all --check
  - cargo clippy --workspace --all-targets -- -D warnings
  - cargo test --workspace
```

### 11.9 Invalid examples (must be rejected)

```yaml
# REJECT: uppercase in name, unknown field, script+steps conflict
name: Deploy_Site
mode: script
script: "echo hi"
steps: ["echo also"]
bogus_field: true
```

```yaml
# REJECT: body over 64 KiB, secret in a step (secret scan, Step 3 §scanner),
# and a YAML alias
name: too-big
mode: steps
steps:
  - &a "export TOKEN=ghp_000000000000000000000000000000000000"
  - *a
```

## 12. Forgenesis compatibility

Reference: `docs/forgenesis-analysis.md` §2 (Forgenesis at `f69cdf2`).

| Forgenesis construct | ForgeHub result |
|----------------------|-----------------|
| Bare-string, map and boolean steps; `command` aliases `run`/`cmd`/`exec` | Imports cleanly |
| `retry: 3` and `retry: {times, delay}`; `times` aliases `attempts`/`count` | Imports cleanly |
| `if: path` shorthand and all six condition keys | Imports cleanly |
| `wait`, `cwd` (alias `workdir`), `env`, `continue_on_error` (alias `ignore_errors`), `background`, `quiet`, `timeout`, `parallel` per-step and top-level | Imports cleanly |
| `variables` aliases `vars`/`params`; bare-string and map params (`def` alias for `default`) | Imports cleanly (`def` accepted on parse, never emitted) |
| Top-level `cwd`, `shell`, `env`, `retry`, `always`, `tags`, `description` | Imports cleanly |
| `created`, `source` | Kept as informational fields; excluded from the hash |
| Uppercase or mixed-case workflow names | **Converted:** lowercased on import with a printed warning; if lowercasing collides with an existing name, the import of that file is rejected and reported (never silently overwritten) |
| Names with characters outside §8 after lowercasing | **Rejected** with the validation error |
| Workflows with no `mode` and no `script:` | Treated as steps mode (Forgenesis has no script mode) |
| Unknown top-level/step fields | **Rejected** in native mode; in `--from-forgenesis` import mode each unknown field is dropped with a printed warning, because Forgenesis's schema may grow fields ForgeHub does not model |
| Forgenesis `parallel` steps combined with `command` on the same step | **Rejected** (Forgenesis validates this too) |

## 13. Differences from roadmap Appendix A (resolved here)

1. Appendix A allowed a step `retry` as a bare integer in examples but did
   not bound it; §1/§5 bound it to 1–10 and durations to 24h (DoS hygiene).
2. Appendix A did not state a steps cap; §1 caps total steps at 256.
3. Appendix A said tags are lowercase by regex but did not say whether
   uppercase input folds or fails; §8 folds **tags only**, and rejects
   mixed-case names/usernames (collision safety).
4. Canonical serialisation in Appendix A was a sketch ("Step 2 finalises");
   §9 is the final, implementable definition, including the exclusion of
   `created`/`source` and the explicit-`shell` rule.
5. YAML anchors/aliases: Appendix A was silent; §10 rejects them (threat
   model T-DOS-2).
