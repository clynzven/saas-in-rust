use anyhow::Result;
use slotbook::config::{Config, LogFormat};
use slotbook::mail::Mailer;
use slotbook::{AppState, db, jobs};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    init_tracing(config.log_format);

    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    tracing::info!("database migrated");

    let mailer = match &config.smtp_url {
        Some(url) => Mailer::smtp(url, &config.mail_from)?,
        None => {
            tracing::warn!("SMTP_URL is not set: emails will be logged, not sent");
            Mailer::Log
        }
    };
    let state = AppState::new(pool, config.clone(), mailer);

    // The job worker runs alongside the web server, in the same process.
    let shutdown = CancellationToken::new();
    let worker = tokio::spawn(jobs::worker(state.clone(), shutdown.clone()));

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(
        listener,
        slotbook::app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    shutdown.cancel();
    worker.await?;
    Ok(())
}

fn init_tracing(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("slotbook=info,tower_http=info"));
    let builder = fmt().with_env_filter(filter);
    match format {
        LogFormat::Json => builder.json().init(),
        LogFormat::Pretty => builder.init(),
    }
}
