//! Presentation conversions used by the HTTP API.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
