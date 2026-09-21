# node-source-client Specification

## Purpose

Fetch the node ranking from mempool.space, validate each record independently and report upstream failures as typed errors.

## Requirements
### Requirement: Node source abstraction
The system SHALL define a `NodeSource` trait with an async `fetch_nodes`
method returning `Result<Vec<Node>, SourceError>`, so consumers depend on the
abstraction and not on the HTTP client.

#### Scenario: Fake source in tests
- **WHEN** a test provides an in-memory `NodeSource` implementation
- **THEN** the importer can use it without any network access

### Requirement: Fetch nodes from mempool.space
`MempoolClient` SHALL issue `GET <MEMPOOL_URL>` and parse the JSON array into
nodes, preserving the order of the response.

#### Scenario: Successful response
- **WHEN** the endpoint returns 200 with the two-node example from the challenge
- **THEN** `fetch_nodes` returns two `Node`s in the same order
- **AND** `public_key` and `alias` are exactly as received
- **AND** `capacity_sats` is `36010516297` and `first_seen` is 2018-04-05T15:13:42Z for ACINQ

#### Scenario: Extra and nullable fields are ignored
- **WHEN** a record has `city: null`, `country: null`, or extra unknown fields
- **THEN** the record is parsed successfully

#### Scenario: Empty array
- **WHEN** the endpoint returns 200 with `[]`
- **THEN** `fetch_nodes` returns `Ok` with an empty vector

### Requirement: Invalid records are skipped, not fatal
The client SHALL validate each record. A record that fails validation SHALL be
logged at `warn` and skipped, and the remaining valid records SHALL still be
returned.

#### Scenario: Negative capacity
- **WHEN** one record has `capacity: -5` and the others are valid
- **THEN** only the valid records are returned

#### Scenario: Out-of-range firstSeen
- **WHEN** one record has a `firstSeen` that can't be represented as a date
- **THEN** that record is skipped and the others are returned

#### Scenario: Empty public key
- **WHEN** one record has `publicKey: ""`
- **THEN** that record is skipped

#### Scenario: Record missing a required field
- **WHEN** one record lacks `capacity`
- **THEN** that record is skipped and the others are returned

### Requirement: Upstream failures return typed errors
The client SHALL never panic on upstream failures. It SHALL return a
`SourceError` describing the failure.

#### Scenario: Non-success status
- **WHEN** the endpoint returns 500
- **THEN** `fetch_nodes` returns `Err(SourceError::Status(500))`

#### Scenario: Timeout
- **WHEN** the endpoint takes longer than the configured timeout
- **THEN** `fetch_nodes` returns `Err(SourceError::Request(_))` within roughly the timeout

#### Scenario: Malformed JSON
- **WHEN** the endpoint returns 200 with a body that is not a JSON array
- **THEN** `fetch_nodes` returns `Err(SourceError::Decode(_))`

#### Scenario: Connection refused
- **WHEN** nothing is listening at the configured URL
- **THEN** `fetch_nodes` returns `Err(SourceError::Request(_))`

