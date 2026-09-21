## Context

Read-only API over the stored snapshot. The API contract is the JSON example in
the challenge (snake_case fields, capacity and first_seen as strings).

## Goals / Non-Goals

**Goals:**
- Response exactly matching the challenge example.
- Handlers testable in-process without a socket or DB.
- No internal error details leaked to clients.

**Non-Goals:**
- Pagination, filtering, sorting params (100 rows).
- Auth, rate limiting, CORS (not requested; noted as future work).
- Caching the response (a DB read of 100 rows is cheap).

## Decisions

### axum 0.8
Tokio-native, tower middleware ecosystem, and `Router::oneshot` for testing.
*Alternatives:* actix-web (own runtime model, heavier), warp (filter
type complexity).

### Separate response DTO (`NodeResponse`)
The domain `Node` holds raw typed values. `NodeResponse` holds presentation
strings. `From<&Node> for NodeResponse` is the single place where formatting
is applied. This keeps serde attributes out of the domain.

### Generic `AppState<R>` instead of `Arc<dyn NodeRepository>`
It matches the importer approach (native async fn in traits isn't
dyn-compatible), with static dispatch and no boxing. Unit tests instantiate the
router with `FakeRepository`.

### `CatchPanicLayer` with a custom JSON response
Defense in depth. Our code can't `unwrap`, but a dependency could panic. Hyper
would already isolate a panicking connection. The layer additionally makes
sure the client gets a proper 500.

### Health is liveness only
It stays green when the DB is down, because the process can still serve
(`/nodes` will return 500). A readiness check (`SELECT 1`) can be added later.
Keeping it separate avoids restart loops in orchestrators during DB blips.

## Risks / Trade-offs

- [`TimeoutLayer` returns 408 by default] → Acceptable. With the pool's acquire
  timeout (5s) the DB path fails first with 500 anyway.
