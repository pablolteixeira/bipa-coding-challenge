## Context

The challenge gives two slightly conflicting hints for capacity: the prose
says "0,00550000 BTC" (comma separator, unit suffix), while the JSON example
shows `"capacity": "360.10516297"` (dot, no suffix, string). For `first_seen`
it asks for a "readable date/time format" and the example shows
`"2018-04-05T15:13:42Z"`.

## Goals / Non-Goals

**Goals:**
- Pure functions with no I/O, fully unit-tested, including edge cases.
- Output exactly matches the JSON examples in the challenge.

**Non-Goals:**
- Localised formatting (comma separator, localised dates).
- Supporting other units (mBTC, msat).

## Decisions

### Follow the JSON example: `"360.10516297"`
The JSON example is the concrete API contract. The prose is a human-readable
illustration. A string (not a JSON number) preserves the 8 trailing digits
and avoids float precision loss in clients. The README will call this choice
out explicitly.

### Integer arithmetic: `format!("{}.{:08}", sats / 100_000_000, sats % 100_000_000)`
*Alternatives:* `f64` division, which loses precision above 2^53 and has
rounding artefacts, or the `rust_decimal` crate, an extra dependency for a
one-liner. Integer division and modulo are exact for the whole `u64` range.

### Capacity type `u64`
Capacity is never negative, so the type carries that invariant. The source
sends a JSON number that could in theory be negative. Phase 3 validates and
rejects it. The DB stores `BIGINT` with a `CHECK >= 0` (phase 4).

### Store `DateTime<Utc>` in the domain, not `i64`
The domain type represents a validated instant. Parsing happens once, at the
boundary (phase 3). `to_rfc3339_opts(SecondsFormat::Secs, true)` gives the
`Z` suffix.

## Risks / Trade-offs

- [Reviewer expected comma or "BTC" suffix] → Documented in the README.
  Changing it is a one-line edit in `format_btc`.
