## Context

This is the only writer to the DB. It has to run for the lifetime of the
process, tolerate every upstream and DB failure, and shut down cleanly.

## Goals / Non-Goals

**Goals:**
- Deterministic tests of timing behaviour (no real sleeps).
- No failure (error or panic) can stop the loop.
- Clean cancellation.

**Non-Goals:**
- Exponential backoff between failed imports. The fixed interval (60s) is
  already gentle on the upstream.
- Distributed locking across multiple replicas (single instance assumed;
  see Risks).

## Decisions

### `run_once` separated from `run`
All business logic (including the empty-list guard) lives in `run_once` and
is tested directly. `run` only handles timing and cancellation.

### Empty list treated as an error
The ranking endpoint always returns ~100 nodes. An empty array almost
certainly means an upstream problem. Wiping the table would make `/nodes`
return `[]` until the next good import. Keeping stale-but-valid data is the
better failure mode.

### `tokio::select!` over `interval.tick()` and `token.cancelled()`
Cancellation interrupts the wait immediately. An in-flight import is allowed
to finish (the transaction is short), because aborting mid-transaction has no
benefit.

### Panic isolation via `tokio::spawn` per iteration
Clippy denies `panic!`/`unwrap` in our own code, but dependencies could
still panic. `tokio::spawn(fut).await` turns a panic into `Err(JoinError)`
that we log. This is why the importer holds `Arc<S>` and `Arc<R>` (tasks need
`'static`).
*Alternative:* `futures::FutureExt::catch_unwind`. That needs `UnwindSafe`
wrappers and an extra crate.

### Paused time in tests
`#[tokio::test(start_paused = true)]` plus `tokio::time::advance` checks the
tick count exactly and runs instantly.

## Risks / Trade-offs

- [Multiple app replicas would import concurrently] → Each import is atomic,
  so the result is still consistent, just redundant. Documented as a known
  limitation. A Postgres advisory lock would fix it if needed.
- [Stale data if upstream is down for a long time] → Visible in logs (warn on
  every failed tick). A future improvement is exposing `last_import_at` in
  `/health`.
