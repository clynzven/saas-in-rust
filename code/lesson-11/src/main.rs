use anyhow::Result;
use slotbook::config::{Config, LogFormat};
use slotbook::mail::Mailer;
use slotbook::{AppState, db, jobs};
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    init_tracing(config.log_format);

    let pool = db::connect(&config.database_url, config.database_max_connections).await?;
    db::migrate(&pool).await?;
    tracing::info!("database migrated");

    let mailer = match &config.smtp_url {
        Some(url) => Mailer::smtp(url, &config.mail_from)?,
        None => {
            tracing::warn!("SMTP_URL is not set: emails will be logged, not sent");
            Mailer::Log
        }
    };
    let state = AppState::new(pool.clone(), config.clone(), mailer);

    // One token tells everything to stop: the web server and the job worker.
    let shutdown = CancellationToken::new();
    let worker = tokio::spawn(jobs::worker(state.clone(), shutdown.clone()));

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, version = env!("CARGO_PKG_VERSION"), "listening");
    let stop = shutdown.clone();
    axum::serve(
        listener,
        slotbook::app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        wait_for_signal().await;
        tracing::info!("shutting down: finishing in-flight requests");
        stop.cancel();
    })
    .await?;

    // The server has drained. Let the worker finish its current job, then close the pool.
    shutdown.cancel();
    worker.await?;
    pool.close().await;
    tracing::info!("bye");
    Ok(())
}

/// Ctrl+C locally; SIGTERM from Docker, Kubernetes, Fly.io or systemd.
async fn wait_for_signal() {
    let ctrl_c = async { signal::ctrl_c().await.expect("Ctrl+C handler") };
    #[cfg(unix)]
    let term = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
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
