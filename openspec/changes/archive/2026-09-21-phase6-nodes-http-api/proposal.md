## Why

The app needs a JSON API: `GET /nodes` returning `public_key`, `alias`,
`capacity` (BTC text), and `first_seen` (date-time text) for each node, read
only from the database.

## What Changes

- Add an axum `Router` built by `http::router(state)` where
  `AppState<R: NodeRepository>` holds `Arc<R>`.
- `GET /nodes`: `repo.list_nodes()`, then map each `Node` to `NodeResponse` using
  `format_btc` / `format_timestamp`, then return `200` JSON array in rank order.
- `GET /health`: liveness probe, `200 {"status":"ok"}` (no DB access).
- `ApiError` implementing `IntoResponse`: storage failures become `500
  {"error":"internal server error"}`. Details are logged, never leaked.
- JSON `404 {"error":"not found"}` fallback for unknown routes.
- Middleware: `TraceLayer` (request logs), `CatchPanicLayer` (panic becomes a JSON
  500), `TimeoutLayer` (10s).

## Capabilities

### New Capabilities
- `nodes-http-api`: the public HTTP JSON API (`/nodes`, `/health`, error responses).

### Modified Capabilities
<!-- none -->

## Impact

- New module `src/http/` (`mod.rs` router, `handlers.rs`, `response.rs` DTOs,
  `error.rs` ApiError).
- New integration test file `tests/api.rs` (real Postgres + real router).
