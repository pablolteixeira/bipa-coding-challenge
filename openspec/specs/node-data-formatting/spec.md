# node-data-formatting Specification

## Purpose

Define the domain `Node` and the exact conversions the API exposes: satoshis to BTC text and unix timestamps to RFC 3339 UTC text.

## Requirements
### Requirement: Capacity is converted from sats to BTC text exactly
The system SHALL convert a capacity in sats (`u64`) into a decimal BTC string
with exactly 8 fractional digits, using `.` as the decimal separator and no
unit suffix. The conversion SHALL use integer arithmetic only, so no
floating-point rounding can occur.

#### Scenario: Challenge example ACINQ
- **WHEN** capacity is `36010516297` sats
- **THEN** the result is `"360.10516297"`

#### Scenario: Challenge example WalletOfSatoshi
- **WHEN** capacity is `15464503162` sats
- **THEN** the result is `"154.64503162"`

#### Scenario: Requirement text example
- **WHEN** capacity is `550000` sats
- **THEN** the result is `"0.00550000"`

#### Scenario: Zero
- **WHEN** capacity is `0` sats
- **THEN** the result is `"0.00000000"`

#### Scenario: One sat
- **WHEN** capacity is `1` sat
- **THEN** the result is `"0.00000001"`

#### Scenario: Exactly one bitcoin
- **WHEN** capacity is `100000000` sats
- **THEN** the result is `"1.00000000"`

#### Scenario: Maximum value does not overflow
- **WHEN** capacity is `u64::MAX` sats
- **THEN** the result is `"184467440737.09551615"`

### Requirement: First-seen timestamp is formatted as RFC 3339 UTC text
The system SHALL format `first_seen` as RFC 3339 in UTC with second precision
and a `Z` suffix (`YYYY-MM-DDTHH:MM:SSZ`).

#### Scenario: Challenge example ACINQ
- **WHEN** first_seen is unix `1522941222`
- **THEN** the result is `"2018-04-05T15:13:42Z"`

#### Scenario: Challenge example WalletOfSatoshi
- **WHEN** first_seen is unix `1601429940`
- **THEN** the result is `"2020-09-30T01:39:00Z"`

#### Scenario: Unix epoch
- **WHEN** first_seen is unix `0`
- **THEN** the result is `"1970-01-01T00:00:00Z"`

### Requirement: Unix timestamps are converted without panicking
The system SHALL convert a raw unix-seconds `i64` into a UTC date-time. A value
outside chrono's representable range SHALL produce a `DomainError`, not a panic.

#### Scenario: Valid timestamp
- **WHEN** converting `1522941222`
- **THEN** the result is `Ok` with 2018-04-05 15:13:42 UTC

#### Scenario: Negative but representable timestamp
- **WHEN** converting `-1`
- **THEN** the result is `Ok` with 1969-12-31 23:59:59 UTC

#### Scenario: Out-of-range timestamp
- **WHEN** converting `i64::MAX`
- **THEN** the result is `Err(DomainError::InvalidTimestamp(i64::MAX))`

