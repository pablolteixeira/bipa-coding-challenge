## Context

Composition root. Everything below this layer is generic over traits. This is
the only place that picks concrete implementations (`MempoolClient`,
`PgNodeRepository`).

## Goals / Non-Goals

**Goals:**
- The whole service can run inside a test (random port, wiremock upstream,
  isolated DB).
- Clean startup and shutdown semantics with no panics.
- One-command local run: `docker compose up --build`.

**Non-Goals:**
- Kubernetes manifests, metrics endpoint, horizontal scaling.

## Decisions

### `CancellationToken` as the single shutdown signal
One token is shared by the signal handler, importer, server, and connection
retry. Tests cancel it directly instead of sending OS signals.

### Retry DB connection, but exit on config, bind, or migration errors
A DB that isn't ready yet is a transient condition (for example, containers
starting in parallel), so retrying is correct. A bad config, a taken port, or a
broken migration are permanent. Retrying would hide the problem, so we log a
clear message and exit 1. This isn't a "crash" (no panic, no undefined
state). It's a deliberate refusal to start.

### Migration failure caused by a connection drop
If migrate fails with a connection-level `sqlx::Error` (Io / PoolTimedOut),
treat it like a connection failure and retry. Any other migrate error is fatal.

### `App::run` takes a `TcpListener`
`main` binds the configured address. Tests bind `127.0.0.1:0` and read
`local_addr()`. There's no port collision between parallel tests.

### Docker: multi-stage with a dependency-caching layer
Build deps against a dummy `main.rs` first, then copy the sources. Rebuilds
after code changes take seconds. The runtime image is `debian:bookworm-slim`
with `ca-certificates` (needed for HTTPS to mempool.space), running as a
non-root user.

### End-to-end test drives the real binary logic in-process
`tests/end_to_end.rs` runs wiremock (fixture), `#[sqlx::test]` pool, and
`App::run` with a 200ms import interval in a spawned task. Then it runs a real
HTTP client against the bound port. It polls with a deadline (no fixed
sleeps) until `/nodes` is non-empty.

## Risks / Trade-offs

- [E2E test timing flakiness] → Poll-until-condition with a generous
  deadline (10s) instead of sleeps.
- [Docker build time for reviewers] → The README also documents the
  `cargo run` path with only the DB in Docker.
