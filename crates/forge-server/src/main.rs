#![forbid(unsafe_code)]
//! `ForgeHub` server binary: load config, init logging, connect,
//! migrate, serve until SIGINT/SIGTERM.

use anyhow::Result;
use forge_server::config::{Config, Env};
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::net::SocketAddr;
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

/// One retention purge pass (Step 9 policy, Step 19 schedule).
async fn purge_retention(state: &AppState) -> Result<()> {
    let now = chrono::Utc::now();
    let before = forge_server::auth::fmt_ts(now - chrono::Duration::days(30));
    let purged = state
        .repos
        .snippets
        .purge_deleted(&before, &forge_server::db::now_iso())
        .await?;
    if purged > 0 {
        tracing::info!(purged, "purged expired snippets and rename redirects");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    // `forge-server admin <command>` runs one moderation command
    // against the database and exits (Step 10; no admin web UI).
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("admin") {
        let pool = forge_server::db::connect(&config.database_url).await?;
        let repos = Repos::sqlite(pool);
        return forge_server::admin::run(&repos, &args[1..]).await;
    }
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
    let state = AppState::new(config.clone(), Repos::sqlite(pool.clone()), pool);
    // Retention: hard-delete snippets soft-deleted over 30 days
    // ago and drop expired rename redirects (Step 9 policy).
    // Runs once at startup and then every 6 hours (Step 19: a
    // long-lived server must not wait for a restart to purge).
    purge_retention(&state).await?;
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_hours(6));
            tick.tick().await; // consume the immediate first tick
            loop {
                tick.tick().await;
                if let Err(e) = purge_retention(&state).await {
                    tracing::warn!(error = %e, "periodic retention purge failed");
                }
            }
        });
    }
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
