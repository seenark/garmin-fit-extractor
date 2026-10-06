use crate::{app::AppState, auth::AuthenticatedUser, error::ApiError};
use axum::{Router, routing::any};

/// Browser Runs clients use the versioned Runs API. FIT Coach routes remain independent.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/extractions", any(retired))
        .route("/api/v1/extractions/{id}", any(retired))
        .route("/api/v1/extractions/{id}/download", any(retired))
        .route("/api/v1", any(not_found))
        .route("/api/v1/{*path}", any(not_found))
}

async fn retired(_: AuthenticatedUser) -> Result<(), ApiError> {
    Err(ApiError::new(
        axum::http::StatusCode::GONE,
        "RUNS_ENDPOINT_RETIRED",
        "Browser Runs endpoints moved to /api/v2/runs. Original-source unavailable activities remain read-only.",
    ))
}

async fn not_found() -> ApiError {
    ApiError::api_route_not_found()
}
