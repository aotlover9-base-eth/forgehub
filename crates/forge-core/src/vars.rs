//! `{{VARIABLE}}` substitution, built-in values, `KEY=value` parsing,
//! path/env expansion, and duration parsing.
//!
//! Ported from Forgenesis (`src/vars.rs` and the duration parser in
//! `src/term.rs`, MIT, Copyright (c) 2026 Aotlover9, the same author),
//! with the resolution order and missing-variable behaviour specified by
//! spec-snippet-format §7.
//!
//! `ForgeHub` substitutes `{{VAR}}` placeholders itself. `$VAR`/`${VAR}`
//! in command text is left for the shell at run time; [`expand_env`]
//! exists for `cwd` path expansion, where `ForgeHub` (not a shell) must
//! produce a concrete path.

use crate::error::{ForgeError, Result};
use crate::model::Snippet;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

fn placeholder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").expect("static regex compiles")
    })
}

/// Every distinct `{{NAME}}` in `text`, in order of first appearance.
///
/// # Examples
/// ```
/// assert_eq!(forge_core::vars::find_placeholders("{{A}} x {{ B }} {{A}}"), vec!["A", "B"]);
/// ```
#[must_use]
pub fn find_placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cap in placeholder_re().captures_iter(text) {
        let name = cap[1].to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Values always available without being declared: `HOME`, `PWD`,
/// `DIRNAME`, `DATE`, `TIME`, `TIMESTAMP`, `USER` (those determinable
/// in the current process; an undeterminable one is simply absent).
#[must_use]
pub fn builtins() -> BTreeMap<String, String> {
    let now = chrono::Local::now();
    let mut m = BTreeMap::new();
    if let Some(h) = home_dir() {
        m.insert("HOME".into(), h.to_string_lossy().to_string());
    }
    if let Ok(pwd) = std::env::current_dir() {
        m.insert("PWD".into(), pwd.to_string_lossy().to_string());
        if let Some(base) = pwd.file_name() {
            m.insert("DIRNAME".into(), base.to_string_lossy().to_string());
        }
    }
    m.insert("DATE".into(), now.format("%Y-%m-%d").to_string());
    m.insert("TIME".into(), now.format("%H:%M:%S").to_string());
    m.insert("TIMESTAMP".into(), now.format("%Y%m%d-%H%M%S").to_string());
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            m.insert("USER".into(), user);
        }
    }
    m
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Replace every `{{NAME}}`; an error lists **all** missing names at once.
///
/// # Errors
/// [`ForgeError::MissingVariables`] with every unresolvable name.
///
/// # Examples
/// ```
/// use std::collections::BTreeMap;
/// let values: BTreeMap<String, String> = [("A".to_string(), "1".to_string())].into_iter().collect();
/// assert_eq!(forge_core::vars::substitute("x={{A}}", &values).unwrap(), "x=1");
/// assert!(forge_core::vars::substitute("{{B}}", &values).is_err());
/// ```
pub fn substitute(text: &str, values: &BTreeMap<String, String>) -> Result<String> {
    let mut missing: Vec<String> = Vec::new();
    let out = placeholder_re().replace_all(text, |caps: &regex::Captures| {
        let key = &caps[1];
        if let Some(v) = values.get(key) {
            v.clone()
        } else {
            if !missing.iter().any(|m| m == key) {
                missing.push(key.to_string());
            }
            caps[0].to_string()
        }
    });
    if !missing.is_empty() {
        missing.sort();
        return Err(ForgeError::MissingVariables(missing));
    }
    Ok(out.into_owned())
}

/// Substitute without failing: unknown placeholders are left as-is.
#[must_use]
pub fn substitute_lossy(text: &str, values: &BTreeMap<String, String>) -> String {
    placeholder_re()
        .replace_all(text, |caps: &regex::Captures| {
            values
                .get(&caps[1])
                .cloned()
                .unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned()
}

/// Parse `KEY=value` pairs from the command line.
///
/// # Errors
/// [`ForgeError::InvalidAssignment`] for an item without `=` or with an
/// empty/invalid key.
pub fn parse_assignments(items: &[String]) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for item in items {
        let (k, v) = item
            .split_once('=')
            .ok_or_else(|| ForgeError::InvalidAssignment(item.clone()))?;
        let k = k.trim();
        if !crate::model::is_valid_variable_name(k) {
            return Err(ForgeError::InvalidAssignment(item.clone()));
        }
        out.insert(k.to_string(), v.to_string());
    }
    Ok(out)
}

/// Resolve the values for a snippet (spec §7): CLI arguments beat
/// declared defaults, which beat built-ins. Referenced variables and
/// variables declared `required` must have a value afterwards.
///
/// # Errors
/// [`ForgeError::MissingVariables`] listing **all** missing names at once.
pub fn resolve(
    snippet: &Snippet,
    cli_args: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut values = builtins();
    for (name, spec) in &snippet.variables {
        if let Some(default) = spec.resolve().default {
            values.insert(name.clone(), default);
        }
    }
    for (k, v) in cli_args {
        values.insert(k.clone(), v.clone());
    }
    let mut missing: Vec<String> = Vec::new();
    for name in snippet.referenced_variables() {
        if !values.contains_key(&name) {
            missing.push(name);
        }
    }
    for (name, spec) in &snippet.variables {
        if spec.resolve().required && !values.contains_key(name) && !missing.contains(name) {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        missing.sort();
        missing.dedup();
        return Err(ForgeError::MissingVariables(missing));
    }
    Ok(values)
}

/// A copy-pasteable hint for a missing-variables error, e.g.
/// `pass on the command line: A=value B=value`.
#[must_use]
pub fn missing_hint(missing: &[String]) -> String {
    let pairs: Vec<String> = missing.iter().map(|m| format!("{m}=value")).collect();
    format!("pass on the command line: {}", pairs.join(" "))
}

/// Expand a leading `~` and `$VAR`/`${VAR}` references against `env`
/// (falling back to the process environment). Used for `cwd` fields.
#[must_use]
pub fn expand_path(raw: &str, env: &BTreeMap<String, String>) -> PathBuf {
    let expanded = expand_env(raw, env);
    if expanded == "~" {
        if let Some(h) = home_dir() {
            return h;
        }
    }
    if let Some(rest) = expanded.strip_prefix("~/") {
        if let Some(h) = home_dir() {
            return h.join(rest);
        }
    }
    PathBuf::from(expanded)
}

/// `$VAR` / `${VAR}` expansion, falling back to the process environment.
/// Unknown variables are left as written.
///
/// # Panics
/// Never in practice: the expansion pattern is a static, valid regex.
#[must_use]
pub fn expand_env(raw: &str, env: &BTreeMap<String, String>) -> String {
    let re: &Regex = {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| {
            Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)")
                .expect("static regex compiles")
        })
    };
    re.replace_all(raw, |caps: &regex::Captures| {
        let key = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map_or("", |m| m.as_str());
        env.get(key)
            .cloned()
            .or_else(|| std::env::var(key).ok())
            .unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

/// Resolve `candidate` against `base` unless it is already absolute.
#[must_use]
pub fn resolve_against(base: &Path, candidate: &Path) -> PathBuf {
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
}

/// Parse a duration string (`500ms`, `5s`, `2m`, `1h`) into a
/// [`Duration`]. The maximum accepted value is 24 hours (spec §1).
///
/// # Errors
/// [`ForgeError::Validation`] for a malformed or out-of-range value.
///
/// # Examples
/// ```
/// use std::time::Duration;
/// assert_eq!(forge_core::vars::parse_duration("5s").unwrap(), Duration::from_secs(5));
/// assert_eq!(forge_core::vars::parse_duration("500ms").unwrap(), Duration::from_millis(500));
/// assert!(forge_core::vars::parse_duration("soon").is_err());
/// ```
pub fn parse_duration(raw: &str) -> Result<Duration> {
    const MAX_MS: u64 = 24 * 3_600_000;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ForgeError::validation("duration is empty"));
    }
    let (num, unit_ms): (&str, u64) = if let Some(n) = raw.strip_suffix("ms") {
        (n, 1)
    } else if let Some(n) = raw.strip_suffix('s') {
        (n, 1_000)
    } else if let Some(n) = raw.strip_suffix('m') {
        (n, 60_000)
    } else if let Some(n) = raw.strip_suffix('h') {
        (n, 3_600_000)
    } else {
        return Err(ForgeError::validation(format!(
            "invalid duration `{raw}` (expected e.g. 500ms, 5s, 2m, 1h)"
        )));
    };
    let value: u64 = num
        .trim()
        .parse()
        .map_err(|_| ForgeError::validation(format!("invalid duration `{raw}`")))?;
    let ms = value
        .checked_mul(unit_ms)
        .ok_or_else(|| ForgeError::validation(format!("duration `{raw}` is too large")))?;
    if ms > MAX_MS {
        return Err(ForgeError::validation(format!(
            "duration `{raw}` exceeds the 24h maximum"
        )));
    }
    Ok(Duration::from_millis(ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn finds_placeholders_in_order_without_dupes() {
        assert_eq!(
            find_placeholders("cd ~/{{PROJECT}} && {{ TOOL }} {{PROJECT}}"),
            vec!["PROJECT", "TOOL"]
        );
    }

    #[test]
    fn no_placeholders_in_plain_text() {
        assert_eq!(find_placeholders("echo {single} and $VAR").len(), 0);
    }

    #[test]
    fn substitutes_all() {
        let out = substitute("cd ~/projects/{{PROJECT}}", &map(&[("PROJECT", "chatapp")])).unwrap();
        assert_eq!(out, "cd ~/projects/chatapp");
    }

    #[test]
    fn missing_lists_all_names_at_once() {
        let err = substitute("{{A}} {{B}} {{A}}", &BTreeMap::new()).unwrap_err();
        match err {
            ForgeError::MissingVariables(names) => {
                assert_eq!(names, vec!["A".to_string(), "B".to_string()]);
            }
            other => panic!("{other}"),
        }
    }

    #[test]
    fn missing_hint_is_copy_pasteable() {
        assert_eq!(
            missing_hint(&["A".into(), "B".into()]),
            "pass on the command line: A=value B=value"
        );
    }

    #[test]
    fn lossy_leaves_unknowns() {
        assert_eq!(
            substitute_lossy("{{A}}/{{B}}", &map(&[("A", "x")])),
            "x/{{B}}"
        );
    }

    #[test]
    fn substituted_value_is_not_reexpanded() {
        let out = substitute("{{A}}", &map(&[("A", "{{B}}")])).unwrap();
        assert_eq!(out, "{{B}}");
    }

    #[test]
    fn parses_assignments() {
        let got = parse_assignments(&["A=1".into(), "B=hello world".into(), "C=".into()]).unwrap();
        assert_eq!(got.get("B").unwrap(), "hello world");
        assert_eq!(got.get("C").unwrap(), "");
    }

    #[test]
    fn assignment_without_equals_rejected() {
        assert!(parse_assignments(&["nope".into()]).is_err());
    }

    #[test]
    fn assignment_with_bad_key_rejected() {
        assert!(parse_assignments(&["9X=1".into()]).is_err());
    }

    #[test]
    fn expands_env_forms() {
        let env = map(&[("PROJECT", "app")]);
        assert_eq!(expand_env("$PROJECT/${PROJECT}", &env), "app/app");
    }

    #[test]
    fn expands_env_unknown_left_alone() {
        assert_eq!(
            expand_env("$NOT_SET_XYZ_123", &BTreeMap::new()),
            "$NOT_SET_XYZ_123"
        );
    }

    #[test]
    fn expands_tilde() {
        if let Some(home) = home_dir() {
            assert_eq!(expand_path("~/x", &BTreeMap::new()), home.join("x"));
        }
    }

    #[test]
    fn resolve_against_joins_relative() {
        assert_eq!(
            resolve_against(Path::new("/base"), Path::new("rel")),
            PathBuf::from("/base/rel")
        );
        assert_eq!(
            resolve_against(Path::new("/base"), Path::new("/abs")),
            PathBuf::from("/abs")
        );
    }

    #[test]
    fn builtins_have_date_and_time() {
        let b = builtins();
        assert_eq!(b["DATE"].len(), 10);
        assert_eq!(b["TIME"].len(), 8);
        assert!(b.contains_key("TIMESTAMP"));
    }

    #[test]
    fn resolve_cli_beats_default_beats_builtin() {
        let s = Snippet::from_yaml(
            "name: x\nvariables:\n  ENV: staging\n  HOME: declared\nsteps:\n  - echo {{ENV}} {{HOME}}\n",
        )
        .unwrap();
        let values = resolve(&s, &map(&[("ENV", "prod")])).unwrap();
        assert_eq!(values["ENV"], "prod");
        assert_eq!(values["HOME"], "declared");
    }

    #[test]
    fn resolve_reports_all_missing() {
        let s = Snippet::from_yaml("name: x\nsteps:\n  - echo {{A}} {{B}}\n").unwrap();
        let err = resolve(&s, &BTreeMap::new()).unwrap_err();
        match err {
            ForgeError::MissingVariables(names) => {
                assert_eq!(names, vec!["A".to_string(), "B".to_string()]);
            }
            other => panic!("{other}"),
        }
    }

    #[test]
    fn resolve_required_without_default_is_missing() {
        let s = Snippet::from_yaml(
            "name: x\nvariables:\n  TOKEN: { required: true }\nsteps: [echo hi]\n",
        )
        .unwrap();
        assert!(resolve(&s, &BTreeMap::new()).is_err());
        assert!(resolve(&s, &map(&[("TOKEN", "v")])).is_ok());
    }

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("24h").unwrap(), Duration::from_hours(24));
    }

    #[test]
    fn durations_reject_garbage() {
        for bad in [
            "",
            "soon",
            "5",
            "5d",
            "-5s",
            "999999999999999999999h",
            "25h",
            "1.5s",
        ] {
            assert!(parse_duration(bad).is_err(), "{bad}");
        }
    }
}
