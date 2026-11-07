//! Configuration comes from environment variables, so the same binary runs
//! on a laptop, in CI and in production.

use std::net::SocketAddr;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    /// Address to listen on, e.g. 0.0.0.0:8080 in a container.
    pub bind_addr: SocketAddr,
    /// postgres://user:password@host/database
    pub database_url: String,
    /// Public URL of the site, used to build links in emails.
    pub base_url: String,
    /// `json` in production (for log collectors), `pretty` locally.
    pub log_format: LogFormat,
    /// Header a trusted proxy sets to the visitor's IP (e.g. Fly-Client-IP); unset without a proxy.
    pub trusted_ip_header: Option<String>,
    /// smtps://user:password@host. Unset in development: emails are logged instead.
    pub smtp_url: Option<String>,
    pub mail_from: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_addr = env_or("BIND_ADDR", "127.0.0.1:3000")
            .parse()
            .context("BIND_ADDR must look like 127.0.0.1:3000")?;
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
        let base_url = env_or("BASE_URL", "http://localhost:3000")
            .trim_end_matches('/')
            .to_string();
        let log_format = match env_or("LOG_FORMAT", "pretty").as_str() {
            "json" => LogFormat::Json,
            "pretty" => LogFormat::Pretty,
            other => anyhow::bail!("LOG_FORMAT must be json or pretty, not {other:?}"),
        };
        let smtp_url = std::env::var("SMTP_URL").ok().filter(|s| !s.is_empty());
        check_mail(&base_url, smtp_url.as_deref())?;
        let mail_from = env_or("MAIL_FROM", "Slotbook <bookings@localhost>");
        let trusted_ip_header = std::env::var("TRUSTED_IP_HEADER")
            .ok()
            .filter(|s| !s.is_empty());
        Ok(Self {
            bind_addr,
            database_url,
            base_url,
            log_format,
            trusted_ip_header,
            smtp_url,
            mail_from,
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Without SMTP the app only logs emails, cancel links included: fine on a
/// laptop, never in production (an https:// site).
fn check_mail(base_url: &str, smtp_url: Option<&str>) -> Result<()> {
    if smtp_url.is_none() && base_url.starts_with("https://") {
        anyhow::bail!("SMTP_URL must be set when BASE_URL is https:// (production)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn production_needs_smtp() {
        assert!(super::check_mail("https://slotbook.example", None).is_err());
        assert!(
            super::check_mail("https://slotbook.example", Some("smtps://u:p@mail.example")).is_ok()
        );
        assert!(super::check_mail("http://localhost:3000", None).is_ok());
    }
}
