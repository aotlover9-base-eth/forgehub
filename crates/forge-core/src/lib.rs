#![forbid(unsafe_code)]
//! `forge-core` — shared `ForgeHub` library.
//!
//! Step 1 provides only the crate skeleton so the workspace builds.
//! The snippet model, YAML parsing and validation, canonical serialisation
//! and hashing, variable resolution, secret scanning and danger lints are
//! implemented in Step 3 against `docs/spec-snippet-format.md`.
//!
//! # Examples
//!
//! ```
//! assert_eq!(forge_core::PRODUCT, "ForgeHub");
//! ```

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
