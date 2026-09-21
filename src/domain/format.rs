//! Conversions between raw source values and their presentation.

use chrono::{DateTime, SecondsFormat, Utc};

use super::DomainError;

const SATS_PER_BTC: u64 = 100_000_000;

/// Formats a capacity in satoshis as a BTC amount with exactly 8 decimals.
///
/// Uses integer division/remainder instead of floating point, so the result is
/// exact for every `u64` (an `f64` loses precision above 2^53). The output
/// follows the challenge's JSON example: `.` separator, no unit suffix,
/// e.g. `36_010_516_297` -> `"360.10516297"`.
pub fn format_btc(sats: u64) -> String {
    format!("{}.{:08}", sats / SATS_PER_BTC, sats % SATS_PER_BTC)
}

/// Converts unix seconds into a UTC instant.
///
/// Returns an error instead of panicking when the value is outside chrono's
/// representable range.
pub fn timestamp_from_unix(secs: i64) -> Result<DateTime<Utc>, DomainError> {
    DateTime::from_timestamp(secs, 0).ok_or(DomainError::InvalidTimestamp(secs))
}

/// Formats an instant as RFC 3339 in UTC with second precision and a `Z`
/// suffix, e.g. `"2018-04-05T15:13:42Z"`.
pub fn format_timestamp(instant: &DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn challenge_example_acinq() {
        assert_eq!(format_btc(36_010_516_297), "360.10516297");
    }

    #[test]
    fn challenge_example_wallet_of_satoshi() {
        assert_eq!(format_btc(15_464_503_162), "154.64503162");
    }

    #[test]
    fn requirement_text_example() {
        assert_eq!(format_btc(550_000), "0.00550000");
    }

    #[test]
    fn zero() {
        assert_eq!(format_btc(0), "0.00000000");
    }

    #[test]
    fn one_sat() {
        assert_eq!(format_btc(1), "0.00000001");
    }

    #[test]
    fn one_sat_below_one_bitcoin() {
        assert_eq!(format_btc(99_999_999), "0.99999999");
    }

    #[test]
    fn exactly_one_bitcoin() {
        assert_eq!(format_btc(100_000_000), "1.00000000");
    }

    #[test]
    fn total_bitcoin_supply() {
        assert_eq!(format_btc(2_100_000_000_000_000), "21000000.00000000");
    }

    #[test]
    fn max_value_does_not_overflow() {
        assert_eq!(format_btc(u64::MAX), "184467440737.09551615");
    }

    fn format_unix(secs: i64) -> String {
        format_timestamp(&timestamp_from_unix(secs).unwrap())
    }

    #[test]
    fn timestamp_challenge_example_acinq() {
        assert_eq!(format_unix(1_522_941_222), "2018-04-05T15:13:42Z");
    }

    #[test]
    fn timestamp_challenge_example_wallet_of_satoshi() {
        assert_eq!(format_unix(1_601_429_940), "2020-09-30T01:39:00Z");
    }

    #[test]
    fn timestamp_unix_epoch() {
        assert_eq!(format_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn timestamp_leap_day() {
        assert_eq!(format_unix(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn converts_valid_timestamp() {
        assert_eq!(
            timestamp_from_unix(1_522_941_222),
            Ok(Utc.with_ymd_and_hms(2018, 4, 5, 15, 13, 42).unwrap())
        );
    }

    #[test]
    fn converts_negative_timestamp() {
        assert_eq!(
            timestamp_from_unix(-1),
            Ok(Utc.with_ymd_and_hms(1969, 12, 31, 23, 59, 59).unwrap())
        );
    }

    #[test]
    fn rejects_timestamp_above_range() {
        assert_eq!(
            timestamp_from_unix(i64::MAX),
            Err(DomainError::InvalidTimestamp(i64::MAX))
        );
    }

    #[test]
    fn rejects_timestamp_below_range() {
        assert_eq!(
            timestamp_from_unix(i64::MIN),
            Err(DomainError::InvalidTimestamp(i64::MIN))
        );
    }

    #[test]
    fn formatted_timestamp_round_trips() {
        for secs in [
            0,
            1,
            1_522_941_222,
            1_601_429_940,
            1_709_164_800,
            4_102_444_800,
        ] {
            let formatted = format_unix(secs);
            let parsed = DateTime::parse_from_rfc3339(&formatted).unwrap();
            assert_eq!(parsed.timestamp(), secs, "{formatted}");
        }
    }
}
