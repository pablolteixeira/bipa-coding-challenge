## Why

The challenge requires "a sub-routine that imports the data periodically",
because the request path may only read from the DB. This phase connects the
source (phase 3) to the storage (phase 4) in a background loop that survives
any failure.

## What Changes

- Add `Importer<S: NodeSource, R: NodeRepository>` holding `Arc<S>` and `Arc<R>`.
- Add `Importer::run_once() -> Result<ImportOutcome, ImportError>`: fetch, then
  guard against empty results, then `replace_all`.
- Add `Importer::run(interval, CancellationToken)`: runs immediately, then on
  every tick (`MissedTickBehavior::Delay`). It logs every outcome and exits only
  when the token is cancelled.
- **Panic isolation:** each iteration runs in its own `tokio::spawn`. A panic
  inside an iteration becomes a `JoinError` that is logged, and the loop
  continues.
- Add `ImportError` (`Source(SourceError)`, `Storage(StorageError)`,
  `EmptySource`, `Panicked`).

## Capabilities

### New Capabilities
- `node-import-scheduler`: the periodic background import from source to storage and its failure semantics.

### Modified Capabilities
<!-- none -->

## Impact

- New module `src/importer.rs`.
- Uses `tokio-util::sync::CancellationToken` (added in phase 1).
- Unit tests with in-memory `FakeSource` / `FakeRepository` and tokio paused
  time. Integration test with wiremock + real Postgres.
