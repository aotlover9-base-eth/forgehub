//! Terminal output design system (Peak B1; agent.md section 8.4,
//! spec-cli section 3).
//!
//! One module owns how the CLI renders to a terminal: a single
//! accent (blue) for headings and selection, dim for secondary
//! text, and red / amber / green reserved for errors, warnings and
//! success. Colour is resolved once at startup from the `--color`
//! flag, `NO_COLOR`, `TERM` and whether stdout is a terminal;
//! non-TTY output is always plain, so pipes and captured test
//! output stay byte-stable. Box-drawing components (cards) fall
//! back to ASCII when the locale is not UTF-8 or `TERM=dumb`.
//!
//! Commands that print through this module get the whole contract
//! for free; plain `println!` output elsewhere is unchanged in
//! non-TTY mode, which is the mode the contract pins down.

use std::io::IsTerminal;
use std::sync::OnceLock;

/// The `--color` flag values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorMode {
    /// Colour when stdout is a terminal, `NO_COLOR` is unset and
    /// `TERM` is not `dumb`.
    Auto,
    /// Colour even when piped (still honours nothing else).
    Always,
    /// Never colour.
    Never,
}

/// The resolved rendering style for this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// Emit ANSI colour codes.
    pub color: bool,
    /// Box-drawing characters are safe to emit.
    pub unicode: bool,
}

impl Style {
    /// No colour, no box-drawing: the safe default before
    /// [`init`] runs and for captured output.
    pub const PLAIN: Style = Style {
        color: false,
        unicode: false,
    };

    /// Pure resolution, split out for tests: every input is a
    /// parameter, nothing is read from the environment here.
    #[must_use]
    pub fn resolve(
        mode: ColorMode,
        no_color: bool,
        term_dumb: bool,
        utf8: bool,
        tty: bool,
    ) -> Style {
        let color = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => tty && !no_color && !term_dumb,
        };
        Style {
            color,
            unicode: utf8 && !term_dumb,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// The one accent: headings and selection only.
    #[must_use]
    pub fn accent(&self, text: &str) -> String {
        self.paint("34", text)
    }

    /// Secondary text.
    #[must_use]
    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    /// Success only.
    #[must_use]
    pub fn success(&self, text: &str) -> String {
        self.paint("32", text)
    }

    /// Warnings only.
    #[must_use]
    pub fn warn(&self, text: &str) -> String {
        self.paint("33", text)
    }

    /// Errors only.
    #[must_use]
    pub fn error(&self, text: &str) -> String {
        self.paint("31", text)
    }

    /// A section heading in the accent colour.
    #[must_use]
    pub fn heading(&self, text: &str) -> String {
        self.accent(text)
    }

    /// An aligned key-value block: keys padded to the widest key,
    /// dimmed when colour is on.
    #[must_use]
    pub fn kv(&self, pairs: &[(&str, &str)]) -> String {
        let width = pairs
            .iter()
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(0);
        pairs
            .iter()
            .map(|(k, v)| format!("{}  {v}", self.dim(&format!("{k:<width$}"))))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A rounded "surface" card (spec-cli section 3): box-drawing
    /// borders in Unicode locales, ASCII otherwise. Widths are
    /// character counts; wide glyphs (CJK, emoji) are not
    /// width-compensated.
    #[must_use]
    pub fn card(&self, title: &str, lines: &[String]) -> String {
        let (tl, tr, bl, br, h, v) = if self.unicode {
            (
                '\u{256d}', '\u{256e}', '\u{2570}', '\u{256f}', '\u{2500}', '\u{2502}',
            )
        } else {
            ('+', '+', '+', '+', '-', '|')
        };
        let inner = lines
            .iter()
            .map(|l| l.chars().count())
            .chain(std::iter::once(title.chars().count() + 2))
            .max()
            .unwrap_or(0);
        let mut out = String::new();
        // Top border carries the title: ╭─ title ───╮
        let title_part = format!("{h} {title} ");
        let fill = inner + 2 - title_part.chars().count();
        out.push(tl);
        out.push_str(&self.accent(&title_part));
        for _ in 0..fill {
            out.push(h);
        }
        out.push(tr);
        for line in lines {
            out.push('\n');
            out.push(v);
            out.push(' ');
            out.push_str(line);
            for _ in 0..(inner - line.chars().count()) {
                out.push(' ');
            }
            out.push(' ');
            out.push(v);
        }
        out.push('\n');
        out.push(bl);
        for _ in 0..(inner + 2) {
            out.push(h);
        }
        out.push(br);
        out
    }
}

static STYLE: OnceLock<Style> = OnceLock::new();

/// The process style, [`Style::PLAIN`] until [`init`] runs.
#[must_use]
pub fn style() -> Style {
    STYLE.get().copied().unwrap_or(Style::PLAIN)
}

/// Resolve and store the process style from the flag and the
/// environment. Idempotent: the first call wins.
pub fn init(mode: ColorMode) -> Style {
    let no_color = std::env::var_os("NO_COLOR").is_some();
    let term_dumb = std::env::var("TERM").is_ok_and(|t| t == "dumb");
    let utf8 = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .any(|v| {
            let lower = v.to_lowercase();
            lower.contains("utf-8") || lower.contains("utf8")
        });
    let tty = std::io::stdout().is_terminal();
    let resolved = Style::resolve(mode, no_color, term_dumb, utf8, tty);
    *STYLE.get_or_init(|| resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_colours_only_on_a_capable_terminal() {
        assert!(Style::resolve(ColorMode::Auto, false, false, true, true).color);
        assert!(
            !Style::resolve(ColorMode::Auto, false, false, true, false).color,
            "piped"
        );
        assert!(
            !Style::resolve(ColorMode::Auto, true, false, true, true).color,
            "NO_COLOR"
        );
        assert!(
            !Style::resolve(ColorMode::Auto, false, true, true, true).color,
            "TERM=dumb"
        );
    }

    #[test]
    fn always_and_never_override_the_terminal() {
        assert!(Style::resolve(ColorMode::Always, true, true, false, false).color);
        assert!(!Style::resolve(ColorMode::Never, false, false, true, true).color);
    }

    #[test]
    fn unicode_needs_utf8_and_a_capable_term() {
        assert!(Style::resolve(ColorMode::Auto, false, false, true, true).unicode);
        assert!(!Style::resolve(ColorMode::Auto, false, false, false, true).unicode);
        assert!(!Style::resolve(ColorMode::Auto, false, true, true, true).unicode);
    }

    #[test]
    fn plain_style_never_emits_codes() {
        let s = Style::PLAIN;
        assert_eq!(s.accent("x"), "x");
        assert_eq!(s.error("boom"), "boom");
        assert_eq!(s.success("ok"), "ok");
    }

    #[test]
    fn colour_style_wraps_and_resets() {
        let s = Style {
            color: true,
            unicode: true,
        };
        assert_eq!(s.accent("x"), "\x1b[34mx\x1b[0m");
        assert_eq!(s.error("boom"), "\x1b[31mboom\x1b[0m");
        assert_eq!(s.warn("w"), "\x1b[33mw\x1b[0m");
        assert_eq!(s.success("ok"), "\x1b[32mok\x1b[0m");
        assert_eq!(s.dim("d"), "\x1b[2md\x1b[0m");
    }

    #[test]
    fn kv_aligns_to_the_widest_key() {
        let out = Style::PLAIN.kv(&[("name", "deploy"), ("registry", "https://x")]);
        assert_eq!(out, "name      deploy\nregistry  https://x");
    }

    #[test]
    fn card_unicode_and_ascii() {
        let uni = Style {
            color: false,
            unicode: true,
        }
        .card("whoami", &["user  aot".to_string()]);
        assert!(uni.starts_with('\u{256d}'), "{uni}");
        assert!(uni.contains("\u{2502} user  aot \u{2502}"), "{uni}");
        let ascii = Style::PLAIN.card("whoami", &["user  aot".to_string()]);
        assert!(ascii.starts_with('+'), "{ascii}");
        assert!(ascii.contains("| user  aot |"), "{ascii}");
        assert!(!ascii.contains('\u{2500}'), "{ascii}");
    }
}
