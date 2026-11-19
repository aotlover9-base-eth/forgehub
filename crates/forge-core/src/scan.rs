//! Secret scanning (ADR 0007): known token formats, credential
//! assignments, and high-entropy values in assignment context.
//!
//! Findings carry a kind and a 1-based line number — **never the matched
//! value**, so findings are safe to show, log and return from the API.
//! The scanner is deliberately conservative about free-floating entropy:
//! command lines are full of hashes and ids that are not secrets, so
//! entropy is only considered in assignment context, and pure-hex values
//! (checksums, git SHAs) are never flagged for entropy.

use regex::Regex;
use std::sync::OnceLock;

/// The kind of secret a finding reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// `-----BEGIN … PRIVATE KEY-----`
    PrivateKey,
    /// AWS access key id (`AKIA…` / `ASIA…`).
    AwsAccessKey,
    /// GitHub token (`ghp_…`, `github_pat_…`, …).
    GitHubToken,
    /// GitLab personal access token (`glpat-…`).
    GitLabToken,
    /// Slack token (`xox[baprs]-…`).
    SlackToken,
    /// Stripe live secret key (`sk_live_…` / `rk_live_…`).
    StripeSecretKey,
    /// `Authorization: Bearer <token>` with a token-shaped value.
    BearerToken,
    /// A password given as a literal (the flag form or an assignment).
    PasswordLiteral,
    /// A secret-named assignment (`TOKEN=…`, `API_KEY=…`, …) with a
    /// literal value.
    SecretAssignment,
    /// A high-entropy value in assignment context.
    HighEntropyAssignment,
}

impl SecretKind {
    /// Stable machine name (API, CLI output).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            SecretKind::PrivateKey => "private_key",
            SecretKind::AwsAccessKey => "aws_access_key",
            SecretKind::GitHubToken => "github_token",
            SecretKind::GitLabToken => "gitlab_token",
            SecretKind::SlackToken => "slack_token",
            SecretKind::StripeSecretKey => "stripe_secret_key",
            SecretKind::BearerToken => "bearer_token",
            SecretKind::PasswordLiteral => "password_literal",
            SecretKind::SecretAssignment => "secret_assignment",
            SecretKind::HighEntropyAssignment => "high_entropy_assignment",
        }
    }
}

/// One finding: what kind of secret, on which 1-based line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finding {
    /// Kind of secret detected.
    pub kind: SecretKind,
    /// 1-based line number in the scanned text.
    pub line: usize,
}

struct Patterns {
    private_key: Regex,
    aws: Regex,
    github: Regex,
    gitlab: Regex,
    slack: Regex,
    stripe: Regex,
    bearer: Regex,
    password: Regex,
    assignment: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        private_key: Regex::new(r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----")
            .expect("static regex"),
        aws: Regex::new(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b").expect("static regex"),
        github: Regex::new(
            r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}\b|\bgithub_pat_[A-Za-z0-9_]{20,}\b",
        )
        .expect("static regex"),
        gitlab: Regex::new(r"\bglpat-[A-Za-z0-9_-]{20,}\b").expect("static regex"),
        slack: Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").expect("static regex"),
        stripe: Regex::new(r"\b(?:sk|rk)_live_[A-Za-z0-9]{16,}\b").expect("static regex"),
        bearer: Regex::new(r"(?i)\bBearer\s+([A-Za-z0-9._~+/=-]{16,})").expect("static regex"),
        password: Regex::new(
            r#"(?i)((?:--password[=\s]+|--passwd[=\s]+|\bpassword\s*[=:]\s*|\bpasswd\s*[=:]\s*))("[^"]*"|'[^']*'|\S+)"#,
        )
        .expect("static regex"),
        assignment: Regex::new(
            r#"(?m)(^|[\s;|&])([A-Za-z_][A-Za-z0-9_.-]*)\s*=\s*("[^"]*"|'[^']*'|[^\s;|&]+)"#,
        )
        .expect("static regex"),
    })
}

/// Is this value a placeholder rather than a literal secret?
/// (`{{VAR}}`, `$VAR`, `${VAR}`, `xxxx`, `****`, `<...>`, `changeme`, …)
fn is_placeholder(value: &str) -> bool {
    let v = value.trim().trim_matches('"').trim_matches('\'');
    if v.is_empty() {
        return true;
    }
    let lower = v.to_lowercase();
    v.starts_with("{{")
        || v.starts_with('$')
        || v.starts_with('<')
        || lower.starts_with("your-")
        || lower.starts_with("your_")
        || matches!(
            lower.as_str(),
            "changeme"
                | "change-me"
                | "example"
                | "placeholder"
                | "redacted"
                | "password"
                | "secret"
                | "token"
                | "none"
                | "null"
        )
        || v.chars()
            .all(|c| c == 'x' || c == 'X' || c == '*' || c == '.')
}

fn is_secret_named(key: &str) -> bool {
    let k = key.to_lowercase();
    k.contains("token")
        || k.contains("secret")
        || k.contains("password")
        || k.contains("passwd")
        || k.contains("api_key")
        || k.contains("apikey")
        || k.contains("api-key")
        || k.contains("auth")
        || k.ends_with("_key")
}

// Counts are bounded by the maximum line length (8,192 bytes), far
// below f64's exact-integer range, so the casts lose nothing.
#[allow(clippy::cast_precision_loss)]
fn shannon_entropy(value: &str) -> f64 {
    let mut counts = [0usize; 256];
    for b in value.bytes() {
        counts[b as usize] += 1;
    }
    let len = value.len() as f64;
    let mut entropy = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / len;
            entropy -= p * p.log2();
        }
    }
    entropy
}

fn is_pure_hex(value: &str) -> bool {
    value.len() >= 16 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Could this value be a token at all? Tokens use a restricted charset
/// (base64/base64url/hex plus `. _ ~ + / = -`). Prose (spaces, commas)
/// and code expressions (brackets, parens, quotes) are never tokens —
/// without this rule the entropy check false-positives on both.
fn has_token_charset(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | '+' | '/' | '=' | '-')
        })
}

/// The shared high-entropy test: long enough, token charset, not pure
/// hex, mixed character classes, entropy above threshold.
fn is_high_entropy_token(value: &str) -> bool {
    value.len() >= 20
        && has_token_charset(value)
        && !is_pure_hex(value)
        && looks_like_token_value(value)
        && shannon_entropy(value) >= 4.2
}

fn looks_like_token_value(value: &str) -> bool {
    // At least two of the three character classes: mixed classes are
    // what separate a token from a word or a version string.
    let classes = [
        value.chars().any(|c| c.is_ascii_lowercase()),
        value.chars().any(|c| c.is_ascii_uppercase()),
        value.chars().any(|c| c.is_ascii_digit()),
    ];
    classes.iter().filter(|&&b| b).count() >= 2
}

/// Scan text for secrets. Returns findings sorted by line; a line can
/// produce more than one finding (different kinds).
#[must_use]
pub fn scan(text: &str) -> Vec<Finding> {
    let p = patterns();
    let mut findings = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let mut push = |kind: SecretKind| {
            let f = Finding {
                kind,
                line: line_no,
            };
            if !findings.contains(&f) {
                findings.push(f);
            }
        };
        if p.private_key.is_match(line) {
            push(SecretKind::PrivateKey);
        }
        if p.aws.is_match(line) {
            push(SecretKind::AwsAccessKey);
        }
        if p.github.is_match(line) {
            push(SecretKind::GitHubToken);
        }
        if p.gitlab.is_match(line) {
            push(SecretKind::GitLabToken);
        }
        if p.slack.is_match(line) {
            push(SecretKind::SlackToken);
        }
        if p.stripe.is_match(line) {
            push(SecretKind::StripeSecretKey);
        }
        if let Some(cap) = p.bearer.captures(line) {
            let token = &cap[1];
            if !is_placeholder(token) && looks_like_token_value(token) {
                push(SecretKind::BearerToken);
            }
        }
        if let Some(cap) = p.password.captures(line) {
            let value = cap[2].trim_matches('"').trim_matches('\'');
            // Code-shaped values (`password: Regex::new(...)`, argument
            // lists) are declarations, not literals; a real password
            // value in a command is a single simple token.
            let code_shaped = value.contains(['(', ')', ',']) || value.contains("::");
            if !is_placeholder(value) && !code_shaped {
                push(SecretKind::PasswordLiteral);
            }
        }
        for cap in p.assignment.captures_iter(line) {
            let key = &cap[2];
            let raw_value = &cap[3];
            let value = raw_value.trim_matches('"').trim_matches('\'');
            if is_placeholder(value) {
                continue;
            }
            // Password assignments are already reported as
            // PasswordLiteral; do not double-report them here.
            let key_lower = key.to_lowercase();
            if key_lower.contains("password") || key_lower.contains("passwd") {
                continue;
            }
            if is_secret_named(key) && value.len() >= 6 {
                push(SecretKind::SecretAssignment);
            } else if is_high_entropy_token(value) {
                push(SecretKind::HighEntropyAssignment);
            }
        }
    }
    findings
}

/// Scan every command/script line of a snippet.
#[must_use]
pub fn scan_snippet(snippet: &crate::model::Snippet) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, text) in snippet.all_command_texts().iter().enumerate() {
        for mut f in scan(text) {
            f.line = i + 1;
            out.push(f);
        }
    }
    out
}

/// Replace detected secret values with `REDACTED`, keeping the line
/// structure intact. Used by the recorder to offer a cleaned history.
///
/// # Examples
/// ```
/// let token = "aB3".repeat(10);
/// let dirty = format!("curl -H 'Authorization: Bearer {token}' https://example.com");
/// let clean = forge_core::scan::redact(&dirty);
/// assert!(clean.contains("REDACTED"));
/// assert!(!clean.contains(&token));
/// ```
#[must_use]
pub fn redact(text: &str) -> String {
    let p = patterns();
    let mut out = text.to_string();
    for re in [
        &p.private_key,
        &p.aws,
        &p.github,
        &p.gitlab,
        &p.slack,
        &p.stripe,
    ] {
        out = re.replace_all(&out, "REDACTED").into_owned();
    }
    out = p.bearer.replace_all(&out, "Bearer REDACTED").into_owned();
    out = p
        .password
        .replace_all(&out, |caps: &regex::Captures| {
            if is_placeholder(&caps[2]) {
                caps[0].to_string()
            } else {
                format!("{}REDACTED", &caps[1])
            }
        })
        .into_owned();
    out = p
        .assignment
        .replace_all(&out, |caps: &regex::Captures| {
            let sep = &caps[1];
            let key = &caps[2];
            let value = caps[3].trim_matches('"').trim_matches('\'');
            let flagged = !is_placeholder(value)
                && ((is_secret_named(key) && value.len() >= 6) || is_high_entropy_token(value));
            if flagged {
                format!("{sep}{key}=REDACTED")
            } else {
                caps[0].to_string()
            }
        })
        .into_owned();
    out
}
