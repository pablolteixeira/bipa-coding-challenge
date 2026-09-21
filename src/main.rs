use std::process::ExitCode;

use bipa_nodes::config::Config;
use bipa_nodes::telemetry;

fn main() -> ExitCode {
    if let Err(err) = telemetry::init() {
        eprintln!("failed to initialise logging: {err}");
        return ExitCode::FAILURE;
    }

    // A bad configuration is a permanent error: refuse to start with a clear
    // message instead of running in an undefined state.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(error = %err, "invalid configuration");
            return ExitCode::FAILURE;
        }
    };

    // `Config`'s Debug impl redacts the database password.
    tracing::info!(?config, "configuration loaded");

    ExitCode::SUCCESS
}
