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

/// The user's home directory.
///
/// Unix shells set `HOME`; Windows does not - it sets
/// `USERPROFILE` (and `HOMEDRIVE` + `HOMEPATH`). All three
/// spellings are honoured, so the CLI works in a plain
/// Windows terminal where `HOME` is unset (the v0.1.5 exe
/// failed every store command there for exactly this).
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    home_dir_from(|k| std::env::var_os(k))
}

/// [`home_dir`] over an injected environment lookup, so the
/// resolution order is testable without mutating the
/// process environment.
fn home_dir_from(get: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let pick = |k: &str| get(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(h) = pick("HOME") {
        return Some(h);
    }
    if let Some(p) = pick("USERPROFILE") {
        return Some(p);
    }
    match (get("HOMEDRIVE"), get("HOMEPATH")) {
        (Some(d), Some(p)) if !d.is_empty() && !p.is_empty() => {
            let mut joined = d;
            joined.push(&p);
            Some(PathBuf::from(joined))
        }
        _ => None,
    }
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

/// Whether a string is a finite number (`1`, `-2.5`, `1e3`).
/// `inf`/`NaN` parse as floats in Rust but are not accepted:
/// a parameter value should be a number a shell script can
/// compare.
#[must_use]
pub fn is_valid_number(value: &str) -> bool {
    value.trim().parse::<f64>().is_ok_and(f64::is_finite)
}

/// Parse a yes/no answer to its canonical form (`yes`/`no`).
/// Accepts yes/y/true/1 and no/n/false/0, case-insensitively.
#[must_use]
pub fn parse_yesno(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "y" | "true" | "1" => Some("yes"),
        "no" | "n" | "false" | "0" => Some("no"),
        _ => None,
    }
}

/// Validate (and, for `yesno`, normalise) one resolved value
/// against its parameter's declared type (Peak C2). The error
/// names the parameter and the expectation; a secret's value
/// is never quoted back.
///
/// # Errors
/// [`ForgeError::Validation`] when the value violates the
/// declared type (not a choice, not a number, not yes/no, or
/// an empty path).
pub fn coerce_value(name: &str, param: &crate::model::Param, value: &str) -> Result<String> {
    match param.kind() {
        "choice" => {
            if param.choices.iter().any(|c| c == value) {
                Ok(value.to_string())
            } else {
                Err(ForgeError::validation(format!(
                    "parameter `{name}`: `{value}` is not one of its choices ({})",
                    param.choices.join(", ")
                )))
            }
        }
        "number" => {
            if is_valid_number(value) {
                Ok(value.to_string())
            } else {
                Err(ForgeError::validation(format!(
                    "parameter `{name}`: `{value}` is not a number"
                )))
            }
        }
        "yesno" => parse_yesno(value).map(str::to_string).ok_or_else(|| {
            ForgeError::validation(format!(
                "parameter `{name}`: `{value}` is not a yes/no value (expected yes or no)"
            ))
        }),
        "path" => {
            if value.trim().is_empty() {
                Err(ForgeError::validation(format!(
                    "parameter `{name}`: a path value must not be empty"
                )))
            } else {
                Ok(value.to_string())
            }
        }
        // text and secret accept any string; the secret's value
        // is deliberately not echoed in any error.
        _ => Ok(value.to_string()),
    }
}

/// Resolve the values for a snippet (spec §7): CLI arguments beat
/// declared defaults, which beat built-ins. Referenced variables and
/// variables declared `required` must have a value afterwards.
/// Declared parameters are then type-checked (Peak C2): a value
/// that violates its parameter's `type:` is an error, and `yesno`
/// values are normalised to `yes`/`no`.
///
/// # Errors
/// [`ForgeError::MissingVariables`] listing **all** missing names at once,
/// or a validation error naming the offending parameter.
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
    // Peak C2: type-check the resolved values of declared
    // parameters (from any source: CLI, prompt or default).
    for (name, spec) in &snippet.variables {
        if let Some(value) = values.get(name).cloned() {
            let coerced = coerce_value(name, &spec.resolve(), &value)?;
            values.insert(name.clone(), coerced);
        }
    }
    Ok(values)
}

/// The mask shown wherever a secret parameter's value would
/// otherwise be displayed (plans, cards, step labels).
pub const SECRET_MASK: &str = "••••••";

/// Names of the snippet's parameters declared `type: secret`.
#[must_use]
pub fn secret_names(snippet: &Snippet) -> Vec<String> {
    snippet
        .variables
        .iter()
        .filter(|(_, spec)| spec.resolve().is_secret())
        .map(|(name, _)| name.clone())
        .collect()
}

/// A copy of `values` with every secret parameter's value
/// replaced by [`SECRET_MASK`], for display surfaces. Execution
/// always uses the unmasked map.
#[must_use]
pub fn display_values(
    snippet: &Snippet,
    values: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let secrets = secret_names(snippet);
    if secrets.is_empty() {
        return values.clone();
    }
    let mut masked = values.clone();
    for name in secrets {
        if masked.contains_key(&name) {
            masked.insert(name, SECRET_MASK.to_string());
        }
    }
    masked
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

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<std::ffi::OsString> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| {
            owned
                .iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| std::ffi::OsString::from(v.clone()))
        }
    }

    #[test]
    fn home_dir_prefers_home_then_userprofile_then_drive_path() {
        assert_eq!(
            home_dir_from(env(&[
                ("HOME", "/home/ada"),
                ("USERPROFILE", "C:\\Users\\ada")
            ])),
            Some(PathBuf::from("/home/ada"))
        );
        assert_eq!(
            home_dir_from(env(&[("USERPROFILE", "C:\\Users\\ada")])),
            Some(PathBuf::from("C:\\Users\\ada"))
        );
        assert_eq!(
            home_dir_from(env(&[("HOMEDRIVE", "C:"), ("HOMEPATH", "\\Users\\ada")])),
            Some(PathBuf::from("C:\\Users\\ada"))
        );
    }

    #[test]
    fn home_dir_treats_empty_as_unset_and_gives_up_cleanly() {
        assert_eq!(
            home_dir_from(env(&[("HOME", ""), ("USERPROFILE", "C:\\Users\\ada")])),
            Some(PathBuf::from("C:\\Users\\ada"))
        );
        assert_eq!(home_dir_from(env(&[])), None);
        assert_eq!(home_dir_from(env(&[("HOMEDRIVE", "C:")])), None);
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

#[cfg(test)]
mod typed_param_tests {
    use super::*;
    use crate::Snippet;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn snippet(yaml: &str) -> Snippet {
        Snippet::from_yaml(yaml).unwrap()
    }

    const TYPED: &str = "name: typed\nvariables:\n  REGION:\n    type: choice\n    choices: [eu, us]\n    default: eu\n  COUNT:\n    type: number\n  CONFIRM:\n    type: yesno\n  TARGET:\n    type: path\n    required: true\n  TOKEN:\n    type: secret\n    required: true\nsteps:\n  - run: echo {{REGION}} {{TARGET}}\n";

    #[test]
    fn typed_document_validates() {
        snippet(TYPED).validate().unwrap();
    }

    #[test]
    fn invalid_param_declarations_are_refused() {
        let cases = [
            // unknown type
            "name: x\nvariables:\n  A:\n    type: date\nsteps: [echo hi]\n",
            // choice without choices
            "name: x\nvariables:\n  A:\n    type: choice\nsteps: [echo hi]\n",
            // choice default outside the list
            "name: x\nvariables:\n  A:\n    type: choice\n    choices: [eu]\n    default: us\nsteps: [echo hi]\n",
            // choices on a text param
            "name: x\nvariables:\n  A:\n    choices: [eu]\nsteps: [echo hi]\n",
            // number with a non-number default
            "name: x\nvariables:\n  A:\n    type: number\n    default: many\nsteps: [echo hi]\n",
            // yesno with a bad default
            "name: x\nvariables:\n  A:\n    type: yesno\n    default: maybe\nsteps: [echo hi]\n",
            // secret with a default
            "name: x\nvariables:\n  A:\n    type: secret\n    default: hunter2\nsteps: [echo hi]\n",
        ];
        for yaml in cases {
            // from_yaml validates, so refusal may surface at
            // parse time or at validate() - either is a refusal.
            let refused = match Snippet::from_yaml(yaml) {
                Ok(s) => s.validate().is_err(),
                Err(_) => true,
            };
            assert!(refused, "must refuse: {yaml}");
        }
    }

    #[test]
    fn resolve_type_checks_and_normalises() {
        let s = snippet(TYPED);
        let ok = resolve(
            &s,
            &map(&[
                ("REGION", "us"),
                ("COUNT", "3.5"),
                ("CONFIRM", "True"),
                ("TARGET", "/tmp/x"),
                ("TOKEN", "s3cr3t"),
            ]),
        )
        .unwrap();
        assert_eq!(ok["REGION"], "us");
        assert_eq!(ok["CONFIRM"], "yes", "yesno is normalised");
        assert_eq!(ok["TOKEN"], "s3cr3t");

        for (name, value) in [
            ("REGION", "moon"),
            ("COUNT", "many"),
            ("COUNT", "inf"),
            ("CONFIRM", "perhaps"),
            ("TARGET", "  "),
        ] {
            let mut args = map(&[("TARGET", "/tmp/x"), ("TOKEN", "t")]);
            args.insert(name.to_string(), value.to_string());
            let err = resolve(&s, &args).unwrap_err();
            assert!(
                err.to_string().contains(name),
                "error names the parameter: {err}"
            );
        }
    }

    #[test]
    fn secret_values_are_masked_for_display_only() {
        let s = snippet(TYPED);
        let values = resolve(&s, &map(&[("TARGET", "/tmp/x"), ("TOKEN", "s3cr3t")])).unwrap();
        assert_eq!(secret_names(&s), ["TOKEN"]);
        let shown = display_values(&s, &values);
        assert_eq!(shown["TOKEN"], SECRET_MASK);
        assert_eq!(shown["TARGET"], "/tmp/x");
        assert_eq!(values["TOKEN"], "s3cr3t", "execution map is untouched");
    }

    #[test]
    fn pre_c2_documents_resolve_as_before() {
        let s = snippet("name: old\nvariables:\n  PLAIN: hello\n  FULL:\n    default: world\n    required: true\nsteps:\n  - run: echo {{PLAIN}} {{FULL}}\n");
        s.validate().unwrap();
        let values = resolve(&s, &map(&[])).unwrap();
        assert_eq!(values["PLAIN"], "hello");
        assert_eq!(values["FULL"], "world");
    }
}
