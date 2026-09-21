//! Lightning Network node rankings service.
//!
//! Periodically imports node data from mempool.space into PostgreSQL and
//! serves it through a JSON API. See `docs/architecture.md` for an overview.

pub mod config;
