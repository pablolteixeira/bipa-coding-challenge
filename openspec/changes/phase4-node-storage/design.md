## Context

The source is a ranking (top 100 by connectivity) that changes over time.
The API should mirror the latest ranking. Volume is tiny (100 rows), but
correctness under concurrency matters: `GET /nodes` can run while an
import is writing.

## Goals / Non-Goals

**Goals:**
- Readers always see a complete snapshot (old or new, never mixed).
- Raw values stored losslessly, with formatting done at the API edge.
- Tests against a real Postgres, each isolated.

**Non-Goals:**
- Historical snapshots / time series.
- Pagination or filtering of `/nodes` (100 rows).

## Decisions

### Snapshot replace (DELETE + INSERT in one transaction) over UPSERT
*Alternative:* `INSERT ... ON CONFLICT DO UPDATE`. That leaves stale nodes that
dropped out of the ranking, so we'd need an extra "delete where not in" step,
and ordering gets harder. For 100 rows, delete-and-insert inside a transaction
is simple and correct. Postgres MVCC means concurrent readers see the
pre-commit snapshot until COMMIT.

### Store a `rank` column
It preserves the ranking order from the source. Without it `SELECT` order is
undefined.

### Bulk insert via `UNNEST`
One statement: `INSERT INTO nodes (...) SELECT * FROM UNNEST($1::text[], $2::text[], $3::bigint[], $4::timestamptz[], $5::int[])`.
*Alternative:* 100 single inserts in the transaction. Fine too, but it's 100
round trips. `QueryBuilder::push_values` is another option. UNNEST keeps the
SQL static.

### Raw values in the DB (`BIGINT` sats, `TIMESTAMPTZ`)
Lossless. The presentation format (`"360.10516297"`) can change without a
migration. `CHECK (capacity_sats >= 0)` enforces the domain invariant in the
DB too.

### `u64` to `i64` conversion is checked
Postgres has no unsigned type. `i64::try_from(capacity_sats)` returns
`StorageError::OutOfRange` before any SQL runs. Realistically unreachable (21M
BTC is about 2.1e15 sats), but it isn't allowed to panic.

### Pool settings
`PgPoolOptions::new().max_connections(5).acquire_timeout(5s)`. A slow or down DB
produces an error quickly instead of hanging requests.

### Tests with `#[sqlx::test]`
Each test gets a fresh database created from `DATABASE_URL`, with migrations
applied automatically. That gives parallel, isolated tests without manual cleanup.

## Risks / Trade-offs

- [Integration tests need Postgres running] → `docker compose up -d db` is
  documented in the README.
- [DELETE + INSERT churns rows every minute] → Negligible at 100 rows.
  Autovacuum handles it.
