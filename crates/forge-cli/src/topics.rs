//! `fg help <topic>` - short explainers for the concepts users
//! trip over (Peak B2). These are not command help; clap still
//! owns `fg help <command>`. `main` intercepts the topic names
//! below before clap parses, so a topic can never shadow a real
//! subcommand: command names are checked by clap first in the
//! sense that every name here is deliberately NOT a command.
//!
//! Content rules: describe the shipped behaviour only, keep the
//! safety model exact (spec-cli sections 6-8), no em dashes.

/// Look up a topic by the name the user typed. Aliases: `hash`
/// for hash-pinning, `-y` for yes. The canonical names are
/// alias, trust, hash-pinning, yes and offline (also listed in
/// the root help footer).
#[must_use]
pub fn topic(name: &str) -> Option<&'static str> {
    match name {
        "alias" => Some(ALIAS),
        "trust" => Some(TRUST),
        "hash-pinning" | "hash" => Some(HASH_PINNING),
        "yes" | "-y" => Some(YES),
        "offline" => Some(OFFLINE),
        _ => None,
    }
}

const ALIAS: &str = "\
The fg alias

`forgehub` is the binary; `fg` is the short name you type. The
shorthand `fg <name>` runs a snippet, so the alias is on the hot
path of every run.

  fg alias install     add the alias to your shell rc file
  fg alias show        show what is installed and where
  fg alias uninstall   remove it again

One trap: `fg` is also a shell builtin (job control: bring a job
to the foreground). The installer therefore adds two lines: the
`fg` alias pointing at forgehub, and `jobs-fg` for the builtin,
so job control stays one word away:

  alias fg='forgehub'
  alias jobs-fg='builtin fg'

Shells pick the rc change up in new sessions. See also:
`fg completions <shell>` for name completion, and
`fg help trust` for what happens when a name is not local.";

const TRUST: &str = "\
Trust: how ForgeHub decides what may run

Running a cloud snippet executes someone else's shell commands on
your machine, so the CLI shows its work before anything runs:

1. The confirmation card prints the FULL commands, the owner,
   the version and the short content hash. Nothing runs before
   you confirm. There is no auto-run on fetch, ever.
2. Your own snippets, and snippets already trusted at that exact
   hash, confirm with [y/N] (default No).
3. Someone else's snippet, first time: you must type the word
   `yes`. -y never skips this (see `fg help yes`).
4. If a trusted snippet's hash has changed since you approved
   it, you see a diff against the trusted version first, and
   approval is asked again for the new hash.
5. Without a terminal (scripts, CI), an unconfirmed run refuses
   and exits 2. Fail-safe beats convenient.

Managing trust:

  fg trust list                 what you have approved
  fg trust <owner/name@hash>    approve one exact version
  fg trust revoke <owner/name>  forget an approval

See `fg help hash-pinning` for what the hash pins, and
`fg help offline` for how cached snippets behave.";

const HASH_PINNING: &str = "\
Hash pinning

Every snippet version has a content hash: a short fingerprint of
its exact commands, computed locally and shown on the
confirmation card, in `fg show`, and in `fg trust list`.

Trust is pinned to the hash, not to the name. When you approve
`alice/deploy`, you approve the version whose hash you saw. If
alice publishes a new version, the hash changes, and the next
run treats it as unapproved: you get a diff against the version
you trusted and a fresh confirmation for the new hash.

Approve one exact version without running it:

  fg trust alice/deploy@<hash>
  fg trust alice/deploy@3        (a version number works too)

Pinning is what makes `fg pull` and the run cache safe to reuse:
a cached or pulled body whose hash matches a trusted pin is the
same content you approved, byte for byte.";

const YES: &str = "\
The -y flag: what it skips and what it never skips

`-y` (or `--yes`) answers ordinary confirmations for you:
deleting a local snippet, overwriting on pull, prompts whose
worst case is a local file change.

It NEVER skips the execution-safety confirmations:

- the typed `yes` for someone else's snippet, first run;
- the re-approval after a trusted snippet's hash changed
  (you see the diff first);
- any confirmation when there is no terminal at all: without a
  TTY the run refuses with exit code 2 instead of treating
  silence as consent.

Rule of thumb: -y is for scripting your own library, not for
waiving the trust model. See `fg help trust`.";

const OFFLINE: &str = "\
Offline mode

ForgeHub is local-first: record, run, list, search, edit and the
rest of the library commands never touch the network.

`--offline` (a global flag) makes that a promise: cloud commands
(login, push, pull, fork, remote) fail fast with an offline
message instead of attempting the network.

`fg run <name> --offline` resolves the name from your local
library and the run cache only. A snippet served from the cache
says so on its card - \"from cache, may be outdated\" - with the
cached version and hash, so you always know the content might
not be the latest. The trust rules are unchanged offline: an
untrusted cached snippet still needs its confirmation, and a
changed hash still shows a diff (see `fg help trust`).

`fg cache list` shows what is cached; `fg cache clear` empties
it.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_topic_resolves() {
        for name in ["alias", "trust", "hash-pinning", "yes", "offline"] {
            let text = topic(name).unwrap_or_else(|| panic!("topic {name}"));
            assert!(text.len() > 200, "topic {name} is suspiciously short");
            assert!(!text.contains('\u{2014}'), "topic {name} has an em dash");
        }
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(topic("hash"), topic("hash-pinning"));
        assert_eq!(topic("-y"), topic("yes"));
    }

    #[test]
    fn unknown_topics_and_commands_do_not_resolve() {
        assert!(topic("run").is_none());
        assert!(topic("list").is_none());
        assert!(topic("nonsense").is_none());
        assert!(topic("").is_none());
    }

    #[test]
    fn trust_topics_state_the_safety_invariants() {
        let trust = topic("trust").unwrap();
        assert!(trust.contains("type the word"), "{trust}");
        assert!(trust.contains("exits 2"), "{trust}");
        let yes = topic("yes").unwrap();
        assert!(yes.contains("NEVER"), "{yes}");
        let offline = topic("offline").unwrap();
        assert!(offline.contains("from cache, may be outdated"), "{offline}");
    }
}
