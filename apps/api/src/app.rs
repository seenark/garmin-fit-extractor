use crate::{auth::AuthState, error::ApiError, routes};
use axum::{Router, extract::State, response::Json, routing::get};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub auth: Arc<AuthState>,
}

pub fn router(state: AppState, static_dir: PathBuf) -> Router {
    let index = static_dir.join("index.html");
    Router::new()
        .merge(routes::router())
        .route("/healthz", get(health))
        .fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(index)))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health(State(state): State<AppState>) -> Result<Json<Health>, ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "health database check failed");
            ApiError::service_unavailable()
        })?;
    Ok(Json(Health { status: "ok" }))
}
