//! Exit codes and the CLI error type (spec-cli §2).
//!
//! Every error the CLI prints follows the what / why / fix shape from
//! `agent.md` §8 wherever the cause is known.

use std::fmt;

/// Exit code: the operation was refused (confirmation declined, secret
/// found, no terminal to confirm on).
pub const EXIT_REFUSED: u8 = 2;
/// Exit code: the named thing was not found.
pub const EXIT_NOT_FOUND: u8 = 3;
/// Exit code: authentication required or expired (spec-cli §2).
pub const EXIT_AUTH: u8 = 4;

/// An error carrying the process exit code the CLI should return.
#[derive(Debug)]
pub struct CliError {
    /// Exit code (1 when not specified otherwise).
    pub code: u8,
    /// The full message to print on stderr.
    pub message: String,
}

impl CliError {
    /// A plain failure (exit 1).
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: message.into(),
        }
    }

    /// A refusal (exit 2).
    pub fn refused(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_REFUSED,
            message: message.into(),
        }
    }

    /// Not found (exit 3).
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_NOT_FOUND,
            message: message.into(),
        }
    }

    /// Authentication required or expired (exit 4).
    pub fn auth(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_AUTH,
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CliError {}

/// Downcast an `anyhow::Error` to a [`CliError`] exit code, defaulting
/// to 1.
#[must_use]
pub fn code_of(err: &anyhow::Error) -> u8 {
    err.downcast_ref::<CliError>().map_or(1, |e| e.code)
}

/// Render the top-level error for stderr (Peak B1): returns the
/// text to print and the exit code.
///
/// Human form is the error chain (`what`), plus a `fix:` line when
/// the chain bottoms out in an `io::Error` whose kind has a known
/// remedy and the message does not already carry one - raw OS
/// errors used to reach the user with no remedy at all (audit
/// F12). With `--json` the spec-cli section 3 envelope is emitted
/// instead: `{ "error": { "code", "message" } }`, exit code
/// unchanged.
#[must_use]
pub fn render_error(err: &anyhow::Error, json: bool, style: crate::output::Style) -> (String, u8) {
    let code = code_of(err);
    let mut message = format!("{err:#}");
    if let Some(io) = err.chain().find_map(|e| e.downcast_ref::<std::io::Error>()) {
        let fix = match io.kind() {
            std::io::ErrorKind::PermissionDenied => {
                "fix: check that the path above is writable, or set FORGEHUB_HOME to a writable directory"
            }
            std::io::ErrorKind::NotFound => {
                "fix: if this is a fresh start, run `fg init` to create the ForgeHub home"
            }
            _ => "fix: check the path above and try again",
        };
        if !message.contains("fix:") {
            message.push('\n');
            message.push_str(fix);
        }
    }
    if json {
        let envelope = serde_json::json!({
            "error": { "code": code, "message": message }
        });
        (envelope.to_string(), code)
    } else {
        (style.error(&message), code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_err(kind: std::io::ErrorKind, text: &str) -> anyhow::Error {
        anyhow::Error::new(std::io::Error::new(kind, "os error")).context(text.to_string())
    }

    #[test]
    fn render_error_keeps_exit_codes() {
        let err = anyhow::Error::new(CliError::auth("forgehub: not logged in"));
        let (text, code) = render_error(&err, false, crate::output::Style::PLAIN);
        assert_eq!(code, EXIT_AUTH);
        assert!(text.contains("not logged in"), "{text}");
        let err = anyhow::Error::new(CliError::not_found("nope"));
        assert_eq!(
            render_error(&err, false, crate::output::Style::PLAIN).1,
            EXIT_NOT_FOUND
        );
        let err = anyhow::Error::new(CliError::refused("no"));
        assert_eq!(
            render_error(&err, false, crate::output::Style::PLAIN).1,
            EXIT_REFUSED
        );
        let err = anyhow::anyhow!("plain failure");
        assert_eq!(render_error(&err, false, crate::output::Style::PLAIN).1, 1);
    }

    #[test]
    fn render_error_adds_a_fix_line_for_io_errors() {
        let err = io_err(
            std::io::ErrorKind::PermissionDenied,
            "writing /x/config.yaml",
        );
        let (text, _) = render_error(&err, false, crate::output::Style::PLAIN);
        assert!(text.contains("writing /x/config.yaml"), "{text}");
        assert!(
            text.contains("fix: check that the path above is writable"),
            "{text}"
        );
        let err = io_err(std::io::ErrorKind::NotFound, "reading /x/config.yaml");
        let (text, _) = render_error(&err, false, crate::output::Style::PLAIN);
        assert!(text.contains("fg init"), "{text}");
    }

    #[test]
    fn render_error_does_not_duplicate_an_existing_fix() {
        let err = anyhow::Error::new(CliError::auth("forgehub: not logged in\n  fix: fg login"));
        let (text, _) = render_error(&err, false, crate::output::Style::PLAIN);
        assert_eq!(text.matches("fix:").count(), 1, "{text}");
    }

    #[test]
    fn render_error_json_envelope_matches_spec() {
        let err = anyhow::Error::new(CliError::not_found("no local snippet named `x`"));
        let (text, code) = render_error(&err, true, crate::output::Style::PLAIN);
        assert_eq!(code, EXIT_NOT_FOUND);
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(
            parsed["error"]["code"].as_u64(),
            Some(u64::from(EXIT_NOT_FOUND))
        );
        assert_eq!(parsed["error"]["message"], "no local snippet named `x`");
    }
}
