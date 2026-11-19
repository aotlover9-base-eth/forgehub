//! The snippet document ("forge file"): YAML schema, parsing, validation.
//!
//! Portions of the step/condition/retry/param normalisation are ported
//! from Forgenesis (`src/model.rs`, MIT, Copyright (c) 2026 Aotlover9, the
//! same author) — see `docs/forgenesis-analysis.md`. `ForgeHub` adds script
//! mode, strict limits and canonical hashing on top of that model.
//!
//! The schema is defined by `docs/spec-snippet-format.md`; this module is
//! its implementation. Unknown fields are errors. Steps accept bare
//! strings or maps, as in Forgenesis.

use crate::error::{ForgeError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum serialised size of a forge file, after LF normalisation.
pub const MAX_BODY_BYTES: usize = 65_536;
/// Maximum description length in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 280;
/// Maximum total steps (`steps` + `always` + parallel branches).
pub const MAX_STEPS: usize = 256;
/// Maximum length of one step command or one script line, in bytes.
pub const MAX_COMMAND_BYTES: usize = 8_192;
/// Maximum declared variables.
pub const MAX_VARIABLES: usize = 64;
/// Maximum env entries in one map.
pub const MAX_ENV_ENTRIES: usize = 128;
/// Maximum YAML nesting depth.
pub const MAX_YAML_DEPTH: usize = 16;

/// Which of the two snippet modes a document is (spec §3–§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One shell invocation; state persists across lines.
    Script,
    /// A list of steps, each in its own shell.
    Steps,
}

/// Retry policy for a single step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    /// Attempts, 1–10.
    #[serde(alias = "attempts", alias = "count")]
    pub times: u32,
    /// Pause between attempts, e.g. `2s`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<String>,
}

/// `retry: 3` and `retry: {times: 3, delay: 2s}` both parse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RetrySpec {
    /// Bare attempt count.
    Times(u32),
    /// Full policy.
    Detailed(Retry),
}

impl RetrySpec {
    /// Resolve either form to a [`Retry`].
    #[must_use]
    pub fn resolve(&self) -> Retry {
        match self {
            RetrySpec::Times(n) => Retry {
                times: *n,
                delay: None,
            },
            RetrySpec::Detailed(r) => r.clone(),
        }
    }
}

/// A condition guarding a step (spec §6).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    /// Path that must exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exists: Option<String>,
    /// Path that must NOT exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
    /// Executable that must be on `PATH`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub which: Option<String>,
    /// Env var that must be set and non-empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// Two values that must be equal after variable substitution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<Vec<String>>,
    /// Shell snippet that must exit 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}

/// `if: package.json` is shorthand for "the path exists".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ConditionSpec {
    /// Bare path existence.
    Path(String),
    /// Full condition map.
    Detailed(Condition),
}

impl ConditionSpec {
    /// Resolve either form to a [`Condition`].
    #[must_use]
    pub fn resolve(&self) -> Condition {
        match self {
            ConditionSpec::Path(p) => Condition {
                exists: Some(p.clone()),
                ..Default::default()
            },
            ConditionSpec::Detailed(c) => c.clone(),
        }
    }
}

/// A step in map form (serde-level type; use [`Step`] for the normalised view).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StepMap {
    /// Human label shown instead of the raw command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The command to run.
    #[serde(
        default,
        alias = "run",
        alias = "cmd",
        alias = "exec",
        skip_serializing_if = "Option::is_none"
    )]
    pub command: Option<String>,
    /// Commands to run concurrently as this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<Vec<String>>,
    /// Guard condition.
    #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionSpec>,
    /// Sleep AFTER the command succeeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<String>,
    /// Retry policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetrySpec>,
    /// Working directory for this step.
    #[serde(default, alias = "workdir", skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Extra env for this step only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Keep going when this step fails.
    #[serde(
        default,
        alias = "ignore_errors",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub continue_on_error: bool,
    /// Detach and do not wait for exit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background: bool,
    /// Suppress child stdout/stderr.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
    /// Duration after which the step is killed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// A step as written: `- git pull`, `- true`, or `- {command: …, retry: 3}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StepSpec {
    /// Bare command string.
    Bare(String),
    /// Map form.
    Map(Box<StepMap>),
    /// YAML reads an unquoted `- true` as a boolean, but `true`/`false`
    /// are real commands. Accept them rather than failing to parse.
    Bool(bool),
}

/// Normalised step: exactly one of `command` or `parallel` is meaningful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Display label.
    pub name: Option<String>,
    /// Single command, if any.
    pub command: Option<String>,
    /// Parallel commands (empty unless this is a parallel step).
    pub parallel: Vec<String>,
    /// Guard condition.
    pub condition: Option<Condition>,
    /// Sleep after success.
    pub wait: Option<String>,
    /// Retry policy.
    pub retry: Option<Retry>,
    /// Working directory.
    pub cwd: Option<String>,
    /// Extra env.
    pub env: BTreeMap<String, String>,
    /// Keep going on failure.
    pub continue_on_error: bool,
    /// Detach.
    pub background: bool,
    /// Suppress output.
    pub quiet: bool,
    /// Kill after this duration.
    pub timeout: Option<String>,
}

impl Step {
    /// What to print for this step in run output.
    #[must_use]
    pub fn label(&self) -> String {
        if let Some(n) = &self.name {
            return n.clone();
        }
        if let Some(c) = &self.command {
            return c.clone();
        }
        if !self.parallel.is_empty() {
            return format!("parallel ({} commands)", self.parallel.len());
        }
        "(empty step)".into()
    }

    /// Every command this step will run, for display and analysis.
    #[must_use]
    pub fn commands(&self) -> Vec<&str> {
        match &self.command {
            Some(c) => vec![c.as_str()],
            None => self.parallel.iter().map(String::as_str).collect(),
        }
    }
}

impl StepSpec {
    /// Normalise any written form to a [`Step`].
    #[must_use]
    pub fn normalize(&self) -> Step {
        match self {
            StepSpec::Bool(b) => StepSpec::Bare(b.to_string()).normalize(),
            StepSpec::Bare(s) => Step {
                name: None,
                command: Some(s.clone()),
                parallel: Vec::new(),
                condition: None,
                wait: None,
                retry: None,
                cwd: None,
                env: BTreeMap::new(),
                continue_on_error: false,
                background: false,
                quiet: false,
                timeout: None,
            },
            StepSpec::Map(m) => Step {
                name: m.name.clone(),
                command: m.command.clone(),
                parallel: m.parallel.clone().unwrap_or_default(),
                condition: m.condition.as_ref().map(ConditionSpec::resolve),
                wait: m.wait.clone(),
                retry: m.retry.as_ref().map(RetrySpec::resolve),
                cwd: m.cwd.clone(),
                env: m.env.clone(),
                continue_on_error: m.continue_on_error,
                background: m.background,
                quiet: m.quiet,
                timeout: m.timeout.clone(),
            },
        }
    }

    /// Build a bare step from a command string.
    #[must_use]
    pub fn from_command(cmd: impl Into<String>) -> Self {
        StepSpec::Bare(cmd.into())
    }

    /// Mutate the command text(s) in place, whatever form the step takes.
    pub fn map_command(&mut self, mut f: impl FnMut(&str) -> String) {
        match self {
            StepSpec::Bool(b) => *self = StepSpec::Bare(f(&b.to_string())),
            StepSpec::Bare(s) => *s = f(s),
            StepSpec::Map(m) => {
                if let Some(c) = &m.command {
                    m.command = Some(f(c));
                }
                if let Some(p) = &mut m.parallel {
                    for c in p.iter_mut() {
                        *c = f(c);
                    }
                }
                if let Some(cwd) = &m.cwd {
                    m.cwd = Some(f(cwd));
                }
            }
        }
    }
}

/// Declared variable, map form.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Param {
    /// What the variable is for (shown in prompts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Default value, used when no value is supplied.
    #[serde(default, alias = "def", skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Whether a value must exist after resolution.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
}

/// A declared variable: a bare string is just a default value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ParamSpec {
    /// `PROJECT: myapp` — just a default.
    Default(String),
    /// Full declaration.
    Detailed(Param),
}

impl ParamSpec {
    /// Resolve either form to a [`Param`].
    #[must_use]
    pub fn resolve(&self) -> Param {
        match self {
            ParamSpec::Default(d) => Param {
                description: None,
                default: Some(d.clone()),
                required: false,
            },
            ParamSpec::Detailed(p) => p.clone(),
        }
    }
}

/// A snippet document (the forge file).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    /// Snippet name (spec §8).
    pub name: String,
    /// Short description, ≤ 280 chars, single line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Tags (normalised on parse by [`Snippet::validate`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Explicit mode; inferred from the body when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// Shell override (`bash`, `sh`, `zsh`, `fish`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    /// Declared variables.
    #[serde(
        default,
        alias = "vars",
        alias = "params",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub variables: BTreeMap<String, ParamSpec>,
    /// Environment applied to every step / the script process.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Working directory for the whole snippet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Default retry for steps without their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetrySpec>,
    /// Steps (steps mode).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepSpec>,
    /// Top-level parallel shorthand, appended after `steps`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<Vec<String>>,
    /// Script body (script mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Steps that always run at the end, even after a failure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always: Vec<StepSpec>,
    /// Informational creation stamp; excluded from the content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Informational provenance; excluded from the content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Snippet {
    /// A new empty steps-mode snippet with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            tags: Vec::new(),
            mode: None,
            shell: None,
            variables: BTreeMap::new(),
            env: BTreeMap::new(),
            cwd: None,
            retry: None,
            steps: Vec::new(),
            parallel: None,
            script: None,
            always: Vec::new(),
            created: None,
            source: None,
        }
    }

    /// The effective mode: the explicit `mode` if set, else `script` when
    /// a script body is present, else `steps`.
    #[must_use]
    pub fn effective_mode(&self) -> Mode {
        match self.mode {
            Some(m) => m,
            None => {
                if self.script.is_some() {
                    Mode::Script
                } else {
                    Mode::Steps
                }
            }
        }
    }

    /// The shell that will run this snippet (`bash` unless overridden).
    #[must_use]
    pub fn effective_shell(&self) -> &str {
        self.shell.as_deref().unwrap_or("bash")
    }

    /// Steps in execution order, including the top-level `parallel:`
    /// shorthand and the workflow default retry.
    #[must_use]
    pub fn resolved_steps(&self) -> Vec<Step> {
        let default_retry = self.retry.as_ref().map(RetrySpec::resolve);
        let mut out: Vec<Step> = self
            .steps
            .iter()
            .map(|s| {
                let mut step = s.normalize();
                if step.retry.is_none() {
                    step.retry.clone_from(&default_retry);
                }
                step
            })
            .collect();
        if let Some(par) = &self.parallel {
            if !par.is_empty() {
                out.push(Step {
                    name: None,
                    command: None,
                    parallel: par.clone(),
                    condition: None,
                    wait: None,
                    retry: default_retry,
                    cwd: None,
                    env: BTreeMap::new(),
                    continue_on_error: false,
                    background: false,
                    quiet: false,
                    timeout: None,
                });
            }
        }
        out
    }

    /// The `always` steps, normalised.
    #[must_use]
    pub fn always_steps(&self) -> Vec<Step> {
        self.always.iter().map(StepSpec::normalize).collect()
    }

    /// Total commands across steps and `always`, counting parallel branches.
    #[must_use]
    pub fn command_count(&self) -> usize {
        self.resolved_steps()
            .iter()
            .chain(self.always_steps().iter())
            .map(|s| s.commands().len())
            .sum()
    }

    /// Every command string that would run, in order (script body counts
    /// as its non-empty lines in script mode).
    #[must_use]
    pub fn all_command_texts(&self) -> Vec<String> {
        match self.effective_mode() {
            Mode::Script => self
                .script
                .as_deref()
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect(),
            Mode::Steps => self
                .resolved_steps()
                .iter()
                .chain(self.always_steps().iter())
                .flat_map(|s| {
                    s.commands()
                        .iter()
                        .map(|c| (*c).to_string())
                        .collect::<Vec<_>>()
                })
                .collect(),
        }
    }

    /// Every `{{VAR}}` referenced anywhere in the document, in order of
    /// first appearance.
    #[must_use]
    pub fn referenced_variables(&self) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        let mut push = |text: &str| {
            for name in crate::vars::find_placeholders(text) {
                if !found.contains(&name) {
                    found.push(name);
                }
            }
        };
        if let Some(c) = &self.cwd {
            push(c);
        }
        for v in self.env.values() {
            push(v);
        }
        if let Some(body) = &self.script {
            push(body);
        }
        for step in self
            .resolved_steps()
            .iter()
            .chain(self.always_steps().iter())
        {
            for c in step.commands() {
                push(c);
            }
            if let Some(c) = &step.cwd {
                push(c);
            }
            for v in step.env.values() {
                push(v);
            }
            if let Some(cond) = &step.condition {
                for text in [
                    cond.exists.as_deref(),
                    cond.missing.as_deref(),
                    cond.which.as_deref(),
                    cond.env.as_deref(),
                    cond.shell.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    push(text);
                }
                if let Some(eq) = &cond.equals {
                    for text in eq {
                        push(text);
                    }
                }
            }
        }
        found
    }

    /// Serialise back to YAML with the `ForgeHub` header.
    ///
    /// # Errors
    /// [`ForgeError::Validation`] if serialisation fails (it cannot for a
    /// validated snippet; the error exists so no caller must unwrap).
    pub fn to_yaml(&self) -> Result<String> {
        let body = serde_yaml::to_string(self)
            .map_err(|e| ForgeError::validation(format!("serializing snippet to YAML: {e}")))?;
        Ok(format!(
            "# ForgeHub snippet\n# Run with: fg {}\n{}",
            self.name, body
        ))
    }

    /// Validate every rule in spec-snippet-format §§1–8 that does not
    /// need the original text. Parsing calls this; call it again after
    /// building or mutating a snippet programmatically.
    ///
    /// # Errors
    /// The first [`ForgeError`] found; messages name the field and rule.
    pub fn validate(&self) -> Result<()> {
        crate::names::validate_snippet_name(&self.name)?;

        if self.description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(ForgeError::validation(format!(
                "description is too long ({} chars, max {MAX_DESCRIPTION_CHARS})",
                self.description.chars().count()
            )));
        }
        if self.description.contains('\n') {
            return Err(ForgeError::validation("description must be a single line"));
        }
        // Tags are validated (not folded) here: folding happens at the
        // API/import boundary via names::normalize_tags, so a stored
        // document with an uppercase tag is an error, not a surprise.
        if self.tags.len() > 20 {
            return Err(ForgeError::validation(format!(
                "too many tags ({}, max 20)",
                self.tags.len()
            )));
        }
        for t in &self.tags {
            crate::names::validate_tag(t)?;
        }
        if let Some(shell) = &self.shell {
            if !matches!(shell.as_str(), "bash" | "sh" | "zsh" | "fish") {
                return Err(ForgeError::validation(format!(
                    "unsupported shell `{shell}` (expected bash, sh, zsh or fish)"
                )));
            }
        }
        if self.variables.len() > MAX_VARIABLES {
            return Err(ForgeError::validation(format!(
                "too many variables ({}, max {MAX_VARIABLES})",
                self.variables.len()
            )));
        }
        for name in self.variables.keys() {
            if !is_valid_variable_name(name) {
                return Err(ForgeError::validation(format!(
                    "invalid variable name `{name}` (expected [A-Za-z_][A-Za-z0-9_]*)"
                )));
            }
        }
        if self.env.len() > MAX_ENV_ENTRIES {
            return Err(ForgeError::validation(format!(
                "too many env entries ({}, max {MAX_ENV_ENTRIES})",
                self.env.len()
            )));
        }

        match self.effective_mode() {
            Mode::Script => {
                if !self.steps.is_empty() || self.parallel.is_some() || !self.always.is_empty() {
                    return Err(ForgeError::validation(
                        "script mode must not also define steps, parallel or always",
                    ));
                }
                let body = self.script.as_deref().unwrap_or_default();
                if body.trim().is_empty() {
                    return Err(ForgeError::validation(
                        "script mode requires a non-empty `script:` body",
                    ));
                }
                for (i, line) in body.lines().enumerate() {
                    if line.len() > MAX_COMMAND_BYTES {
                        return Err(ForgeError::validation(format!(
                            "script line {} is too long ({} bytes, max {MAX_COMMAND_BYTES})",
                            i + 1,
                            line.len()
                        )));
                    }
                }
            }
            Mode::Steps => {
                if self.script.is_some() {
                    return Err(ForgeError::validation(
                        "steps mode must not also define a `script:` body",
                    ));
                }
                if self.steps.is_empty() && self.parallel.is_none() && self.always.is_empty() {
                    return Err(ForgeError::validation(format!(
                        "snippet `{}` has no steps",
                        self.name
                    )));
                }
            }
        }

        let total_steps = self.resolved_steps().len() + self.always_steps().len();
        if total_steps > MAX_STEPS {
            return Err(ForgeError::validation(format!(
                "too many steps ({total_steps}, max {MAX_STEPS})"
            )));
        }
        if let Some(r) = &self.retry {
            validate_retry(&r.resolve(), "workflow retry")?;
        }
        for (i, spec) in self.steps.iter().chain(self.always.iter()).enumerate() {
            validate_step(&spec.normalize(), i + 1)?;
        }
        Ok(())
    }
}

fn validate_step(step: &Step, index: usize) -> Result<()> {
    if step.command.is_none() && step.parallel.is_empty() {
        return Err(ForgeError::validation(format!(
            "step {index} has neither `command` nor `parallel`"
        )));
    }
    if step.command.is_some() && !step.parallel.is_empty() {
        return Err(ForgeError::validation(format!(
            "step {index} sets both `command` and `parallel`; use one"
        )));
    }
    for c in step.commands() {
        if c.len() > MAX_COMMAND_BYTES {
            return Err(ForgeError::validation(format!(
                "step {index} command is too long ({} bytes, max {MAX_COMMAND_BYTES})",
                c.len()
            )));
        }
        if c.trim().is_empty() {
            return Err(ForgeError::validation(format!(
                "step {index} command is empty"
            )));
        }
    }
    if let Some(name) = &step.name {
        if name.chars().count() > 120 {
            return Err(ForgeError::validation(format!(
                "step {index} name is too long (max 120 chars)"
            )));
        }
    }
    if let Some(w) = &step.wait {
        crate::vars::parse_duration(w)
            .map_err(|e| ForgeError::validation(format!("step {index} wait: {e}")))?;
    }
    if let Some(t) = &step.timeout {
        crate::vars::parse_duration(t)
            .map_err(|e| ForgeError::validation(format!("step {index} timeout: {e}")))?;
    }
    if let Some(r) = &step.retry {
        validate_retry(r, &format!("step {index} retry"))?;
    }
    if let Some(cond) = &step.condition {
        if let Some(eq) = &cond.equals {
            if eq.len() != 2 {
                return Err(ForgeError::validation(format!(
                    "step {index} condition `equals` needs exactly 2 values, got {}",
                    eq.len()
                )));
            }
        }
    }
    if step.env.len() > MAX_ENV_ENTRIES {
        return Err(ForgeError::validation(format!(
            "step {index} has too many env entries (max {MAX_ENV_ENTRIES})"
        )));
    }
    Ok(())
}

fn validate_retry(retry: &Retry, where_: &str) -> Result<()> {
    if retry.times == 0 || retry.times > 10 {
        return Err(ForgeError::validation(format!(
            "{where_}: times must be between 1 and 10, got {}",
            retry.times
        )));
    }
    if let Some(d) = &retry.delay {
        crate::vars::parse_duration(d)
            .map_err(|e| ForgeError::validation(format!("{where_} delay: {e}")))?;
    }
    Ok(())
}

/// Is `name` a valid variable name (`[A-Za-z_][A-Za-z0-9_]*`, ≤ 64 chars)?
#[must_use]
pub fn is_valid_variable_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or_default();
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps_yaml(body: &str) -> Snippet {
        Snippet::from_yaml(body).unwrap()
    }

    #[test]
    fn bare_and_map_steps_parse() {
        let wf = steps_yaml(
            "name: setup\nsteps:\n  - git pull\n  - run: pnpm install\n    if: package.json\n  - command: docker compose up -d\n    wait: 5s\n    retry: 3\n",
        );
        let steps = wf.resolved_steps();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].command.as_deref(), Some("git pull"));
        assert_eq!(
            steps[1].condition.as_ref().unwrap().exists.as_deref(),
            Some("package.json")
        );
        assert_eq!(steps[2].retry.as_ref().unwrap().times, 3);
        assert_eq!(steps[2].wait.as_deref(), Some("5s"));
    }

    #[test]
    fn bool_step_is_a_command() {
        let wf = steps_yaml("name: bools\nsteps:\n  - true\n  - false\n");
        let steps = wf.resolved_steps();
        assert_eq!(steps[0].command.as_deref(), Some("true"));
        assert_eq!(steps[1].command.as_deref(), Some("false"));
    }

    #[test]
    fn command_aliases_all_work() {
        for alias in ["command", "run", "cmd", "exec"] {
            let wf = steps_yaml(&format!("name: a\nsteps:\n  - {alias}: echo hi\n"));
            assert_eq!(
                wf.resolved_steps()[0].command.as_deref(),
                Some("echo hi"),
                "alias {alias}"
            );
        }
    }

    #[test]
    fn top_level_parallel_becomes_a_step() {
        let wf = steps_yaml("name: dev\nparallel:\n  - docker compose up\n  - code .\n");
        let steps = wf.resolved_steps();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].parallel.len(), 2);
        assert_eq!(wf.command_count(), 2);
    }

    #[test]
    fn workflow_retry_defaults_onto_steps() {
        let wf = steps_yaml(
            "name: flaky\nretry:\n  times: 2\n  delay: 1s\nsteps:\n  - curl example.com\n  - command: stable\n    retry: 5\n",
        );
        let steps = wf.resolved_steps();
        assert_eq!(steps[0].retry.as_ref().unwrap().times, 2);
        assert_eq!(
            steps[0].retry.as_ref().unwrap().delay.as_deref(),
            Some("1s")
        );
        assert_eq!(steps[1].retry.as_ref().unwrap().times, 5);
    }

    #[test]
    fn script_mode_parses() {
        let wf = steps_yaml(
            "name: ubuntu-setup\nmode: script\nscript: |\n  sudo apt update\n  sudo apt -y upgrade\n",
        );
        assert_eq!(wf.effective_mode(), Mode::Script);
        assert_eq!(wf.all_command_texts().len(), 2);
    }

    #[test]
    fn mode_inferred_script_without_explicit_mode() {
        let wf = steps_yaml("name: s\nscript: |\n  echo hi\n");
        assert_eq!(wf.effective_mode(), Mode::Script);
    }

    #[test]
    fn mode_inferred_steps_without_script() {
        let wf = steps_yaml("name: s\nsteps: [echo hi]\n");
        assert_eq!(wf.effective_mode(), Mode::Steps);
    }

    #[test]
    fn script_and_steps_conflict_rejected() {
        let err =
            Snippet::from_yaml("name: x\nmode: script\nscript: 'echo a'\nsteps: ['echo b']\n")
                .unwrap_err();
        assert!(err.to_string().contains("script mode"), "{err}");
    }

    #[test]
    fn steps_with_script_body_rejected() {
        assert!(
            Snippet::from_yaml("name: x\nmode: steps\nscript: 'echo a'\nsteps: ['echo b']\n")
                .is_err()
        );
    }

    #[test]
    fn empty_script_rejected() {
        assert!(Snippet::from_yaml("name: x\nmode: script\nscript: '   '\n").is_err());
    }

    #[test]
    fn no_steps_rejected() {
        assert!(Snippet::from_yaml("name: x\ndescription: nothing\n").is_err());
    }

    #[test]
    fn roundtrip_preserves_structure() {
        let wf = steps_yaml(
            "name: deploy\ntags: [production]\nenv:\n  NODE_ENV: production\nvariables:\n  TARGET:\n    description: server\n    required: true\nsteps:\n  - command: ssh {{TARGET}}\n    timeout: 30s\n",
        );
        let round = Snippet::from_yaml(&wf.to_yaml().unwrap()).unwrap();
        assert_eq!(round.name, "deploy");
        assert_eq!(round.tags, vec!["production"]);
        assert_eq!(round.env.get("NODE_ENV").unwrap(), "production");
        assert!(round.variables.contains_key("TARGET"));
        assert_eq!(round.referenced_variables(), vec!["TARGET".to_string()]);
    }

    #[test]
    fn conflicting_step_rejected() {
        let wf = Snippet::from_yaml("name: bad\nsteps:\n  - command: a\n    parallel: [b]\n");
        assert!(wf.is_err());
    }

    #[test]
    fn empty_step_command_rejected() {
        assert!(Snippet::from_yaml("name: bad\nsteps:\n  - '  '\n").is_err());
    }

    #[test]
    fn unknown_top_level_field_rejected() {
        assert!(Snippet::from_yaml("name: x\nsteps: [echo hi]\nbogus: 1\n").is_err());
    }

    #[test]
    fn unknown_step_field_rejected() {
        assert!(
            Snippet::from_yaml("name: x\nsteps:\n  - command: echo hi\n    bogus: 1\n").is_err()
        );
    }

    #[test]
    fn bad_shell_rejected() {
        assert!(Snippet::from_yaml("name: x\nshell: powershell\nsteps: [echo hi]\n").is_err());
    }

    #[test]
    fn each_supported_shell_ok() {
        for sh in ["bash", "sh", "zsh", "fish"] {
            assert!(
                Snippet::from_yaml(&format!("name: x\nshell: {sh}\nsteps: [echo hi]\n")).is_ok(),
                "{sh}"
            );
        }
    }

    #[test]
    fn description_too_long_rejected() {
        let d = "d".repeat(281);
        assert!(
            Snippet::from_yaml(&format!("name: x\ndescription: {d}\nsteps: [echo hi]\n")).is_err()
        );
    }

    #[test]
    fn description_at_limit_ok() {
        let d = "d".repeat(280);
        assert!(
            Snippet::from_yaml(&format!("name: x\ndescription: {d}\nsteps: [echo hi]\n")).is_ok()
        );
    }

    #[test]
    fn too_many_tags_rejected() {
        let tags: Vec<String> = (0..21).map(|i| format!("t{i}")).collect();
        let yaml = format!("name: x\ntags: [{}]\nsteps: [echo hi]\n", tags.join(", "));
        assert!(Snippet::from_yaml(&yaml).is_err());
    }

    #[test]
    fn uppercase_tag_rejected_by_validate() {
        // Parsing folds tags (spec §8), but a programmatically built
        // snippet carrying a raw uppercase tag must fail validate().
        let mut s = Snippet::new("x");
        s.tags = vec!["Ubuntu".to_string()];
        s.steps = vec![StepSpec::from_command("echo hi")];
        assert!(s.validate().is_err());
    }

    #[test]
    fn retry_zero_rejected() {
        assert!(Snippet::from_yaml("name: x\nsteps:\n  - command: a\n    retry: 0\n").is_err());
    }

    #[test]
    fn retry_eleven_rejected() {
        assert!(Snippet::from_yaml("name: x\nsteps:\n  - command: a\n    retry: 11\n").is_err());
    }

    #[test]
    fn bad_duration_rejected() {
        assert!(
            Snippet::from_yaml("name: x\nsteps:\n  - command: a\n    timeout: soon\n").is_err()
        );
    }

    #[test]
    fn duration_over_24h_rejected() {
        assert!(Snippet::from_yaml("name: x\nsteps:\n  - command: a\n    timeout: 25h\n").is_err());
    }

    #[test]
    fn equals_needs_two_values() {
        assert!(Snippet::from_yaml(
            "name: x\nsteps:\n  - command: a\n    if: { equals: ['only-one'] }\n"
        )
        .is_err());
    }

    #[test]
    fn full_condition_map_parses() {
        let wf = steps_yaml(
            "name: x\nsteps:\n  - command: a\n    if: { missing: .venv, which: docker, env: CI, shell: 'test -d .git' }\n",
        );
        let steps = wf.resolved_steps();
        let cond = steps[0].condition.as_ref().unwrap();
        assert_eq!(cond.missing.as_deref(), Some(".venv"));
        assert_eq!(cond.which.as_deref(), Some("docker"));
        assert_eq!(cond.env.as_deref(), Some("CI"));
        assert_eq!(cond.shell.as_deref(), Some("test -d .git"));
    }

    #[test]
    fn variables_bare_and_map_forms() {
        let wf = steps_yaml(
            "name: x\nvariables:\n  A: one\n  B: { default: two, required: true }\nsteps: [echo hi]\n",
        );
        assert_eq!(wf.variables["A"].resolve().default.as_deref(), Some("one"));
        assert!(wf.variables["B"].resolve().required);
    }

    #[test]
    fn variable_aliases_vars_and_params() {
        for alias in ["vars", "params"] {
            let wf = steps_yaml(&format!("name: x\n{alias}:\n  A: one\nsteps: [echo hi]\n"));
            assert!(wf.variables.contains_key("A"), "alias {alias}");
        }
    }

    #[test]
    fn invalid_variable_name_rejected() {
        assert!(
            Snippet::from_yaml("name: x\nvariables:\n  '9BAD': one\nsteps: [echo hi]\n").is_err()
        );
    }

    #[test]
    fn always_steps_normalised() {
        let wf = steps_yaml("name: x\nsteps: [echo a]\nalways: [echo done]\n");
        assert_eq!(wf.always_steps().len(), 1);
        assert_eq!(wf.command_count(), 2);
    }

    #[test]
    fn step_label_prefers_name() {
        let wf = steps_yaml("name: x\nsteps:\n  - { name: Build, command: make }\n");
        assert_eq!(wf.resolved_steps()[0].label(), "Build");
    }

    #[test]
    fn variable_name_checker() {
        assert!(is_valid_variable_name("_A1"));
        assert!(!is_valid_variable_name("1A"));
        assert!(!is_valid_variable_name("A-B"));
        assert!(!is_valid_variable_name(""));
    }
}
