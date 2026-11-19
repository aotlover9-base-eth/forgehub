//! Name validation and normalisation (spec-snippet-format §8).
//!
//! One definition, used by the CLI, the server and (through the API) the
//! web app. Snippet names and usernames are **rejected** when mixed-case;
//! tags are folded to lowercase (the only folding in the system).

use crate::error::ForgeError;

/// Words that can never be snippet names (and never usernames): every CLI
/// subcommand, plus web/API route words. This list is the single source;
/// spec-snippet-format §8 mirrors it and a test in this module guards the
/// CLI-critical entries.
pub const RESERVED_WORDS: &[&str] = &[
    "run",
    "init",
    "record",
    "login",
    "logout",
    "whoami",
    "push",
    "pull",
    "fork",
    "status",
    "diff",
    "list",
    "show",
    "search",
    "tag",
    "rename",
    "clone",
    "delete",
    "edit",
    "import",
    "export",
    "analyze",
    "parameterize",
    "stats",
    "backup",
    "restore",
    "doctor",
    "alias",
    "completions",
    "ui",
    "cache",
    "update",
    "trust",
    "remote",
    "help",
    "version",
    "config",
    "me",
    "api",
    "admin",
    "settings",
    "explore",
    "new",
    "device",
    "signup",
    "static",
    "assets",
    "health",
    "tokens",
    "users",
    "snippets",
];

/// Is `word` reserved?
#[must_use]
pub fn is_reserved(word: &str) -> bool {
    RESERVED_WORDS.contains(&word)
}

fn is_lower_alnum(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// Validate a snippet name: `^[a-z0-9][a-z0-9._-]{0,63}$`, no `..`, does
/// not end with `.`, not reserved.
///
/// # Errors
/// [`ForgeError::InvalidName`] with the specific rule that failed.
///
/// # Examples
/// ```
/// assert!(forge_core::names::validate_snippet_name("ubuntu-setup").is_ok());
/// assert!(forge_core::names::validate_snippet_name("Ubuntu").is_err());
/// assert!(forge_core::names::validate_snippet_name("run").is_err());
/// ```
pub fn validate_snippet_name(name: &str) -> Result<(), ForgeError> {
    if name.is_empty() {
        return Err(ForgeError::InvalidName(
            "snippet name cannot be empty".into(),
        ));
    }
    if name.len() > 64 {
        return Err(ForgeError::InvalidName(format!(
            "snippet name is too long ({} chars, max 64)",
            name.len()
        )));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or_default();
    if !is_lower_alnum(first) {
        return Err(ForgeError::InvalidName(format!(
            "snippet name `{name}` must start with a lowercase letter or digit"
        )));
    }
    if !chars.all(|c| is_lower_alnum(c) || c == '.' || c == '_' || c == '-') {
        return Err(ForgeError::InvalidName(format!(
            "snippet name `{name}` may only contain lowercase letters, digits, `.`, `_`, `-`"
        )));
    }
    if name.contains("..") {
        return Err(ForgeError::InvalidName(format!(
            "snippet name `{name}` must not contain `..`"
        )));
    }
    if name.ends_with('.') {
        return Err(ForgeError::InvalidName(format!(
            "snippet name `{name}` must not end with `.`"
        )));
    }
    if is_reserved(name) {
        return Err(ForgeError::InvalidName(format!(
            "`{name}` is a reserved word and cannot be a snippet name"
        )));
    }
    Ok(())
}

/// Validate a username: `^[a-z0-9][a-z0-9-]{1,38}$` (so 2–39 chars), ASCII
/// only, no consecutive hyphens, does not end with `-`, not reserved.
///
/// # Errors
/// [`ForgeError::InvalidName`] with the specific rule that failed.
///
/// # Examples
/// ```
/// assert!(forge_core::names::validate_username("alice-9").is_ok());
/// assert!(forge_core::names::validate_username("a").is_err());
/// assert!(forge_core::names::validate_username("al--ice").is_err());
/// ```
pub fn validate_username(name: &str) -> Result<(), ForgeError> {
    if name.len() < 2 || name.len() > 39 {
        return Err(ForgeError::InvalidName(format!(
            "username must be 2-39 characters, got {}",
            name.len()
        )));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or_default();
    if !is_lower_alnum(first) {
        return Err(ForgeError::InvalidName(format!(
            "username `{name}` must start with a lowercase letter or digit"
        )));
    }
    if !chars.all(|c| is_lower_alnum(c) || c == '-') {
        return Err(ForgeError::InvalidName(format!(
            "username `{name}` may only contain lowercase letters, digits and `-`"
        )));
    }
    if name.contains("--") {
        return Err(ForgeError::InvalidName(format!(
            "username `{name}` must not contain consecutive hyphens"
        )));
    }
    if name.ends_with('-') {
        return Err(ForgeError::InvalidName(format!(
            "username `{name}` must not end with `-`"
        )));
    }
    if is_reserved(name) {
        return Err(ForgeError::InvalidName(format!(
            "`{name}` is a reserved word and cannot be a username"
        )));
    }
    Ok(())
}

/// Normalise a tag: fold to lowercase (the only case folding in the
/// system), then validate it.
///
/// # Errors
/// [`ForgeError::InvalidName`] when the folded tag breaks the tag rules.
///
/// # Examples
/// ```
/// assert_eq!(forge_core::names::normalize_tag("Ubuntu").unwrap(), "ubuntu");
/// assert!(forge_core::names::normalize_tag("bad tag").is_err());
/// ```
pub fn normalize_tag(raw: &str) -> Result<String, ForgeError> {
    let tag = raw.trim().to_lowercase();
    validate_tag(&tag)?;
    Ok(tag)
}

/// Validate an already-normalised tag: `^[a-z0-9][a-z0-9-]{0,29}$`.
///
/// # Errors
/// [`ForgeError::InvalidName`] with the specific rule that failed.
pub fn validate_tag(tag: &str) -> Result<(), ForgeError> {
    if tag.is_empty() {
        return Err(ForgeError::InvalidName("tag cannot be empty".into()));
    }
    if tag.len() > 30 {
        return Err(ForgeError::InvalidName(format!(
            "tag is too long ({} chars, max 30)",
            tag.len()
        )));
    }
    let mut chars = tag.chars();
    let first = chars.next().unwrap_or_default();
    if !is_lower_alnum(first) {
        return Err(ForgeError::InvalidName(format!(
            "tag `{tag}` must start with a lowercase letter or digit"
        )));
    }
    if !chars.all(|c| is_lower_alnum(c) || c == '-') {
        return Err(ForgeError::InvalidName(format!(
            "tag `{tag}` may only contain lowercase letters, digits and `-`"
        )));
    }
    Ok(())
}

/// Normalise a whole tag list: fold each tag, deduplicate preserving
/// first-seen order, and enforce the 20-tag limit.
///
/// # Errors
/// [`ForgeError::InvalidName`] or [`ForgeError::Validation`] (too many).
pub fn normalize_tags(raw: &[String]) -> Result<Vec<String>, ForgeError> {
    let mut out: Vec<String> = Vec::new();
    for t in raw {
        let n = normalize_tag(t)?;
        if !out.contains(&n) {
            out.push(n);
        }
    }
    if out.len() > 20 {
        return Err(ForgeError::validation(format!(
            "too many tags ({} after deduplication, max 20)",
            out.len()
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_simple_ok() {
        assert!(validate_snippet_name("ubuntu-setup").is_ok());
    }
    #[test]
    fn snippet_single_char_ok() {
        assert!(validate_snippet_name("a").is_ok());
    }
    #[test]
    fn snippet_with_dots_underscores_digits_ok() {
        assert!(validate_snippet_name("a.b_c-2").is_ok());
    }
    #[test]
    fn snippet_max_len_ok() {
        assert!(validate_snippet_name(&"a".repeat(64)).is_ok());
    }
    #[test]
    fn snippet_too_long_rejected() {
        assert!(validate_snippet_name(&"a".repeat(65)).is_err());
    }
    #[test]
    fn snippet_empty_rejected() {
        assert!(validate_snippet_name("").is_err());
    }
    #[test]
    fn snippet_uppercase_rejected() {
        assert!(validate_snippet_name("Ubuntu").is_err());
    }
    #[test]
    fn snippet_space_rejected() {
        assert!(validate_snippet_name("has space").is_err());
    }
    #[test]
    fn snippet_slash_rejected() {
        assert!(validate_snippet_name("a/b").is_err());
    }
    #[test]
    fn snippet_leading_dot_rejected() {
        assert!(validate_snippet_name(".hidden").is_err());
    }
    #[test]
    fn snippet_double_dot_rejected() {
        assert!(validate_snippet_name("a..b").is_err());
    }
    #[test]
    fn snippet_trailing_dot_rejected() {
        assert!(validate_snippet_name("abc.").is_err());
    }
    #[test]
    fn snippet_leading_dash_rejected() {
        assert!(validate_snippet_name("-abc").is_err());
    }
    #[test]
    fn snippet_reserved_subcommand_rejected() {
        assert!(validate_snippet_name("push").is_err());
    }
    #[test]
    fn snippet_reserved_route_rejected() {
        assert!(validate_snippet_name("settings").is_err());
    }
    #[test]
    fn reserved_list_covers_all_cli_subcommands() {
        for w in [
            "run",
            "init",
            "record",
            "login",
            "logout",
            "whoami",
            "push",
            "pull",
            "fork",
            "status",
            "diff",
            "list",
            "show",
            "search",
            "tag",
            "rename",
            "clone",
            "delete",
            "edit",
            "import",
            "export",
            "analyze",
            "parameterize",
            "stats",
            "backup",
            "restore",
            "doctor",
            "alias",
            "completions",
            "ui",
            "cache",
            "update",
            "trust",
            "remote",
            "help",
            "version",
        ] {
            assert!(is_reserved(w), "missing reserved word {w}");
        }
    }
    #[test]
    fn username_ok() {
        assert!(validate_username("alice-9").is_ok());
    }
    #[test]
    fn username_two_chars_ok() {
        assert!(validate_username("ab").is_ok());
    }
    #[test]
    fn username_one_char_rejected() {
        assert!(validate_username("a").is_err());
    }
    #[test]
    fn username_uppercase_rejected() {
        assert!(validate_username("Alice").is_err());
    }
    #[test]
    fn username_underscore_rejected() {
        assert!(validate_username("al_ice").is_err());
    }
    #[test]
    fn username_double_hyphen_rejected() {
        assert!(validate_username("al--ice").is_err());
    }
    #[test]
    fn username_trailing_hyphen_rejected() {
        assert!(validate_username("alice-").is_err());
    }
    #[test]
    fn username_reserved_rejected() {
        assert!(validate_username("admin").is_err());
    }
    #[test]
    fn username_too_long_rejected() {
        assert!(validate_username(&"a".repeat(40)).is_err());
    }
    #[test]
    fn username_non_ascii_rejected() {
        assert!(validate_username("alicé").is_err());
    }
    #[test]
    fn tag_folds_uppercase() {
        assert_eq!(normalize_tag("Ubuntu").unwrap(), "ubuntu");
    }
    #[test]
    fn tag_trims_whitespace() {
        assert_eq!(normalize_tag("  ci ").unwrap(), "ci");
    }
    #[test]
    fn tag_with_space_rejected() {
        assert!(normalize_tag("bad tag").is_err());
    }
    #[test]
    fn tag_empty_rejected() {
        assert!(normalize_tag("   ").is_err());
    }
    #[test]
    fn tag_too_long_rejected() {
        assert!(normalize_tag(&"a".repeat(31)).is_err());
    }
    #[test]
    fn tags_dedupe_after_folding() {
        let out = normalize_tags(&["CI".into(), "ci".into(), "fast".into()]).unwrap();
        assert_eq!(out, vec!["ci".to_string(), "fast".to_string()]);
    }
    #[test]
    fn tags_over_limit_rejected() {
        let many: Vec<String> = (0..21).map(|i| format!("t{i}")).collect();
        assert!(normalize_tags(&many).is_err());
    }
}
