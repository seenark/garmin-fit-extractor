pub mod activities;
pub mod auth;
pub mod extractions;
pub mod oauth;
pub mod runs;

pub fn router() -> axum::Router<crate::app::AppState> {
    axum::Router::new()
        .merge(auth::router())
        .merge(oauth::router())
        .merge(activities::router())
        .merge(extractions::router())
        .merge(runs::router())
        .merge(crate::admin::router())
}
