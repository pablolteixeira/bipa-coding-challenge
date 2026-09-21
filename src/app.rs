//! Composition root: builds the concrete components and runs them.

use std::future::Future;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::migrate::MigrateError;
use tokio_util::sync::CancellationToken;

use crate::storage::{self, StorageError};

const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Delay before retry number `attempt` (0-based): 1s, 2s, 4s, ... capped at 30s.
pub fn backoff_delay(attempt: u32) -> Duration {
    let secs = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    Duration::from_secs(secs).min(MAX_BACKOFF)
}

/// Runs `op` until it succeeds, retrying transient errors with exponential
/// backoff.
///
/// Returns `Ok(None)` if `token` is cancelled first, and the error right away
/// if it isn't transient: retrying a permanent problem would only hide it.
async fn retry<T, E, F, Fut>(
    what: &str,
    token: &CancellationToken,
    is_transient: impl Fn(&E) -> bool,
    mut op: F,
) -> Result<Option<T>, E>
where
    E: std::fmt::Display,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut attempt = 0;
    loop {
        // `biased`: shutdown always wins over a ready operation or timer.
        let result = tokio::select! {
            biased;
            () = token.cancelled() => return Ok(None),
            result = op() => result,
        };
        match result {
            Ok(value) => return Ok(Some(value)),
            Err(err) if is_transient(&err) => {
                let delay = backoff_delay(attempt);
                tracing::warn!(error = %err, attempt = attempt + 1, ?delay, "{what} failed, retrying");
                tokio::select! {
                    biased;
                    () = token.cancelled() => return Ok(None),
                    () = tokio::time::sleep(delay) => {}
                }
                attempt = attempt.saturating_add(1);
            }
            Err(err) => return Err(err),
        }
    }
}

/// Connects to the database, retrying while it is unreachable (e.g. the
/// container is still starting). Returns `Ok(None)` on shutdown.
pub async fn connect_with_retry(
    database_url: &str,
    token: &CancellationToken,
) -> Result<Option<PgPool>, StorageError> {
    retry(
        "database connection",
        token,
        is_transient_connect_error,
        || storage::connect(database_url),
    )
    .await
}

/// Applies migrations, retrying only if the connection dropped meanwhile.
/// Returns `Ok(None)` on shutdown.
pub async fn migrate_with_retry(
    pool: &PgPool,
    token: &CancellationToken,
) -> Result<Option<()>, StorageError> {
    retry(
        "database migration",
        token,
        is_transient_migrate_error,
        || storage::migrate(pool),
    )
    .await
}

/// While connecting, everything except a malformed URL/configuration may be
/// transient: Postgres even rejects logins while it is starting up.
fn is_transient_connect_error(err: &StorageError) -> bool {
    !matches!(err, StorageError::Database(sqlx::Error::Configuration(_)))
}

/// A migration is only retried when the failure is about connectivity; a
/// broken or inconsistent migration is permanent.
fn is_transient_migrate_error(err: &StorageError) -> bool {
    match err {
        StorageError::Database(err) | StorageError::Migration(MigrateError::Execute(err)) => {
            is_connection_error(err)
        }
        _ => false,
    }
}

fn is_connection_error(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn backoff_doubles_up_to_thirty_seconds() {
        let delays: Vec<u64> = (0..8).map(|n| backoff_delay(n).as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn backoff_does_not_overflow() {
        assert_eq!(backoff_delay(64), MAX_BACKOFF);
        assert_eq!(backoff_delay(u32::MAX), MAX_BACKOFF);
    }

    #[derive(Debug, PartialEq)]
    enum TestError {
        Transient,
        Permanent,
    }

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }

    fn transient(err: &TestError) -> bool {
        *err == TestError::Transient
    }

    #[tokio::test(start_paused = true)]
    async fn retry_succeeds_after_transient_failures_with_backoff() {
        let attempts = Cell::new(0);
        let started = tokio::time::Instant::now();

        let result = retry("op", &CancellationToken::new(), transient, || {
            attempts.set(attempts.get() + 1);
            let n = attempts.get();
            async move {
                if n < 4 {
                    Err(TestError::Transient)
                } else {
                    Ok(n)
                }
            }
        })
        .await;

        assert_eq!(result, Ok(Some(4)));
        // Waited 1s + 2s + 4s between the four attempts.
        assert_eq!(started.elapsed(), Duration::from_secs(7));
    }

    #[tokio::test(start_paused = true)]
    async fn retry_returns_permanent_error_immediately() {
        let attempts = Cell::new(0);

        let result: Result<Option<()>, _> =
            retry("op", &CancellationToken::new(), transient, || {
                attempts.set(attempts.get() + 1);
                async { Err(TestError::Permanent) }
            })
            .await;

        assert_eq!(result, Err(TestError::Permanent));
        assert_eq!(attempts.get(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_stops_when_cancelled_while_waiting() {
        let token = CancellationToken::new();
        let attempts = Cell::new(0);

        let result: Result<Option<()>, _> = retry("op", &token, transient, || {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 2 {
                token.cancel();
            }
            async { Err(TestError::Transient) }
        })
        .await;

        assert_eq!(result, Ok(None));
        assert_eq!(attempts.get(), 2);
    }

    #[tokio::test]
    async fn retry_does_not_run_when_already_cancelled() {
        let token = CancellationToken::new();
        token.cancel();

        let result: Result<Option<()>, TestError> =
            retry("op", &token, transient, || async { unreachable!() }).await;

        assert_eq!(result, Ok(None));
    }

    #[tokio::test(start_paused = true)]
    async fn connect_with_retry_to_unreachable_database_stops_on_shutdown() {
        // Port 1 is never listening, so every attempt fails with an I/O error.
        let token = CancellationToken::new();
        let canceller = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            canceller.cancel();
        });

        let result = connect_with_retry("postgres://bipa:bipa@127.0.0.1:1/bipa", &token).await;

        assert!(matches!(result, Ok(None)), "{result:?}");
    }

    #[tokio::test]
    async fn connect_with_retry_fails_fast_on_invalid_url() {
        let result = connect_with_retry("not-a-url", &CancellationToken::new()).await;

        assert!(
            matches!(
                result,
                Err(StorageError::Database(sqlx::Error::Configuration(_)))
            ),
            "{result:?}"
        );
    }

    fn io_error() -> sqlx::Error {
        sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused))
    }

    #[test]
    fn connect_errors_are_transient_except_configuration() {
        assert!(is_transient_connect_error(&StorageError::Database(
            io_error()
        )));
        assert!(is_transient_connect_error(&StorageError::Database(
            sqlx::Error::PoolTimedOut
        )));
        assert!(!is_transient_connect_error(&StorageError::Database(
            sqlx::Error::Configuration("bad url".into())
        )));
    }

    #[test]
    fn migrate_errors_are_transient_only_for_connectivity() {
        assert!(is_transient_migrate_error(&StorageError::Migration(
            MigrateError::Execute(io_error())
        )));
        assert!(is_transient_migrate_error(&StorageError::Migration(
            MigrateError::Execute(sqlx::Error::PoolTimedOut)
        )));
        assert!(is_transient_migrate_error(&StorageError::Database(
            sqlx::Error::PoolClosed
        )));
        assert!(!is_transient_migrate_error(&StorageError::Migration(
            MigrateError::VersionMissing(1)
        )));
        assert!(!is_transient_migrate_error(&StorageError::Migration(
            MigrateError::Execute(sqlx::Error::Protocol("syntax error".into()))
        )));
    }
}
