//! `fg analyze` - mine shell history for repeated command sequences.
//!
//! The mining pipeline itself (parse bash/zsh/fish history, drop
//! noise and anything secret-shaped, segment into sessions, mine
//! repeated sequences) lives in `forge_core::history_mine` since
//! Peak C5, so the server's web history import runs the identical
//! code. This module keeps the CLI-only parts: where the history
//! file lives, the configured ignore list, and the file pipeline.
//! Secret scrubbing happens inside the shared filter - **before**
//! anything is printed or written.

use crate::store::Config;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub use forge_core::history_mine::{
    estimated_savings_secs, mine_sequences, parse_history_text, segment_sessions, Candidate,
    HistEntry,
};

/// Where the shell keeps its history, honouring config and env
/// overrides: config `history_file`, then `$HISTFILE`, then the
/// per-shell default locations under `$HOME`.
#[must_use]
pub fn history_path(cfg: &Config) -> Option<PathBuf> {
    if let Some(f) = &cfg.history_file {
        if !f.trim().is_empty() {
            return Some(expand_home(f));
        }
    }
    if let Ok(hf) = std::env::var("HISTFILE") {
        if !hf.trim().is_empty() {
            let p = PathBuf::from(hf);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let home = forge_core::vars::home_dir()?;
    let shell = cfg
        .default_shell
        .clone()
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "bash".into());
    let shell = Path::new(&shell)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("bash");
    let preferred: &[&str] = match shell {
        "zsh" => &[".zsh_history", ".histfile", ".bash_history"],
        "fish" => &[".local/share/fish/fish_history", ".bash_history"],
        _ => &[".bash_history", ".zsh_history"],
    };
    for name in preferred {
        let p = home.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = forge_core::vars::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Parse a history file. Handles bash plain lines, bash `#<epoch>`
/// timestamp pairs, zsh extended `: <epoch>:<elapsed>;cmd`, and fish
/// YAML-ish records.
///
/// # Errors
/// When the file cannot be read. Invalid UTF-8 never fails: history
/// files routinely contain it, so reading is lossy.
pub fn parse_history(path: &Path) -> Result<Vec<HistEntry>> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    Ok(parse_history_text(&raw))
}

/// Drop noise (`ls`, `cd`, editors, config `ignore_commands`) and
/// anything that looks like a secret - by the marker screen **or**
/// the `forge-core` scanner (see `forge_core::history_mine`).
#[must_use]
pub fn filter_commands(entries: &[HistEntry], cfg: &Config) -> Vec<HistEntry> {
    forge_core::history_mine::filter_entries(entries, &|cmd| cfg.is_ignored(cmd))
}

/// The outcome of analyzing one history file.
pub struct Analysis {
    /// Entries parsed from the file.
    pub entries: usize,
    /// Entries left after noise/secret filtering.
    pub filtered: usize,
    /// Sessions after time-gap segmentation.
    pub sessions: usize,
    /// Ranked candidates (already truncated to the limit).
    pub candidates: Vec<Candidate>,
}

/// Full pipeline: file → ranked candidates.
///
/// # Errors
/// When the history file cannot be read.
pub fn analyze_file(path: &Path, cfg: &Config, limit: usize) -> Result<Analysis> {
    let entries = parse_history(path)?;
    let filtered = filter_commands(&entries, cfg);
    let sessions = segment_sessions(&filtered, 30 * 60);
    let mut found = mine_sequences(
        &sessions,
        2,
        cfg.analyze_max_len.max(2),
        cfg.analyze_min_count.max(2),
    );
    found.truncate(limit);
    Ok(Analysis {
        entries: entries.len(),
        filtered: filtered.len(),
        sessions: sessions.len(),
        candidates: found,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::history_mine::{looks_sensitive, normalize};

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn parses_plain_bash_history() {
        let got = parse_history_text("git pull\npnpm install\n\ndocker compose up -d\n");
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].command, "git pull");
        assert!(got[0].timestamp.is_none());
    }

    #[test]
    fn parses_bash_timestamped_history() {
        let got = parse_history_text("#1700000000\ngit pull\n#1700000060\npnpm install\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].timestamp, Some(1_700_000_000));
        assert_eq!(got[1].command, "pnpm install");
    }

    #[test]
    fn parses_zsh_extended_history() {
        let got = parse_history_text(": 1700000000:0;git pull\n: 1700000005:12;pnpm install\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].command, "git pull");
        assert_eq!(got[1].timestamp, Some(1_700_000_005));
    }

    #[test]
    fn parses_fish_history() {
        let text = "- cmd: git pull\n  when: 1700000000\n- cmd: pnpm install\n  when: 1700000010\n";
        let got = parse_history_text(text);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].timestamp, Some(1_700_000_000));
    }

    #[test]
    fn normalizes_whitespace_and_trailing_tokens() {
        assert_eq!(normalize("git   pull  "), "git pull");
        assert_eq!(normalize("docker compose up -d;"), "docker compose up -d");
        assert_eq!(normalize("code . &"), "code .");
    }

    #[test]
    fn filters_noise_and_secrets() {
        let entries: Vec<HistEntry> = [
            "ls -la",
            "git pull",
            "cd /tmp",
            "export API_KEY=$FROM_VAULT",
            "pnpm install",
        ]
        .iter()
        .map(|c| HistEntry {
            command: c.to_string(),
            timestamp: None,
        })
        .collect();
        let got = filter_commands(&entries, &cfg());
        let cmds: Vec<&str> = got.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(cmds, vec!["git pull", "pnpm install"]);
    }

    #[test]
    fn secret_screen_catches_common_shapes() {
        assert!(looks_sensitive("curl -H 'Authorization: Bearer abc' x"));
        assert!(looks_sensitive("mysql -u root --password=hunter2"));
        assert!(looks_sensitive("export AWS_SECRET_ACCESS_KEY=zz"));
        assert!(!looks_sensitive("git push origin main"));
    }

    #[test]
    fn sessions_split_on_long_gaps() {
        let entries = vec![
            HistEntry {
                command: "a".into(),
                timestamp: Some(1000),
            },
            HistEntry {
                command: "b".into(),
                timestamp: Some(1060),
            },
            HistEntry {
                command: "c".into(),
                timestamp: Some(1000 + 10_800),
            },
        ];
        let sessions = segment_sessions(&entries, 1800);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0], vec!["a", "b"]);
        assert_eq!(sessions[1], vec!["c"]);
    }

    #[test]
    fn mines_the_repeated_setup_sequence() {
        let seq = ["git pull", "pnpm install", "docker compose up -d", "code ."];
        let mut session: Vec<String> = Vec::new();
        for _ in 0..4 {
            session.extend(seq.iter().map(ToString::to_string));
            session.push("git status".into());
        }
        let found = mine_sequences(&[session], 2, 6, 3);
        assert_ne!(found.len(), 0);
        assert!(found[0].commands.len() >= 4, "got {:?}", found[0].commands);
        assert_eq!(&found[0].commands[..4], &seq[..]);
        assert_eq!(found[0].count, 4);
    }

    #[test]
    fn counts_non_overlapping_occurrences() {
        let session: Vec<String> = "a b a b a b".split(' ').map(String::from).collect();
        let found = mine_sequences(&[session], 2, 2, 2);
        let ab = found
            .iter()
            .find(|c| c.commands == vec!["a".to_string(), "b".to_string()])
            .expect("a b should be found");
        assert_eq!(ab.count, 3);
    }

    #[test]
    fn ignores_a_single_command_repeated() {
        let session: Vec<String> = std::iter::repeat_n("git status".to_string(), 10).collect();
        assert_eq!(mine_sequences(&[session], 2, 4, 2).len(), 0);
    }

    #[test]
    fn shorter_subsequences_are_suppressed() {
        let seq = ["git pull", "pnpm install", "pnpm build"];
        let mut session = Vec::new();
        for i in 0..5 {
            session.extend(seq.iter().map(ToString::to_string));
            session.push(format!("noise-{i}"));
        }
        let found = mine_sequences(&[session], 2, 5, 3);
        assert_eq!(found[0].commands.len(), 3);
        let prefix = vec!["git pull".to_string(), "pnpm install".to_string()];
        assert!(!found.iter().any(|c| c.commands == prefix));
    }

    #[test]
    fn rotations_of_one_loop_are_reported_once() {
        let mut session = Vec::new();
        for _ in 0..4 {
            session.push("git pull origin main".to_string());
            session.push("pnpm install".to_string());
            session.push("pnpm dev".to_string());
        }
        let found = mine_sequences(&[session], 2, 5, 2);
        assert_eq!(found.len(), 1, "got {found:#?}");
        assert_eq!(found[0].commands.len(), 3);
    }

    #[test]
    fn respects_min_count() {
        let session: Vec<String> = ["a", "b", "c", "a", "b", "c"]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            mine_sequences(std::slice::from_ref(&session), 2, 4, 3).len(),
            0
        );
        assert_ne!(mine_sequences(&[session], 2, 4, 2).len(), 0);
    }

    #[test]
    fn suggests_readable_names() {
        let c = Candidate {
            commands: vec!["docker compose up -d".into(), "code .".into()],
            count: 3,
            score: 1.0,
        };
        assert_eq!(c.suggested_name(), "docker-compose");
        let c2 = Candidate {
            commands: vec!["pnpm install".into()],
            count: 3,
            score: 1.0,
        };
        assert_eq!(c2.suggested_name(), "pnpm-install");
        let c3 = Candidate {
            commands: vec!["make -j8".into()],
            count: 3,
            score: 1.0,
        };
        assert_eq!(c3.suggested_name(), "make");
    }

    #[test]
    fn end_to_end_on_a_history_file() {
        let dir =
            std::env::temp_dir().join(format!("forgehub-analyze-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history");
        let mut text = String::new();
        let mut ts = 1_700_000_000i64;
        for _ in 0..5 {
            for cmd in ["git pull", "pnpm install", "docker compose up -d", "ls -la"] {
                use std::fmt::Write as _;
                let _ = write!(text, "#{ts}\n{cmd}\n");
                ts += 30;
            }
            ts += 60;
        }
        std::fs::write(&path, text).unwrap();
        let found = analyze_file(&path, &cfg(), 5).unwrap();
        assert_ne!(found.candidates.len(), 0);
        assert!(!found.candidates[0]
            .commands
            .iter()
            .any(|c| c.starts_with("ls")));
        assert!(found.candidates[0]
            .commands
            .contains(&"git pull".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
