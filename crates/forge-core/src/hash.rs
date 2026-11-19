//! Canonical serialisation and content hashing (spec-snippet-format §9,
//! ADR 0006).
//!
//! The canonical form is a JSON object with a fixed key order, serialised
//! with `serde_json` compact separators. Two documents that differ only
//! in YAML formatting hash equally; any semantic change changes the hash.

use crate::model::{Condition, Mode, Param, Retry, Snippet, Step};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Canonicalise a structural string (command, path, name, value):
/// per-line trailing spaces/tabs are stripped; leading and trailing
/// blank lines are removed. Applied to every string except the script
/// body (see [`canonical_script_body`]).
#[must_use]
pub fn canonical_text(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized
        .lines()
        .map(|l| l.trim_end_matches([' ', '\t']))
        .collect();
    lines.join("\n").trim_matches('\n').to_string()
}

/// Canonicalise a script body: line endings normalised, then exactly one
/// trailing `\n`. Trailing spaces and blank lines **inside** the body are
/// preserved — shell heredocs can depend on them.
#[must_use]
pub fn canonical_script_body(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim_end_matches('\n');
    format!("{trimmed}\n")
}

fn s(v: &str) -> Value {
    Value::String(canonical_text(v))
}

fn param_json(p: &Param) -> Value {
    let mut m = Map::new();
    if let Some(d) = &p.default {
        m.insert("default".into(), s(d));
    }
    if let Some(d) = &p.description {
        m.insert("description".into(), s(d));
    }
    if p.required {
        m.insert("required".into(), Value::Bool(true));
    }
    Value::Object(m)
}

fn retry_json(r: &Retry) -> Value {
    let mut m = Map::new();
    m.insert("times".into(), Value::Number(r.times.into()));
    if let Some(d) = &r.delay {
        m.insert("delay".into(), s(d));
    }
    Value::Object(m)
}

fn condition_json(c: &Condition) -> Value {
    let mut m = Map::new();
    if let Some(v) = &c.exists {
        m.insert("exists".into(), s(v));
    }
    if let Some(v) = &c.missing {
        m.insert("missing".into(), s(v));
    }
    if let Some(v) = &c.which {
        m.insert("which".into(), s(v));
    }
    if let Some(v) = &c.env {
        m.insert("env".into(), s(v));
    }
    if let Some(v) = &c.equals {
        m.insert(
            "equals".into(),
            Value::Array(v.iter().map(|x| s(x)).collect()),
        );
    }
    if let Some(v) = &c.shell {
        m.insert("shell".into(), s(v));
    }
    Value::Object(m)
}

fn step_json(step: &Step) -> Value {
    let mut m = Map::new();
    if let Some(n) = &step.name {
        m.insert("name".into(), s(n));
    }
    if let Some(c) = &step.command {
        m.insert("command".into(), s(c));
    }
    if !step.parallel.is_empty() {
        m.insert(
            "parallel".into(),
            Value::Array(step.parallel.iter().map(|c| s(c)).collect()),
        );
    }
    if let Some(c) = &step.condition {
        m.insert("if".into(), condition_json(c));
    }
    if let Some(w) = &step.wait {
        m.insert("wait".into(), s(w));
    }
    if let Some(r) = &step.retry {
        m.insert("retry".into(), retry_json(r));
    }
    if let Some(c) = &step.cwd {
        m.insert("cwd".into(), s(c));
    }
    if !step.env.is_empty() {
        let mut env = Map::new();
        for (k, v) in &step.env {
            env.insert(k.clone(), s(v));
        }
        m.insert("env".into(), Value::Object(env));
    }
    if step.continue_on_error {
        m.insert("continue_on_error".into(), Value::Bool(true));
    }
    if step.background {
        m.insert("background".into(), Value::Bool(true));
    }
    if step.quiet {
        m.insert("quiet".into(), Value::Bool(true));
    }
    if let Some(t) = &step.timeout {
        m.insert("timeout".into(), s(t));
    }
    Value::Object(m)
}

/// The canonical serialisation of a snippet (spec §9), as bytes.
///
/// # Panics
/// Never in practice: serialising plain JSON values (strings, numbers,
/// booleans, arrays, objects) to a string cannot fail.
#[must_use]
pub fn canonical_bytes(snippet: &Snippet) -> Vec<u8> {
    let mut m = Map::new();
    m.insert("name".into(), s(&snippet.name));
    let description = canonical_text(&snippet.description);
    if !description.is_empty() {
        m.insert("description".into(), Value::String(description));
    }
    let mut tags = snippet.tags.clone();
    tags.sort();
    tags.dedup();
    if !tags.is_empty() {
        m.insert(
            "tags".into(),
            Value::Array(tags.iter().map(|t| Value::String(t.clone())).collect()),
        );
    }
    let mode = match snippet.effective_mode() {
        Mode::Script => "script",
        Mode::Steps => "steps",
    };
    m.insert("mode".into(), Value::String(mode.into()));
    // The shell counts only when the author wrote it: defaulting is
    // presentation, not content.
    if let Some(shell) = &snippet.shell {
        m.insert("shell".into(), Value::String(shell.clone()));
    }
    if !snippet.variables.is_empty() {
        let mut vars = Map::new();
        for (name, spec) in &snippet.variables {
            vars.insert(name.clone(), param_json(&spec.resolve()));
        }
        m.insert("variables".into(), Value::Object(vars));
    }
    if !snippet.env.is_empty() {
        let mut env = Map::new();
        for (k, v) in &snippet.env {
            env.insert(k.clone(), s(v));
        }
        m.insert("env".into(), Value::Object(env));
    }
    if let Some(cwd) = &snippet.cwd {
        m.insert("cwd".into(), s(cwd));
    }
    if let Some(r) = &snippet.retry {
        m.insert("retry".into(), retry_json(&r.resolve()));
    }
    match snippet.effective_mode() {
        Mode::Script => {
            m.insert(
                "script".into(),
                Value::String(canonical_script_body(
                    snippet.script.as_deref().unwrap_or_default(),
                )),
            );
        }
        Mode::Steps => {
            let steps: Vec<Value> = snippet.resolved_steps().iter().map(step_json).collect();
            m.insert("steps".into(), Value::Array(steps));
            let always: Vec<Value> = snippet.always_steps().iter().map(step_json).collect();
            if !always.is_empty() {
                m.insert("always".into(), Value::Array(always));
            }
        }
    }
    serde_json::to_string(&Value::Object(m))
        .expect("serialising JSON values cannot fail")
        .into_bytes()
}

/// Lowercase hex SHA-256 of a byte string.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        // Writing to a String is infallible.
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The content hash of a snippet: SHA-256 over [`canonical_bytes`].
///
/// # Examples
/// ```
/// let s = forge_core::Snippet::from_yaml("name: demo\nsteps: [echo hi]\n").unwrap();
/// let h = forge_core::hash::content_hash(&s);
/// assert_eq!(h.len(), 64);
/// assert_eq!(&forge_core::hash::short_hash(&s), &h[..8]);
/// ```
#[must_use]
pub fn content_hash(snippet: &Snippet) -> String {
    sha256_hex(&canonical_bytes(snippet))
}

/// The first 8 hex chars of the content hash — the display identity of a
/// version (spec §9).
#[must_use]
pub fn short_hash(snippet: &Snippet) -> String {
    content_hash(snippet)[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Snippet {
        Snippet::from_yaml(yaml).unwrap()
    }

    #[test]
    fn hash_is_64_lowercase_hex() {
        let h = content_hash(&parse("name: d\nsteps: [echo hi]\n"));
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn short_hash_is_prefix() {
        let s = parse("name: d\nsteps: [echo hi]\n");
        assert_eq!(short_hash(&s), content_hash(&s)[..8]);
    }

    #[test]
    fn deterministic_same_input_same_hash() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b = parse("name: d\nsteps: [echo hi]\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn key_order_does_not_change_hash() {
        let a = parse("name: d\ndescription: x\ntags: [ci]\nsteps: [echo hi]\n");
        let b = parse("tags: [ci]\nsteps: [echo hi]\ndescription: x\nname: d\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn quoting_style_does_not_change_hash() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b = parse("name: \"d\"\nsteps:\n  - 'echo hi'\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn crlf_does_not_change_hash() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b = Snippet::from_bytes(b"name: d\r\nsteps: [echo hi]\r\n").unwrap();
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn tag_order_and_dupes_do_not_change_hash() {
        let a = parse("name: d\ntags: [b, a]\nsteps: [echo hi]\n");
        let b = parse("name: d\ntags: [a, b, a]\nsteps: [echo hi]\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn created_and_source_do_not_change_hash() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b =
            parse("name: d\ncreated: '2026-01-01'\nsource: 'imported from x'\nsteps: [echo hi]\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn trailing_spaces_in_commands_do_not_change_hash() {
        let a = parse("name: d\nsteps: ['echo hi']\n");
        let b = parse("name: d\nsteps: ['echo hi   ']\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn changed_command_changes_hash() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b = parse("name: d\nsteps: [echo bye]\n");
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn added_tag_changes_hash() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b = parse("name: d\ntags: [ci]\nsteps: [echo hi]\n");
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn changed_default_changes_hash() {
        let a = parse("name: d\nvariables: {A: one}\nsteps: [echo hi]\n");
        let b = parse("name: d\nvariables: {A: two}\nsteps: [echo hi]\n");
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn changed_shell_changes_hash_when_explicit() {
        let a = parse("name: d\nshell: bash\nsteps: [echo hi]\n");
        let b = parse("name: d\nshell: sh\nsteps: [echo hi]\n");
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn script_trailing_newlines_do_not_change_hash() {
        let a = parse("name: d\nmode: script\nscript: |\n  echo hi\n");
        let b = parse("name: d\nmode: script\nscript: \"echo hi\\n\\n\\n\"\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn script_internal_trailing_spaces_change_hash() {
        // Spec §9: the script body is preserved byte-for-byte inside.
        let a = parse("name: d\nmode: script\nscript: |\n  echo hi \n  echo bye\n");
        let b = parse("name: d\nmode: script\nscript: |\n  echo hi\n  echo bye\n");
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn bare_param_equals_default_map() {
        let a = parse("name: d\nvariables: {A: one}\nsteps: [echo hi]\n");
        let b = parse("name: d\nvariables: {A: {default: one}}\nsteps: [echo hi]\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn bare_step_equals_map_step() {
        let a = parse("name: d\nsteps: [echo hi]\n");
        let b = parse("name: d\nsteps:\n  - {command: echo hi}\n");
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn canonical_text_trims_trailing_whitespace() {
        assert_eq!(canonical_text("a  \n b\t\n\n"), "a\n b");
    }

    #[test]
    fn canonical_script_keeps_inner_spaces() {
        assert_eq!(canonical_script_body("a \n\nb\n\n\n"), "a \n\nb\n");
    }
}
