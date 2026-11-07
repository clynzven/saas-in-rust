//! Configuration comes from environment variables, so the same binary runs
//! on a laptop, in CI and in production.

use std::net::SocketAddr;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    /// Address to listen on, e.g. 0.0.0.0:8080 in a container.
    pub bind_addr: SocketAddr,
    /// Public URL of the site, used to build links in emails.
    pub base_url: String,
    /// `json` in production (for log collectors), `pretty` locally.
    pub log_format: LogFormat,
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
        let base_url = env_or("BASE_URL", "http://localhost:3000")
            .trim_end_matches('/')
            .to_string();
        let log_format = match env_or("LOG_FORMAT", "pretty").as_str() {
            "json" => LogFormat::Json,
            "pretty" => LogFormat::Pretty,
            other => anyhow::bail!("LOG_FORMAT must be json or pretty, not {other:?}"),
        };
        Ok(Self { bind_addr, base_url, log_format })
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
