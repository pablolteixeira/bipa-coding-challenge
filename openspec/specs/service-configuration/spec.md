# service-configuration Specification

## Purpose

Load, default and validate the service's runtime configuration from environment variables, and keep panic shortcuts out of non-test code.

## Requirements
### Requirement: Configuration is loaded from environment variables
The system SHALL build its runtime configuration from these environment
variables: `DATABASE_URL`, `BIND_ADDR`, `MEMPOOL_URL`, `IMPORT_INTERVAL_SECS`,
and `HTTP_CLIENT_TIMEOUT_SECS`. The loader SHALL take a key-lookup function
rather than reading the process environment directly, so it can be tested
without mutating global state.

#### Scenario: All variables provided
- **WHEN** every variable is set to a valid value
- **THEN** the resulting `Config` contains exactly those values

#### Scenario: Only required variable provided
- **WHEN** only `DATABASE_URL` is set
- **THEN** `BIND_ADDR` defaults to `0.0.0.0:3000`
- **AND** `MEMPOOL_URL` defaults to `https://mempool.space/api/v1/lightning/nodes/rankings/connectivity`
- **AND** `IMPORT_INTERVAL_SECS` defaults to `60`
- **AND** `HTTP_CLIENT_TIMEOUT_SECS` defaults to `10`

### Requirement: Invalid configuration is rejected with a descriptive error
The system SHALL return a typed `ConfigError` that names the offending variable
instead of panicking when configuration is missing or invalid.

#### Scenario: Missing DATABASE_URL
- **WHEN** `DATABASE_URL` is not set
- **THEN** loading fails with `ConfigError::Missing("DATABASE_URL")`

#### Scenario: Non-numeric interval
- **WHEN** `IMPORT_INTERVAL_SECS` is `abc`
- **THEN** loading fails with `ConfigError::Invalid` naming `IMPORT_INTERVAL_SECS`

#### Scenario: Zero interval
- **WHEN** `IMPORT_INTERVAL_SECS` is `0`
- **THEN** loading fails with `ConfigError::Invalid` naming `IMPORT_INTERVAL_SECS`

#### Scenario: Zero timeout
- **WHEN** `HTTP_CLIENT_TIMEOUT_SECS` is `0`
- **THEN** loading fails with `ConfigError::Invalid` naming `HTTP_CLIENT_TIMEOUT_SECS`

#### Scenario: Invalid bind address
- **WHEN** `BIND_ADDR` is `not-an-address`
- **THEN** loading fails with `ConfigError::Invalid` naming `BIND_ADDR`

#### Scenario: Invalid mempool URL
- **WHEN** `MEMPOOL_URL` is `not a url`
- **THEN** loading fails with `ConfigError::Invalid` naming `MEMPOOL_URL`

### Requirement: Non-test code cannot panic through unwrap, expect, or panic
The crate SHALL configure clippy lints so that `unwrap()`, `expect()`, and
`panic!` in non-test code fail `cargo clippy -- -D warnings`.

#### Scenario: Lint enforcement
- **WHEN** a developer adds `.unwrap()` to non-test code
- **THEN** `cargo clippy --all-targets -- -D warnings` fails

