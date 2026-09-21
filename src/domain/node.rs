use chrono::{DateTime, Utc};

/// A Lightning Network node as the service understands it.
///
/// Values are kept raw and typed (sats, UTC instant). Formatting for the API
/// happens at the edge, so storage stays lossless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub public_key: String,
    pub alias: String,
    /// Total channel capacity in satoshis (1 BTC = 100,000,000 sats).
    pub capacity_sats: u64,
    pub first_seen: DateTime<Utc>,
}
