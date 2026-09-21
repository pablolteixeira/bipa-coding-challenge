## Why

Phases 1–6 build independent, tested components. This phase assembles them
into the running server: connect, migrate, start the importer and HTTP
server concurrently, and shut down cleanly. It also adds the end-to-end test
that proves the whole flow (mempool, importer, Postgres, `GET /nodes`) works
together.

## What Changes

- Add `app` module:
  - `connect_with_retry(url, token)`: exponential backoff (1s doubling to a 30s
    cap), logs every failure, gives up only on shutdown.
  - `App::run(config, listener, token)`: migrate, build `MempoolClient` +
    `PgNodeRepository`, spawn `Importer::run`, serve axum with
    `with_graceful_shutdown(token.cancelled())`, then wait for the importer to
    stop and close the pool.
- `main.rs`: telemetry, config, bind `TcpListener` (clear error + exit 1 if
  the port is taken), signal handler (SIGINT + SIGTERM) cancels the token,
  then `App::run`.
- Accept an already-bound `TcpListener` so tests can use port 0.
- Add a multi-stage `Dockerfile` (rust:1.92 builder, debian-slim runtime,
  non-root user) and an `app` service in `docker-compose.yml`
  (`depends_on: db: condition: service_healthy`).

## Capabilities

### New Capabilities
- `service-lifecycle`: startup, dependency retry, concurrent tasks, and graceful shutdown of the whole service.

### Modified Capabilities
<!-- none -->

## Impact

- New `src/app.rs`, updated `src/main.rs`.
- New `Dockerfile`, `.dockerignore`, updated `docker-compose.yml`.
- New `tests/end_to_end.rs`.
