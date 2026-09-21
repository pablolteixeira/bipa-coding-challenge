//! Logging setup.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};

// sqlx logs harmless Postgres notices (e.g. "relation already exists") at info.
const DEFAULT_FILTER: &str = "info,sqlx=warn";

/// Installs the global `tracing` subscriber.
///
/// The filter comes from `RUST_LOG` (e.g. `RUST_LOG=bipa_nodes=debug`) and
/// defaults to `info` (sqlx at `warn`). Returns an error instead of panicking when a subscriber
/// is already installed.
pub fn init() -> Result<(), TryInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_init_returns_error_instead_of_panicking() {
        // Only this test installs a global subscriber, so the first call wins.
        let _ = init();
        assert!(init().is_err());
    }
}
