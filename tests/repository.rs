//! Integration tests for `PgNodeRepository` against a real PostgreSQL.
//!
//! `#[sqlx::test]` creates a fresh database per test (from `DATABASE_URL`) and
//! applies the migrations, so tests are isolated and can run in parallel.
//! Requires `docker compose up -d db`.

// Test helpers are plain functions, so clippy's test exemption doesn't cover them.
#![allow(clippy::unwrap_used)]

use bipa_nodes::domain::{Node, timestamp_from_unix};
use bipa_nodes::storage::{self, NodeRepository, PgNodeRepository, StorageError};
use sqlx::PgPool;

fn node(public_key: &str, alias: &str, capacity_sats: u64, first_seen: i64) -> Node {
    Node {
        public_key: public_key.to_owned(),
        alias: alias.to_owned(),
        capacity_sats,
        first_seen: timestamp_from_unix(first_seen).unwrap(),
    }
}

fn keys(nodes: &[Node]) -> Vec<&str> {
    nodes.iter().map(|n| n.public_key.as_str()).collect()
}

#[sqlx::test]
async fn migrations_create_nodes_table_and_are_idempotent(pool: PgPool) {
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
         WHERE table_name = 'nodes' ORDER BY ordinal_position",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        [
            "public_key",
            "alias",
            "capacity_sats",
            "first_seen",
            "rank",
            "imported_at"
        ]
    );

    // Already applied by #[sqlx::test]; running again must be a no-op.
    storage::migrate(&pool).await.unwrap();
}

#[sqlx::test]
async fn list_is_empty_before_any_import(pool: PgPool) {
    let repo = PgNodeRepository::new(pool);
    assert!(repo.list_nodes().await.unwrap().is_empty());
}

#[sqlx::test]
async fn first_import_stores_nodes_in_order(pool: PgPool) {
    let repo = PgNodeRepository::new(pool);
    let nodes = vec![
        node("c", "third-by-key", 3, 30),
        node("a", "first-by-key", 1, 10),
        node("b", "second-by-key", 2, 20),
    ];

    assert_eq!(repo.replace_all(&nodes).await.unwrap(), 3);
    assert_eq!(repo.list_nodes().await.unwrap(), nodes);
}

#[sqlx::test]
async fn second_import_replaces_the_snapshot(pool: PgPool) {
    let repo = PgNodeRepository::new(pool);
    repo.replace_all(&[
        node("a", "A", 1, 0),
        node("b", "B", 2, 0),
        node("c", "C", 3, 0),
    ])
    .await
    .unwrap();

    let updated = vec![node("c", "C renamed", 30, 0), node("d", "D", 4, 0)];
    repo.replace_all(&updated).await.unwrap();

    let stored = repo.list_nodes().await.unwrap();
    assert_eq!(keys(&stored), ["c", "d"]);
    assert_eq!(stored, updated);
}

#[sqlx::test]
async fn failed_import_keeps_previous_snapshot(pool: PgPool) {
    let repo = PgNodeRepository::new(pool);
    let original = vec![node("a", "A", 1, 0), node("b", "B", 2, 0)];
    repo.replace_all(&original).await.unwrap();

    // Duplicate primary key makes the INSERT fail after the DELETE ran.
    let err = repo
        .replace_all(&[node("x", "X", 1, 0), node("x", "X again", 2, 0)])
        .await
        .unwrap_err();
    assert!(matches!(err, StorageError::Database(_)), "{err:?}");

    assert_eq!(repo.list_nodes().await.unwrap(), original);
}

#[sqlx::test]
async fn edge_values_round_trip_exactly(pool: PgPool) {
    let repo = PgNodeRepository::new(pool);
    let nodes = vec![
        node(
            "emoji",
            "DiamondHands💎🙌 \"quoted\" | torq.co",
            1,
            1_617_669_406,
        ),
        node("empty-alias", "", 0, 0),
        node("large", "large", i64::MAX as u64, 1_522_941_222),
        node("pre-epoch", "pre-epoch", 42, -1),
    ];

    repo.replace_all(&nodes).await.unwrap();

    assert_eq!(repo.list_nodes().await.unwrap(), nodes);
}

#[sqlx::test]
async fn out_of_range_capacity_is_rejected_before_writing(pool: PgPool) {
    let repo = PgNodeRepository::new(pool);
    let original = vec![node("a", "A", 1, 0)];
    repo.replace_all(&original).await.unwrap();

    let err = repo
        .replace_all(&[node("b", "B", u64::MAX, 0)])
        .await
        .unwrap_err();
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

    assert_eq!(repo.list_nodes().await.unwrap(), original);
}

#[sqlx::test]
async fn empty_replace_clears_the_table(pool: PgPool) {
    // The repository allows it; the importer is responsible for never calling
    // it with an empty ranking.
    let repo = PgNodeRepository::new(pool);
    repo.replace_all(&[node("a", "A", 1, 0)]).await.unwrap();

    assert_eq!(repo.replace_all(&[]).await.unwrap(), 0);
    assert!(repo.list_nodes().await.unwrap().is_empty());
}

#[sqlx::test]
async fn closed_pool_returns_error_instead_of_panicking(pool: PgPool) {
    let repo = PgNodeRepository::new(pool.clone());
    pool.close().await;

    let err = repo.list_nodes().await.unwrap_err();
    assert!(
        matches!(err, StorageError::Database(sqlx::Error::PoolClosed)),
        "{err:?}"
    );
    assert!(repo.replace_all(&[node("a", "A", 1, 0)]).await.is_err());
}
