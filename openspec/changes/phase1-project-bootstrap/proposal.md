## Why

The repository is empty. Every later phase needs the same foundation: a
compiling Rust crate split into a library and a binary, the dependency set,
lints that enforce the "never crash" rule, structured logging, validated
configuration, and a local PostgreSQL to develop and test against.

## What Changes

- Create the Cargo project `bipa-nodes` (edition 2024) as a **library + binary**
  (`src/lib.rs` + `src/main.rs`) so integration tests in `tests/` can use the
  same code as the binary.
- Add the dependencies used across phases (tokio, axum, reqwest with rustls,
  sqlx with postgres, serde, chrono, thiserror, tracing) and dev-dependencies
  (wiremock, tower, http-body-util).
- Configure crate-level lints in `Cargo.toml`: `clippy::unwrap_used`,
  `clippy::expect_used`, `clippy::panic` = `deny` (tests are exempt).
- Add `rust-toolchain.toml` (stable, pinned to 1.92) and `rustfmt.toml`.
- Add a `config` module that reads environment variables into a typed
  `Config`, applies defaults, and rejects invalid values with a typed
  `ConfigError`.
- Add tracing initialisation (`RUST_LOG`, default `info`).
- Add `docker-compose.yml` with PostgreSQL 16 (plus a healthcheck) and
  `.env.example`.
- Add `.gitignore`.

## Capabilities

### New Capabilities
- `service-configuration`: loading, defaulting, and validating runtime configuration from environment variables.

### Modified Capabilities
<!-- none -->

## Impact

- New files: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`,
  `src/lib.rs`, `src/main.rs`, `src/config.rs`, `src/telemetry.rs`,
  `docker-compose.yml`, `.env.example`, `.gitignore`.
- No runtime behaviour beyond "binary starts, loads config, logs, exits". Real
  wiring happens in phase 7.
