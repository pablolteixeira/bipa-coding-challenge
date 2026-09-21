//! Persistence of the imported node snapshot.
//!
//! The importer and the HTTP layer depend on the [`NodeRepository`] trait
//! only, so they can be unit-tested with an in-memory fake.

mod postgres;

use std::future::Future;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::domain::Node;

pub use postgres::PgNodeRepository;

const MAX_CONNECTIONS: u32 = 5;
/// Fail fast when the database is unreachable instead of hanging requests.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// Storage for the latest node ranking.
pub trait NodeRepository {
    /// Atomically replaces every stored node with `nodes`, keeping their order.
    /// On error the previous snapshot is left untouched.
    fn replace_all(
        &self,
        nodes: &[Node],
    ) -> impl Future<Output = Result<usize, StorageError>> + Send;

    /// Returns all stored nodes in ranking order.
    fn list_nodes(&self) -> impl Future<Output = Result<Vec<Node>, StorageError>> + Send;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    /// A value doesn't fit the column type (e.g. u64 capacity above i64::MAX).
    #[error("{field} value {value} is out of range")]
    OutOfRange { field: &'static str, value: String },
}

/// Opens a connection pool and checks that the database is reachable.
pub async fn connect(database_url: &str) -> Result<PgPool, StorageError> {
    let pool = PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Applies the SQL migrations embedded from `migrations/`. Already applied
/// migrations are skipped, so this is safe to run on every startup.
pub async fn migrate(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::migrate!().run(pool).await?;
    Ok(())
}
