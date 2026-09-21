## ADDED Requirements

### Requirement: Node repository abstraction
The system SHALL define a `NodeRepository` trait with `replace_all` and
`list_nodes`, so the HTTP layer and the importer don't depend on sqlx.

#### Scenario: Fake repository in tests
- **WHEN** a unit test provides an in-memory `NodeRepository`
- **THEN** handlers and the importer work with it unchanged

### Requirement: Schema is created by embedded migrations
The system SHALL embed its SQL migrations in the binary and apply them at
startup. Applying them again SHALL be a no-op.

#### Scenario: Fresh database
- **WHEN** migrations run against an empty database
- **THEN** the `nodes` table exists with the documented columns and constraints

#### Scenario: Already migrated database
- **WHEN** migrations run a second time
- **THEN** they succeed without changes

### Requirement: Import replaces the snapshot atomically
`replace_all` SHALL replace every stored node with the given list inside a
single transaction and store each node's position as `rank`. If any statement
fails, the previous snapshot SHALL remain untouched.

#### Scenario: First import
- **WHEN** `replace_all` is called with 3 nodes on an empty table
- **THEN** it returns `Ok(3)` and `list_nodes` returns those 3 nodes in the same order

#### Scenario: Subsequent import replaces old data
- **WHEN** the table holds nodes A, B, C and `replace_all` is called with C, D
- **THEN** `list_nodes` returns exactly C, D in that order

#### Scenario: Failure keeps the previous snapshot
- **WHEN** the table holds nodes A, B and `replace_all` is called with a list containing a duplicate public key
- **THEN** it returns `Err(StorageError)`
- **AND** `list_nodes` still returns A, B

#### Scenario: Fields round-trip exactly
- **WHEN** a node with a unicode/emoji alias, `capacity_sats` 0, and `first_seen` at unix 0 is stored
- **THEN** `list_nodes` returns an identical `Node`

### Requirement: Listing nodes
`list_nodes` SHALL return all stored nodes ordered by `rank` ascending.

#### Scenario: Empty table
- **WHEN** no import has happened
- **THEN** `list_nodes` returns `Ok` with an empty vector

### Requirement: Storage failures are returned as errors
The repository SHALL never panic on database failures. It SHALL return a
`StorageError`.

#### Scenario: Database unavailable
- **WHEN** the pool can't acquire a connection within the acquire timeout
- **THEN** the call returns `Err(StorageError)` instead of hanging or panicking

#### Scenario: Capacity out of range for BIGINT
- **WHEN** `replace_all` receives a node with `capacity_sats` > `i64::MAX`
- **THEN** it returns `Err(StorageError::OutOfRange)` and nothing is written
