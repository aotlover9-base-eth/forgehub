//! Error types for `forge-core`.
//!
//! Errors are structured so the CLI and the server can render them in
//! their own envelopes without string matching.

use thiserror::Error;

/// Everything that can go wrong in `forge-core`.
#[derive(Debug, Error)]
pub enum ForgeError {
    /// The input was not valid UTF-8 (only from [`crate::Snippet::from_bytes`]).
    #[error("input is not valid UTF-8: {0}")]
    InvalidUtf8(String),

    /// YAML parsing failed. `line`/`column` are 1-based when the parser
    /// reported a location.
    #[error("parse error at line {line}, column {column}: {message}")]
    Parse {
        /// Human-readable parser message.
        message: String,
        /// 1-based line, or 0 when unknown.
        line: usize,
        /// 1-based column, or 0 when unknown.
        column: usize,
    },

    /// The document parsed but broke a format rule (limit, shape, name).
    /// The message names the field and the rule.
    #[error("validation error: {0}")]
    Validation(String),

    /// A name (snippet, username or tag) is invalid. The message says why.
    #[error("invalid name: {0}")]
    InvalidName(String),

    /// Variables were referenced (or declared required) but have no value.
    /// Contains **all** missing names, sorted, so callers can report them
    /// in one message (spec-snippet-format §7).
    #[error("missing values for variables: {}", .0.join(", "))]
    MissingVariables(Vec<String>),

    /// A `KEY=value` assignment was malformed.
    #[error("invalid assignment `{0}`: expected KEY=value")]
    InvalidAssignment(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, ForgeError>;

impl ForgeError {
    /// Build a validation error from anything displayable.
    pub(crate) fn validation(msg: impl Into<String>) -> Self {
        ForgeError::Validation(msg.into())
    }
}
