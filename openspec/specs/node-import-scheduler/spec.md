# node-import-scheduler Specification

## Purpose

Periodically copy the ranking from the node source into storage, keeping the last good snapshot on any failure and never letting a failed iteration stop the loop.

## Requirements
### Requirement: Single import run
`run_once` SHALL fetch nodes from the `NodeSource` and, on success with a
non-empty list, replace the stored snapshot via `NodeRepository::replace_all`.

#### Scenario: Successful import
- **WHEN** the source returns 3 nodes and the repository accepts them
- **THEN** `run_once` returns `Ok(ImportOutcome { imported: 3 })`
- **AND** the repository received exactly those 3 nodes in order

#### Scenario: Source failure leaves storage untouched
- **WHEN** the source returns `Err(SourceError)`
- **THEN** `run_once` returns `Err(ImportError::Source(_))`
- **AND** `replace_all` is not called

#### Scenario: Empty source result does not wipe data
- **WHEN** the source returns `Ok(vec![])`
- **THEN** `run_once` returns `Err(ImportError::EmptySource)`
- **AND** `replace_all` is not called

#### Scenario: Storage failure is reported
- **WHEN** the source succeeds but `replace_all` fails
- **THEN** `run_once` returns `Err(ImportError::Storage(_))`

### Requirement: Periodic execution
`run` SHALL execute an import immediately on start and then once every
configured interval until its cancellation token is cancelled. If an import
overruns the interval, the next one SHALL be delayed instead of fired in a
burst.

#### Scenario: Immediate first run
- **WHEN** `run` starts
- **THEN** the first import happens without waiting for the interval

#### Scenario: Runs on every tick
- **WHEN** time advances by 3 intervals (with paused tokio time)
- **THEN** the source was called 4 times (initial + 3)

#### Scenario: Stops on cancellation
- **WHEN** the cancellation token is cancelled
- **THEN** `run` returns promptly, including while it is waiting for the next tick

### Requirement: The import loop never dies from a failed iteration
A failed or panicking iteration SHALL be logged and SHALL NOT stop subsequent
iterations.

#### Scenario: Errors then recovery
- **WHEN** the source fails on the first two ticks and succeeds on the third
- **THEN** the loop keeps running and the third tick stores the nodes

#### Scenario: Panic inside an iteration
- **WHEN** the source implementation panics during one iteration
- **THEN** the panic is logged as `ImportError::Panicked`
- **AND** the next tick runs normally

