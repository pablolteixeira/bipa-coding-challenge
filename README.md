# bipa-nodes

A Rust service that imports the Lightning Network node connectivity ranking from
[mempool.space](https://mempool.space) on a schedule, stores it in PostgreSQL and
serves it as JSON.

```
GET /nodes
[
  {
    "public_key": "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
    "alias": "ACINQ",
    "capacity": "360.10516297",
    "first_seen": "2018-04-05T15:13:42Z"
  },
  ...
]
```

`GET /nodes` only reads from the database. A background task is the only thing
that talks to mempool.space, so the API keeps answering with the last good
snapshot even when the upstream is slow or down.

The architecture, with diagrams of every flow, is in
[`docs/architecture.md`](docs/architecture.md).

## Quick start

Requirements: Docker with the Compose plugin.

```sh
docker compose up --build
```

This starts PostgreSQL and the app. The app waits for the database
healthcheck, applies migrations, imports the ranking right away and then
every 60 seconds.

```sh
curl localhost:3000/nodes
curl localhost:3000/health   # {"status":"ok"}
```

Stop everything with `docker compose down`. Add `-v` to also drop the data.

## API

| Route | Response |
|-------|----------|
| `GET /nodes` | `200` with the stored ranking, in ranking order. `[]` until the first import succeeds. `500 {"error":"internal server error"}` if the database is unavailable. |
| `GET /health` | `200 {"status":"ok"}`. Liveness only; it doesn't query the database. |
| anything else | `404 {"error":"not found"}` (or `405` for a wrong method on a known route) |

Fields of each node:

| Field | Source (`mempool.space`) | Output |
|-------|--------------------------|--------|
| `public_key` | `publicKey` | unchanged |
| `alias` | `alias` | unchanged |
| `capacity` | `capacity` in sats | BTC as a string with 8 decimals: `36010516297` becomes `"360.10516297"` |
| `first_seen` | `firstSeen` unix seconds | RFC 3339 in UTC: `1522941222` becomes `"2018-04-05T15:13:42Z"` |

## Configuration

Environment variables (see [`.env.example`](.env.example)):

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | required | PostgreSQL connection string |
| `BIND_ADDR` | `0.0.0.0:3000` | HTTP listen address |
| `MEMPOOL_URL` | `https://mempool.space/api/v1/lightning/nodes/rankings/connectivity` | Source endpoint |
| `IMPORT_INTERVAL_SECS` | `60` | Seconds between imports (> 0) |
| `HTTP_CLIENT_TIMEOUT_SECS` | `10` | Timeout of the mempool.space request (> 0) |
| `RUST_LOG` | `info,sqlx=warn` | Log filter |

An invalid value stops the service at startup with a message naming the
variable (exit code 1).

## Running locally without the app container

Requirements: [rustup](https://rustup.rs) (the pinned toolchain in
`rust-toolchain.toml` is installed automatically) and Docker for PostgreSQL.

```sh
docker compose up -d db
cargo run
```

`.cargo/config.toml` points `DATABASE_URL` at the compose database, so no
exports are needed. An exported `DATABASE_URL` takes precedence.

## Tests

```sh
docker compose up -d db   # integration tests need PostgreSQL
cargo test
```

There are 120 tests:

- **Unit tests (87)**, in `src/`, with no network or database. They cover the
  sats→BTC and timestamp conversions (including edge values like 0, 1 sat and
  `u64::MAX`), config validation, JSON parsing and validation of mempool
  records, the importer with in-memory fakes and tokio's paused clock, the
  HTTP router in-process, and the retry/backoff logic.
- **Integration tests (33)**, in `tests/`:
  - `mempool_client.rs`: the HTTP client against a wiremock server (5xx, 429,
    timeouts, malformed bodies, unreachable host).
  - `repository.rs`: the repository against real PostgreSQL, including
    rollback on failure.
  - `importer.rs` and `api.rs`: the importer and the API with real components.
  - `end_to_end.rs`: the whole service on a random port, plus the compiled
    binary (exit codes, database retry, clean exit on SIGTERM).

`#[sqlx::test]` gives every database test its own freshly migrated database,
so tests are isolated and run in parallel.

One test calls the real mempool.space and is ignored by default:

```sh
cargo test -- --ignored
```

Lints: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.

## Project layout

```
src/
  main.rs        entrypoint: logging, config, signal handling
  app.rs         startup (DB retry, migrations), wiring, graceful shutdown
  config.rs      environment configuration
  domain/        Node + sats→BTC and timestamp conversions (pure)
  source/        NodeSource trait + mempool.space client and validation
  storage/       NodeRepository trait + PostgreSQL implementation
  importer.rs    periodic import loop
  http/          axum router, handlers, response and error types
migrations/      SQL migrations (embedded in the binary)
tests/           integration and end-to-end tests
docs/            architecture documentation
openspec/        implementation plan, one change per phase
```

---

## Build tools & versions used

- Rust 1.92.0 (edition 2024), Cargo 1.92.0
- PostgreSQL 16 (Docker image `postgres:16-alpine`)
- Docker 28.1 / Docker Compose 2.35
- Main crates: tokio 1.53, axum 0.8, reqwest 0.13 (rustls), sqlx 0.8
  (PostgreSQL, migrations), serde 1, chrono 0.4, thiserror 2, tracing 0.1,
  tower-http 0.7
- Test crates: wiremock 0.6, tower 0.5, http-body-util 0.1, plus tokio's
  `test-util` for a controllable clock
- Planning: [OpenSpec](https://github.com/Fission-AI/OpenSpec) (`openspec/`)

## Steps to run the app

1. `docker compose up --build`
2. `curl localhost:3000/nodes`

To run it outside Docker, and to run the tests, see
[Running locally](#running-locally-without-the-app-container) and
[Tests](#tests) above.

## What was the reason for your focus? What problems were you trying to solve?

The core logic (two conversions and a periodic copy) is small. So I focused
on what makes a service like this trustworthy in production:

- **Never crashing.** Clippy denies `unwrap`, `expect` and `panic!` outside
  tests. Every failure is a typed error. The import loop survives upstream
  errors, database errors and even panics (each run is its own task). Handler
  panics become a JSON 500. Every external call has a timeout. The database
  being down at startup is retried with backoff instead of exiting.
- **Keeping reads and imports separate.** `GET /nodes` never touches the
  upstream. Imports replace the snapshot inside one transaction, so readers
  never see half an import. Failed or empty imports keep the last good data.
- **Exact values.** The sats→BTC conversion uses integer math, so there's no
  float rounding, and it's tested up to `u64::MAX`. Raw values are stored and
  formatted only at the API edge.
- **Testability.** The importer and HTTP layer depend on two small traits
  (`NodeSource`, `NodeRepository`), so they can be tested with in-memory fakes.
  The real implementations are tested against wiremock and a real PostgreSQL,
  and the whole binary is tested end to end.

## How long did you spend on this project?

About 2 hours.

## Did you make any trade-offs for this project? What would you have done differently with more time?

Trade-offs:

- **`capacity` format.** The statement's prose shows `0,00550000 BTC`, while
  its JSON example shows `"360.10516297"`. I followed the JSON example (a
  string with a `.` separator and no unit), since that's the concrete API
  contract, and a string avoids float precision loss in clients.
- **Replace the snapshot instead of upserting.** The source is a top-100
  ranking, so the API mirrors the latest ranking and nodes that drop out
  disappear. There's no history.
- **PostgreSQL over SQLite.** It's closer to production and allows
  `TIMESTAMPTZ` and isolated per-test databases. The cost is that Docker is
  needed.
- **Runtime SQL queries** (`sqlx::query_as`) instead of compile-time checked
  macros, so `cargo build` doesn't need a database or an offline query cache.
  The queries are covered by integration tests instead.
- **No CI pipeline.** Docker Compose is the single entry point.

With more time I would:

- Expose data freshness (`last_import_at`, last error) in the response or in
  a readiness endpoint, so clients can tell when data is stale.
- Take a PostgreSQL advisory lock around imports, so several replicas don't
  import concurrently.
- Add pagination/filtering to `/nodes`, rate limiting, and metrics
  (import duration, failures, request latency).
- Add jitter to the import schedule and honour `Retry-After` on HTTP 429.

## What do you think is the weakest part of your project?

Staleness is invisible to clients. If mempool.space stays down, `/nodes`
keeps serving the last snapshot with a `200`, and the only signal is a
warning in the logs. That's the right default for availability, but a client
can't tell one-minute-old data from one-day-old data.

Also, the integration and end-to-end tests need a running PostgreSQL
(`docker compose up -d db`), so `cargo test` alone fails on a machine without
it.

## Is there any other information you’d like us to know?

- **Use of AI.** The challenge statement forbids AI tools, but HR told me AI
  use was allowed in my case. I used Claude Code as a pair programmer. The
  work was planned up front as phased OpenSpec changes (`openspec/changes/`)
  that I reviewed, and each phase was implemented only after my go-ahead. The
  commit history reflects that phase-by-phase progression.
- `docs/architecture.md` has diagrams of the import flow, the request flow,
  the module dependencies, the data model and the process lifecycle.
- Each OpenSpec change has its proposal, requirements as testable scenarios,
  design decisions with alternatives, and the task checklist used during
  implementation.
