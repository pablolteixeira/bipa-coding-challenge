//! PostgreSQL implementation of [`NodeRepository`].

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::{NodeRepository, StorageError};
use crate::domain::Node;

#[derive(Debug, Clone)]
pub struct PgNodeRepository {
    pool: PgPool,
}

impl PgNodeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct NodeRow {
    public_key: String,
    alias: String,
    capacity_sats: i64,
    first_seen: DateTime<Utc>,
}

impl TryFrom<NodeRow> for Node {
    type Error = StorageError;

    fn try_from(row: NodeRow) -> Result<Self, Self::Error> {
        Ok(Node {
            capacity_sats: capacity_from_db(row.capacity_sats)?,
            public_key: row.public_key,
            alias: row.alias,
            first_seen: row.first_seen,
        })
    }
}

impl NodeRepository for PgNodeRepository {
    async fn replace_all(&self, nodes: &[Node]) -> Result<usize, StorageError> {
        // Convert everything up front so an out-of-range value fails before
        // any SQL runs.
        let mut public_keys = Vec::with_capacity(nodes.len());
        let mut aliases = Vec::with_capacity(nodes.len());
        let mut capacities = Vec::with_capacity(nodes.len());
        let mut first_seens = Vec::with_capacity(nodes.len());
        let mut ranks = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            public_keys.push(node.public_key.clone());
            aliases.push(node.alias.clone());
            capacities.push(capacity_to_db(node.capacity_sats)?);
            first_seens.push(node.first_seen);
            ranks.push(rank_to_db(index)?);
        }

        // DELETE + INSERT in one transaction: concurrent readers keep seeing
        // the previous snapshot until COMMIT, and nodes that dropped out of the
        // ranking disappear. If anything fails, the transaction rolls back on
        // drop and the old snapshot stays.
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM nodes").execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO nodes (public_key, alias, capacity_sats, first_seen, rank)
             SELECT * FROM UNNEST($1::text[], $2::text[], $3::bigint[], $4::timestamptz[], $5::int[])",
        )
        .bind(&public_keys)
        .bind(&aliases)
        .bind(&capacities)
        .bind(&first_seens)
        .bind(&ranks)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(nodes.len())
    }

    async fn list_nodes(&self) -> Result<Vec<Node>, StorageError> {
        let rows: Vec<NodeRow> = sqlx::query_as(
            "SELECT public_key, alias, capacity_sats, first_seen FROM nodes ORDER BY rank",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Node::try_from).collect()
    }
}

// Postgres has no unsigned integers, so the u64 <-> i64 conversions are
// checked instead of cast.

fn capacity_to_db(sats: u64) -> Result<i64, StorageError> {
    i64::try_from(sats).map_err(|_| StorageError::OutOfRange {
        field: "capacity_sats",
        value: sats.to_string(),
    })
}

fn capacity_from_db(sats: i64) -> Result<u64, StorageError> {
    u64::try_from(sats).map_err(|_| StorageError::OutOfRange {
        field: "capacity_sats",
        value: sats.to_string(),
    })
}

fn rank_to_db(index: usize) -> Result<i32, StorageError> {
    i32::try_from(index).map_err(|_| StorageError::OutOfRange {
        field: "rank",
        value: index.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_to_db_accepts_values_up_to_i64_max() {
        assert_eq!(capacity_to_db(0).unwrap(), 0);
        assert_eq!(capacity_to_db(36_010_516_297).unwrap(), 36_010_516_297);
        assert_eq!(capacity_to_db(i64::MAX as u64).unwrap(), i64::MAX);
    }

    #[test]
    fn capacity_to_db_rejects_values_above_i64_max() {
        let err = capacity_to_db(u64::MAX).unwrap_err();
        assert!(
            matches!(
                err,
                StorageError::OutOfRange {
                    field: "capacity_sats",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn capacity_from_db_accepts_non_negative_values() {
        assert_eq!(capacity_from_db(0).unwrap(), 0);
        assert_eq!(capacity_from_db(i64::MAX).unwrap(), i64::MAX as u64);
    }

    #[test]
    fn capacity_from_db_rejects_negative_values() {
        assert!(matches!(
            capacity_from_db(-1),
            Err(StorageError::OutOfRange {
                field: "capacity_sats",
                ..
            })
        ));
    }

    #[test]
    fn rank_to_db_converts_and_rejects_overflow() {
        assert_eq!(rank_to_db(0).unwrap(), 0);
        assert_eq!(rank_to_db(99).unwrap(), 99);
        assert!(matches!(
            rank_to_db(usize::MAX),
            Err(StorageError::OutOfRange { field: "rank", .. })
        ));
    }
}
