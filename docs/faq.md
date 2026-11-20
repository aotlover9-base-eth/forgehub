# FAQ

### Is it safe to run someone else's snippet?

Safer than copy-pasting from a web page - which is the
alternative ForgeHub replaces - but never "safe" in absolute
terms, and ForgeHub does not claim sandboxing. What you get:
the **full command list before anything runs**, the owner's
name, version and content hash; a typed `yes` for a first run;
a **diff and a fresh confirmation if the content ever
changes**; lint warnings for known-dangerous patterns; and a
secret scanner keeping credentials out of snippets. You are
still the last line of defence: read the commands. If you
wouldn't paste it into your terminal, don't run it.

### How do I stop a snippet while it's running?

Ctrl-C, like any shell command. The run is recorded as
interrupted and doesn't count in your stats. In steps mode,
`--step` confirms each step before it runs, and a failed step
stops the run unless you pass `--keep-going`.

### Why does `fg` need an alias? Can't it just be a binary named `fg`?

Because your shell already has an `fg`: the job-control
builtin, which shadows any `fg` file on your PATH. The alias
is the reliable route. `forgehub alias install` writes a
clearly marked block to your shell's rc file (backing it up
first) defining `fg` for ForgeHub and `jobs-fg` for the
builtin; `alias uninstall` removes exactly that block. The
installer script offers to run it - it never edits rc files
itself.

### I forgot my password. Now what?

v0.1.0 has no email system, so there is no self-serve reset
(see `docs/known-gaps.md`). On your own instance, the operator
can help you; on someone else's, contact them. Choose a
password you store in a password manager.

### Can I use ForgeHub without the cloud?

Yes - the local library is complete on its own (record, run,
analyze, backup, TUI). Cloud commands simply stay unused;
`--offline` makes the CLI refuse network access up front.
Your snippets live as plain YAML files in `~/.forgehub`,
readable and greppable without ForgeHub at all.

### Does ForgeHub phone home?

No. No telemetry, no analytics, no third-party requests from
the web app (fonts and icons are self-hosted). The CLI talks
only to the registry you configure, plus - for
`fg update --check`, only when you run it - the release API.
`update` never downloads or replaces anything by itself.

### What happens if the server is down mid-push?

Pushes are version-checked transactions: either the new
version exists completely or the push failed and your local
copy is untouched. `fg status` shows where you stand, and
retrying a push whose content already arrived answers
"up to date" instead of duplicating.

### Windows?

The CLI builds for Windows (release matrix includes
`x86_64-pc-windows-msvc`) and local snippet management works;
running snippets assumes a POSIX shell, and the alias feature
is Unix-shell only. Those limits are listed in
`docs/known-gaps.md`.
