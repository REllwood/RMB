//! Money as integer **minor units** (e.g. cents) — the single source of truth for all
//! monetary values in RMB. No floating point: arithmetic is exact. The active currency and
//! its minor-unit scale live in Settings; the frontend only *formats* (never computes).
//!
//! Overflow is treated as a bug: `i64` minor units cover ±92 trillion major units, far beyond
//! any small-business figure, so the operator traits panic on overflow while `checked_*`
//! variants are available where a fallible path is genuinely needed.

use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// A monetary amount in integer minor units.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    /// Zero money.
    pub const ZERO: Money = Money(0);

    /// Construct from minor units (cents).
    pub const fn from_minor(minor: i64) -> Self {
        Money(minor)
    }

    /// The amount in minor units.
    pub const fn minor(self) -> i64 {
        self.0
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

    /// Checked addition (`None` on overflow).
    pub fn checked_add(self, rhs: Money) -> Option<Money> {
        self.0.checked_add(rhs.0).map(Money)
    }

    /// Checked subtraction (`None` on overflow).
    pub fn checked_sub(self, rhs: Money) -> Option<Money> {
        self.0.checked_sub(rhs.0).map(Money)
    }

    /// Multiply by an integer quantity (e.g. unit price × qty). `None` on overflow.
    pub fn checked_mul(self, qty: i64) -> Option<Money> {
        self.0.checked_mul(qty).map(Money)
    }

    /// Absolute value.
    pub fn abs(self) -> Money {
        Money(self.0.abs())
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        self.checked_add(rhs).expect("money addition overflow")
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        self.checked_sub(rhs).expect("money subtraction overflow")
    }
}

impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(self.0.checked_neg().expect("money negation overflow"))
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        *self = *self + rhs;
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Money) {
        *self = *self - rhs;
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + m)
    }
}

impl<'a> Sum<&'a Money> for Money {
    fn sum<I: Iterator<Item = &'a Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + *m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub_neg() {
        assert_eq!(
            Money::from_minor(150) + Money::from_minor(75),
            Money::from_minor(225)
        );
        assert_eq!(
            Money::from_minor(150) - Money::from_minor(75),
            Money::from_minor(75)
        );
        assert_eq!(-Money::from_minor(150), Money::from_minor(-150));
    }

    #[test]
    fn sum_over_iter() {
        let lines = [
            Money::from_minor(100),
            Money::from_minor(250),
            Money::from_minor(33),
        ];
        let total: Money = lines.iter().copied().sum();
        assert_eq!(total, Money::from_minor(383));
        let total_ref: Money = lines.iter().sum();
        assert_eq!(total_ref, Money::from_minor(383));
    }

    #[test]
    fn checked_arithmetic_guards_overflow() {
        assert_eq!(
            Money::from_minor(i64::MAX).checked_add(Money::from_minor(1)),
            None
        );
        assert_eq!(
            Money::from_minor(i64::MIN).checked_sub(Money::from_minor(1)),
            None
        );
        assert_eq!(Money::from_minor(i64::MAX).checked_mul(2), None);
        assert_eq!(
            Money::from_minor(21).checked_mul(2),
            Some(Money::from_minor(42))
        );
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn operator_panics_on_overflow() {
        let _ = Money::from_minor(i64::MAX) + Money::from_minor(1);
    }

    #[test]
    fn serializes_transparently_as_integer() {
        // The frontend receives a plain integer (minor units) and formats it.
        assert_eq!(
            serde_json::to_string(&Money::from_minor(123456)).unwrap(),
            "123456"
        );
        let parsed: Money = serde_json::from_str("123456").unwrap();
        assert_eq!(parsed, Money::from_minor(123456));
    }

    #[test]
    fn predicates() {
        assert!(Money::ZERO.is_zero());
        assert!(Money::from_minor(-5).is_negative());
        assert!(Money::from_minor(5).is_positive());
        assert_eq!(Money::from_minor(-5).abs(), Money::from_minor(5));
    }
}
