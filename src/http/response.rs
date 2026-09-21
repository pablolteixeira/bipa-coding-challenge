//! Public JSON shapes of the API.

use serde::Serialize;

use crate::domain::{Node, format_btc, format_timestamp};

/// One element of the `GET /nodes` response.
///
/// Kept separate from [`Node`] so presentation (strings, field names) never
/// leaks into the domain, and this is the single place formatting happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeResponse {
    pub public_key: String,
    pub alias: String,
    /// BTC with 8 decimals, e.g. `"360.10516297"`.
    pub capacity: String,
    /// RFC 3339 UTC, e.g. `"2018-04-05T15:13:42Z"`.
    pub first_seen: String,
}

impl From<&Node> for NodeResponse {
    fn from(node: &Node) -> Self {
        Self {
            public_key: node.public_key.clone(),
            alias: node.alias.clone(),
            capacity: format_btc(node.capacity_sats),
            first_seen: format_timestamp(&node.first_seen),
        }
    }
}

/// Body of every error response: `{"error": "..."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::node;

    #[test]
    fn formats_challenge_example_acinq() {
        let acinq = node(
            "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
            "ACINQ",
            36_010_516_297,
            1_522_941_222,
        );
        assert_eq!(
            NodeResponse::from(&acinq),
            NodeResponse {
                public_key: "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f"
                    .to_owned(),
                alias: "ACINQ".to_owned(),
                capacity: "360.10516297".to_owned(),
                first_seen: "2018-04-05T15:13:42Z".to_owned(),
            }
        );
    }

    #[test]
    fn formats_challenge_example_wallet_of_satoshi() {
        let wos = node("035e", "WalletOfSatoshi.com", 15_464_503_162, 1_601_429_940);
        let response = NodeResponse::from(&wos);
        assert_eq!(response.capacity, "154.64503162");
        assert_eq!(response.first_seen, "2020-09-30T01:39:00Z");
    }

    #[test]
    fn serializes_with_snake_case_field_names() {
        let json = serde_json::to_value(NodeResponse::from(&node("k", "a", 1, 0))).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "public_key": "k",
                "alias": "a",
                "capacity": "0.00000001",
                "first_seen": "1970-01-01T00:00:00Z"
            })
        );
    }
}
