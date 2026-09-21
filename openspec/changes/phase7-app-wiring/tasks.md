## 1. Composition

- [x] 1.1 `src/app.rs`: `connect_with_retry(url, token)` with capped exponential backoff and cancellation
- [x] 1.2 `src/app.rs`: migrate with retry on connection-level errors and fatal on others
- [ ] 1.3 `src/app.rs`: `App::run(config, listener, token)`: build client + repo, spawn importer, serve with graceful shutdown, join the importer, close the pool
- [ ] 1.4 `src/main.rs`: telemetry, then config, then bind, then signal handler (ctrl_c + SIGTERM via `tokio::signal::unix`), then `App::run`. Map fatal errors to a logged message and `ExitCode::FAILURE`

## 2. Unit tests

- [x] 2.1 Backoff schedule helper: 1, 2, 4, 8, 16, 30, 30, ... (pure function)
- [x] 2.2 `connect_with_retry` with an unreachable URL and a token cancelled after 2 attempts (paused time) returns a cancellation result, no panic
- [x] 2.3 Error classification: connection-level `sqlx::Error` variants are retryable, others are fatal

## 3. End-to-end tests (`tests/end_to_end.rs`)

- [ ] 3.1 Happy path: wiremock fixture + DB + `App::run` on port 0. `/health` gives 200, `/nodes` eventually equals the formatted fixture
- [ ] 3.2 Upstream outage: after the first import, switch wiremock to 500. `/nodes` still returns the old data across several intervals
- [ ] 3.3 Ranking change: switch wiremock to a different fixture. `/nodes` eventually reflects the new ranking and order
- [ ] 3.4 Graceful shutdown: cancel the token. `App::run` returns `Ok(())` within 5s and the port stops accepting connections
- [ ] 3.5 Invalid config smoke test: run the built binary (`env!("CARGO_BIN_EXE_bipa-nodes")`) with `IMPORT_INTERVAL_SECS=0`. It exits with code 1, stderr names the variable, and there is no "panicked" in the output

## 4. Containerisation

- [ ] 4.1 Multi-stage `Dockerfile` + `.dockerignore`
- [ ] 4.2 `app` service in `docker-compose.yml` (env, port 3000, depends_on db healthy)
- [ ] 4.3 Manual check: `docker compose up --build`, then `curl localhost:3000/nodes` returns real data
- [ ] 4.4 Manual check: `docker compose stop db` while running. `/nodes` returns 500 and the process stays up. `docker compose start db` and it recovers
