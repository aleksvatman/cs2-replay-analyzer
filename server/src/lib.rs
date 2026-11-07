//! HTTP server around `parser-core`. See `server-doc.md` for the full specification.

pub mod api;
pub mod config;
pub mod error;
pub mod jobs;
pub mod state;
pub mod storage;

use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::error::ApiError;
use crate::state::AppState;

/// Builds the router with all routes. Used by `main` and by the tests.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(api::health::health))
        .route("/matches", post(api::upload::upload))
        .fallback(fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn fallback() -> ApiError {
    ApiError::NotFound
}
