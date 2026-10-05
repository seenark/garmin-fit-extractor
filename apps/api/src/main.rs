use std::{error::Error, sync::Arc};

use garmin_fit_extractor_api::{
    app::{AppState, router},
    auth::AuthState,
    config::Config,
    db,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn Error>> {
    if let Some(status) = garmin_fit_extractor_api::runs::process::child_mode() {
        std::process::exit(status);
    }
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    let db = db::connect(&config.database_url).await?;
    garmin_fit_extractor_api::runs::jobs::start_worker(db.clone());
    let listener = TcpListener::bind(config.bind).await?;
    let auth = Arc::new(
        AuthState::new(config.google, config.coach_oauth).with_admin_emails(config.admin_emails),
    );

    tracing::info!(address = %config.bind, "listening for Garmin FIT extractor requests");
    axum::serve(
        listener,
        router(
            AppState {
                db,
                auth,
                app_origin: config.app_origin,
            },
            config.static_dir,
        ),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install shutdown signal handler");
    }
}
