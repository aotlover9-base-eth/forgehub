#![forbid(unsafe_code)]
//! `forge-core` - the shared `ForgeHub` library used by the CLI and the
//! server. No network access and no filesystem side effects: everything
//! here is pure computation over text and values.
//!
//! Modules:
//! - [`model`] - the snippet document (both modes) and its limits.
//! - [`parse`] - strict parsing: `Snippet::from_yaml` / `from_bytes`.
//! - [`names`] - snippet-name, username and tag rules, reserved words.
//! - [`vars`] - `{{VAR}}` substitution, resolution order, durations.
//! - [`hash`] - canonical serialisation (spec §9) and SHA-256 hashing.
//! - [`scan`] - secret scanning (ADR 0007).
//! - [`lint`] - danger lints (warnings only).
//! - [`import`] - the Forgenesis conversion layer.
//!
//! # Examples
//!
//! ```
//! let snippet = forge_core::Snippet::from_yaml(
//!     "name: hello\nsteps:\n  - echo hello\n",
//! )
//! .unwrap();
//! assert_eq!(snippet.command_count(), 1);
//! assert_eq!(forge_core::hash::short_hash(&snippet).len(), 8);
//! assert_eq!(forge_core::scan::scan_snippet(&snippet).len(), 0);
//! ```

pub mod error;
pub mod hash;
pub mod import;
pub mod lint;
pub mod model;
pub mod names;
pub mod parse;
pub mod scan;
pub mod vars;

pub use error::{ForgeError, Result};
pub use import::{import_forgenesis, ImportOutcome};
pub use model::{Mode, Snippet};

/// Product name shared by the CLI, server and web app.
pub const PRODUCT: &str = "ForgeHub";

/// Binary name for the `ForgeHub` CLI (the `fg` alias points at this binary).
pub const BINARY_NAME: &str = "forgehub";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_and_binary_names_are_stable() {
        assert_eq!(PRODUCT, "ForgeHub");
        assert_eq!(BINARY_NAME, "forgehub");
    }
}
