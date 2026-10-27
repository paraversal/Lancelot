//! Money as integer cents, and the rounding and splitting rules (R-COR-1, R-COR-3).
//!
//! There is no floating point anywhere in this module. Every operation that could overflow
//! returns a [`MoneyError`] instead of wrapping.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An amount in euro cents. Negative values are allowed; the meaning of the sign depends on
/// where the amount is used (see `docs/technical-design.md`, §3.1).
///
/// The absolute value never exceeds [`MAX_AMOUNT`].
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(try_from = "i64", into = "i64")]
pub struct Cents(i64);

/// The largest absolute amount Lancelot accepts: 10 billion euros.
///
/// Far beyond any event, and far below both `i64::MAX` and the 2⁵³ that a JavaScript number
/// can hold exactly, so cents survive the trip to the UI.
pub const MAX_AMOUNT: Cents = Cents(1_000_000_000_000);

/// One percent in basis points.
pub const BP_PER_PERCENT: u32 = 100;

/// 100 % in basis points.
pub const BP_FULL: u32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum MoneyError {
    #[error("amount is out of range")]
    Overflow,
    #[error("cannot split an amount into zero parts")]
    ZeroParts,
}

impl MoneyError {
    /// Stable identifier for the UI.
    pub fn code(self) -> &'static str {
        match self {
            MoneyError::Overflow => "overflow",
            MoneyError::ZeroParts => "zero_parts",
        }
    }
}

impl Cents {
    pub const ZERO: Cents = Cents(0);

    /// Wraps a cent value, rejecting anything beyond [`MAX_AMOUNT`].
    pub fn new(cents: i64) -> Result<Cents, MoneyError> {
        if cents.unsigned_abs() > MAX_AMOUNT.0.unsigned_abs() {
            Err(MoneyError::Overflow)
        } else {
            Ok(Cents(cents))
        }
    }

    pub fn get(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Cents) -> Result<Cents, MoneyError> {
        // Both operands are within MAX_AMOUNT, so the i64 sum cannot overflow.
        Cents::new(self.0 + other.0)
    }

    pub fn checked_sub(self, other: Cents) -> Result<Cents, MoneyError> {
        Cents::new(self.0 - other.0)
    }

    pub fn checked_mul(self, factor: i64) -> Result<Cents, MoneyError> {
        self.0
            .checked_mul(factor)
            .ok_or(MoneyError::Overflow)
            .and_then(Cents::new)
    }

    pub fn abs(self) -> Cents {
        Cents(self.0.abs())
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// −1, 0 or 1.
    pub fn signum(self) -> i64 {
        self.0.signum()
    }
}

impl std::ops::Neg for Cents {
    type Output = Cents;

    /// Never overflows: the range is symmetric.
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}

impl TryFrom<i64> for Cents {
    type Error = MoneyError;

    fn try_from(cents: i64) -> Result<Cents, MoneyError> {
        Cents::new(cents)
    }
}

impl From<Cents> for i64 {
    fn from(c: Cents) -> i64 {
        c.0
    }
}

/// Sums amounts, failing if any intermediate or final result leaves the valid range.
pub fn try_sum<I: IntoIterator<Item = Cents>>(amounts: I) -> Result<Cents, MoneyError> {
    amounts
        .into_iter()
        .try_fold(Cents::ZERO, |acc, c| acc.checked_add(c))
}

/// Splits `total` into `n` parts that differ by at most one cent and sum exactly to `total`.
///
/// The remainder cents go to the first parts: `allocate(10,00 €, 3)` is `3,34 · 3,33 · 3,33`,
/// and `allocate(−0,05 €, 2)` is `−0,03 · −0,02`.
pub fn allocate(total: Cents, n: usize) -> Result<Vec<Cents>, MoneyError> {
    if n == 0 {
        return Err(MoneyError::ZeroParts);
    }
    let n_i64 = i64::try_from(n).map_err(|_| MoneyError::Overflow)?;
    let base = total.0 / n_i64; // truncates towards zero
    let remainder = (total.0 - base * n_i64).unsigned_abs();
    let extra = total.0.signum();
    Ok((0..n)
        .map(|i| {
            if (i as u64) < remainder {
                Cents(base + extra)
            } else {
                Cents(base)
            }
        })
        .collect())
}

/// `amount × bp / 10 000`, rounded half away from zero to the cent.
///
/// For the non-negative amounts where percentages are used (fees, refunds) this is the
/// same as rounding half-up: `percent_bp(25,00 €, 250)` is `0,63 €` (exactly 0,625 €).
pub fn percent_bp(amount: Cents, bp: u32) -> Result<Cents, MoneyError> {
    let scaled = i128::from(amount.0) * i128::from(bp);
    let denom = i128::from(BP_FULL);
    let half = denom / 2;
    let rounded = if scaled >= 0 {
        (scaled + half) / denom
    } else {
        (scaled - half) / denom
    };
    i64::try_from(rounded)
        .map_err(|_| MoneyError::Overflow)
        .and_then(Cents::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn c(cents: i64) -> Cents {
        Cents::new(cents).unwrap()
    }

    #[test]
    fn new_rejects_values_beyond_max() {
        assert_eq!(Cents::new(MAX_AMOUNT.get()), Ok(MAX_AMOUNT));
        assert_eq!(Cents::new(-MAX_AMOUNT.get()), Ok(-MAX_AMOUNT));
        assert_eq!(Cents::new(MAX_AMOUNT.get() + 1), Err(MoneyError::Overflow));
        assert_eq!(Cents::new(i64::MIN), Err(MoneyError::Overflow));
    }

    #[test]
    fn arithmetic_stays_in_range() {
        assert_eq!(c(150).checked_add(c(-200)), Ok(c(-50)));
        assert_eq!(c(150).checked_sub(c(200)), Ok(c(-50)));
        assert_eq!(c(3200).checked_mul(128), Ok(c(409_600)));
        assert_eq!(MAX_AMOUNT.checked_add(c(1)), Err(MoneyError::Overflow));
        assert_eq!(MAX_AMOUNT.checked_mul(2), Err(MoneyError::Overflow));
        assert_eq!(MAX_AMOUNT.checked_mul(i64::MAX), Err(MoneyError::Overflow));
    }

    #[test]
    fn try_sum_adds_and_detects_overflow() {
        assert_eq!(try_sum([c(100), c(250), c(-50)]), Ok(c(300)));
        assert_eq!(try_sum([]), Ok(Cents::ZERO));
        assert_eq!(try_sum([MAX_AMOUNT, c(1)]), Err(MoneyError::Overflow));
    }

    #[test]
    fn allocate_gives_remainder_to_first_parts() {
        assert_eq!(allocate(c(1000), 3), Ok(vec![c(334), c(333), c(333)]));
        assert_eq!(allocate(c(-5), 2), Ok(vec![c(-3), c(-2)]));
        assert_eq!(allocate(c(2), 4), Ok(vec![c(1), c(1), c(0), c(0)]));
        assert_eq!(allocate(c(900), 3), Ok(vec![c(300), c(300), c(300)]));
        assert_eq!(allocate(c(42), 1), Ok(vec![c(42)]));
        assert_eq!(allocate(c(42), 0), Err(MoneyError::ZeroParts));
    }

    #[test]
    fn percent_rounds_half_up() {
        // R-TKT-10 examples: 2.5 % of 25 € and 32 €.
        assert_eq!(percent_bp(c(2500), 250), Ok(c(63)));
        assert_eq!(percent_bp(c(3200), 250), Ok(c(80)));
        // 50 % refund of 25 €.
        assert_eq!(percent_bp(c(2500), 5000), Ok(c(1250)));
        // Exactly half a cent rounds up, just below half rounds down.
        assert_eq!(percent_bp(c(1), 5000), Ok(c(1)));
        assert_eq!(percent_bp(c(1), 4999), Ok(c(0)));
        assert_eq!(percent_bp(c(1999), 0), Ok(c(0)));
        assert_eq!(percent_bp(c(1999), BP_FULL), Ok(c(1999)));
    }

    #[test]
    fn percent_is_symmetric_for_negative_amounts() {
        assert_eq!(percent_bp(c(-2500), 250), Ok(c(-63)));
        assert_eq!(percent_bp(c(-1), 5000), Ok(c(-1)));
    }

    #[test]
    fn percent_above_full_can_overflow() {
        assert_eq!(
            percent_bp(MAX_AMOUNT, 2 * BP_FULL),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn serde_is_a_plain_integer_and_validates() {
        assert_eq!(serde_json::to_string(&c(-1234)).unwrap(), "-1234");
        assert_eq!(serde_json::from_str::<Cents>("1234").unwrap(), c(1234));
        assert!(serde_json::from_str::<Cents>("1000000000001").is_err());
    }

    fn any_cents() -> impl Strategy<Value = Cents> {
        (-MAX_AMOUNT.get()..=MAX_AMOUNT.get()).prop_map(Cents)
    }

    proptest! {
        #[test]
        fn allocate_sums_to_total_and_is_even(total in any_cents(), n in 1usize..50) {
            let parts = allocate(total, n).unwrap();
            prop_assert_eq!(parts.len(), n);
            prop_assert_eq!(try_sum(parts.iter().copied()).unwrap(), total);
            let min = parts.iter().map(|p| p.get()).min().unwrap();
            let max = parts.iter().map(|p| p.get()).max().unwrap();
            prop_assert!(max - min <= 1);
            // Larger parts (in absolute value) come first.
            prop_assert!(parts.windows(2).all(|w| w[0].abs() >= w[1].abs()));
        }

        #[test]
        fn percent_matches_exact_rational_rounding(amount in any_cents(), bp in 0u32..=BP_FULL) {
            // Reference: round |amount × bp| / 10 000 half-up, then restore the sign.
            let exact = i128::from(amount.get()) * i128::from(bp);
            let magnitude = exact.unsigned_abs();
            let mut q = magnitude / 10_000;
            if (magnitude % 10_000) * 2 >= 10_000 {
                q += 1;
            }
            let expected = if exact < 0 { -(q as i128) } else { q as i128 };
            prop_assert_eq!(i128::from(percent_bp(amount, bp).unwrap().get()), expected);
        }

        #[test]
        fn per_ticket_rule_is_independent_of_grouping(price in 0i64..100_000, bp in 0u32..=BP_FULL, a in 0i64..500, b in 0i64..500) {
            // R-TKT-10: fee per ticket × count, so two groups add up to the whole.
            let fee = percent_bp(c(price), bp).unwrap();
            let whole = fee.checked_mul(a + b).unwrap();
            let parts = fee.checked_mul(a).unwrap().checked_add(fee.checked_mul(b).unwrap()).unwrap();
            prop_assert_eq!(whole, parts);
        }
    }
}
