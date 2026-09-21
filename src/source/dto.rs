//! Wire format of the mempool.space rankings endpoint and its validation.

use serde::Deserialize;
use serde_json::Value;

use crate::domain::{self, Node};

/// One record from `/api/v1/lightning/nodes/rankings/connectivity`.
///
/// Only the fields the service needs are mapped; everything else (`channels`,
/// `city`, `country`, ...) is ignored so changes there can't break parsing.
/// Numbers are `i64` to mirror the wire format: a negative value is parsed and
/// then rejected with a clear message instead of a serde error.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MempoolNodeDto {
    pub public_key: String,
    pub alias: String,
    pub capacity: i64,
    pub first_seen: i64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("public key is empty")]
    EmptyPublicKey,
    #[error("capacity {0} is negative")]
    NegativeCapacity(i64),
    #[error("firstSeen {0} is not a valid timestamp")]
    InvalidFirstSeen(i64),
}

impl TryFrom<MempoolNodeDto> for Node {
    type Error = ValidationError;

    fn try_from(dto: MempoolNodeDto) -> Result<Self, Self::Error> {
        if dto.public_key.is_empty() {
            return Err(ValidationError::EmptyPublicKey);
        }
        let capacity_sats = u64::try_from(dto.capacity)
            .map_err(|_| ValidationError::NegativeCapacity(dto.capacity))?;
        let first_seen = domain::timestamp_from_unix(dto.first_seen)
            .map_err(|_| ValidationError::InvalidFirstSeen(dto.first_seen))?;

        Ok(Node {
            public_key: dto.public_key,
            alias: dto.alias,
            capacity_sats,
            first_seen,
        })
    }
}

/// Turns the raw JSON records into validated nodes, preserving order.
///
/// Each record is handled on its own: one malformed or invalid record is
/// logged and skipped rather than failing the whole import.
pub fn parse_records(records: Vec<Value>) -> Vec<Node> {
    records
        .into_iter()
        .enumerate()
        .filter_map(|(index, record)| {
            let dto = match serde_json::from_value::<MempoolNodeDto>(record) {
                Ok(dto) => dto,
                Err(err) => {
                    tracing::warn!(index, error = %err, "skipping malformed node record");
                    return None;
                }
            };
            let public_key = dto.public_key.clone();
            match Node::try_from(dto) {
                Ok(node) => Some(node),
                Err(err) => {
                    tracing::warn!(index, %public_key, error = %err, "skipping invalid node record");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ACINQ_KEY: &str = "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f";
    const WOS_KEY: &str = "035e4ff418fc8b5554c5d9eea66396c227bd429a3251c8cbc711002ba215bfc226";

    /// The two-record example from the challenge statement.
    fn challenge_example() -> Value {
        json!([
            {
                "publicKey": ACINQ_KEY,
                "alias": "ACINQ",
                "channels": 2908,
                "capacity": 36010516297_i64,
                "firstSeen": 1522941222,
                "updatedAt": 1661274935,
                "city": null,
                "country": {
                    "de": "Vereinigte Staaten", "en": "United States", "es": "Estados Unidos",
                    "fr": "États Unis", "ja": "アメリカ", "pt-BR": "EUA", "ru": "США", "zh-CN": "美国"
                }
            },
            {
                "publicKey": WOS_KEY,
                "alias": "WalletOfSatoshi.com",
                "channels": 2772,
                "capacity": 15464503162_i64,
                "firstSeen": 1601429940,
                "updatedAt": 1661812116,
                "city": {
                    "de": "Vancouver", "en": "Vancouver", "es": "Vancouver", "fr": "Vancouver",
                    "ja": "バンクーバー市", "pt-BR": "Vancôver", "ru": "Ванкувер"
                },
                "country": {
                    "de": "Kanada", "en": "Canada", "es": "Canadá", "fr": "Canada",
                    "ja": "カナダ", "pt-BR": "Canadá", "ru": "Канада", "zh-CN": "加拿大"
                }
            }
        ])
    }

    fn records(value: Value) -> Vec<Value> {
        serde_json::from_value(value).unwrap()
    }

    fn dto(public_key: &str, capacity: i64, first_seen: i64) -> MempoolNodeDto {
        MempoolNodeDto {
            public_key: public_key.to_owned(),
            alias: "alias".to_owned(),
            capacity,
            first_seen,
        }
    }

    #[test]
    fn parses_challenge_example_dtos() {
        let dtos: Vec<MempoolNodeDto> = serde_json::from_value(challenge_example()).unwrap();
        assert_eq!(
            dtos,
            vec![
                MempoolNodeDto {
                    public_key: ACINQ_KEY.to_owned(),
                    alias: "ACINQ".to_owned(),
                    capacity: 36_010_516_297,
                    first_seen: 1_522_941_222,
                },
                MempoolNodeDto {
                    public_key: WOS_KEY.to_owned(),
                    alias: "WalletOfSatoshi.com".to_owned(),
                    capacity: 15_464_503_162,
                    first_seen: 1_601_429_940,
                },
            ]
        );
    }

    #[test]
    fn ignores_unknown_and_null_fields() {
        let record = json!({
            "publicKey": "02ab", "alias": "x", "capacity": 1, "firstSeen": 0,
            "city": null, "country": null, "iso_code": null, "brandNewField": [1, 2, 3]
        });
        assert!(serde_json::from_value::<MempoolNodeDto>(record).is_ok());
    }

    #[test]
    fn fails_on_missing_required_field() {
        let record = json!({ "publicKey": "02ab", "alias": "x", "firstSeen": 0 });
        assert!(serde_json::from_value::<MempoolNodeDto>(record).is_err());
    }

    #[test]
    fn fails_on_wrong_field_type() {
        let record = json!({ "publicKey": "02ab", "alias": "x", "capacity": "1", "firstSeen": 0 });
        assert!(serde_json::from_value::<MempoolNodeDto>(record).is_err());
    }

    #[test]
    fn converts_valid_dto_into_node() {
        let node = Node::try_from(dto(ACINQ_KEY, 36_010_516_297, 1_522_941_222)).unwrap();
        assert_eq!(node.public_key, ACINQ_KEY);
        assert_eq!(node.alias, "alias");
        assert_eq!(node.capacity_sats, 36_010_516_297);
        assert_eq!(node.first_seen.timestamp(), 1_522_941_222);
    }

    #[test]
    fn accepts_zero_capacity() {
        assert_eq!(Node::try_from(dto("02ab", 0, 0)).unwrap().capacity_sats, 0);
    }

    #[test]
    fn rejects_negative_capacity() {
        assert_eq!(
            Node::try_from(dto("02ab", -5, 0)),
            Err(ValidationError::NegativeCapacity(-5))
        );
    }

    #[test]
    fn rejects_out_of_range_first_seen() {
        assert_eq!(
            Node::try_from(dto("02ab", 1, i64::MAX)),
            Err(ValidationError::InvalidFirstSeen(i64::MAX))
        );
    }

    #[test]
    fn rejects_empty_public_key() {
        assert_eq!(
            Node::try_from(dto("", 1, 0)),
            Err(ValidationError::EmptyPublicKey)
        );
    }

    #[test]
    fn parse_records_keeps_valid_records_in_order() {
        let nodes = parse_records(records(challenge_example()));
        let keys: Vec<_> = nodes.iter().map(|n| n.public_key.as_str()).collect();
        assert_eq!(keys, [ACINQ_KEY, WOS_KEY]);
    }

    #[test]
    fn parse_records_skips_invalid_and_malformed_records() {
        let input = json!([
            { "publicKey": "a", "alias": "ok-1", "capacity": 1, "firstSeen": 0 },
            { "publicKey": "b", "alias": "negative", "capacity": -5, "firstSeen": 0 },
            { "publicKey": "c", "alias": "missing capacity", "firstSeen": 0 },
            { "publicKey": "d", "alias": "ok-2", "capacity": 2, "firstSeen": 0 },
            { "publicKey": "", "alias": "empty key", "capacity": 3, "firstSeen": 0 },
            { "publicKey": "f", "alias": "bad ts", "capacity": 4, "firstSeen": i64::MAX },
            "not an object",
            { "publicKey": "g", "alias": "ok-3", "capacity": 5, "firstSeen": 0 }
        ]);

        let aliases: Vec<_> = parse_records(records(input))
            .into_iter()
            .map(|n| n.alias)
            .collect();
        assert_eq!(aliases, ["ok-1", "ok-2", "ok-3"]);
    }

    #[test]
    fn parse_records_of_empty_input_is_empty() {
        assert!(parse_records(Vec::new()).is_empty());
    }
}
