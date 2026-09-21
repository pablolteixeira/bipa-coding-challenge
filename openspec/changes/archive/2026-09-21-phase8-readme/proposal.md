## Why

The challenge requires a README following their template (build tools, how
to run, focus, time spent, trade-offs, weakest part, other info) and
instructions to run the app and the tests. Docker Compose is the single entry
point for reviewers to start the whole stack.

## What Changes

- `README.md` with the exact template sections from the challenge, plus a
  quick start (`docker compose up --build`), API example, configuration table,
  how to run the tests, and a link to `docs/architecture.md`.
- Final pass on code comments (English, explaining the *why* where it's not
  obvious).
- No CI pipeline. The reviewer runs the project through Docker Compose.

## Capabilities

### New Capabilities
- `developer-workflow`: documentation for running, testing, and reviewing the project.

### Modified Capabilities
<!-- none -->

## Impact

- New `README.md`.
- No runtime behaviour change.
