#![forbid(unsafe_code)]
//! `ForgeHub` server binary: load config, init logging, connect,
//! migrate, serve until SIGINT/SIGTERM.

use anyhow::Result;
use forge_server::config::{Config, Env};
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

fn init_tracing(env: Env) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("forge_server=info,tower_http=info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match env {
        Env::Prod => builder.json().init(),
        Env::Dev => builder.init(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "failed to install Ctrl+C handler");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to install SIGTERM handler");
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received; draining connections");
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    init_tracing(config.env);
    tracing::info!(
        "{} server starting: {}",
        forge_core::PRODUCT,
        config.summary()
    );
    if config.env == Env::Dev && config.secret_is_weak() {
        tracing::warn!(
            "FORGEHUB_SECRET_KEY is unset or shorter than 32 bytes; fine for local dev, never for production"
        );
    }
    let pool = forge_server::db::connect(&config.database_url).await?;
    forge_server::db::migrate(&pool).await?;
    tracing::info!("database ready and migrations applied");
    let state = AppState {
        login_throttle: Arc::new(forge_server::auth::LoginThrottle::new(
            std::time::Duration::from_millis(config.login_backoff_base_ms),
        )),
        config: Arc::new(config.clone()),
        repos: Arc::new(Repos::sqlite(pool.clone())),
        pool,
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!("listening on http://{}", config.bind);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("forge-server stopped");
    Ok(())
}
