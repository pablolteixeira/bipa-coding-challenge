## 1. Implementation

- [ ] 1.1 Create `src/importer.rs` with `Importer<S, R>`, `ImportOutcome`, `ImportError`
- [ ] 1.2 Implement `run_once` (fetch, empty guard, replace_all) with `tracing` logs (info on success with count and duration, warn/error on failure)
- [ ] 1.3 Implement `run(interval, token)`: `interval` with `MissedTickBehavior::Delay`, `select!` on tick vs cancelled, each iteration in `tokio::spawn`, map `JoinError` to `ImportError::Panicked`

## 2. Test doubles

- [ ] 2.1 `FakeSource` with a scripted queue of responses (Ok / Err / panic) and a call counter
- [ ] 2.2 `FakeRepository` (in-memory `Mutex<Vec<Node>>`) with configurable failure and a record of `replace_all` calls. Kept in a `#[cfg(test)] test_support` module so phase 6 can reuse it

## 3. Unit tests

- [ ] 3.1 `run_once` success stores nodes in order and returns the count
- [ ] 3.2 `run_once` source error: repository not called
- [ ] 3.3 `run_once` empty list: `EmptySource`, repository not called, previous data kept
- [ ] 3.4 `run_once` storage error: `ImportError::Storage`
- [ ] 3.5 `run` first import is immediate (paused time, no advance)
- [ ] 3.6 `run` advancing 3 intervals gives 4 calls
- [ ] 3.7 `run` errors on ticks 1–2 then success on tick 3 stores the data
- [ ] 3.8 `run` a panicking source on one tick is followed by a successful next tick
- [ ] 3.9 `run` cancelling while waiting returns within one scheduler turn

## 4. Integration test (`tests/importer.rs`)

- [ ] 4.1 wiremock serving the fixture + `#[sqlx::test]` Postgres: `run_once` gives the DB contents that `list_nodes` returns
- [ ] 4.2 wiremock returns 500 after a successful import: `run_once` errors and the DB still holds the first snapshot
- [ ] 4.3 wiremock changes the ranking between runs: DB reflects the new ranking only
