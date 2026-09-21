## 1. Response and errors

- [x] 1.1 `src/http/response.rs`: `NodeResponse { public_key, alias, capacity: String, first_seen: String }` + `From<&Node>`, and `ErrorBody { error }`
- [x] 1.2 `src/http/error.rs`: `ApiError` (`Storage(StorageError)`) with `IntoResponse` (log at error, return generic 500 JSON)

## 2. Router and handlers

- [x] 2.1 `src/http/handlers.rs`: `list_nodes`, `health`
- [x] 2.2 `src/http/mod.rs`: `router<R>(state) -> Router` with routes, JSON 404 fallback, TraceLayer, CatchPanicLayer (custom JSON), TimeoutLayer

## 3. Unit tests (in-process, `FakeRepository`, `tower::ServiceExt::oneshot`)

- [x] 3.1 `From<&Node> for NodeResponse` on both challenge examples
- [x] 3.2 `GET /nodes` with the challenge data gives exact JSON body, 200, content-type application/json
- [x] 3.3 `GET /nodes` with an empty repo gives `200 []`
- [x] 3.4 Response objects have exactly 4 keys (no leaked internal fields like rank)
- [x] 3.5 Order of the response matches repository order
- [x] 3.6 Repo error gives 500 with a generic body (asserts no internal message leak)
- [x] 3.7 Panicking repo gives 500 JSON, and a second request on the same router gives 200
- [x] 3.8 `GET /health` gives 200 `{"status":"ok"}`
- [x] 3.9 Unknown route gives 404 JSON; `POST /nodes` gives 405

## 4. Integration tests (`tests/api.rs`, `#[sqlx::test]`)

- [x] 4.1 Seed via `PgNodeRepository::replace_all` with the challenge example, then `GET /nodes` returns the exact challenge JSON
- [x] 4.2 Fresh DB: `GET /nodes` gives `[]`
- [x] 4.3 Pool closed before the request: 500 JSON and no panic
