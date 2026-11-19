//! `fg analyze` — mine shell history for repeated command sequences.
//!
//! Ported from Forgenesis `analyze.rs` (see
//! `docs/forgenesis-analysis.md`): parse bash/zsh/fish history, drop
//! noise and anything secret-shaped, segment into sessions by time
//! gaps, and mine non-overlapping repeated sequences with
//! fragment/rotation suppression. Secret scrubbing happens at the
//! filter stage — **before** anything is printed or written — and uses
//! both the Forgenesis marker screen and the `forge-core` scanner.
//!
//! Everything except file IO is pure functions over `Vec<HistEntry>`,
//! so the mining logic is testable without touching a shell.

use crate::store::Config;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One line of shell history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistEntry {
    /// The command text.
    pub command: String,
    /// Unix seconds, when the history format records it.
    pub timestamp: Option<i64>,
}

/// A repeated command sequence worth turning into a snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The sequence, in order.
    pub commands: Vec<String>,
    /// How many non-overlapping times the sequence occurs.
    pub count: u32,
    /// Ranking heuristic: longer sequences repeated more often score higher.
    pub score: f64,
}

impl Candidate {
    /// Suggested snippet name, derived from the commands themselves:
    /// `docker compose up` → `docker-compose`, `git pull` → `git-pull`.
    #[must_use]
    pub fn suggested_name(&self) -> String {
        let first = self.commands.first().map_or("workflow", String::as_str);
        let head = first.split_whitespace().next().unwrap_or("workflow");
        let sub = first.split_whitespace().nth(1).unwrap_or("");
        let base = if sub.is_empty() || sub.starts_with('-') {
            head.to_string()
        } else {
            format!("{head}-{sub}")
        };
        let cleaned: String = base
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let cleaned = cleaned.trim_matches('-').to_lowercase();
        if cleaned.is_empty() || forge_core::names::validate_snippet_name(&cleaned).is_err() {
            "workflow".into()
        } else {
            cleaned
        }
    }
}

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
    let home = PathBuf::from(std::env::var_os("HOME")?);
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
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
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

/// Parse history text in any of the supported formats.
#[must_use]
pub fn parse_history_text(text: &str) -> Vec<HistEntry> {
    let mut out: Vec<HistEntry> = Vec::new();
    let mut pending_ts: Option<i64> = None;
    for line in text.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }
        // fish: `- cmd: git pull` followed by `  when: 1700000000`
        if let Some(rest) = line.strip_prefix("- cmd: ") {
            out.push(HistEntry {
                command: unescape_fish(rest.trim()),
                timestamp: None,
            });
            continue;
        }
        if let Some(rest) = line.trim_start().strip_prefix("when: ") {
            if let Some(last) = out.last_mut() {
                last.timestamp = rest.trim().parse().ok();
            }
            continue;
        }
        if line.trim_start().starts_with("paths:") || line.trim_start().starts_with("- /") {
            continue;
        }
        // zsh extended history: `: 1700000000:0;git pull`
        if let Some(rest) = line.strip_prefix(": ") {
            if let Some((meta, cmd)) = rest.split_once(';') {
                let ts = meta.split(':').next().and_then(|s| s.trim().parse().ok());
                push_command(&mut out, cmd, ts);
                continue;
            }
        }
        // bash HISTTIMEFORMAT: a `#<epoch>` line preceding the command
        if let Some(rest) = line.strip_prefix('#') {
            if let Ok(ts) = rest.trim().parse::<i64>() {
                pending_ts = Some(ts);
                continue;
            }
        }
        push_command(&mut out, line, pending_ts.take());
    }
    out
}

fn push_command(out: &mut Vec<HistEntry>, cmd: &str, timestamp: Option<i64>) {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return;
    }
    out.push(HistEntry {
        command: cmd.to_string(),
        timestamp,
    });
}

fn unescape_fish(s: &str) -> String {
    s.replace("\\\\", "\\").replace("\\n", "\n")
}

/// Normalize a command so trivially different invocations group
/// together: collapse whitespace and drop a trailing `;`/`&`.
#[must_use]
pub fn normalize(cmd: &str) -> String {
    let mut s = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    while s.ends_with(';') || s.ends_with('&') {
        s.pop();
        s = s.trim_end().to_string();
    }
    s
}

/// Marker substrings that make a history command secret-shaped
/// (Forgenesis screen).
const SECRET_MARKERS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "access_key",
    "private_key",
    "credential",
    "-----begin",
    "authorization:",
    "aws_secret",
    "bearer ",
];

/// Conservative secret screen (Forgenesis markers). `fg analyze`
/// writes suggestions to disk, so a false positive (skipping a
/// command) is far cheaper than a false negative.
#[must_use]
pub fn looks_sensitive(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    if SECRET_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    lower.starts_with("export ") && lower.contains("key=")
}

/// Drop noise (`ls`, `cd`, editors, config `ignore_commands`) and
/// anything that looks like a secret — by the marker screen **or** the
/// `forge-core` scanner. This runs before candidates are mined,
/// printed or saved: secrets never leave this function.
#[must_use]
pub fn filter_commands(entries: &[HistEntry], cfg: &Config) -> Vec<HistEntry> {
    entries
        .iter()
        .filter_map(|e| {
            let cmd = normalize(&e.command);
            if cmd.is_empty()
                || cfg.is_ignored(&cmd)
                || looks_sensitive(&cmd)
                || !forge_core::scan::scan(&cmd).is_empty()
            {
                return None;
            }
            Some(HistEntry {
                command: cmd,
                timestamp: e.timestamp,
            })
        })
        .collect()
}

/// Split history into sessions. A gap longer than `gap_secs` starts a
/// new one, so sequences are not stitched across unrelated work.
/// Untimestamped history is treated as a single session.
#[must_use]
pub fn segment_sessions(entries: &[HistEntry], gap_secs: i64) -> Vec<Vec<String>> {
    let mut sessions: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut last_ts: Option<i64> = None;
    for e in entries {
        if let (Some(prev), Some(now)) = (last_ts, e.timestamp) {
            if now - prev > gap_secs && !current.is_empty() {
                sessions.push(std::mem::take(&mut current));
            }
        }
        if e.timestamp.is_some() {
            last_ts = e.timestamp;
        }
        current.push(e.command.clone());
    }
    if !current.is_empty() {
        sessions.push(current);
    }
    sessions
}

/// Mine sessions for repeated sequences.
///
/// Counts **non-overlapping** occurrences: `a b a b a b` contains
/// `a b` three times, not five. Longer candidates win — a sequence
/// fully contained in an already-accepted, equally-frequent longer
/// one (or a rotation of it) is dropped.
#[must_use]
pub fn mine_sequences(
    sessions: &[Vec<String>],
    min_len: usize,
    max_len: usize,
    min_count: u32,
) -> Vec<Candidate> {
    let min_len = min_len.max(2);
    if max_len < min_len {
        return Vec::new();
    }
    let mut counts: HashMap<Vec<String>, u32> = HashMap::new();
    for session in sessions {
        for len in min_len..=max_len {
            if session.len() < len {
                continue;
            }
            // Collect every start position of every window, then count
            // each sequence's occurrences greedily and without overlap.
            let mut positions: HashMap<&[String], Vec<usize>> = HashMap::new();
            for i in 0..=(session.len() - len) {
                let window = &session[i..i + len];
                if window.iter().any(|c| c.trim().is_empty()) {
                    continue;
                }
                // A window that is just one command repeated is not a workflow.
                if window.iter().all(|c| c == &window[0]) {
                    continue;
                }
                positions.entry(window).or_default().push(i);
            }
            for (seq, starts) in positions {
                let mut n = 0u32;
                let mut next_free = 0usize;
                for start in starts {
                    if start >= next_free {
                        n += 1;
                        next_free = start + len;
                    }
                }
                *counts.entry(seq.to_vec()).or_insert(0) += n;
            }
        }
    }
    let mut candidates: Vec<Candidate> = counts
        .into_iter()
        .filter(|(_, n)| *n >= min_count)
        .map(|(commands, count)| {
            let score = score_of(&commands, count);
            Candidate {
                commands,
                count,
                score,
            }
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.commands.len().cmp(&a.commands.len()))
            .then(a.commands.cmp(&b.commands))
    });
    let mut accepted: Vec<Candidate> = Vec::new();
    for c in candidates {
        if !accepted.iter().any(|a| subsumes(a, &c)) {
            accepted.push(c);
        }
    }
    accepted
}

/// Does the already-accepted candidate `a` make `c` redundant?
/// Fragments (prefix/suffix/middle), rotations, and loop-spanning
/// windows of an accepted loop all say nothing new.
fn subsumes(a: &Candidate, c: &Candidate) -> bool {
    if a.commands == c.commands || a.count < c.count {
        return false;
    }
    if a.commands.len() > c.commands.len() && is_contiguous_sub(&a.commands, &c.commands) {
        return true;
    }
    if c.commands.len() > a.commands.len() && is_contiguous_sub(&c.commands, &a.commands) {
        return true;
    }
    c.commands.iter().all(|cmd| a.commands.contains(cmd))
}

/// Length matters more than raw frequency: a 4-command sequence seen
/// 5 times saves more typing than a 2-command one seen 8 times.
#[allow(clippy::cast_precision_loss)]
fn score_of(commands: &[String], count: u32) -> f64 {
    let len = commands.len() as f64;
    let n = f64::from(count);
    let typing = commands.iter().map(|c| c.len() as f64).sum::<f64>() / 40.0;
    (len * 1.5 + typing) * n.ln_1p() * n.min(50.0).sqrt()
}

fn is_contiguous_sub(haystack: &[String], needle: &[String]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Estimated typing saved by a candidate so far, in seconds.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn estimated_savings_secs(c: &Candidate, per_command_secs: f64) -> f64 {
    c.commands.len() as f64 * per_command_secs * f64::from(c.count)
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
