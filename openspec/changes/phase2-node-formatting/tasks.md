## 1. Domain types

- [x] 1.1 Create `src/domain/mod.rs` and `src/domain/node.rs` with `Node { public_key, alias, capacity_sats: u64, first_seen: DateTime<Utc> }` (derive Debug, Clone, PartialEq, Eq)
- [x] 1.2 Add `DomainError` (thiserror) with `InvalidTimestamp(i64)`

## 2. Conversions

- [x] 2.1 Implement `format_btc(u64) -> String` with integer math and doc comment explaining the choice
- [ ] 2.2 Implement `timestamp_from_unix(i64) -> Result<DateTime<Utc>, DomainError>` via `DateTime::from_timestamp`
- [ ] 2.3 Implement `format_timestamp(&DateTime<Utc>) -> String` via `to_rfc3339_opts(SecondsFormat::Secs, true)`

## 3. Unit tests

- [x] 3.1 `format_btc`: both challenge examples, 550000, 0, 1, 99_999_999, 100_000_000, 2_100_000_000_000_000 (21M BTC supply), u64::MAX
- [ ] 3.2 `format_timestamp`: both challenge examples, epoch 0, a leap day (2024-02-29)
- [ ] 3.3 `timestamp_from_unix`: valid, negative (-1), i64::MAX error, i64::MIN error
- [ ] 3.4 Round-trip test: `format_timestamp(timestamp_from_unix(x)?)` parses back to the same unix value for a sample of values
