## Why

`GET /nodes` must present `capacity` in BTC (source is in sats, 1 BTC =
100,000,000 sats) and `first_seen` as human-readable date-time (source is unix
seconds). These conversions are the core business rules of the challenge. They
deserve a pure, dependency-free module with exhaustive unit tests, before any
I/O exists.

## What Changes

- Add `domain::Node`, the internal representation shared by source, storage,
  and HTTP: `public_key: String`, `alias: String`, `capacity_sats: u64`,
  `first_seen: DateTime<Utc>`.
- Add `domain::format_btc(sats: u64) -> String`: exact integer conversion,
  always 8 decimal places, `.` as the decimal separator (matches the JSON example
  `"360.10516297"`).
- Add `domain::format_timestamp(DateTime<Utc>) -> String`: RFC 3339 / ISO 8601
  in UTC with a `Z` suffix and second precision (`"2018-04-05T15:13:42Z"`).
- Add `domain::timestamp_from_unix(i64) -> Result<DateTime<Utc>, DomainError>`
  to convert the raw source value safely (out-of-range becomes an error, not a
  panic).

## Capabilities

### New Capabilities
- `node-data-formatting`: the domain `Node` type and the sats→BTC and unix→text conversions.

### Modified Capabilities
<!-- none -->

## Impact

- New module `src/domain/` (`mod.rs`, `node.rs`, `format.rs`).
- No new dependencies (chrono was added in phase 1).
- Consumed by phase 3 (validation), phase 4 (storage), and phase 6 (response).
