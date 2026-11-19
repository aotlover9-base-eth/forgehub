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
