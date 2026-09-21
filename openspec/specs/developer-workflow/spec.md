# developer-workflow Specification

## Purpose

Document how reviewers run the service and its test suite from a clean clone, following the challenge's README template.

## Requirements
### Requirement: README follows the challenge template
The README SHALL be in English and contain these headings verbatim: "Build
tools & versions used", "Steps to run the app", "What was the reason for your
focus? What problems were you trying to solve?", "How long did you spend on
this project?", "Did you make any trade-offs for this project? What would you
have done differently with more time?", "What do you think is the weakest part
of your project?", and "Is there any other information you'd like us to know?".

#### Scenario: Reviewer reads the README
- **WHEN** a reviewer opens `README.md`
- **THEN** every template heading is present with content

### Requirement: Run and test instructions work from a clean clone
The README SHALL document commands that, from a clean clone with Docker and
rustup installed, start the app and run the full test suite.

#### Scenario: Run with Docker only
- **WHEN** the reviewer runs `docker compose up --build`
- **THEN** `curl localhost:3000/nodes` returns node data within about a minute

#### Scenario: Run the tests
- **WHEN** the reviewer runs `docker compose up -d db`, then exports `DATABASE_URL` as documented, then runs `cargo test`
- **THEN** all unit and integration tests pass

