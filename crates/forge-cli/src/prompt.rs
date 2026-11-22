//! Terminal prompts. All confirmation logic fails safe: no terminal,
//! EOF, or anything other than an explicit yes means NO.

use std::io::{BufRead, IsTerminal, Write};

/// Is stdin attached to a terminal?
#[must_use]
pub fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

/// Ask a `[y/N]` question on stderr (so stdout stays machine-clean) and
/// read the answer from `input`. Only `y`/`yes` (any case) means yes;
/// EOF or anything else means no.
/// The answer to a step-through prompt (`--step`).
#[derive(Debug, PartialEq, Eq)]
pub enum StepChoice {
    /// Run this step.
    Run,
    /// Skip this step.
    Skip,
    /// Abort the whole run.
    Abort,
}

/// Ask about one step: `y` runs it, `n`/Enter/EOF skips it,
/// `q` aborts. Anything else is treated as skip (fail safe:
/// unrecognised input never runs code).
pub fn ask_step(input: &mut impl BufRead) -> StepChoice {
    eprint!("  Run this step? [y/N/q] ");
    let _ = std::io::Write::flush(&mut std::io::stderr());
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(_) => match line.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => StepChoice::Run,
            "q" | "quit" => StepChoice::Abort,
            _ => StepChoice::Skip,
        },
        Err(_) => StepChoice::Skip,
    }
}

pub fn ask_yes_no(input: &mut impl BufRead, question: &str) -> bool {
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => false,
        Ok(_) => matches!(line.trim().to_lowercase().as_str(), "y" | "yes"),
    }
}

/// Ask for a typed `yes` (third-party trust confirmations,
/// Step 13). Anything else - including `y` - means no.
pub fn ask_typed_yes(input: &mut impl BufRead, question: &str) -> bool {
    eprint!("{question} ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => false,
        Ok(_) => line.trim() == "yes",
    }
}

/// Read one line of free text (prompts for names, descriptions).
/// Returns `None` on EOF.
pub fn ask_line(input: &mut impl BufRead, question: &str) -> Option<String> {
    eprint!("{question} ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// Ask for a secret value with terminal echo disabled (Peak
/// C2, for `type: secret` parameters). Reads key events from
/// the terminal directly under raw mode - nothing is echoed,
/// nothing is printed back, and the value never passes through
/// a line buffer that could be logged. Returns `None` when
/// there is no terminal, on Ctrl-C / Esc, or on any terminal
/// error (fail safe: no secret is better than a leaked one).
/// An empty answer (bare Enter) also returns `None`, matching
/// `ask_line`'s "no answer" treatment in the prompt loop.
pub fn ask_secret(question: &str) -> Option<String> {
    if !stdin_is_tty() {
        return None;
    }
    eprint!("{question} ");
    let _ = std::io::stderr().flush();
    if crossterm::terminal::enable_raw_mode().is_err() {
        return None;
    }
    let result = read_secret_keys();
    let _ = crossterm::terminal::disable_raw_mode();
    eprintln!();
    result.filter(|s| !s.is_empty())
}

/// The raw-mode key loop behind [`ask_secret`]: collect
/// characters until Enter; Backspace deletes; Ctrl-C and Esc
/// cancel.
fn read_secret_keys() -> Option<String> {
    use crossterm::event::{read, Event, KeyCode, KeyEventKind, KeyModifiers};
    let mut value = String::new();
    loop {
        let Ok(Event::Key(key)) = read() else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        match key.code {
            KeyCode::Enter => return Some(value),
            KeyCode::Backspace => {
                value.pop();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return None;
            }
            KeyCode::Esc => return None,
            KeyCode::Char(c) => value.push(c),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn cursor(s: &str) -> Cursor<Vec<u8>> {
        Cursor::new(s.as_bytes().to_vec())
    }

    #[test]
    fn yes_no_accepts_only_yes_forms() {
        assert!(ask_yes_no(&mut cursor("y\n"), "q"));
        assert!(ask_yes_no(&mut cursor("YES\n"), "q"));
        assert!(!ask_yes_no(&mut cursor("n\n"), "q"));
        assert!(!ask_yes_no(&mut cursor("\n"), "q"));
        assert!(!ask_yes_no(&mut cursor(""), "q"));
        assert!(!ask_yes_no(&mut cursor("yes please\n"), "q"));
    }

    #[test]
    fn typed_yes_requires_exact_yes() {
        assert!(ask_typed_yes(&mut cursor("yes\n"), "q"));
        assert!(!ask_typed_yes(&mut cursor("y\n"), "q"));
        assert!(!ask_typed_yes(&mut cursor("Yes\n"), "q"));
        assert!(!ask_typed_yes(&mut cursor(""), "q"));
    }

    #[test]
    fn ask_line_trims_and_eof_is_none() {
        assert_eq!(
            ask_line(&mut cursor("  hello \n"), "q"),
            Some("hello".into())
        );
        assert_eq!(ask_line(&mut cursor(""), "q"), None);
    }
}
