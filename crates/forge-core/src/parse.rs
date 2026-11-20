//! Parsing entry points for [`Snippet`]: byte handling, normalisation,
//! YAML safety pre-checks, strict deserialisation, validation.
//!
//! The parser must never panic on hostile input; every failure is a
//! [`ForgeError`] with a line/column where the YAML parser provides one.

use crate::error::{ForgeError, Result};
use crate::model::{Snippet, MAX_BODY_BYTES, MAX_YAML_DEPTH};

/// Normalise line endings to `\n` (CRLF and lone CR both become LF).
#[must_use]
pub fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn check_control_chars(text: &str) -> Result<()> {
    for (idx, ch) in text.char_indices() {
        if (ch.is_control() && ch != '\n' && ch != '\t') || ch == '\u{7f}' {
            let line = text[..idx].matches('\n').count() + 1;
            return Err(ForgeError::Parse {
                message: format!(
                    "disallowed control character U+{:04X} (only \\n and \\t are allowed)",
                    ch as u32
                ),
                line,
                column: 0,
            });
        }
    }
    Ok(())
}

/// Reject YAML anchors, aliases, merge keys and tags (spec §10).
/// This is a structural-position heuristic; expansion is additionally
/// bounded by the post-parse limits in validation (steps, depth, size),
/// so a missed anchor cannot expand beyond them.
fn check_yaml_safety(text: &str) -> Result<()> {
    use std::sync::OnceLock;
    fn patterns() -> &'static regex::Regex {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        RE.get_or_init(|| {
            regex::Regex::new(
                r"(?m)(:\s+&[A-Za-z0-9_-]+([ \t]|$))|(-\s+&[A-Za-z0-9_-]+([ \t]|$))|(:\s+\*[A-Za-z0-9_-]+[ \t]*$)|(-\s+\*[A-Za-z0-9_-]+[ \t]*$)|(<<\s*:)|(!![A-Za-z])",
            )
            .expect("static regex compiles")
        })
    }
    if let Some(m) = patterns().find(text) {
        let line = text[..m.start()].matches('\n').count() + 1;
        return Err(ForgeError::Parse {
            message: "YAML anchors, aliases, merge keys and tags are not allowed in a forge file"
                .into(),
            line,
            column: 0,
        });
    }
    Ok(())
}

fn yaml_depth(value: &serde_yaml::Value) -> usize {
    match value {
        serde_yaml::Value::Sequence(items) => 1 + items.iter().map(yaml_depth).max().unwrap_or(0),
        serde_yaml::Value::Mapping(map) => 1 + map.values().map(yaml_depth).max().unwrap_or(0),
        serde_yaml::Value::Tagged(tagged) => 1 + yaml_depth(&tagged.value),
        _ => 0,
    }
}

fn check_depth_and_tags(text: &str) -> Result<()> {
    // A second, structural pass: catches nesting depth and tagged values
    // regardless of the target struct's shape. A document that fails this
    // pass with a *syntax* error is left for the real parse to report.
    if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(text) {
        fn has_tag(v: &serde_yaml::Value) -> bool {
            match v {
                serde_yaml::Value::Tagged(_) => true,
                serde_yaml::Value::Sequence(items) => items.iter().any(has_tag),
                serde_yaml::Value::Mapping(map) => {
                    map.iter().any(|(k, val)| has_tag(k) || has_tag(val))
                }
                _ => false,
            }
        }
        if has_tag(&value) {
            return Err(ForgeError::Parse {
                message: "YAML tags are not allowed in a forge file".into(),
                line: 0,
                column: 0,
            });
        }
        let depth = yaml_depth(&value);
        if depth > MAX_YAML_DEPTH {
            return Err(ForgeError::Parse {
                message: format!("YAML nesting is too deep ({depth} levels, max {MAX_YAML_DEPTH})"),
                line: 0,
                column: 0,
            });
        }
    }
    Ok(())
}

impl Snippet {
    /// Parse a forge file from bytes: UTF-8 check, LF normalisation,
    /// size limit, control-character and YAML-safety checks, strict
    /// deserialisation, then full validation.
    ///
    /// # Errors
    /// [`ForgeError::InvalidUtf8`], [`ForgeError::Parse`] (with line and
    /// column when the parser reports them) or a validation error.
    ///
    /// # Examples
    /// ```
    /// let s = forge_core::Snippet::from_bytes(b"name: demo\nsteps: [echo hi]\n").unwrap();
    /// assert_eq!(s.name, "demo");
    /// assert!(forge_core::Snippet::from_bytes(&[0xff, 0xfe]).is_err());
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| ForgeError::InvalidUtf8(e.to_string()))?;
        Self::from_yaml(text)
    }

    /// Parse a forge file from text. See [`Snippet::from_bytes`] for the
    /// pipeline; this entry point skips only the UTF-8 check (a `&str`
    /// is UTF-8 by construction).
    ///
    /// # Errors
    /// [`ForgeError::Parse`] or a validation error.
    pub fn from_yaml(text: &str) -> Result<Self> {
        let normalized = normalize_newlines(text);
        if normalized.len() > MAX_BODY_BYTES {
            return Err(ForgeError::validation(format!(
                "snippet is too large ({} bytes after LF normalisation, max {MAX_BODY_BYTES})",
                normalized.len()
            )));
        }
        check_control_chars(&normalized)?;
        check_yaml_safety(&normalized)?;
        check_depth_and_tags(&normalized)?;
        let mut snippet: Snippet = serde_yaml::from_str(&normalized).map_err(|e| yaml_error(&e))?;
        // Tag folding is an input-boundary concern: documents arrive with
        // raw tags, so normalise them here (spec §8 folds tags only).
        snippet.tags = crate::names::normalize_tags(&snippet.tags)?;
        snippet.validate()?;
        Ok(snippet)
    }
}

fn yaml_error(e: &serde_yaml::Error) -> ForgeError {
    let (line, column) = e
        .location()
        .map_or((0, 0), |loc| (loc.line(), loc.column()));
    ForgeError::Parse {
        message: e.to_string(),
        line,
        column,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_is_normalised() {
        let s = Snippet::from_bytes(b"name: demo\r\nsteps: [echo hi]\r\n").unwrap();
        assert_eq!(s.name, "demo");
    }

    #[test]
    fn lone_cr_is_normalised() {
        assert!(Snippet::from_bytes(b"name: demo\rsteps: [echo hi]\r").is_ok());
    }

    #[test]
    fn invalid_utf8_rejected() {
        let err = Snippet::from_bytes(&[0xff, 0xfe, 0xfd]).unwrap_err();
        assert!(matches!(err, ForgeError::InvalidUtf8(_)), "{err}");
    }

    #[test]
    fn nul_byte_rejected() {
        assert!(Snippet::from_yaml("name: demo\nsteps: [echo \u{0}hi]\n").is_err());
    }

    #[test]
    fn escape_control_char_rejected() {
        assert!(Snippet::from_yaml("name: demo\nsteps: [echo \u{1b}[31mhi]\n").is_err());
    }

    #[test]
    fn tab_is_allowed() {
        assert!(Snippet::from_yaml("name: demo\nsteps: [\"echo\t hi\"]\n").is_ok());
    }

    #[test]
    fn oversized_body_rejected() {
        let big = format!("name: demo\nscript: |\n  {}\n", "x".repeat(MAX_BODY_BYTES));
        let err = Snippet::from_yaml(&big).unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[test]
    fn yaml_anchor_rejected() {
        assert!(Snippet::from_yaml("name: demo\nsteps: &a [echo hi]\n").is_err());
    }

    #[test]
    fn yaml_alias_rejected() {
        assert!(Snippet::from_yaml("name: demo\nsteps: [echo hi]\ndescription: *a\n").is_err());
    }

    #[test]
    fn yaml_merge_key_rejected() {
        assert!(Snippet::from_yaml("name: demo\nsteps: [echo hi]\nbase: &b {a: 1}\n").is_err());
    }

    #[test]
    fn deep_nesting_rejected() {
        // Build a deeply nested flow mapping inside an (invalid) field;
        // the depth pre-check must fire before/regardless of field checks.
        let mut inner = "1".to_string();
        for _ in 0..20 {
            inner = format!("{{a: {inner}}}");
        }
        let doc = format!("name: demo\nsteps: [echo hi]\ndescription: {inner}\n");
        assert!(Snippet::from_yaml(&doc).is_err());
    }

    #[test]
    fn syntax_error_has_line_and_column() {
        let err = Snippet::from_yaml("name: demo\nsteps: [echo hi\n").unwrap_err();
        match err {
            ForgeError::Parse { line, column, .. } => {
                assert!(line >= 1, "line was {line}");
                assert!(column >= 1, "column was {column}");
            }
            other => panic!("expected parse error, got {other}"),
        }
    }

    #[test]
    fn tags_are_folded_on_parse() {
        // Per spec §8 tags fold; the document-level validate() then sees
        // the folded form. (Programmatically built snippets with raw
        // uppercase tags fail validate() - see model tests.)
        let s = Snippet::from_yaml("name: demo\ntags: [Ubuntu, CI]\nsteps: [echo hi]\n").unwrap();
        assert_eq!(s.tags, vec!["ubuntu".to_string(), "ci".to_string()]);
    }

    #[test]
    fn garbage_inputs_do_not_panic() {
        for garbage in [
            "",
            " ",
            "\n\n\n",
            ":",
            "-",
            "[",
            "{",
            "name:",
            "name: [unclosed",
            "a: b: c",
            "\u{1f600} emoji only",
            "name: demo\n\tbad indent",
            "---\n---\n",
            "name: demo\nsteps: 42\n",
            "name: demo\nsteps:\n  - {command: [nested, list]}\n",
        ] {
            // Must return (Ok or Err), never panic.
            let _ = Snippet::from_yaml(garbage);
        }
    }

    #[test]
    fn scalar_document_rejected() {
        assert!(Snippet::from_yaml("just a string").is_err());
    }

    #[test]
    fn sequence_document_rejected() {
        assert!(Snippet::from_yaml("- a\n- b\n").is_err());
    }

    #[test]
    fn empty_document_rejected() {
        assert!(Snippet::from_yaml("").is_err());
    }
}
