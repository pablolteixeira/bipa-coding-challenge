## 1. Implementation

- [x] 1.1 Create `src/importer.rs` with `Importer<S, R>`, `ImportOutcome`, `ImportError`
- [x] 1.2 Implement `run_once` (fetch, empty guard, replace_all) with `tracing` logs (info on success with count and duration, warn/error on failure)
- [x] 1.3 Implement `run(interval, token)`: `interval` with `MissedTickBehavior::Delay`, `select!` on tick vs cancelled, each iteration in `tokio::spawn`, map `JoinError` to `ImportError::Panicked`

## 2. Test doubles

- [x] 2.1 `FakeSource` with a scripted queue of responses (Ok / Err / panic) and a call counter
- [x] 2.2 `FakeRepository` (in-memory `Mutex<Vec<Node>>`) with configurable failure and a record of `replace_all` calls. Kept in a `#[cfg(test)] test_support` module so phase 6 can reuse it

## 3. Unit tests

- [x] 3.1 `run_once` success stores nodes in order and returns the count
- [x] 3.2 `run_once` source error: repository not called
- [x] 3.3 `run_once` empty list: `EmptySource`, repository not called, previous data kept
- [x] 3.4 `run_once` storage error: `ImportError::Storage`
- [x] 3.5 `run` first import is immediate (paused time, no advance)
- [x] 3.6 `run` advancing 3 intervals gives 4 calls
- [x] 3.7 `run` errors on ticks 1–2 then success on tick 3 stores the data
- [x] 3.8 `run` a panicking source on one tick is followed by a successful next tick
- [x] 3.9 `run` cancelling while waiting returns within one scheduler turn

## 4. Integration test (`tests/importer.rs`)

- [x] 4.1 wiremock serving the fixture + `#[sqlx::test]` Postgres: `run_once` gives the DB contents that `list_nodes` returns
- [x] 4.2 wiremock returns 500 after a successful import: `run_once` errors and the DB still holds the first snapshot
- [x] 4.3 wiremock changes the ranking between runs: DB reflects the new ranking only
