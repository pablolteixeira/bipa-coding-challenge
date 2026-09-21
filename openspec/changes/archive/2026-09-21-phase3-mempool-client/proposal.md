## Why

The service must collect node data from
`https://mempool.space/api/v1/lightning/nodes/rankings/connectivity`. This
phase isolates everything about that external API (HTTP, JSON shape,
validation, failure modes) behind a trait, so the importer (phase 5) can be
tested without the network.

## What Changes

- Add the `NodeSource` async trait: `async fn fetch_nodes(&self) -> Result<Vec<Node>, SourceError>`.
- Add `MempoolClient`, a `NodeSource` implementation using `reqwest` with a
  configurable URL and request timeout.
- Add `MempoolNodeDto` (serde, `camelCase`). It maps only the fields we need
  (`publicKey`, `alias`, `capacity`, `firstSeen`) and ignores the rest
  (`channels`, `city`, `country`, `iso_code`, ...), so nullable or changing
  extra fields can't break parsing.
- Add DTO→`Node` validation: capacity must be ≥ 0, `firstSeen` must be a
  representable timestamp, and `publicKey` must be non-empty. Invalid records
  are **skipped and logged**. The rest of the batch still goes through.
- Add `SourceError`: `Request` (network/timeout), `Status(u16)`, `Decode`.

## Capabilities

### New Capabilities
- `node-source-client`: fetching, parsing, and validating node data from mempool.space.

### Modified Capabilities
<!-- none -->

## Impact

- New module `src/source/` (`mod.rs` trait + error, `mempool.rs` client, `dto.rs`).
- New test fixture `tests/fixtures/mempool_rankings.json` (a real response
  sample, trimmed).
- New integration test file `tests/mempool_client.rs` (wiremock).
