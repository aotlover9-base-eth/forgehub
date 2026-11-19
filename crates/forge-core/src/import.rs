//! Importing Forgenesis YAML (spec-snippet-format §12).
//!
//! Forgenesis files are almost `ForgeHub` files. The import layer is
//! deliberately a *conversion layer over YAML values*, not a second
//! parser: it drops fields `ForgeHub` removed (each drop is reported),
//! filters unknown step keys, lowercases the name, and then hands the
//! cleaned value to the strict `ForgeHub` parser so imported files obey
//! exactly the same rules as native ones.

use crate::error::{ForgeError, Result};
use crate::model::Snippet;
use serde_yaml::{Mapping, Value};

const KNOWN_TOP_LEVEL: &[&str] = &[
    "name",
    "description",
    "tags",
    "mode",
    "shell",
    "variables",
    "vars",
    "params",
    "env",
    "cwd",
    "retry",
    "steps",
    "parallel",
    "script",
    "always",
    "created",
    "source",
];

const KNOWN_STEP_KEYS: &[&str] = &[
    "name",
    "command",
    "run",
    "cmd",
    "exec",
    "parallel",
    "if",
    "wait",
    "retry",
    "cwd",
    "workdir",
    "env",
    "continue_on_error",
    "ignore_errors",
    "background",
    "quiet",
    "timeout",
];

/// The result of a Forgenesis import: the converted snippet plus a
/// human-readable list of every conversion and drop, so nothing about
/// the file changes silently.
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    /// The converted, fully validated snippet.
    pub snippet: Snippet,
    /// Notes like "dropped unknown field `packs`".
    pub notes: Vec<String>,
}

fn filter_step(value: &Value, notes: &mut Vec<String>) -> Value {
    match value {
        Value::Mapping(map) => {
            let mut out = Mapping::new();
            for (k, v) in map {
                let key = k.as_str().unwrap_or_default().to_string();
                if KNOWN_STEP_KEYS.contains(&key.as_str()) {
                    out.insert(k.clone(), v.clone());
                } else {
                    notes.push(format!("dropped unknown step field `{key}`"));
                }
            }
            Value::Mapping(out)
        }
        other => other.clone(),
    }
}

/// Convert a Forgenesis document to a `ForgeHub` [`Snippet`].
///
/// Conversions (each recorded in the outcome's notes): unknown top-level
/// and step fields are dropped (Forgenesis-only features such as packs
/// land here); the name is lowercased if needed; tags are folded by the
/// normal parse path.
///
/// # Errors
/// A parse/validation error if the converted document still breaks a
/// `ForgeHub` rule — the message says which.
pub fn import_forgenesis(text: &str) -> Result<ImportOutcome> {
    let normalized = crate::parse::normalize_newlines(text);
    if normalized.len() > crate::model::MAX_BODY_BYTES {
        return Err(ForgeError::validation(format!(
            "imported file is too large ({} bytes, max {})",
            normalized.len(),
            crate::model::MAX_BODY_BYTES
        )));
    }
    let value: Value = serde_yaml::from_str(&normalized).map_err(|e| ForgeError::Parse {
        message: format!("importing Forgenesis file: {e}"),
        line: e.location().map_or(0, |l| l.line()),
        column: e.location().map_or(0, |l| l.column()),
    })?;
    let Value::Mapping(map) = value else {
        return Err(ForgeError::validation(
            "a Forgenesis file must be a YAML mapping at the top level",
        ));
    };

    let mut notes: Vec<String> = Vec::new();
    let mut out = Mapping::new();
    for (k, v) in &map {
        let key = k.as_str().unwrap_or_default().to_string();
        if !KNOWN_TOP_LEVEL.contains(&key.as_str()) {
            notes.push(format!("dropped unknown field `{key}`"));
            continue;
        }
        let mut v = v.clone();
        if key == "name" {
            if let Some(name) = v.as_str() {
                let lowered = name.to_lowercase();
                if lowered != name {
                    notes.push(format!("lowercased name `{name}` to `{lowered}`"));
                    v = Value::String(lowered);
                }
            }
        }
        if key == "steps" || key == "always" {
            if let Value::Sequence(items) = &v {
                v = Value::Sequence(items.iter().map(|s| filter_step(s, &mut notes)).collect());
            }
        }
        out.insert(k.clone(), v);
    }

    let cleaned = serde_yaml::to_string(&Value::Mapping(out))
        .map_err(|e| ForgeError::validation(format!("re-serialising import: {e}")))?;
    let snippet = Snippet::from_yaml(&cleaned)?;
    Ok(ImportOutcome { snippet, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_forgenesis_file_imports_without_notes() {
        let out = import_forgenesis("name: setup\nsteps:\n  - git pull\n  - run: make\n").unwrap();
        assert_eq!(out.snippet.name, "setup");
        assert_eq!(out.snippet.resolved_steps().len(), 2);
        assert_eq!(out.notes.len(), 0, "{:?}", out.notes);
    }

    #[test]
    fn unknown_top_level_fields_are_dropped_and_noted() {
        let out =
            import_forgenesis("name: setup\npacks: [web]\nsync: true\nsteps: [echo hi]\n").unwrap();
        assert_eq!(out.snippet.name, "setup");
        assert_eq!(out.notes.len(), 2);
        assert!(out.notes.iter().any(|n| n.contains("packs")));
        assert!(out.notes.iter().any(|n| n.contains("sync")));
    }

    #[test]
    fn unknown_step_fields_are_dropped_and_noted() {
        let out =
            import_forgenesis("name: setup\nsteps:\n  - command: echo hi\n    plugin: fancy\n")
                .unwrap();
        assert!(out.notes.iter().any(|n| n.contains("plugin")));
        assert_eq!(out.snippet.command_count(), 1);
    }

    #[test]
    fn uppercase_name_is_lowercased_and_noted() {
        let out = import_forgenesis("name: MySetup\nsteps: [echo hi]\n").unwrap();
        assert_eq!(out.snippet.name, "mysetup");
        assert!(out.notes.iter().any(|n| n.contains("lowercased")));
    }

    #[test]
    fn still_invalid_after_conversion_errors() {
        // Lowercasing cannot save a name with spaces.
        assert!(import_forgenesis("name: 'My Setup'\nsteps: [echo hi]\n").is_err());
    }

    #[test]
    fn non_mapping_document_errors() {
        assert!(import_forgenesis("- just\n- a\n- list\n").is_err());
    }

    #[test]
    fn garbage_errors_without_panic() {
        assert!(import_forgenesis("name: [unclosed").is_err());
        assert!(import_forgenesis("").is_err());
    }

    #[test]
    fn forgenesis_features_survive() {
        // Conditions, retries, variables with `def`, always-steps — the
        // Forgenesis feature set the analysis said to keep.
        let out = import_forgenesis(
            "name: ci\nvars:\n  BRANCH: { def: main }\nretry: 2\nsteps:\n  - command: git checkout {{BRANCH}}\n    if: .git\nalways: [echo done]\n",
        )
        .unwrap();
        let s = &out.snippet;
        assert_eq!(
            s.variables["BRANCH"].resolve().default.as_deref(),
            Some("main")
        );
        assert_eq!(s.always_steps().len(), 1);
        assert_eq!(
            s.resolved_steps()[0]
                .condition
                .as_ref()
                .unwrap()
                .exists
                .as_deref(),
            Some(".git")
        );
    }
}
