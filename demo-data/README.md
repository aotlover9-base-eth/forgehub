# Demo data pack

**105 ready-to-use snippets** for a new ForgeHub library — system
administration, files, logs, network, Docker, git, monitoring,
dev workflows, backups, security and media one-liners. Some use
variables (`{{PORT}}`, `{{DIR}}`, …) with defaults, so they prompt
only when it matters.

## Load it

```console
sh demo-data/load.sh
```

1. Imports every file in `snippets/` into your local library.
   Names you already have are skipped, never overwritten.
2. If you're logged in (`forgehub whoami`), pushes each snippet
   that isn't on your server yet — **private by default**, under
   your account.

Local only, no server: `sh demo-data/load.sh --local-only`

Re-running is safe: both steps skip what's already there.

## Make one public later

```console
forgehub push <name> --public
```

## Remove them

Local: `forgehub delete <name>` · Server: `forgehub remote delete <name>`

## Notes

- Snippets are plain forge files (`snippets/*.yml`) — read them,
  edit them, take only the ones you like:
  `forgehub import demo-data/snippets/<name>.yml`
- Distro-specific ones are tagged (`apt`, `pacman`, `dnf`) so you
  can ignore the ones that don't match your system.
