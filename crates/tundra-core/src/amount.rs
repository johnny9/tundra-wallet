use crate::{Error, Result};
pub const MAX_SATS: u64 = 21_000_000 * 100_000_000;

/// Parse user-entered BTC without binary floating point or silent rounding.
pub fn parse_btc(input: &str) -> Result<u64> {
    let s = input.trim();
    if s.is_empty() || s.len() > 32 {
        return Err(Error::InvalidInput("enter a BTC amount"));
    }
    let mut parts = s.split('.');
    let whole = parts.next().ok_or(Error::InvalidInput("amount"))?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 8
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (s.contains('.') && fraction.is_empty())
    {
        return Err(Error::InvalidInput(
            "use BTC with at most eight decimal places",
        ));
    }
    let n = whole
        .parse::<u64>()
        .map_err(|_| Error::InvalidInput("amount overflow"))?;
    let f = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u64>()
            .map_err(|_| Error::InvalidInput("amount"))?
            * 10_u64.pow(8 - fraction.len() as u32)
    };
    let result = n
        .checked_mul(100_000_000)
        .and_then(|n| n.checked_add(f))
        .filter(|n| *n <= MAX_SATS)
        .ok_or(Error::InvalidInput("amount exceeds Bitcoin supply"))?;
    Ok(result)
}

pub fn format_btc(sats: u64) -> String {
    let whole = sats / 100_000_000;
    let fraction = sats % 100_000_000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{}", format!("{fraction:08}").trim_end_matches('0'))
    }
}

/// Decimal sat/vB input, rounded upward to BDK's exact integer sat/kwu precision.
/// The normalized rate is included in the immutable review. No floating point.
pub fn parse_fee_rate(input: &str) -> Result<u64> {
    let sats = parse_btc(input)?;
    if sats == 0 || sats > 1000 * 100_000_000 {
        return Err(Error::InvalidInput(
            "fee rate must be greater than zero and at most 1000 sat/vB",
        ));
    }
    Ok(sats.div_ceil(400_000))
}
pub fn format_fee_rate(sat_per_kwu: u64) -> String {
    format_btc(sat_per_kwu.saturating_mul(400_000))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_satoshi() {
        assert_eq!(parse_btc("0.00000001").unwrap(), 1);
    }
    #[test]
    fn fee_rates_are_precise_and_never_round_down() {
        for (text, units, display) in [
            ("0.1", 25, "0.1"),
            ("1.001", 251, "1.004"),
            ("2.5", 625, "2.5"),
            ("1000", 250_000, "1000"),
        ] {
            assert_eq!(parse_fee_rate(text).unwrap(), units);
            assert_eq!(format_fee_rate(units), display);
        }
        for text in ["0", "1000.1", "-1", "1e2", "1,1"] {
            assert!(parse_fee_rate(text).is_err());
        }
    }
    #[test]
    fn rejects_sub_satoshi() {
        assert!(parse_btc("0.000000001").is_err());
    }
    #[test]
    fn rejects_exponent() {
        assert!(parse_btc("1e-8").is_err());
    }
    #[test]
    fn rejects_sign() {
        assert!(parse_btc("-1").is_err());
        assert!(parse_btc("+1").is_err());
    }
    #[test]
    fn rejects_overflow() {
        assert!(parse_btc("99999999999999999999").is_err());
    }
    #[test]
    fn supply_cap() {
        assert!(parse_btc("21000000.00000001").is_err());
    }
    #[test]
    fn rejects_locale_separators() {
        assert!(parse_btc("0,1").is_err());
    }
    #[test]
    fn trims_without_rounding() {
        assert_eq!(format_btc(80_000_001), "0.80000001");
    }
    #[test]
    fn roundtrips() {
        for x in [0, 1, 10, 99, 100_000_000, 80_000_001, MAX_SATS] {
            assert_eq!(parse_btc(&format_btc(x)).unwrap(), x);
        }
    }
}
