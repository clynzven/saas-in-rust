//! Sending email. Production uses SMTP; development logs messages; tests keep
//! them in memory so they can be inspected.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use lettre::message::Mailbox;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Clone)]
pub enum Mailer {
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: Mailbox,
    },
    Log,
    Memory(Arc<Mutex<Vec<Email>>>),
}

impl Mailer {
    /// `smtp_url` looks like smtps://user:password@smtp.example.com
    /// (see lettre's `from_url` for the details).
    pub fn smtp(smtp_url: &str, from: &str) -> Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(smtp_url)
            .context("SMTP_URL is not a valid smtp:// or smtps:// URL")?
            .build();
        let from = from
            .parse()
            .context("MAIL_FROM must look like Slotbook <bookings@example.com>")?;
        Ok(Mailer::Smtp { transport, from })
    }

    pub fn memory() -> Self {
        Mailer::Memory(Arc::default())
    }

    pub async fn send(&self, email: &Email) -> Result<()> {
        match self {
            Mailer::Smtp { transport, from } => {
                let msg = Message::builder()
                    .from(from.clone())
                    .to(email.to.parse().context("bad recipient address")?)
                    .subject(&email.subject)
                    .body(email.text.clone())?;
                transport.send(msg).await?;
            }
            Mailer::Log => {
                tracing::info!(to = %email.to, subject = %email.subject, "email (not sent: no SMTP_URL)\n{}", email.text)
            }
            Mailer::Memory(sent) => sent.lock().expect("mailbox lock").push(email.clone()),
        }
        Ok(())
    }

    /// Everything sent so far (memory mailer only).
    pub fn sent(&self) -> Vec<Email> {
        match self {
            Mailer::Memory(sent) => sent.lock().expect("mailbox lock").clone(),
            _ => vec![],
        }
    }
}
