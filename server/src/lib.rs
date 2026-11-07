//! HTTP server around `parser-core`. See `server-doc.md` for the full specification.

pub mod api;
pub mod auth;
pub mod config;
pub mod error;
pub mod jobs;
pub mod limits;
pub mod retention;
pub mod state;
pub mod storage;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::error::ApiError;
use crate::state::AppState;

/// Builds the router with all routes. Used by `main` and by the tests.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(api::health::health))
        .route(
            "/matches",
            post(api::upload::upload).get(api::matches::list),
        )
        .route(
            "/matches/{id}",
            get(api::matches::status).delete(api::matches::delete),
        )
        .route("/matches/{id}/data", get(api::matches::data))
        .route("/matches/{id}/reparse", post(api::matches::reparse))
        .fallback(fallback)
        // layers run from the last one added to the first: log, check the token, then limit
        .layer(from_fn_with_state(state.clone(), limits::rate_limit))
        .layer(from_fn_with_state(state.clone(), auth::require_token))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn fallback() -> ApiError {
    ApiError::NotFound
}
