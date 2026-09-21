## ADDED Requirements

### Requirement: Startup sequence
On start the service SHALL load configuration, connect to the database,
apply migrations, then run the importer and the HTTP server concurrently.

#### Scenario: Normal startup
- **WHEN** configuration is valid and the database is reachable
- **THEN** migrations are applied, the first import is triggered, and `GET /health` returns 200

#### Scenario: Invalid configuration
- **WHEN** configuration is invalid
- **THEN** the process logs which variable is wrong and exits with code 1 without a panic backtrace

#### Scenario: Port already in use
- **WHEN** `BIND_ADDR` is already bound by another process
- **THEN** the process logs the bind error and exits with code 1 without a panic backtrace

### Requirement: Database unavailable at startup is retried
If the database is unreachable at startup, the service SHALL retry with
exponential backoff capped at 30 seconds and log each failed attempt, instead
of exiting.

#### Scenario: Database comes up later
- **WHEN** the database becomes reachable after a few failed attempts
- **THEN** the service continues the startup sequence normally

#### Scenario: Shutdown while retrying
- **WHEN** a shutdown signal arrives while waiting to retry the connection
- **THEN** the process exits cleanly with code 0

### Requirement: HTTP serving is independent of the external API
`GET /nodes` SHALL keep returning the last stored snapshot while mempool.space
is failing.

#### Scenario: Upstream outage after a successful import
- **WHEN** one import succeeded and mempool.space then starts returning 500
- **THEN** `GET /nodes` keeps returning the previously imported nodes with status 200

### Requirement: Graceful shutdown
On SIGINT or SIGTERM the service SHALL stop accepting connections, finish
in-flight requests, stop the importer, close the pool, and exit with code 0.

#### Scenario: Shutdown signal
- **WHEN** the cancellation token is cancelled while the server is running
- **THEN** `App::run` returns `Ok(())` and both the importer task and the server have stopped
