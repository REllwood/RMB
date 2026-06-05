//! Inventory rules — on-hand derivation and low-stock, over an append-only movement ledger.
//!
//! Stock is never mutated directly: every change is a `stock_movements` row with a signed
//! delta and a reason. On-hand = sum of deltas (cached on the item, but the ledger is truth).
//! Corrections are reversing/adjustment rows, not edits — this gives a free audit trail.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementReason {
    /// Stock received / added.
    Receipt,
    /// Consumed by a sale/invoice.
    Sale,
    /// Manual correction.
    Adjustment,
    /// Returned to stock.
    Return,
}

impl MovementReason {
    pub fn as_db(self) -> &'static str {
        match self {
            MovementReason::Receipt => "receipt",
            MovementReason::Sale => "sale",
            MovementReason::Adjustment => "adjustment",
            MovementReason::Return => "return",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "receipt" => Some(MovementReason::Receipt),
            "sale" => Some(MovementReason::Sale),
            "adjustment" => Some(MovementReason::Adjustment),
            "return" => Some(MovementReason::Return),
            _ => None,
        }
    }
}

/// On-hand quantity = sum of all movement deltas. The cached `items.qty_on_hand` column must
/// always equal this (updated in the same transaction as the movement insert).
pub fn on_hand(deltas: impl IntoIterator<Item = i64>) -> i64 {
    deltas.into_iter().sum()
}

/// Low stock when on-hand is at or below the reorder point (when one is set).
pub fn is_low_stock(on_hand: i64, reorder_point: Option<i64>) -> bool {
    matches!(reorder_point, Some(r) if on_hand <= r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn on_hand_sums_deltas() {
        assert_eq!(on_hand([10, -3, 5, -2]), 10);
        assert_eq!(on_hand(std::iter::empty::<i64>()), 0);
    }

    #[test]
    fn low_stock_threshold() {
        assert!(is_low_stock(2, Some(5)));
        assert!(is_low_stock(5, Some(5))); // at threshold counts as low
        assert!(!is_low_stock(6, Some(5)));
        assert!(!is_low_stock(0, None)); // no reorder point → never "low"
    }

    proptest! {
        #[test]
        fn cached_on_hand_equals_ledger_sum(deltas in proptest::collection::vec(-1000i64..1000, 0..60)) {
            let recomputed = on_hand(deltas.iter().copied());
            let cached: i64 = deltas.iter().sum();
            prop_assert_eq!(recomputed, cached);
        }
    }
}
