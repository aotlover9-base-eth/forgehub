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

/// The environment capabilities style resolution needs,
/// precomputed from the raw inputs (`NO_COLOR`, `TERM`, locale,
/// TTY) by [`Environment::detect`], so the resolver itself stays
/// a pure two-input decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Environment {
    /// Whether `auto` may colour: a terminal, no `NO_COLOR`,
    /// `TERM` not `dumb`.
    pub auto_color: bool,
    /// Whether box-drawing is safe: UTF-8 locale, `TERM` not
    /// `dumb`.
    pub unicode: bool,
}

impl Environment {
    /// Read the real environment (the only place this module
    /// touches it, besides the style store).
    #[must_use]
    pub fn detect() -> Environment {
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
        Environment {
            auto_color: tty && !no_color && !term_dumb,
            unicode: utf8 && !term_dumb,
        }
    }
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
    pub fn resolve(mode: ColorMode, env: Environment) -> Style {
        let color = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => env.auto_color,
        };
        Style {
            color,
            unicode: env.unicode,
        }
    }

    fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// The one accent: headings and selection only.
    #[must_use]
    pub fn accent(self, text: &str) -> String {
        self.paint("34", text)
    }

    /// Secondary text.
    // Staged adoption: dim/kv/card/warn are exercised by this
    // module's tests now and land at their call sites in B3
    // (run card), B6 (status/whoami) and B8 (cache warning).
    #[allow(dead_code)]
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }

    /// Success only.
    #[must_use]
    pub fn success(self, text: &str) -> String {
        self.paint("32", text)
    }

    /// Warnings only.
    #[allow(dead_code)]
    #[must_use]
    pub fn warn(self, text: &str) -> String {
        self.paint("33", text)
    }

    /// Errors only.
    #[must_use]
    pub fn error(self, text: &str) -> String {
        self.paint("31", text)
    }

    /// A section heading in the accent colour.
    #[must_use]
    pub fn heading(self, text: &str) -> String {
        self.accent(text)
    }

    /// An aligned key-value block: keys padded to the widest key,
    /// dimmed when colour is on.
    #[allow(dead_code)]
    #[must_use]
    pub fn kv(self, pairs: &[(&str, &str)]) -> String {
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
    #[allow(dead_code)]
    #[must_use]
    pub fn card(self, title: &str, lines: &[String]) -> String {
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
    let resolved = Style::resolve(mode, Environment::detect());
    *STYLE.get_or_init(|| resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A capable environment: auto may colour, Unicode is safe.
    const CAPABLE: Environment = Environment {
        auto_color: true,
        unicode: true,
    };

    #[test]
    fn auto_follows_the_environment_capability() {
        assert!(Style::resolve(ColorMode::Auto, CAPABLE).color);
        assert!(
            !Style::resolve(
                ColorMode::Auto,
                Environment {
                    auto_color: false,
                    ..CAPABLE
                }
            )
            .color
        );
    }

    #[test]
    fn always_and_never_override_the_environment() {
        let incapable = Environment {
            auto_color: false,
            unicode: false,
        };
        assert!(Style::resolve(ColorMode::Always, incapable).color);
        assert!(!Style::resolve(ColorMode::Never, CAPABLE).color);
    }

    #[test]
    fn unicode_passes_through_from_the_environment() {
        assert!(Style::resolve(ColorMode::Auto, CAPABLE).unicode);
        assert!(
            !Style::resolve(
                ColorMode::Auto,
                Environment {
                    unicode: false,
                    ..CAPABLE
                }
            )
            .unicode
        );
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
