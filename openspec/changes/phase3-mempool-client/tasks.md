## 1. Types

- [x] 1.1 Create `src/source/mod.rs` with the `NodeSource` trait and `SourceError` (`Request(reqwest::Error)`, `Status(u16)`, `Decode(serde_json::Error)`)
- [x] 1.2 Create `src/source/dto.rs` with `MempoolNodeDto { public_key, alias, capacity: i64, first_seen: i64 }` (`#[serde(rename_all = "camelCase")]`)
- [x] 1.3 Implement `TryFrom<MempoolNodeDto> for Node` with a `ValidationError` (negative capacity, invalid timestamp, empty public key)

## 2. Client

- [x] 2.1 Implement `MempoolClient::new(url, timeout) -> Result<Self, SourceError>`
- [x] 2.2 Implement `fetch_nodes`: GET, status check, decode `Vec<Value>`, per-record DTO parse + validate, skip and warn on invalid, preserve order
- [x] 2.3 Add a `tracing` span with the URL and log counts (received / valid / skipped)

## 3. Unit tests (in `src/source/`)

- [x] 3.1 DTO parses the challenge example (two records) including `city: null` and a full `country` object
- [x] 3.2 DTO ignores unknown extra fields
- [x] 3.3 DTO fails on a missing required field
- [x] 3.4 `TryFrom`: valid record; negative capacity; `firstSeen = i64::MAX`; empty public key; capacity 0 accepted
- [x] 3.5 Batch parse helper: mixed valid/invalid records returns only valid ones in their original order

## 4. Integration tests (`tests/mempool_client.rs`, wiremock)

- [ ] 4.1 Save a trimmed real response as `tests/fixtures/mempool_rankings.json`
- [ ] 4.2 200 with fixture returns all nodes, order preserved, fields correct
- [ ] 4.3 200 with `[]` returns `Ok(vec![])`
- [ ] 4.4 500 returns `SourceError::Status(500)`; 429 returns `Status(429)`
- [ ] 4.5 Response delayed past a 1s timeout returns `SourceError::Request`, and the test asserts it finished in under 3s
- [ ] 4.6 200 with `{"not":"an array"}` and with `garbage` returns `SourceError::Decode`
- [ ] 4.7 Unreachable port returns `SourceError::Request`
- [ ] 4.8 `#[ignore]` live test against the real mempool.space (run manually with `cargo test -- --ignored`)
