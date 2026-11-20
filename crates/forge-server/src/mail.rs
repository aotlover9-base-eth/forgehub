//! Outgoing mail: the password-reset delivery path.
//!
//! Three modes, chosen by [`crate::config::Config::mail_mode`]:
//!
//! - **SMTP** — real delivery through `FORGEHUB_SMTP_URL`
//!   (lettre). The URL usually carries a password; it is never
//!   logged or printed.
//! - **Console** — dev only: the reset link is written to the
//!   server log so local testing works with zero setup. This
//!   is the standard console-mail backend pattern; it exists
//!   only when `FORGEHUB_ENV=dev` and no SMTP URL is set.
//! - **Disabled** — prod without SMTP: requests are accepted
//!   (the endpoint must not reveal whether mail exists) and a
//!   warning is logged; nothing is delivered.
//!
//! The trait is synchronous because lettre's transport is;
//! handlers call it through `spawn_blocking`.

use crate::config::{Config, MailMode};
use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::{Message, SmtpTransport, Transport};
use std::sync::Arc;

/// Sends password-reset mail.
pub trait Mailer: Send + Sync {
    /// Deliver a reset link to `to`. Errors are the caller's to
    /// log; the HTTP response never depends on them (the reset
    /// endpoint answers identically either way, by design).
    ///
    /// # Errors
    /// On a delivery failure.
    fn send_password_reset(&self, to: &str, reset_url: &str) -> Result<()>;

    /// Short mode name for logs ("smtp", "console", "disabled").
    fn describe(&self) -> &'static str;
}

/// Build the mailer a configuration calls for. An SMTP URL
/// that lettre cannot parse degrades to the disabled mailer
/// with a loud error log — mail must never stop the server
/// from booting, but the operator must be told.
#[must_use]
pub fn mailer_from_config(config: &Config) -> Arc<dyn Mailer> {
    match config.mail_mode() {
        MailMode::Smtp => {
            let url = config.smtp_url.as_deref().unwrap_or_default();
            match SmtpTransport::from_url(url) {
                Ok(builder) => Arc::new(SmtpMailer {
                    transport: builder.build(),
                    from: config.mail_from.clone(),
                }),
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "FORGEHUB_SMTP_URL could not be parsed; mail is DISABLED until it is fixed"
                    );
                    Arc::new(DisabledMailer)
                }
            }
        }
        MailMode::Console => Arc::new(ConsoleMailer),
        MailMode::Disabled => Arc::new(DisabledMailer),
    }
}

struct SmtpMailer {
    transport: SmtpTransport,
    from: String,
}

impl Mailer for SmtpMailer {
    fn send_password_reset(&self, to: &str, reset_url: &str) -> Result<()> {
        let message = Message::builder()
            .from(
                self.from
                    .parse()
                    .with_context(|| format!("parsing FORGEHUB_MAIL_FROM `{}`", self.from))?,
            )
            .to(to
                .parse()
                .with_context(|| "parsing the recipient".to_string())?)
            .subject("Reset your ForgeHub password")
            .header(ContentType::TEXT_PLAIN)
            .body(reset_email_body(reset_url))
            .context("building the reset email")?;
        self.transport.send(&message).context("SMTP delivery")?;
        Ok(())
    }

    fn describe(&self) -> &'static str {
        "smtp"
    }
}

/// Dev console mailer: the link goes to the server log.
struct ConsoleMailer;

impl Mailer for ConsoleMailer {
    fn send_password_reset(&self, to: &str, reset_url: &str) -> Result<()> {
        tracing::warn!(
            recipient = %to,
            reset_url = %reset_url,
            "DEV console mailer: password reset link (set FORGEHUB_SMTP_URL for real delivery)"
        );
        Ok(())
    }

    fn describe(&self) -> &'static str {
        "console"
    }
}

/// No delivery: prod without SMTP, or an unparseable SMTP URL.
struct DisabledMailer;

impl Mailer for DisabledMailer {
    fn send_password_reset(&self, to: &str, _reset_url: &str) -> Result<()> {
        tracing::warn!(
            recipient = %to,
            "password reset requested but mail is not configured (set FORGEHUB_SMTP_URL); nothing was sent"
        );
        Ok(())
    }

    fn describe(&self) -> &'static str {
        "disabled"
    }
}

/// The plain-text body of the reset email. The link is the
/// whole point; everything else is one short paragraph so a
/// user who did not request it knows to ignore it.
fn reset_email_body(reset_url: &str) -> String {
    format!(
        "Someone asked to reset the password for this ForgeHub account.\n\
         \n\
         Reset it within the next hour:\n\
         {reset_url}\n\
         \n\
         If that was not you, ignore this email — your password has not changed.\n"
    )
}

/// Guard rail for the mode selection: console (log-the-link)
/// must be unreachable in production configuration.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Env};
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Config {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_vars(&move |k: &str| map.get(k).cloned()).unwrap()
    }

    #[test]
    fn dev_without_smtp_uses_the_console() {
        assert_eq!(mailer_from_config(&cfg(&[])).describe(), "console");
    }

    #[test]
    fn prod_without_smtp_sends_nothing() {
        let secret = "x".repeat(32);
        let config = cfg(&[
            ("FORGEHUB_ENV", "prod"),
            ("FORGEHUB_SECRET_KEY", secret.as_str()),
        ]);
        assert_eq!(config.env, Env::Prod);
        assert_eq!(mailer_from_config(&config).describe(), "disabled");
    }

    #[test]
    fn smtp_url_selects_smtp() {
        let config = cfg(&[("FORGEHUB_SMTP_URL", "smtp://127.0.0.1:2525")]);
        assert_eq!(mailer_from_config(&config).describe(), "smtp");
    }

    #[test]
    fn the_body_carries_the_link_and_the_warning() {
        let body = reset_email_body("https://example.com/reset-password?token=abc");
        assert!(body.contains("https://example.com/reset-password?token=abc"));
        assert!(body.contains("If that was not you"));
    }
}
