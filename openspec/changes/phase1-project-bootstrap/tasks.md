## 1. Cargo project skeleton

- [x] 1.1 `cargo init --name bipa-nodes`, set edition 2024, create `src/lib.rs` and a thin `src/main.rs`
- [x] 1.2 Add `rust-toolchain.toml` (1.92, components rustfmt + clippy) and `rustfmt.toml`
- [x] 1.3 Add `.gitignore` (`/target`, `.env`)
- [x] 1.4 Add dependencies: tokio (full), axum, reqwest (json, rustls-tls, no default features), sqlx (postgres, runtime-tokio, tls-rustls, chrono, migrate), serde, serde_json, chrono, thiserror, anyhow, tracing, tracing-subscriber (env-filter, fmt), tower-http (trace, catch-panic, timeout), tokio-util
- [x] 1.5 Add dev-dependencies: wiremock, tower (util), http-body-util
- [x] 1.6 Add `[lints.clippy]` (unwrap_used, expect_used, panic = deny) and `clippy.toml` (allow-unwrap-in-tests, allow-expect-in-tests, allow-panic-in-tests)
- [x] 1.7 Verify `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` pass

## 2. Configuration module

- [x] 2.1 Define `Config` { database_url, bind_addr: SocketAddr, mempool_url: reqwest::Url, import_interval: Duration, http_client_timeout: Duration }
- [x] 2.2 Define `ConfigError` (`Missing(&'static str)`, `Invalid { key, value, reason }`) with thiserror
- [x] 2.3 Implement `Config::from_lookup(impl Fn(&str) -> Option<String>)` with defaults and validation, plus `Config::from_env()`
- [x] 2.4 Unit tests: all vars set; only DATABASE_URL (defaults); missing DATABASE_URL; non-numeric interval; zero interval; zero timeout; invalid bind addr; invalid URL; empty-string DATABASE_URL treated as missing

## 3. Telemetry

- [x] 3.1 Add `telemetry::init()` configuring `tracing-subscriber` with `EnvFilter` (default `info`), returning `Result` instead of panicking when already initialised
- [x] 3.2 `main.rs`: init telemetry, load config (log and exit(1) on error), log the loaded config (redacting the DB password)
- [x] 3.3 Unit test for the DB URL redaction helper (`config::redact_database_url`) (with password, without password, unparsable URL)

## 4. Local infrastructure

- [ ] 4.1 `docker-compose.yml` with a `db` service (postgres:16-alpine, healthcheck `pg_isready`, port 5432, named volume)
- [ ] 4.2 `.env.example` documenting every variable and its default
- [ ] 4.3 Verify `docker compose up -d db` becomes healthy
