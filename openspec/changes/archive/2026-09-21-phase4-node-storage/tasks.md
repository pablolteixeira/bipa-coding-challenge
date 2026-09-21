## 1. Schema

- [x] 1.1 Create `migrations/0001_create_nodes.sql` (table, PK, CHECK capacity_sats >= 0, index on rank)

## 2. Repository

- [x] 2.1 Create `src/storage/mod.rs` with the `NodeRepository` trait, `StorageError` (`Database(sqlx::Error)`, `OutOfRange { field, value }`), `connect(url) -> Result<PgPool>`, `migrate(&PgPool)`
- [x] 2.2 Create `src/storage/postgres.rs` with `PgNodeRepository { pool }`
- [x] 2.3 Implement `replace_all`: checked conversions first, then transaction (DELETE, UNNEST insert with rank = index), commit, return count
- [x] 2.4 Implement `list_nodes`: `SELECT ... ORDER BY rank`, map rows back to `Node` (checked `i64` to `u64`)

## 3. Unit tests

- [x] 3.1 Conversion helpers: `u64` to `i64` ok / out-of-range; `i64` to `u64` ok / negative

## 4. Integration tests (`tests/repository.rs`, `#[sqlx::test]`)

- [x] 4.1 Migrations apply on a fresh DB and running them twice is a no-op
- [x] 4.2 `list_nodes` on an empty table returns `[]`
- [x] 4.3 First `replace_all` stores nodes and `list_nodes` returns them in order
- [x] 4.4 Second `replace_all` fully replaces (stale nodes gone, new order kept)
- [x] 4.5 Duplicate public key in the batch returns an error and the previous snapshot is intact (atomicity)
- [x] 4.6 Round-trip of edge values: emoji alias, empty alias, capacity 0, large capacity, first_seen epoch 0
- [x] 4.7 `capacity_sats = u64::MAX` returns `OutOfRange` and nothing is written
- [x] 4.8 `replace_all(&[])` is valid at the repository level (clears the table). The importer guards against calling it with an empty list (phase 5)
- [x] 4.9 Closed pool returns `StorageError::Database` instead of panicking
