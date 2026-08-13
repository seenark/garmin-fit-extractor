use std::{error::Error, path::Path, sync::Arc};

use garmin_fit_extractor_api::{
    app::{AppState, router},
    auth::AuthState,
    config::Config,
    db,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    ensure_database_parent(&config.database_url)?;
    let db = db::connect(&config.database_url).await?;
    let listener = TcpListener::bind(config.bind).await?;
    let auth = Arc::new(AuthState::new(config.google, config.coach_oauth));

    tracing::info!(address = %config.bind, "listening for Garmin FIT extractor requests");
    axum::serve(listener, router(AppState { db, auth }, config.static_dir))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn ensure_database_parent(database_url: &str) -> Result<(), std::io::Error> {
    let Some(path) = database_url.strip_prefix("sqlite://").and_then(|value| {
        value
            .split_once('?')
            .map_or(Some(value), |(path, _)| Some(path))
    }) else {
        return Ok(());
    };
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install shutdown signal handler");
    }
}
