## Context

Greenfield Rust project. The challenge rewards architecture, code
organisation, and robustness ("must not crash"). See `docs/architecture.md`
for the full picture. This phase only lays the foundation.

## Goals / Non-Goals

**Goals:**
- A crate layout that every later phase can slot into without restructuring.
- Mechanical enforcement of the no-panic rule from day one.
- Configuration that can be tested without touching process env.

**Non-Goals:**
- Any HTTP, DB, or import logic (phases 3 to 7).
- Docker image for the app itself (phase 7).

## Decisions

### Library + binary crate
`src/lib.rs` exposes the modules and `src/main.rs` is a thin entrypoint.
*Alternative:* binary only. Rejected because integration tests in `tests/`
can only import from a library target.

### PostgreSQL over SQLite
Postgres runs via `docker compose up -d db`. `#[sqlx::test]` gives each test a
fresh isolated database.
*Alternative:* SQLite (zero setup for the reviewer). Rejected because a real
server DB with `TIMESTAMPTZ` and transactions is closer to production, and
Docker is a reasonable reviewer prerequisite. **Open for review.**

### sqlx runtime queries (`query_as`) instead of compile-time macros (`query!`)
Macros need `DATABASE_URL` or an offline `.sqlx` cache at build time. That
adds friction for the reviewer (`cargo build` would fail without a DB).
Runtime queries are covered by integration tests instead.

### Config loader takes a lookup closure
`Config::from_lookup(|key| std::env::var(key).ok())` in production and a
`HashMap` in tests. This avoids `unsafe { std::env::set_var }` (unsafe in
edition 2024) and flaky tests running in parallel.

### Lints in `Cargo.toml` `[lints.clippy]`
`unwrap_used`, `expect_used`, and `panic` set to `deny`. Tests are exempt
through `clippy.toml` (`allow-unwrap-in-tests`, `allow-expect-in-tests`,
`allow-panic-in-tests` = true). In tests, a panic is the correct way to fail.

### Error handling: `thiserror` for library errors, `anyhow` only in `main`
Typed errors let callers match on failure kinds (the importer needs to tell
source errors from storage errors). `anyhow` stays at the binary boundary only.

## Risks / Trade-offs

- [Pinning the toolchain may force reviewers to download it] → Pinned to the
  current stable (1.92), which rustup installs automatically.
- [Denying `panic` also flags `unreachable!`/`todo!`] → Intended. Those
  shouldn't ship.
