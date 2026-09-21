//! Where node data comes from.
//!
//! The importer depends on the [`NodeSource`] trait only, so it can be tested
//! with an in-memory fake instead of the real HTTP client.

mod dto;
mod mempool;

use std::future::Future;

use crate::domain::Node;

pub use mempool::MempoolClient;

/// A provider of the current node ranking.
pub trait NodeSource {
    /// Fetches the ranking, in source order. Invalid records are skipped; only
    /// failures of the request as a whole are returned as errors.
    fn fetch_nodes(&self) -> impl Future<Output = Result<Vec<Node>, SourceError>> + Send;
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Network failure, timeout, or client misconfiguration.
    #[error("request to node source failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("node source responded with HTTP status {0}")]
    Status(u16),
    /// The body was not a JSON array.
    #[error("node source returned an invalid body: {0}")]
    Decode(#[from] serde_json::Error),
}
