## Why

`GET /nodes` must be served from a database, never from the external API. We
need durable storage for the latest imported ranking, with a write
operation that is atomic (readers never see half an import) and a read
operation that returns the nodes in ranking order.

## What Changes

- Add a SQL migration creating the `nodes` table (`public_key` PK, `alias`,
  `capacity_sats BIGINT CHECK >= 0`, `first_seen TIMESTAMPTZ`, `rank INTEGER`,
  `imported_at TIMESTAMPTZ`).
- Add the `NodeRepository` async trait:
  - `replace_all(&self, nodes: &[Node]) -> Result<usize, StorageError>`
  - `list_nodes(&self) -> Result<Vec<Node>, StorageError>`
- Add `PgNodeRepository` (sqlx + `PgPool`): `replace_all` runs `DELETE` plus bulk
  `INSERT` in a single transaction, and `list_nodes` selects `ORDER BY rank`.
- Add `StorageError` (thiserror) wrapping `sqlx::Error` plus a conversion
  error for out-of-range values.
- Add `storage::connect(url)` (pool with acquire timeout) and
  `storage::migrate(pool)` using `sqlx::migrate!()` (embedded migrations).

## Capabilities

### New Capabilities
- `node-storage`: persisting and reading the imported node snapshot in PostgreSQL.

### Modified Capabilities
<!-- none -->

## Impact

- New `migrations/0001_create_nodes.sql`.
- New module `src/storage/` (`mod.rs` trait + error + connect/migrate, `postgres.rs`).
- New integration test file `tests/repository.rs` (needs Postgres from
  `docker compose up -d db`, with `DATABASE_URL` set).
