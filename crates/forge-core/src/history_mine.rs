//! Shell-history mining (ported from Forgenesis via the CLI's
//! `analyze` module, Peak C5): parse bash/zsh/fish history,
//! drop noise and anything secret-shaped, segment into
//! sessions by time gaps, and mine non-overlapping repeated
//! sequences with fragment/rotation suppression.
//!
//! The module lives in forge-core so the CLI (`fg analyze`)
//! and the server (web history-file import) run the **same**
//! pipeline: secret scrubbing happens at the filter stage -
//! before anything is printed, returned or written - and uses
//! both the Forgenesis marker screen and the core scanner
//! ([`crate::scan`]).
//!
//! Everything here is a pure function over [`HistEntry`]
//! values; file IO stays with the callers.

use std::collections::HashMap;

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
    /// `docker compose up` -> `docker-compose`, `git pull` -> `git-pull`.
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
        if cleaned.is_empty() || crate::names::validate_snippet_name(&cleaned).is_err() {
            "workflow".into()
        } else {
            cleaned
        }
    }
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

/// Conservative secret screen (Forgenesis markers). Callers
/// write suggestions to disk or return them over the API, so
/// a false positive (skipping a command) is far cheaper than
/// a false negative.
#[must_use]
pub fn looks_sensitive(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    if SECRET_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    lower.starts_with("export ") && lower.contains("key=")
}

/// Drop noise (the caller's `ignore` predicate - e.g. the
/// CLI's configured `ignore_commands`, or `ls`/`cd`-style
/// lists) and anything that looks like a secret - by the
/// marker screen **or** the core scanner. This runs before
/// candidates are mined, printed or saved: secrets never
/// leave this function.
#[must_use]
pub fn filter_entries(entries: &[HistEntry], ignore: &dyn Fn(&str) -> bool) -> Vec<HistEntry> {
    entries
        .iter()
        .filter_map(|e| {
            let cmd = normalize(&e.command);
            if cmd.is_empty()
                || ignore(&cmd)
                || looks_sensitive(&cmd)
                || !crate::scan::scan(&cmd).is_empty()
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
/// `a b` three times, not five. Longer candidates win - a sequence
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

/// The full pipeline over history TEXT (Peak C5): parse,
/// filter (secrets never leave the filter), segment, mine.
/// Returns the candidates plus the counts a UI needs to
/// explain itself (parsed entries, entries after filtering).
#[must_use]
pub fn mine_history_text(
    text: &str,
    ignore: &dyn Fn(&str) -> bool,
    max_len: usize,
    min_count: u32,
    limit: usize,
) -> (Vec<Candidate>, usize, usize) {
    let entries = parse_history_text(text);
    let parsed = entries.len();
    let filtered = filter_entries(&entries, ignore);
    let kept = filtered.len();
    let sessions = segment_sessions(&filtered, 30 * 60);
    let mut found = mine_sequences(&sessions, 2, max_len.max(2), min_count.max(2));
    found.truncate(limit);
    (found, parsed, kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_three_formats() {
        let bash = parse_history_text("#1700000000\ngit pull\nls\n");
        assert_eq!(bash[0].command, "git pull");
        assert_eq!(bash[0].timestamp, Some(1_700_000_000));
        let zsh = parse_history_text(": 1700000000:0;git pull\n");
        assert_eq!(zsh[0].command, "git pull");
        let fish = parse_history_text("- cmd: git pull\n  when: 1700000000\n");
        assert_eq!(fish[0].command, "git pull");
        assert_eq!(fish[0].timestamp, Some(1_700_000_000));
    }

    #[test]
    fn filter_drops_secrets_and_ignored() {
        let entries = parse_history_text(
            "git pull\nexport AWS_SECRET_ACCESS_KEY=abc\ncurl -H 'Authorization: Bearer <redacted>' x\n",
        );
        let kept = filter_entries(&entries, &|cmd| cmd == "ls");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].command, "git pull");
    }

    #[test]
    fn mines_a_repeated_sequence_with_a_name() {
        let text = "docker compose build\ndocker compose up -d\nkubectl get pods\ndocker compose build\ndocker compose up -d\nkubectl get pods\ndocker compose build\ndocker compose up -d\n";
        let (found, parsed, kept) = mine_history_text(text, &|_| false, 4, 2, 10);
        assert_eq!(parsed, 8);
        assert_eq!(kept, 8);
        assert_eq!(found[0].suggested_name(), "docker-compose");
    }
}
