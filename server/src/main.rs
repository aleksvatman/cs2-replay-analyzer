use std::process::ExitCode;

use server::config::Config;
use server::state::AppState;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

/// `server healthcheck`: exit code 0 if the server on `BIND_ADDR` answers `/health`.
fn healthcheck_command() -> ExitCode {
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let result = addr
        .parse()
        .map_err(|_| format!("BIND_ADDR={addr:?} is not a host:port address"))
        .and_then(server::healthcheck);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("unhealthy: {message}");
            ExitCode::FAILURE
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck_command();
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("invalid configuration: {err}");
            return ExitCode::from(2);
        }
    };
    tracing::info!(?config, "starting");

    let state = match AppState::new(config.clone()) {
        Ok(state) => state,
        Err(err) => {
            tracing::error!("cannot open DATA_DIR {}: {err}", config.data_dir.display());
            return ExitCode::FAILURE;
        }
    };
    match state.recover_and_resume() {
        Ok(recovery) => tracing::info!(
            requeue = recovery.requeue.len(),
            failed = recovery.failed.len(),
            incomplete = recovery.incomplete,
            "storage scanned"
        ),
        Err(err) => {
            tracing::error!("cannot scan DATA_DIR: {err}");
            return ExitCode::FAILURE;
        }
    }

    server::retention::spawn(state.clone());

    let listener = match TcpListener::bind(config.bind_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("cannot bind {}: {err}", config.bind_addr);
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("listening on {}", config.bind_addr);

    let app = server::router(state);
    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("server error: {err}");
        return ExitCode::FAILURE;
    }
    tracing::info!("stopped");
    ExitCode::SUCCESS
}

/// Resolves on Ctrl+C or SIGTERM (what Docker sends).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
