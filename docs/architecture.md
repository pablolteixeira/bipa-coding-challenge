# Architecture

This service imports Lightning Network node rankings from mempool.space on a
schedule, persists them in PostgreSQL, and serves them through a JSON API.

The one rule that shapes everything: **`GET /nodes` never talks to
mempool.space.** The HTTP path only reads the database; a separate background
task is the only thing that writes to it.

---

## 1. Big picture

```mermaid
flowchart LR
    subgraph External
        MP[(mempool.space<br/>/api/v1/lightning/nodes/<br/>rankings/connectivity)]
        CL[Client app<br/>curl / mobile]
    end

    subgraph Service["bipa-nodes (single Rust binary, tokio runtime)"]
        direction LR
        IMP["Importer task<br/>(background loop)"]
        SRC["MempoolClient<br/>impl NodeSource"]
        REPO["PgNodeRepository<br/>impl NodeRepository"]
        API["HTTP API (axum)<br/>GET /nodes<br/>GET /health"]
        FMT["Domain formatting<br/>sats → BTC<br/>unix → RFC 3339"]
    end

    DB[(PostgreSQL<br/>table: nodes)]

    IMP -- "1. every N seconds" --> SRC
    SRC -- "2. HTTPS GET (timeout)" --> MP
    SRC -- "3. Vec&lt;Node&gt; (validated)" --> IMP
    IMP -- "4. replace snapshot<br/>(one transaction)" --> REPO
    REPO --> DB

    CL -- "GET /nodes" --> API
    API -- "list_nodes()" --> REPO
    API -- "format each node" --> FMT
    API -- "JSON 200 / 500" --> CL
```

Two independent flows share only the database:

| Flow | Trigger | Touches mempool.space? | Writes DB? |
|------|---------|------------------------|------------|
| Import | timer (`IMPORT_INTERVAL_SECS`, default 60s), plus once at startup | yes | yes |
| Request | client `GET /nodes` | **no** | no (read only) |

If mempool.space is slow or down, `/nodes` keeps answering with the last
successful snapshot.

---

## 2. Modules (crate layout)

The crate is a **library plus a thin binary**, so integration tests in `tests/`
can build the same components the binary uses.

```mermaid
flowchart TB
    main["main.rs<br/>(binary: compose + run)"]
    app["app.rs<br/>startup, shutdown, wiring"]
    config["config.rs<br/>env → Config"]
    domain["domain/<br/>Node, format_btc, format_timestamp"]
    source["source/<br/>trait NodeSource<br/>MempoolClient, DTOs"]
    storage["storage/<br/>trait NodeRepository<br/>PgNodeRepository, migrations"]
    importer["importer.rs<br/>run_once, run_forever"]
    http["http/<br/>router, handlers, response DTOs, ApiError"]

    main --> app
    app --> config
    app --> importer
    app --> http
    app --> storage
    app --> source
    importer --> source
    importer --> storage
    http --> storage
    http --> domain
    source --> domain
    storage --> domain
```

Dependency rule: **arrows only point towards `domain`**. `domain` depends on
nothing internal. `importer` and `http` depend on the traits (`NodeSource`,
`NodeRepository`), not on reqwest or sqlx. That's what lets us unit-test them
with in-memory fakes.

| Module | Responsibility | Knows about |
|--------|----------------|-------------|
| `domain` | `Node` struct; pure functions to format sats as BTC and timestamps as text | `chrono` only |
| `source` | Calls mempool.space, deserializes JSON, validates each record into a `Node` | `reqwest`, `serde` |
| `storage` | Saves and reads `Node`s in Postgres; owns SQL migrations | `sqlx` |
| `importer` | Runs "fetch, then replace snapshot" on an interval; logs failures and keeps going | traits only |
| `http` | axum router; turns `Node` into the public JSON shape; maps errors to HTTP status codes | trait `NodeRepository`, `domain` |
| `config` | Reads env vars, applies defaults, rejects invalid values | std |
| `app` | Builds everything, runs the importer and HTTP server side by side, shuts down gracefully | everything |

---

## 3. Import flow (write path)

```mermaid
sequenceDiagram
    autonumber
    participant T as tokio interval
    participant I as Importer
    participant S as MempoolClient (NodeSource)
    participant M as mempool.space
    participant R as PgNodeRepository
    participant DB as PostgreSQL

    Note over I: runs once immediately at startup,<br/>then on every tick
    T->>I: tick
    I->>S: fetch_nodes()
    S->>M: GET /rankings/connectivity (10s timeout)
    alt network error / timeout / non-2xx / bad JSON
        M-->>S: error
        S-->>I: Err(SourceError)
        I->>I: log warn, keep old snapshot, wait for next tick
    else 200 OK
        M-->>S: JSON array
        S->>S: deserialize into MempoolNodeDto
        S->>S: validate each record into Node<br/>(skip + log invalid ones)
        S-->>I: Ok(Vec<Node>)
        alt list is empty
            I->>I: log warn, do NOT wipe DB
        else has nodes
            I->>R: replace_all(nodes)
            R->>DB: BEGIN
            R->>DB: DELETE FROM nodes
            R->>DB: INSERT ... (bulk, keeps rank order)
            R->>DB: COMMIT
            alt DB error
                DB-->>R: error, transaction rolls back
                R-->>I: Err(StorageError)
                I->>I: log error, old snapshot intact
            else ok
                R-->>I: Ok(count)
                I->>I: log info "imported N nodes"
            end
        end
    end
```

Why these choices:

- **Replace the whole snapshot in one transaction.** The source is a *ranking*
  (top 100 by connectivity), and nodes drop in and out of it. Replacing inside
  one transaction means readers see either the old ranking or the new one,
  never a half-written mix, and nodes that left the ranking disappear.
- **One bad record shouldn't block the rest.** A record with a negative
  capacity or an out-of-range timestamp gets logged and skipped. The other
  99 are still imported.
- **An empty response doesn't count as "zero nodes".** It most likely means
  something went wrong upstream, so we keep the last good data.
- **`MissedTickBehavior::Delay`.** If an import takes longer than the
  interval, we don't fire a burst of catch-up imports.

---

## 4. Request flow (read path)

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant A as axum router
    participant H as nodes handler
    participant R as NodeRepository
    participant DB as PostgreSQL
    participant F as domain formatting

    C->>A: GET /nodes
    A->>H: route
    H->>R: list_nodes()
    R->>DB: SELECT ... ORDER BY rank
    alt DB error
        DB-->>R: error
        R-->>H: Err(StorageError)
        H-->>C: 500 {"error":"internal server error"}<br/>(details only in logs)
    else ok
        DB-->>R: rows
        R-->>H: Vec<Node>
        loop each node
            H->>F: format_btc(capacity_sats)
            H->>F: format_timestamp(first_seen)
        end
        H-->>C: 200 [{public_key, alias, capacity, first_seen}]
    end
```

Response example:

```json
[
  {
    "public_key": "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
    "alias": "ACINQ",
    "capacity": "360.10516297",
    "first_seen": "2018-04-05T15:13:42Z"
  }
]
```

- If no import has succeeded yet, the table is empty and the response is
  `200 []`.
- Unknown routes return `404` with a JSON body.
- A panic in a handler is caught by `CatchPanicLayer` and returned as `500`.
  The server keeps running.

---

## 5. Data model

The database stores **raw values** (sats as an integer, timestamp as
`TIMESTAMPTZ`). Formatting happens only at the API edge. That keeps the
data lossless, and the presentation format can change without a migration.

```mermaid
erDiagram
    NODES {
        TEXT public_key PK "exactly as imported"
        TEXT alias "exactly as imported"
        BIGINT capacity_sats "CHECK >= 0"
        TIMESTAMPTZ first_seen "from unix firstSeen"
        INTEGER rank "position in the ranking (0..n)"
        TIMESTAMPTZ imported_at "when this snapshot was written"
    }
```

Transformations along the way:

| Field | mempool.space | Domain `Node` | DB column | API output |
|-------|---------------|---------------|-----------|------------|
| public key | `publicKey: string` | `public_key: String` | `TEXT` | `"public_key"`, unchanged |
| alias | `alias: string` | `alias: String` | `TEXT` | `"alias"`, unchanged |
| capacity | `capacity: int` (sats) | `capacity_sats: u64` | `BIGINT` | `"capacity": "360.10516297"` |
| first seen | `firstSeen: int` (unix s) | `first_seen: DateTime<Utc>` | `TIMESTAMPTZ` | `"first_seen": "2018-04-05T15:13:42Z"` |
| order | array index | `rank` (import order) | `INTEGER` | array order |

**sats → BTC** uses integer math only (no `f64`), so there's no rounding error:
`format!("{}.{:08}", sats / 100_000_000, sats % 100_000_000)`.
For example, `550_000` becomes `"0.00550000"` and `36_010_516_297` becomes `"360.10516297"`.

---

## 6. Process lifecycle and "never crash"

```mermaid
stateDiagram-v2
    [*] --> LoadConfig
    LoadConfig --> Exit1: invalid config<br/>(clear error, exit code 1)
    LoadConfig --> ConnectDB
    ConnectDB --> ConnectDB: fail, retry with backoff (logged)
    ConnectDB --> Migrate
    Migrate --> Running
    state Running {
        [*] --> Importer
        [*] --> HttpServer
        Importer --> Importer: tick, import, errors logged
        HttpServer --> HttpServer: serve requests
    }
    Running --> ShuttingDown: SIGINT / SIGTERM
    ShuttingDown --> [*]: stop importer, drain HTTP, close pool
```

How "the server must never crash" is enforced:

1. **No `unwrap`/`expect`/`panic!` in non-test code.** Enforced by clippy lints
   (`unwrap_used`, `expect_used`, `panic` set to `deny`) in `Cargo.toml`.
2. **Typed errors everywhere** (`thiserror`): `ConfigError`, `SourceError`,
   `StorageError`, `ApiError`. Every fallible call returns `Result`.
3. **The importer loop can't die.** Each iteration's error is logged and the
   loop continues. The loop only exits on the shutdown signal.
4. **HTTP errors become responses, not crashes.** `ApiError` implements
   `IntoResponse`, and `CatchPanicLayer` covers anything unexpected.
5. **External calls have timeouts.** A request timeout on reqwest, a connection
   acquire timeout on the pool, and a request timeout layer on axum.
6. **The database being down isn't fatal.** At startup we retry the connection.
   At runtime, `/nodes` returns 500 and the importer logs and retries on the
   next tick.
7. The only intentional exit is a **config error at startup**. Running with a
   wrong config is worse than failing fast with a clear message.

---

## 7. Testing strategy

```mermaid
flowchart TB
    subgraph Unit["Unit tests (cargo test --lib), no network, no DB"]
        U1["domain: format_btc<br/>0, 1 sat, 550000, 1 BTC, u64::MAX"]
        U2["domain: format_timestamp<br/>spec examples, epoch 0"]
        U3["source: DTO parsing<br/>fixture JSON, null city/country,<br/>extra fields, missing fields"]
        U4["source: DTO to Node validation<br/>negative capacity, bad timestamp"]
        U5["importer: with FakeSource + FakeRepo<br/>success, source error, empty list, repo error"]
        U6["http: router with FakeRepo<br/>200 shape, empty 200 [], 500, 404"]
        U7["config: defaults, overrides, invalid values"]
    end
    subgraph Integration["Integration tests (tests/), real components"]
        I1["mempool_client.rs<br/>wiremock: 200, 500, timeout, malformed JSON"]
        I2["repository.rs<br/>#[sqlx::test] real Postgres:<br/>replace_all, ordering, atomic rollback"]
        I6["importer.rs<br/>wiremock + real Postgres:<br/>run_once, outage keeps snapshot"]
        I3["api.rs<br/>real Postgres + real router"]
        I4["end_to_end.rs<br/>wiremock mempool, importer, Postgres, GET /nodes"]
        I5["live_mempool.rs (#[ignore])<br/>hits the real mempool.space"]
    end
    Unit --> Integration
```

- **Traits as seams.** `NodeSource` and `NodeRepository` are async traits.
  Unit tests use in-memory fakes; integration tests use the real
  implementations.
- **`#[sqlx::test]`** creates a fresh, isolated database per test and runs the
  migrations, so tests don't interfere with each other.
- **`wiremock`** simulates mempool.space deterministically, including failure
  modes (500s, delays past the timeout, invalid JSON).
- Integration tests need Postgres: `docker compose up -d db`, export
  `DATABASE_URL`, then `cargo test`. There's no CI pipeline. Docker Compose is
  the single entry point for running the stack.

---

## 8. Configuration

| Env var | Default | Meaning |
|---------|---------|---------|
| `DATABASE_URL` | none (required) | Postgres connection string |
| `BIND_ADDR` | `0.0.0.0:3000` | HTTP listen address |
| `MEMPOOL_URL` | `https://mempool.space/api/v1/lightning/nodes/rankings/connectivity` | Source endpoint |
| `IMPORT_INTERVAL_SECS` | `60` | Seconds between imports (must be > 0) |
| `HTTP_CLIENT_TIMEOUT_SECS` | `10` | Timeout for the mempool.space request |
| `RUST_LOG` | `info` | Log filter (`tracing-subscriber`) |

---

## 9. Implementation phases

Each phase is an OpenSpec change, archived under `openspec/changes/archive/`
(the resulting specs are in `openspec/specs/`). Each maps to
one or more logical commits.

```mermaid
flowchart LR
    P1["phase1<br/>project-bootstrap"] --> P2["phase2<br/>node-formatting"]
    P2 --> P3["phase3<br/>mempool-client"]
    P2 --> P4["phase4<br/>node-storage"]
    P3 --> P5["phase5<br/>periodic-importer"]
    P4 --> P5
    P4 --> P6["phase6<br/>nodes-http-api"]
    P5 --> P7["phase7<br/>app-wiring"]
    P6 --> P7
    P7 --> P8["phase8<br/>readme"]
```
