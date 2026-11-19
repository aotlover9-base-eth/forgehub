# ADR 0003 — Two snippet modes: `script` and `steps`

- **Status:** Accepted (Step 2, 2026-11-19)

## Context
Two real authoring styles exist. On the web a person pastes a block of shell
text and expects `cd`/`export` to persist across its lines. `fg record`
produces a list of discrete commands, each historically run in its own
shell, with per-step conditions, retries and timeouts (Forgenesis model).
Forcing either style into the other loses semantics: splitting a script on
newlines breaks heredocs, functions and multi-line constructs; joining
steps into one script breaks per-step conditions/retries and changes
failure behaviour.

## Decision
Keep both modes as first-class citizens of one format (spec-snippet-format
§§3–5): `mode: script` with a single `script:` body, and `mode: steps` with
structured steps. One parser, one validator, one hash covers both; the mode
is part of the canonical form. The web defaults to Script, the recorder
produces Steps.

## Alternatives rejected
- **Script only, steps as a UI convenience that joins lines:** breaks
  Forgenesis compatibility and per-step semantics.
- **Steps only, script pasted as one giant step:** a one-step "steps"
  snippet is actually a script with worse display and no line semantics in
  the UI; the distinction would reappear informally.
- **Two separate formats:** doubles validation, hashing, editor and API
  surface for no user benefit.

## Consequences
The runner has two execution paths (ADR 0004 for the script path), and the
editor has two tabs (Step 16). Both are specified in one document so the
CLI, server and web cannot drift.
