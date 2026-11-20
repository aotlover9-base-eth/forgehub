# Migrating from Forgenesis

ForgeHub is the successor to Forgenesis (the local-only tool);
its local feature set was ported deliberately, and the
analysis behind every keep/redesign/drop decision is in
`docs/forgenesis-analysis.md`.

## Importing your library

```console
$ fg import --from-forgenesis <your-forgenesis-home>
``` Each Forgenesis
workflow becomes a ForgeHub snippet in `~/.forgehub/snippets`:

- Forgenesis YAML is read with the same parser family; files
  that fail ForgeHub validation are reported by name with the
  reason, and the rest still import.
- Commands, descriptions and tags carry over; ForgeHub adds
  the canonical form (sorted tags, normalised newlines) and a
  content hash on first save.
- History and statistics do **not** migrate - ForgeHub starts
  its own `history.db`. Your Forgenesis directory is never
  modified.

After importing, run `fg doctor` and `fg list` to sanity-
check the result, then `fg backup` before you delete anything.

## What changed on purpose

| Forgenesis | ForgeHub |
|---|---|
| Local only | Local + optional cloud (login, push/pull, fork) |
| `sync` of a directory | Dropped - superseded by push/pull with version checks (see the analysis doc) |
| Packs | Dropped - tags + search cover the use case |
| `config` subcommand | Dropped - `config.yaml` is a plain file you edit |
| Runs without confirmation | Cloud runs confirm with the trust model; local runs keep the classic prompt |

## Staying on Forgenesis

Nothing breaks: ForgeHub uses `~/.forgehub`, Forgenesis keeps
its own home, and the two can coexist. If both provide an
`fg` on your PATH, ForgeHub's alias block is clearly marked
in your rc file and removable with `fg alias uninstall`.
