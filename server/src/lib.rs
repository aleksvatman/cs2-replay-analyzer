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

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

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

/// Asks a running server whether it is alive: `GET /health` over plain TCP, no extra tools.
/// Used by `server healthcheck` (the Docker health check; the image has no curl).
/// An unspecified bind address (`0.0.0.0`) is reached through loopback.
pub fn healthcheck(bind_addr: SocketAddr) -> Result<(), String> {
    let ip = if bind_addr.ip().is_unspecified() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        bind_addr.ip()
    };
    let addr = SocketAddr::new(ip, bind_addr.port());
    let timeout = Duration::from_secs(3);
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("cannot connect to {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("cannot send the request: {e}"))?;
    let mut answer = String::new();
    // the status line is all that is needed
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("no answer: {e}"))?;
    answer.push_str(&String::from_utf8_lossy(&buf[..n]));
    match answer.split_whitespace().nth(1) {
        Some("200") => Ok(()),
        _ => Err(format!(
            "unexpected answer: {:?}",
            answer.lines().next().unwrap_or("")
        )),
    }
}
