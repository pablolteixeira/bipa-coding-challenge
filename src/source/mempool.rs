//! [`NodeSource`] backed by the mempool.space REST API.

use std::time::Duration;

use serde_json::Value;
use url::Url;

use super::dto::parse_records;
use super::{NodeSource, SourceError};
use crate::domain::Node;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// HTTP client for the mempool.space connectivity ranking.
///
/// The underlying `reqwest::Client` is built once and reused, so connections
/// are pooled across imports.
#[derive(Debug, Clone)]
pub struct MempoolClient {
    http: reqwest::Client,
    url: Url,
}

impl MempoolClient {
    /// Builds a client for `url` whose requests fail after `timeout`.
    pub fn new(url: Url, timeout: Duration) -> Result<Self, SourceError> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
            .build()?;
        Ok(Self { http, url })
    }
}

impl NodeSource for MempoolClient {
    #[tracing::instrument(name = "fetch_nodes", skip(self), fields(url = %self.url))]
    async fn fetch_nodes(&self) -> Result<Vec<Node>, SourceError> {
        let response = self.http.get(self.url.clone()).send().await?;

        // Check the status before decoding, so a 5xx with an HTML body is
        // reported as a status error rather than a confusing decode error.
        let status = response.status();
        if !status.is_success() {
            return Err(SourceError::Status(status.as_u16()));
        }

        // Decode as generic values first: this only fails if the body is not
        // a JSON array. Individual records are validated one by one below.
        let body = response.bytes().await?;
        let records: Vec<Value> = serde_json::from_slice(&body)?;
        let received = records.len();

        let nodes = parse_records(records);
        tracing::debug!(
            received,
            valid = nodes.len(),
            skipped = received - nodes.len(),
            "fetched node ranking"
        );
        Ok(nodes)
    }
}
