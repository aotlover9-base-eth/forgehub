//! Step 5 local-parity commands: `parameterize`, `clone`, `import`,
//! `export`, `analyze`, `stats`, `doctor`, `alias`, `completions`.

use crate::exit::CliError;
use crate::store::Store;
use anyhow::{Context, Result};
use forge_core::{Mode, Snippet};
use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::PathBuf;

// ---------------------------------------------------------------- parameterize

/// A literal that appears in at least two commands and could become a
/// variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The literal text.
    pub literal: String,
    /// How many commands contain it.
    pub occurrences: usize,
    /// Suggested variable name.
    pub suggested_name: String,
}

fn candidate_literals(text: &str, out: &mut BTreeMap<String, usize>) {
    use std::sync::OnceLock;
    fn patterns() -> &'static Vec<regex::Regex> {
        static P: OnceLock<Vec<regex::Regex>> = OnceLock::new();
        P.get_or_init(|| {
            vec![
                // URLs
                regex::Regex::new(r#"https?://[^\s'"]+"#).expect("static regex"),
                // absolute or home-relative paths with at least two segments
                regex::Regex::new(r"(?:~|/)[\w.-]+(?:/[\w.-]+)+").expect("static regex"),
                // quoted strings of reasonable length
                regex::Regex::new(r#"'([^']{3,60})'|"([^"]{3,60})""#).expect("static regex"),
            ]
        })
    }
    for (i, re) in patterns().iter().enumerate() {
        for cap in re.captures_iter(text) {
            let literal = if i == 2 {
                cap.get(1)
                    .or_else(|| cap.get(2))
                    .map(|m| m.as_str().to_string())
            } else {
                cap.get(0).map(|m| m.as_str().to_string())
            };
            if let Some(lit) = literal {
                // A literal that is already (part of) a placeholder is not a candidate.
                if !lit.contains("{{") {
                    *out.entry(lit).or_insert(0) += 1;
                }
            }
        }
    }
}

fn suggest_name(literal: &str, used: &[String]) -> String {
    let base = if literal.starts_with("http") {
        "URL"
    } else if literal.starts_with('/') || literal.starts_with('~') {
        "DIR"
    } else {
        "VALUE"
    };
    let mut name = base.to_string();
    let mut n = 2;
    while used.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

/// Find parameterization candidates in a snippet: literals occurring
/// in at least two distinct commands.
#[must_use]
pub fn find_candidates(snippet: &Snippet) -> Vec<Candidate> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    // Count per command, not per occurrence, so one long command with a
    // repeated word is not a candidate on its own.
    for text in snippet.all_command_texts() {
        let mut per_command: BTreeMap<String, usize> = BTreeMap::new();
        candidate_literals(&text, &mut per_command);
        for lit in per_command.keys() {
            *counts.entry(lit.clone()).or_insert(0) += 1;
        }
    }
    let mut used: Vec<String> = snippet.variables.keys().cloned().collect();
    let mut out = Vec::new();
    for (literal, occurrences) in counts {
        if occurrences >= 2 {
            let suggested_name = suggest_name(&literal, &used);
            used.push(suggested_name.clone());
            out.push(Candidate {
                literal,
                occurrences,
                suggested_name,
            });
        }
    }
    // Longest first within the same occurrence count, then drop any
    // candidate that is a substring of a kept one (a URL and the path
    // inside it are the same literal for our purposes; the URL wins).
    out.sort_by(|a, b| {
        b.occurrences
            .cmp(&a.occurrences)
            .then(b.literal.len().cmp(&a.literal.len()))
            .then(a.literal.cmp(&b.literal))
    });
    let mut kept: Vec<Candidate> = Vec::new();
    for cand in out {
        if kept.iter().any(|k| k.literal.contains(&cand.literal)) {
            continue;
        }
        kept.push(cand);
    }
    kept
}

/// Apply chosen parameterizations: declare each variable with the
/// literal as its default and replace the literal in every command
/// (and the script body) with `{{NAME}}`.
pub fn apply_parameterization(
    snippet: &mut Snippet,
    choices: &[(String, String)], // (literal, variable name)
) {
    for (literal, name) in choices {
        snippet.variables.insert(
            name.clone(),
            forge_core::model::ParamSpec::Detailed(forge_core::model::Param {
                description: Some(
                    "Extracted from a repeated literal by `fg parameterize`".to_string(),
                ),
                default: Some(literal.clone()),
                required: false,
            }),
        );
        let replacement = format!("{{{{{name}}}}}");
        match snippet.effective_mode() {
            Mode::Script => {
                if let Some(body) = &snippet.script {
                    snippet.script = Some(body.replace(literal, &replacement));
                }
            }
            Mode::Steps => {
                for step in snippet.steps.iter_mut().chain(snippet.always.iter_mut()) {
                    step.map_command(|c| c.replace(literal, &replacement));
                }
                if let Some(par) = &mut snippet.parallel {
                    for c in par.iter_mut() {
                        *c = c.replace(literal, &replacement);
                    }
                }
            }
        }
    }
}

/// `fg parameterize <name> [--literal TEXT --as NAME …]`: a preview
/// of the ranked candidates is always printed first. Explicit
/// `--literal/--as` pairs apply exactly those; `--yes` applies every
/// candidate with its suggested name; on a terminal each candidate is
/// confirmed individually; without a terminal and without `--yes`
/// nothing is applied (exit 2 — the fail-safe rule).
#[allow(clippy::too_many_arguments)]
pub fn cmd_parameterize(
    store: &Store,
    stdin: &mut impl BufRead,
    name: &str,
    yes: bool,
    json: bool,
    literal_pairs: &[(String, String)],
) -> Result<i32> {
    let mut snippet = store.load(name)?;
    let candidates = find_candidates(&snippet);
    if candidates.is_empty() && literal_pairs.is_empty() {
        println!("No repeated literals found in `{name}`; nothing to parameterize.");
        return Ok(0);
    }
    // Preview (roadmap Step 5 item 1): every candidate, ranked.
    if !json && !candidates.is_empty() {
        println!("Repeated literals in `{name}`:");
        for cand in &candidates {
            println!(
                "  {} command(s) · {} · suggested: {{{{{}}}}}",
                cand.occurrences, cand.literal, cand.suggested_name
            );
        }
    }
    if !literal_pairs.is_empty() {
        let choices: Vec<(String, String)> = literal_pairs.to_vec();
        return apply_and_report(&mut snippet, store, name, &choices, json);
    }
    let mut choices: Vec<(String, String)> = Vec::new();
    let interactive = !yes && crate::prompt::stdin_is_tty();
    if !yes && !interactive {
        return Err(CliError::refused(format!(
            "forgehub: not confirmed\n  what: parameterize would rewrite `{name}`\n  why: there is no terminal to confirm on\n  fix: pass --yes (-y) to apply all suggestions, or --literal TEXT --as NAME for specific ones"
        ))
        .into());
    }
    for cand in &candidates {
        if interactive {
            eprintln!(
                "Literal `{}` appears in {} commands.",
                cand.literal, cand.occurrences
            );
            match crate::prompt::ask_line(
                stdin,
                &format!(
                    "Variable name (Enter for {}, or type 'skip'):",
                    cand.suggested_name
                ),
            ) {
                Some(s) if s == "skip" => {
                    // Explicitly skipped; fall through to the next candidate.
                }
                Some(s) if s.is_empty() => {
                    choices.push((cand.literal.clone(), cand.suggested_name.clone()));
                }
                Some(s) => choices.push((cand.literal.clone(), s)),
                None => break,
            }
        } else {
            choices.push((cand.literal.clone(), cand.suggested_name.clone()));
        }
    }
    if choices.is_empty() {
        println!("No parameterizations applied.");
        return Ok(0);
    }
    apply_and_report(&mut snippet, store, name, &choices, json)
}

/// Validate variable names, apply the choices, save, and report.
fn apply_and_report(
    snippet: &mut Snippet,
    store: &Store,
    name: &str,
    choices: &[(String, String)],
    json: bool,
) -> Result<i32> {
    for (_, var) in choices {
        if !forge_core::model::is_valid_variable_name(var) {
            return Err(CliError::failure(format!(
                "forgehub: `{var}` is not a valid variable name\n  what: variable names must match [A-Za-z_][A-Za-z0-9_]*\n  fix: choose a different name"
            ))
            .into());
        }
    }
    apply_parameterization(snippet, choices);
    snippet.validate()?;
    store.save(snippet)?;
    if json {
        let arr: Vec<_> = choices
            .iter()
            .map(|(lit, var)| serde_json::json!({"literal": lit, "variable": var}))
            .collect();
        println!(
            "{}",
            serde_json::json!({"name": name, "parameterized": arr})
        );
    } else {
        for (lit, var) in choices {
            println!("{lit}  ->  {{{{{var}}}}}");
        }
        println!("Saved `{name}` with {} new variable(s).", choices.len());
    }
    Ok(0)
}

// ---------------------------------------------------------------- clone / import / export

/// `fg clone <name> <new>` — duplicate a local snippet.
pub fn cmd_clone(store: &Store, name: &str, new: &str, json: bool) -> Result<i32> {
    forge_core::names::validate_snippet_name(new)?;
    if store.exists(new) {
        return Err(CliError::failure(format!(
            "forgehub: a snippet named `{new}` already exists\n  what: clone would overwrite it\n  fix: choose another name"
        ))
        .into());
    }
    let mut snippet = store.load(name)?;
    snippet.name = new.to_string();
    snippet.source = Some(format!("cloned from local `{name}`"));
    store.save(&snippet)?;
    if json {
        println!("{}", serde_json::json!({"cloned_from": name, "name": new}));
    } else {
        println!("Cloned `{name}` to `{new}`.");
    }
    Ok(0)
}

/// Import one file: strict `ForgeHub` parse first, Forgenesis
/// conversion second. Returns the saved name and conversion notes.
///
/// # Errors
/// On unreadable/invalid files and (without `force`) name collisions.
fn import_one(
    store: &Store,
    file: &std::path::Path,
    name_override: Option<&str>,
    force: bool,
) -> Result<(String, Vec<String>)> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let text = String::from_utf8(bytes).map_err(|e| {
        CliError::failure(format!(
            "forgehub: {} is not valid UTF-8: {e}",
            file.display()
        ))
    })?;
    let (mut snippet, notes) = match Snippet::from_yaml(&text) {
        Ok(s) => (s, Vec::new()),
        Err(strict_err) => match forge_core::import_forgenesis(&text) {
            Ok(outcome) => (outcome.snippet, outcome.notes),
            Err(_) => return Err(strict_err.into()),
        },
    };
    if let Some(new_name) = name_override {
        forge_core::names::validate_snippet_name(new_name)?;
        snippet.name = new_name.to_string();
    }
    if store.exists(&snippet.name) && !force {
        return Err(CliError::failure(format!(
            "forgehub: a snippet named `{}` already exists\n  what: import would overwrite it\n  fix: pass --name <other> or --force",
            snippet.name
        ))
        .into());
    }
    snippet.validate()?;
    let saved_name = snippet.name.clone();
    store.save(&snippet)?;
    Ok((saved_name, notes))
}

/// The Forgenesis workflows directory: `$FORGENESIS_HOME/workflows`,
/// or `~/.forgenesis/workflows`.
#[must_use]
pub fn forgenesis_workflows_dir() -> PathBuf {
    match std::env::var_os("FORGENESIS_HOME") {
        Some(h) if !h.is_empty() => PathBuf::from(h).join("workflows"),
        _ => std::env::var_os("HOME").map_or_else(
            || PathBuf::from(".forgenesis/workflows"),
            |h| PathBuf::from(h).join(".forgenesis").join("workflows"),
        ),
    }
}

/// `fg import <path|dir> [--from-forgenesis]`: a single file, or every
/// `.yaml`/`.yml` file in a directory. Collisions are reported and
/// skipped (never silently overwritten); `--force` overwrites.
pub fn cmd_import(
    store: &Store,
    file: &PathBuf,
    name_override: Option<&str>,
    force: bool,
    json: bool,
) -> Result<i32> {
    if file.is_dir() {
        if name_override.is_some() {
            return Err(CliError::failure(
                "forgehub: --name only applies to a single file\n  what: a directory import keeps each file's own name\n  fix: import files one at a time to rename them",
            )
            .into());
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(file)
            .with_context(|| format!("reading directory {}", file.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e == "yaml" || e == "yml")
            })
            .collect();
        entries.sort();
        if entries.is_empty() {
            return Err(CliError::failure(format!(
                "forgehub: no .yaml files in {}\n  what: a directory import needs snippet files\n  fix: pass a snippet file or a directory containing them",
                file.display()
            ))
            .into());
        }
        let mut imported: Vec<String> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut all_notes: Vec<String> = Vec::new();
        for entry in &entries {
            match import_one(store, entry, None, force) {
                Ok((saved, notes)) => {
                    for n in &notes {
                        all_notes.push(format!("{}: {n}", entry.display()));
                    }
                    imported.push(saved);
                }
                Err(e) => skipped.push(format!("{}: {e:#}", entry.display())),
            }
        }
        if json {
            println!(
                "{}",
                serde_json::json!({"imported": imported, "skipped": skipped, "notes": all_notes})
            );
        } else {
            for note in &all_notes {
                println!("note: {note}");
            }
            for s in &skipped {
                println!("skipped: {s}");
            }
            println!(
                "Imported {} snippet(s) from {}; {} skipped.",
                imported.len(),
                file.display(),
                skipped.len()
            );
        }
        return Ok(i32::from(imported.is_empty() && !skipped.is_empty()));
    }
    let (saved_name, notes) = import_one(store, file, name_override, force)?;
    if json {
        println!(
            "{}",
            serde_json::json!({"imported": saved_name, "notes": notes})
        );
    } else {
        for note in &notes {
            println!("note: {note}");
        }
        println!("Imported `{saved_name}` from {}.", file.display());
    }
    Ok(0)
}

/// `fg export <name> [--out file]`.
pub fn cmd_export(store: &Store, name: &str, out: Option<&PathBuf>) -> Result<i32> {
    let snippet = store.load(name)?;
    let yaml = snippet.to_yaml()?;
    match out {
        Some(path) => {
            std::fs::write(path, &yaml).with_context(|| format!("writing {}", path.display()))?;
            println!("Wrote `{name}` to {}", path.display());
        }
        None => print!("{yaml}"),
    }
    Ok(0)
}

// ---------------------------------------------------------------- analyze

/// Options for `fg analyze`.
pub struct AnalyzeArgs {
    /// History file override (`--file`).
    pub file: Option<PathBuf>,
    /// Minimum repetitions (`--min-count`).
    pub min_count: Option<u32>,
    /// Longest sequence to look for (`--max-len`).
    pub max_len: Option<usize>,
    /// Cap on candidates shown (`--limit`).
    pub limit: usize,
    /// Print candidates only; create nothing (`--dry`).
    pub dry: bool,
}

/// A name for a candidate that is free in the store: the suggestion,
/// or suggestion-2, -3, … on collision.
fn unique_name(store: &Store, base: &str) -> String {
    if !store.exists(base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !store.exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Ask (or, with `-y`, just do it) and save one candidate as a
/// snippet. Returns true when a snippet was created.
fn offer_candidate(
    store: &Store,
    stdin: &mut impl BufRead,
    cand: &crate::analyze::Candidate,
    yes: bool,
    interactive: bool,
) -> bool {
    let default_name = unique_name(store, &cand.suggested_name());
    let create = if yes {
        true
    } else if !interactive {
        false
    } else {
        crate::prompt::ask_yes_no(stdin, &format!("Create snippet `{default_name}`?"))
    };
    if !create {
        return false;
    }
    let name = if yes || !interactive {
        default_name.clone()
    } else {
        match crate::prompt::ask_line(stdin, &format!("Name (Enter for {default_name}):")) {
            Some(s) if !s.is_empty() => unique_name(store, &s),
            _ => default_name.clone(),
        }
    };
    let mut snippet = Snippet::new(&name);
    snippet.description = format!("Detected in shell history ({}x repeated)", cand.count);
    snippet.source = Some("analyze".to_string());
    snippet.steps = cand
        .commands
        .iter()
        .map(|c| forge_core::model::StepSpec::from_command(c.clone()))
        .collect();
    match store.save(&snippet) {
        Ok(path) => {
            println!("  saved `{name}` to {}", path.display());
            println!("  run it with `fg run {name}`");
            true
        }
        Err(e) => {
            println!("  could not save `{name}`: {e:#}");
            false
        }
    }
}

/// `fg analyze [--min-count n] [--limit n] [--max-len n] [--dry] [-y]
/// [--file path]` — mine shell history for repeated sequences and
/// offer to save them as snippets. Secrets are scrubbed inside
/// `analyze::filter_commands`, before anything is printed or written.
pub fn cmd_analyze(
    store: &Store,
    stdin: &mut impl BufRead,
    args: &AnalyzeArgs,
    yes: bool,
    json: bool,
) -> Result<i32> {
    let mut cfg = store.load_config()?;
    if let Some(m) = args.min_count {
        cfg.analyze_min_count = m.max(2);
    }
    if let Some(m) = args.max_len {
        cfg.analyze_max_len = m.max(2);
    }
    let path = match &args.file {
        Some(p) => p.clone(),
        None => crate::analyze::history_path(&cfg).ok_or_else(|| {
            CliError::failure(
                "forgehub: no shell history found\n  what: no history file in the usual locations\n  fix: set history_file in config.yaml or pass --file <path>",
            )
        })?,
    };
    if !path.is_file() {
        return Err(CliError::failure(format!(
            "forgehub: history file not found: {}",
            path.display()
        ))
        .into());
    }
    let analysis = crate::analyze::analyze_file(&path, &cfg, args.limit)?;
    if json {
        let arr: Vec<_> = analysis
            .candidates
            .iter()
            .map(|c| {
                serde_json::json!({
                    "commands": c.commands,
                    "count": c.count,
                    "suggested_name": c.suggested_name(),
                    "estimated_saved_secs": crate::analyze::estimated_savings_secs(c, cfg.human_overhead_seconds),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "file": path.display().to_string(),
                "entries": analysis.entries,
                "filtered": analysis.filtered,
                "sessions": analysis.sessions,
                "candidates": arr,
            })
        );
        return Ok(0);
    }
    println!(
        "Analyzed {} — {} commands, {} after filtering, {} session(s).",
        path.display(),
        analysis.entries,
        analysis.filtered,
        analysis.sessions
    );
    if analysis.candidates.is_empty() {
        println!(
            "No repeated sequences found (needs a sequence repeated at least {} times; try --min-count 2).",
            cfg.analyze_min_count
        );
        return Ok(0);
    }
    let interactive = crate::prompt::stdin_is_tty();
    let mut created = 0usize;
    for cand in &analysis.candidates {
        println!();
        println!("Repeated sequence ({} times):", cand.count);
        for cmd in &cand.commands {
            println!("  {}", crate::cmds::escape_display(cmd));
        }
        let saved = crate::analyze::estimated_savings_secs(cand, cfg.human_overhead_seconds);
        println!("  \u{2248}{saved:.0}s of typing so far");
        if args.dry {
            continue;
        }
        if offer_candidate(store, stdin, cand, yes, interactive) {
            created += 1;
        }
    }
    if !args.dry {
        println!();
        if created > 0 {
            println!("Created {created} snippet(s).");
        } else {
            println!("Nothing created.");
        }
    }
    Ok(0)
}

// ---------------------------------------------------------------- stats

/// Human duration: `45s`, `3m 20s`, `2h 5m`.
fn human_duration(ms: u128) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if s > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{m}m")
    }
}

/// `fg stats [--limit n] [--explain]` — run statistics from
/// `history.db` (spec-cli §3/§4): saved = (commands x
/// `human_overhead_seconds`) - actual runtime; interrupted runs are
/// excluded from every total and reported separately.
pub fn cmd_stats(store: &Store, limit: u32, explain: bool, json: bool) -> Result<i32> {
    let cfg = store.load_config()?;
    let db = crate::history::Db::open(&store.history_db_path())?;
    let totals = db.totals()?;
    let top = db.top_workflows(limit)?;
    let saved = db.saved_ms(cfg.human_overhead_seconds)?;
    if json {
        // spec-cli section 3 key set, exactly.
        let arr: Vec<_> = top
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.workflow,
                    "runs": s.runs,
                    "commands": s.commands,
                    "total_ms": s.total_ms,
                    "failures": s.failures,
                    "last_run": s.last_run,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "runs": totals.runs,
                "commands": totals.commands,
                "total_ms": totals.total_ms,
                "failures": totals.failures,
                "estimated_saved_ms": saved,
                "workflows": arr,
            })
        );
        return Ok(0);
    }
    if totals.runs == 0 {
        println!("No runs recorded yet.");
        println!("  stats appear once you `fg run` something");
        if totals.interrupted > 0 {
            println!("  ({} interrupted run(s) excluded)", totals.interrupted);
        }
        return Ok(0);
    }
    println!("Most used:");
    let width = top.iter().map(|s| s.workflow.len()).max().unwrap_or(0);
    for s in &top {
        println!(
            "  {:<width$}  {} run(s) · {} command(s){}",
            s.workflow,
            s.runs,
            s.commands,
            if s.failures > 0 {
                format!(" · {} failed", s.failures)
            } else {
                String::new()
            },
            width = width
        );
    }
    println!();
    println!("Saved time (estimated): {}", human_duration(saved));
    println!("Commands automated: {}", totals.commands);
    println!(
        "{} run(s) across {} snippet(s) · {} spent running",
        totals.runs,
        totals.workflows,
        human_duration(u128::from(totals.total_ms))
    );
    if totals.interrupted > 0 {
        println!(
            "{} interrupted run(s) excluded from these numbers.",
            totals.interrupted
        );
    }
    if explain {
        println!();
        println!(
            "Saved time = (commands x {}s of recall/typing) - time actually spent running.",
            cfg.human_overhead_seconds
        );
        println!("Tune `human_overhead_seconds` in config.yaml.");
    }
    Ok(0)
}

// ---------------------------------------------------------------- doctor

/// One doctor check. `level` is one of `ok`, `warn`, `fail`
/// (spec-cli section 3 key names).
struct Check {
    name: String,
    level: &'static str,
    detail: String,
}

fn check(name: impl Into<String>, level: &'static str, detail: String) -> Check {
    Check {
        name: name.into(),
        level,
        detail,
    }
}

fn on_path(program: &str) -> bool {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|d| std::path::Path::new(d).join(program).exists())
}

#[cfg(unix)]
#[allow(clippy::verbose_bit_mask)] // a mask reads clearer than trailing_zeros for permission bits
fn home_perms_check(store: &Store) -> Check {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(store.root()) {
        Ok(md) => {
            let mode = md.permissions().mode() & 0o777;
            if mode & 0o077 == 0 {
                check(
                    "home perms",
                    "ok",
                    format!(
                        "{} is {mode:03o} (not group/other accessible)",
                        store.root().display()
                    ),
                )
            } else {
                check(
                    "home perms",
                    "warn",
                    format!(
                        "{} is {mode:03o}; snippets can contain sensitive commands — consider chmod 700",
                        store.root().display()
                    ),
                )
            }
        }
        Err(e) => check("home perms", "fail", format!("cannot stat home: {e}")),
    }
}

fn home_and_config_checks(store: &Store) -> Vec<Check> {
    let mut checks = Vec::new();
    if store.root().exists() {
        checks.push(check(
            "home",
            "ok",
            format!("{} exists", store.root().display()),
        ));
    } else {
        checks.push(check(
            "home",
            "fail",
            format!("{} does not exist — run `fg init`", store.root().display()),
        ));
    }
    #[cfg(unix)]
    checks.push(home_perms_check(store));
    match store.load_config() {
        Ok(cfg) => {
            checks.push(check("config", "ok", "config.yaml parses".into()));
            let shell = cfg
                .default_shell
                .clone()
                .or_else(|| std::env::var("SHELL").ok())
                .unwrap_or_else(|| "bash".into());
            let shell_name = std::path::Path::new(&shell)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("bash")
                .to_string();
            if on_path(&shell_name) || std::path::Path::new(&shell).exists() {
                checks.push(check("shell", "ok", format!("{shell_name} is available")));
            } else {
                checks.push(check(
                    "shell",
                    "fail",
                    format!("shell `{shell_name}` was not found on PATH"),
                ));
            }
            match cfg.editor.clone().or_else(|| std::env::var("EDITOR").ok()) {
                Some(e) => checks.push(check("editor", "ok", format!("editor is {e}"))),
                None => checks.push(check(
                    "editor",
                    "warn",
                    "no editor configured (config editor or $EDITOR); `fg edit` will use vi".into(),
                )),
            }
        }
        Err(e) => checks.push(check(
            "config",
            "fail",
            format!("config.yaml is broken: {e}"),
        )),
    }
    checks
}

fn snippet_checks(store: &Store) -> Check {
    let dir = store.snippets_dir();
    let mut total = 0usize;
    let mut broken: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                total += 1;
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        if let Err(e) = Snippet::from_bytes(&bytes) {
                            broken.push(format!("{}: {e}", path.display()));
                        }
                    }
                    Err(e) => broken.push(format!("{}: {e}", path.display())),
                }
            }
        }
    }
    if broken.is_empty() {
        check(
            "snippets",
            "ok",
            format!("{total} snippet file(s), all parse"),
        )
    } else {
        check(
            "snippets",
            "fail",
            format!(
                "{} of {total} snippet file(s) are broken: {}",
                broken.len(),
                broken.join("; ")
            ),
        )
    }
}

fn alias_check() -> Check {
    let mut states = Vec::new();
    for shell in ["bash", "zsh", "fish"] {
        match alias_state(shell) {
            Ok(true) => states.push(format!("{shell}: installed")),
            Ok(false) => states.push(format!("{shell}: not installed")),
            Err(_) => states.push(format!("{shell}: unknown")),
        }
    }
    if states
        .iter()
        .any(|s| s.contains("installed") && !s.contains("not installed"))
    {
        check("alias", "ok", states.join(", "))
    } else {
        check(
            "alias",
            "warn",
            format!("{} — run `fg alias install`", states.join(", ")),
        )
    }
}

fn database_check(store: &Store) -> Check {
    match crate::history::Db::open(&store.history_db_path()) {
        Ok(db) => match db.totals() {
            Ok(t) => check(
                "database",
                "ok",
                format!("history.db opens; {} run(s) recorded", t.runs),
            ),
            Err(e) => check("database", "fail", format!("history.db query failed: {e}")),
        },
        Err(e) => check("database", "fail", format!("history.db cannot open: {e}")),
    }
}

#[allow(clippy::verbose_bit_mask)] // as above: permission bits read as a mask
fn credentials_check() -> Check {
    let Some(home) = std::env::var_os("HOME") else {
        return check(
            "credentials",
            "warn",
            "HOME is not set; cannot locate credentials".into(),
        );
    };
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map_or_else(|| PathBuf::from(&home).join(".config"), PathBuf::from);
    let file = dir.join("forgehub").join("credentials.toml");
    if !file.exists() {
        return check(
            "credentials",
            "ok",
            "no credentials stored yet (created by `fg login`, Step 11)".into(),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(&file) {
            Ok(md) => {
                let mode = md.permissions().mode() & 0o777;
                if mode & 0o077 == 0 {
                    check(
                        "credentials",
                        "ok",
                        format!("{} is {mode:03o}", file.display()),
                    )
                } else {
                    check(
                        "credentials",
                        "fail",
                        format!(
                            "{} is {mode:03o}; credentials must not be group/other readable — chmod 600 it",
                            file.display()
                        ),
                    )
                }
            }
            Err(e) => check(
                "credentials",
                "fail",
                format!("cannot stat credentials: {e}"),
            ),
        }
    }
    #[cfg(not(unix))]
    {
        check("credentials", "ok", format!("{} exists", file.display()))
    }
}

/// Cloud checks (Step 11): when a registry is configured, probe
/// it — reachability, clock skew against the server's `Date`
/// header, and validity of the stored token. All probes use a 3 s
/// timeout and never prompt. No registry → a single ok note, so
/// offline-only users see no change.
fn cloud_checks(store: &Store) -> Vec<Check> {
    let registry = match crate::cloud::resolve_registry(store, None) {
        Ok(Some(url)) => url,
        Ok(None) => {
            return vec![check(
                "registry",
                "ok",
                "no registry configured; cloud checks skipped (offline-safe)".into(),
            )];
        }
        Err(e) => {
            return vec![check("registry", "fail", format!("{e}"))];
        }
    };
    let mut out = Vec::new();
    if crate::cloud::registry_is_insecure(&registry) {
        out.push(check(
            "registry transport",
            "warn",
            format!(
                "registry {registry} uses plain http:// to a non-loopback host; tokens cross the network unencrypted — prefer https://"
            ),
        ));
    }
    let client =
        match crate::cloud::ApiClient::with_timeout(&registry, std::time::Duration::from_secs(3)) {
            Ok(c) => c,
            Err(e) => {
                return vec![check(
                    "registry",
                    "warn",
                    format!("registry {registry}: cannot build a client: {e}"),
                )];
            }
        };
    match client.get("/health", None) {
        Ok((_, _, headers)) => {
            out.push(check(
                "registry",
                "ok",
                format!("registry {registry} is reachable"),
            ));
            if let Some(date) = headers
                .get(reqwest::header::DATE)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| chrono::DateTime::parse_from_rfc2822(s).ok())
            {
                let skew = (chrono::Utc::now() - date.with_timezone(&chrono::Utc))
                    .num_seconds()
                    .abs();
                if skew > 300 {
                    out.push(check(
                        "clock",
                        "warn",
                        format!(
                            "local clock differs from the server by about {} min; token expiry and TLS can misbehave",
                            skew / 60
                        ),
                    ));
                } else {
                    out.push(check(
                        "clock",
                        "ok",
                        "local clock matches the server".into(),
                    ));
                }
            }
        }
        Err(e) => {
            out.push(check(
                "registry",
                "warn",
                format!("registry {registry} is unreachable: {}", e.message),
            ));
            return out;
        }
    }
    out.push(token_check(&client, &registry));
    out
}

/// The doctor token check: is there a credential for this
/// registry, and does the server still accept it? `FORGEHUB_TOKEN`
/// is honoured too.
fn token_check(client: &crate::cloud::ApiClient, registry: &str) -> Check {
    let env_token = std::env::var("FORGEHUB_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());
    let stored = crate::cloud::stored_entry(registry);
    let Some(token) = env_token.or_else(|| stored.map(|e| e.token)) else {
        return check(
            "token",
            "ok",
            "not logged in to this registry (cloud commands need `fg login`)".into(),
        );
    };
    match client.get("/me", Some(&token)) {
        Ok((_, me, _)) => check(
            "token",
            "ok",
            format!(
                "token is valid for {}",
                me["username"].as_str().unwrap_or("?")
            ),
        ),
        Err(e) if e.status == 401 => check(
            "token",
            "fail",
            "the stored token is invalid or expired — run `fg login`".into(),
        ),
        Err(e) => check(
            "token",
            "warn",
            format!("token could not be verified: {}", e.message),
        ),
    }
}

fn run_checks(store: &Store) -> Vec<Check> {
    let mut checks = home_and_config_checks(store);
    checks.push(snippet_checks(store));
    checks.push(alias_check());
    checks.push(database_check(store));
    checks.push(credentials_check());
    checks.extend(cloud_checks(store));
    let probe = store.tmp_dir().join(".doctor-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            checks.push(check("tmp", "ok", "temp dir is writable".into()));
        }
        Err(e) => checks.push(check(
            "tmp",
            "fail",
            format!("temp dir is not writable: {e}"),
        )),
    }
    if on_path("git") {
        checks.push(check("git", "ok", "git is on PATH".into()));
    } else {
        checks.push(check("git", "warn", "git was not found on PATH".into()));
    }
    checks
}

/// Ecosystem topics for `fg doctor <topic>`: optional toolchains a
/// snippet library commonly depends on. Presence is `ok`, absence is
/// `warn` (never a failure — they are optional by definition).
const ECOSYSTEM_TOPICS: &[(&str, &[&str])] = &[
    ("rust", &["rustc", "cargo"]),
    ("node", &["node", "npm"]),
    ("python", &["python3", "pip3"]),
    ("docker", &["docker"]),
    ("go", &["go"]),
];

fn topic_checks(store: &Store, topic: &str) -> Result<Vec<Check>> {
    if let Some((_, tools)) = ECOSYSTEM_TOPICS.iter().find(|(name, _)| *name == topic) {
        return Ok(tools
            .iter()
            .map(|tool| {
                if on_path(tool) {
                    check(*tool, "ok", format!("{tool} is on PATH"))
                } else {
                    check(
                        *tool,
                        "warn",
                        format!("{tool} was not found on PATH (optional)"),
                    )
                }
            })
            .collect());
    }
    let all = run_checks(store);
    let filtered: Vec<Check> = all
        .into_iter()
        .filter(|c| c.name == topic || c.name.starts_with(&format!("{topic} ")))
        .collect();
    if filtered.is_empty() {
        let mut topics: Vec<&str> = vec![
            "home",
            "config",
            "shell",
            "editor",
            "snippets",
            "alias",
            "database",
            "credentials",
            "registry",
            "token",
            "clock",
            "tmp",
            "git",
        ];
        topics.extend(ECOSYSTEM_TOPICS.iter().map(|(n, _)| *n));
        return Err(CliError::failure(format!(
            "forgehub: unknown doctor topic `{topic}`\n  what: no checks match that topic\n  fix: use one of: {}",
            topics.join(", ")
        ))
        .into());
    }
    Ok(filtered)
}

/// `fg doctor [topic]` — environment and library health checks.
/// JSON shape is spec-cli section 3: `{ "checks": [{ "name", "level",
/// "detail" }], "ok": bool }`. Exit 1 when any check fails.
pub fn cmd_doctor(store: &Store, topic: Option<&str>, json: bool) -> Result<i32> {
    let checks = match topic {
        Some(t) => topic_checks(store, t)?,
        None => run_checks(store),
    };
    let any_fail = checks.iter().any(|c| c.level == "fail");
    if json {
        let arr: Vec<_> = checks
            .iter()
            .map(|c| serde_json::json!({"name": c.name, "level": c.level, "detail": c.detail}))
            .collect();
        println!("{}", serde_json::json!({"ok": !any_fail, "checks": arr}));
    } else {
        if let Some(t) = topic {
            println!("Topic: {t}");
        }
        for c in &checks {
            let mark = match c.level {
                "ok" => "OK  ",
                "warn" => "WARN",
                _ => "FAIL",
            };
            println!("[{mark}] {}: {}", c.name, c.detail);
        }
        println!(
            "{}",
            if any_fail {
                "Some checks failed; see above."
            } else {
                "Everything looks healthy."
            }
        );
    }
    Ok(i32::from(any_fail))
}

// ---------------------------------------------------------------- alias

/// The marked block written into bash/zsh rc files (roadmap section 6).
pub const ALIAS_BEGIN: &str = "# >>> forgehub alias >>>";
/// End marker of the alias block.
pub const ALIAS_END: &str = "# <<< forgehub alias <<<";

/// The alias block body for bash/zsh.
#[must_use]
pub fn alias_block() -> String {
    format!("{ALIAS_BEGIN}\nalias fg='forgehub'\nalias jobs-fg='builtin fg'\n{ALIAS_END}\n")
}

/// Which shell to manage: the `--shell` flag, else `$SHELL`, else bash.
fn resolve_shell(flag: Option<&str>) -> String {
    if let Some(s) = flag {
        return s.to_string();
    }
    if let Ok(sh) = std::env::var("SHELL") {
        if let Some(name) = std::path::Path::new(&sh)
            .file_name()
            .and_then(|s| s.to_str())
        {
            return name.to_string();
        }
    }
    "bash".to_string()
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        CliError::failure(
            "forgehub: HOME is not set\n  what: cannot locate your shell rc file\n  fix: set HOME, or pass --shell and edit the rc file manually",
        )
        .into()
    })
}

/// The rc file for bash/zsh, or the function directory file for fish.
fn alias_target(shell: &str) -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(match shell {
        "bash" => home.join(".bashrc"),
        "zsh" => {
            if let Some(z) = std::env::var_os("ZDOTDIR") {
                PathBuf::from(z).join(".zshrc")
            } else {
                home.join(".zshrc")
            }
        }
        "fish" => home.join(".config").join("fish").join("functions").join("fg.fish"),
        other => {
            return Err(CliError::failure(format!(
                "forgehub: unsupported shell `{other}`\n  what: alias install supports bash, zsh and fish\n  fix: add `alias fg='forgehub'` to your rc file manually"
            ))
            .into())
        }
    })
}

/// Is the alias installed for `shell`?
///
/// # Errors
/// For an unsupported shell name.
pub fn alias_state(shell: &str) -> Result<bool> {
    let target = alias_target(shell)?;
    if shell == "fish" {
        let jobs = target.with_file_name("jobs-fg.fish");
        return Ok(target.exists() && jobs.exists());
    }
    let Ok(text) = std::fs::read_to_string(&target) else {
        return Ok(false);
    };
    Ok(text.contains(ALIAS_BEGIN) && text.contains("alias fg='forgehub'"))
}

/// Remove the marked block (and only the block) from rc text.
fn strip_alias_block(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_block = false;
    for line in text.lines() {
        if line.trim() == ALIAS_BEGIN {
            in_block = true;
            continue;
        }
        if line.trim() == ALIAS_END {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Back up `path` next to itself with a timestamp; returns the
/// backup path. Called before every rc edit.
fn backup_rc(path: &std::path::Path) -> Result<PathBuf> {
    let ts = forge_core::vars::builtins()
        .get("TIMESTAMP")
        .cloned()
        .unwrap_or_else(|| "unknown-time".to_string());
    let backup = path.with_file_name(format!(
        "{}.forgehub-backup-{ts}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("rc")
    ));
    std::fs::copy(path, &backup)
        .with_context(|| format!("backing up {} to {}", path.display(), backup.display()))?;
    Ok(backup)
}

/// Confirmation gate for rc edits: `-y` passes; a terminal asks
/// `[y/N]`; no terminal without `-y` refuses (exit 2).
fn confirm_rc_edit(stdin: &mut impl BufRead, yes: bool, what: &str) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !crate::prompt::stdin_is_tty() {
        return Err(CliError::refused(format!(
            "forgehub: not confirmed\n  what: {what} edits your shell rc file\n  why: there is no terminal to confirm on\n  fix: pass --yes (-y) if you have reviewed the change"
        ))
        .into());
    }
    Ok(crate::prompt::ask_yes_no(
        stdin,
        &format!("{what}. Continue?"),
    ))
}

/// `fg alias install|uninstall|show [--shell bash|zsh|fish] [-y]` —
/// the roadmap section 6 mechanism: a marked, idempotent block in the
/// shell rc file (`alias fg='forgehub'` + `alias jobs-fg='builtin
/// fg'`; fish gets `fg`/`jobs-fg` function files, the `funcsave`
/// layout). The rc file is backed up before every edit.
pub fn cmd_alias(
    stdin: &mut impl BufRead,
    action: &str,
    shell_flag: Option<&str>,
    yes: bool,
    json: bool,
) -> Result<i32> {
    let shell = resolve_shell(shell_flag);
    let target = alias_target(&shell)?;
    match action {
        "install" => {
            if alias_state(&shell)? {
                println!("`fg` alias is already installed for {shell} ({})", target.display());
                return Ok(0);
            }
            if !confirm_rc_edit(stdin, yes, &format!("alias install for {shell}"))? {
                println!("Alias install cancelled.");
                return Ok(2);
            }
            if shell == "fish" {
                install_fish(&target)?;
            } else {
                let existing = std::fs::read_to_string(&target).unwrap_or_default();
                if target.exists() {
                    let backup = backup_rc(&target)?;
                    println!("Backed up {} to {}", target.display(), backup.display());
                }
                let mut text = existing;
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&alias_block());
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                std::fs::write(&target, text)
                    .with_context(|| format!("writing {}", target.display()))?;
            }
            if json {
                println!(
                    "{}",
                    serde_json::json!({"shell": shell, "rc_file": target.display().to_string(), "installed": true})
                );
            } else {
                println!(
                    "Installed `fg` alias for {shell} in {}. Open a new shell (or source the file) to use `fg`.",
                    target.display()
                );
            }
            Ok(0)
        }
        "uninstall" => {
            if !alias_state(&shell)? {
                println!("No `fg` alias installed for {shell}.");
                return Ok(0);
            }
            if !confirm_rc_edit(stdin, yes, &format!("alias uninstall for {shell}"))? {
                println!("Alias uninstall cancelled.");
                return Ok(2);
            }
            if shell == "fish" {
                let jobs = target.with_file_name("jobs-fg.fish");
                let _ = std::fs::remove_file(&target);
                let _ = std::fs::remove_file(&jobs);
            } else {
                let backup = backup_rc(&target)?;
                println!("Backed up {} to {}", target.display(), backup.display());
                let text = std::fs::read_to_string(&target).unwrap_or_default();
                std::fs::write(&target, strip_alias_block(&text))
                    .with_context(|| format!("writing {}", target.display()))?;
            }
            if json {
                println!(
                    "{}",
                    serde_json::json!({"shell": shell, "rc_file": target.display().to_string(), "installed": false})
                );
            } else {
                println!("Removed `fg` alias for {shell} from {}.", target.display());
            }
            Ok(0)
        }
        "show" => {
            let installed = alias_state(&shell)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"shell": shell, "rc_file": target.display().to_string(), "installed": installed})
                );
            } else if installed {
                println!("`fg` alias is installed for {shell} ({})", target.display());
            } else {
                println!(
                    "`fg` alias is not installed for {shell} (would write to {})",
                    target.display()
                );
            }
            Ok(0)
        }
        other => Err(CliError::failure(format!(
            "forgehub: unknown alias action `{other}`\n  what: expected install, uninstall or show\n  fix: fg alias install"
        ))
        .into()),
    }
}

/// Write the fish function files (the layout `funcsave` produces).
fn install_fish(fg_file: &std::path::Path) -> Result<()> {
    let dir = fg_file
        .parent()
        .context("fish function path has no parent")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(
        fg_file,
        "# forgehub alias (managed by `forgehub alias install`)\nfunction fg\n    forgehub $argv\nend\n",
    )
    .with_context(|| format!("writing {}", fg_file.display()))?;
    let jobs = fg_file.with_file_name("jobs-fg.fish");
    std::fs::write(
        &jobs,
        "# forgehub alias (managed by `forgehub alias install`)\nfunction jobs-fg\n    builtin fg $argv\nend\n",
    )
    .with_context(|| format!("writing {}", jobs.display()))?;
    Ok(())
}
