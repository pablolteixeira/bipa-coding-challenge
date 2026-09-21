## ADDED Requirements

### Requirement: List nodes endpoint
`GET /nodes` SHALL respond `200` with `Content-Type: application/json` and a
JSON array. Each element SHALL contain exactly the fields `public_key`,
`alias`, `capacity`, and `first_seen`, ordered by ranking. The handler SHALL
read only from the `NodeRepository` and SHALL NOT call the external API.

#### Scenario: Challenge example
- **WHEN** the repository holds ACINQ (36010516297 sats, first seen 1522941222) and WalletOfSatoshi.com (15464503162 sats, first seen 1601429940)
- **THEN** the response body is
  `[{"public_key":"03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f","alias":"ACINQ","capacity":"360.10516297","first_seen":"2018-04-05T15:13:42Z"},{"public_key":"035e4ff418fc8b5554c5d9eea66396c227bd429a3251c8cbc711002ba215bfc226","alias":"WalletOfSatoshi.com","capacity":"154.64503162","first_seen":"2020-09-30T01:39:00Z"}]`

#### Scenario: No data imported yet
- **WHEN** the repository is empty
- **THEN** the response is `200` with body `[]`

#### Scenario: Values passed through unchanged
- **WHEN** a node's alias contains unicode, emoji, or quotes
- **THEN** `alias` in the response is byte-for-byte the stored value (JSON-escaped only)

### Requirement: Errors are JSON and never crash the server
The API SHALL turn every failure into a JSON error response and keep serving
subsequent requests.

#### Scenario: Database failure
- **WHEN** `list_nodes` returns `Err(StorageError)`
- **THEN** the response is `500` with body `{"error":"internal server error"}`
- **AND** the error details are logged, not returned

#### Scenario: Panic in a handler
- **WHEN** the repository implementation panics during a request
- **THEN** the response is `500` with a JSON error body
- **AND** a following request is served normally

#### Scenario: Unknown route
- **WHEN** a client requests `GET /does-not-exist`
- **THEN** the response is `404` with body `{"error":"not found"}`

#### Scenario: Wrong method
- **WHEN** a client sends `POST /nodes`
- **THEN** the response is `405 Method Not Allowed`

### Requirement: Health endpoint
`GET /health` SHALL respond `200 {"status":"ok"}` whenever the process is
serving HTTP, without touching the database.

#### Scenario: Liveness
- **WHEN** a client requests `GET /health`
- **THEN** the response is `200` with body `{"status":"ok"}`
