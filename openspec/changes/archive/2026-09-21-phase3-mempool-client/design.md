## Context

Real response (checked 2026-09-21): 100 records. `publicKey`, `alias`,
`channels`, `capacity`, `firstSeen`, and `updatedAt` were always present.
`city` was null in 24/100 records, `country` and `iso_code` in 4/100, and
`subdivision` in 23/100. No duplicate public keys. Max capacity was about 4.25e10
sats, far below `i64::MAX`.

## Goals / Non-Goals

**Goals:**
- Resilient to upstream schema drift in fields we don't use.
- One bad record never discards a whole import.
- Every failure mode is covered by a deterministic test.

**Non-Goals:**
- Retries within a single fetch (the importer retries on the next tick).
- Pagination (the endpoint returns the full ranking).

## Decisions

### Two-step parsing: `Vec<serde_json::Value>`, then per-record DTO
Parsing straight into `Vec<MempoolNodeDto>` fails the *whole* batch if one
record is malformed. Instead we deserialize the body as `Vec<Value>`, which fails
only if the body isn't a JSON array. Then we deserialize each element into
the DTO and validate it. A failure is logged with the record index and skipped.
*Trade-off:* a little extra allocation for 100 records. Negligible.

### DTO uses `i64` for capacity and firstSeen
This mirrors the wire format so a negative value is *parsed* and then rejected
with a clear validation message, instead of a cryptic serde error. Converted
to `u64` / `DateTime<Utc>` in `TryFrom<MempoolNodeDto> for Node`.

### Trait via native `async fn` in traits
Rust 1.75+ supports `async fn` in traits. The importer is generic over
`S: NodeSource`, so no `dyn` is needed and there's no `async-trait` crate.
If `dyn` is ever needed, we'll switch to returning a boxed future.

### reqwest client built once, reused
Created in `MempoolClient::new(url, timeout)`, which returns a `Result`
(builder errors are surfaced, not unwrapped). Sets a `User-Agent` and the
request timeout. Uses `rustls` so there's no OpenSSL system dependency.

### Status check before decode
`response.status().is_success()` is checked explicitly, so a 500 with an HTML
body is reported as `Status(500)`, not as a confusing decode error.

## Risks / Trade-offs

- [mempool.space rate-limits us] → Default interval is 60s, one request per
  tick. A 429 surfaces as `Status(429)` and is logged.
- [Schema changes in fields we use] → Records are skipped with a warning.
  If *all* fail, the importer sees an empty list and keeps the previous
  snapshot (phase 5).
