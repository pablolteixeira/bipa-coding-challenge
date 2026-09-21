use std::process::ExitCode;

use bipa_nodes::app;
use bipa_nodes::config::Config;
use bipa_nodes::telemetry;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = telemetry::init() {
        eprintln!("failed to initialise logging: {err}");
        return ExitCode::FAILURE;
    }

    // Configuration and bind errors are permanent: refuse to start with a
    // clear message instead of running in an undefined state.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(error = %err, "invalid configuration");
            return ExitCode::FAILURE;
        }
    };
    // `Config`'s Debug impl redacts the database password.
    tracing::info!(?config, "configuration loaded");

    let listener = match TcpListener::bind(config.bind_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(error = %err, addr = %config.bind_addr, "failed to bind HTTP listener");
            return ExitCode::FAILURE;
        }
    };

    let token = CancellationToken::new();
    tokio::spawn(cancel_on_shutdown_signal(token.clone()));

    match app::run(config, listener, token).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "service stopped with an error");
            ExitCode::FAILURE
        }
    }
}

/// Cancels `token` on Ctrl+C (SIGINT) or SIGTERM (sent by `docker stop`).
async fn cancel_on_shutdown_signal(token: CancellationToken) {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %err, "cannot listen for Ctrl+C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(err) => {
                tracing::warn!(error = %err, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
    token.cancel();
}
