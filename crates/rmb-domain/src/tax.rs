//! Tax calculation — exact, per-line, multi-rate.
//!
//! Implements the rule grounded in RESEARCH (matches Xero/Stripe behaviour):
//! - round **per line, then sum** (never tax the grand total);
//! - tax-**exclusive**: `tax = round_half_up(net × rate)`, `gross = net + tax`;
//! - tax-**inclusive**: derive `net = round_half_up(gross / (1 + rate))`, then `tax = gross − net`
//!   (by subtraction, so net + tax always reconciles to the entered gross — no penny leak);
//! - **group lines by rate** to produce the per-rate tax summary required on VAT/GST invoices.
//!
//! `rust_decimal` is used only for the intermediate multiply/divide+round; all stored values
//! are exact [`Money`] minor units.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::money::Money;

/// A tax rate. `rate_bp` is basis points (1% = 100 bp, 20% = 2000 bp) for exact representation
/// without floating point. `inclusive` means line prices already include this tax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxRate {
    pub name: String,
    pub rate_bp: i64,
    pub inclusive: bool,
}

impl TaxRate {
    pub fn new(name: impl Into<String>, rate_bp: i64, inclusive: bool) -> Self {
        Self {
            name: name.into(),
            rate_bp,
            inclusive,
        }
    }

    /// A zero / "No Tax" rate.
    pub fn none(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rate_bp: 0,
            inclusive: false,
        }
    }
}

/// Net / tax / gross breakdown of a single line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineTax {
    pub net: Money,
    pub tax: Money,
    pub gross: Money,
}

/// One line's taxable amount + its rate. `amount` is the line's net (exclusive) or gross
/// (inclusive) amount, per `rate.inclusive`.
#[derive(Debug, Clone)]
pub struct TaxLine {
    pub amount: Money,
    pub rate: TaxRate,
}

/// One row of the tax summary (one per distinct rate present on the document).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateSummary {
    pub rate_name: String,
    pub rate_bp: i64,
    pub inclusive: bool,
    pub taxable_net: Money,
    pub tax: Money,
}

/// Document totals + the grouped tax summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentTotals {
    /// Sum of line nets.
    pub subtotal: Money,
    /// Sum of line taxes.
    pub tax_total: Money,
    /// `subtotal + tax_total` (== sum of line grosses).
    pub total: Money,
    pub tax_summary: Vec<RateSummary>,
}

pub(crate) fn round_half_up(d: Decimal) -> i64 {
    d.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .expect("tax computation overflow")
}

/// Compute net/tax/gross for a single line.
pub fn line_tax(amount: Money, rate: &TaxRate) -> LineTax {
    let bp = Decimal::from(rate.rate_bp);
    let ten_k = Decimal::from(10_000);
    if rate.inclusive {
        let gross = amount;
        let net_minor = round_half_up(Decimal::from(gross.minor()) * ten_k / (ten_k + bp));
        let net = Money::from_minor(net_minor);
        let tax = gross - net; // by subtraction → always reconciles
        LineTax { net, tax, gross }
    } else {
        let net = amount;
        let tax_minor = round_half_up(Decimal::from(net.minor()) * bp / ten_k);
        let tax = Money::from_minor(tax_minor);
        let gross = net + tax;
        LineTax { net, tax, gross }
    }
}

/// Compute document totals from lines: round per line, then sum; group by rate (first-seen order).
pub fn compute_document(lines: &[TaxLine]) -> DocumentTotals {
    let mut subtotal = Money::ZERO;
    let mut tax_total = Money::ZERO;
    let mut total = Money::ZERO;
    let mut tax_summary: Vec<RateSummary> = Vec::new();

    for line in lines {
        let lt = line_tax(line.amount, &line.rate);
        subtotal += lt.net;
        tax_total += lt.tax;
        total += lt.gross;

        match tax_summary.iter_mut().find(|r| {
            r.rate_bp == line.rate.rate_bp
                && r.inclusive == line.rate.inclusive
                && r.rate_name == line.rate.name
        }) {
            Some(row) => {
                row.taxable_net += lt.net;
                row.tax += lt.tax;
            }
            None => tax_summary.push(RateSummary {
                rate_name: line.rate.name.clone(),
                rate_bp: line.rate.rate_bp,
                inclusive: line.rate.inclusive,
                taxable_net: lt.net,
                tax: lt.tax,
            }),
        }
    }

    DocumentTotals {
        subtotal,
        tax_total,
        total,
        tax_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn m(x: i64) -> Money {
        Money::from_minor(x)
    }

    // ---- Golden cases (hand-verified) ----

    #[test]
    fn exclusive_uk_vat_20() {
        let lt = line_tax(m(10000), &TaxRate::new("VAT 20%", 2000, false));
        assert_eq!(lt.net, m(10000));
        assert_eq!(lt.tax, m(2000));
        assert_eq!(lt.gross, m(12000));
    }

    #[test]
    fn inclusive_nz_gst_15_reconciles() {
        // $115.00 inclusive @ 15% → net 100.00, tax 15.00.
        let lt = line_tax(m(11500), &TaxRate::new("GST 15%", 1500, true));
        assert_eq!(lt.net, m(10000));
        assert_eq!(lt.tax, m(1500));
        assert_eq!(lt.gross, m(11500));
        assert_eq!(lt.net + lt.tax, lt.gross);
    }

    #[test]
    fn rounds_half_up_per_line() {
        // $1.00 @ 12.5% = $0.125 → rounds up to $0.13.
        let lt = line_tax(m(100), &TaxRate::new("X 12.5%", 1250, false));
        assert_eq!(lt.tax, m(13));
    }

    #[test]
    fn no_tax_rate_passes_through() {
        let lt = line_tax(m(9999), &TaxRate::none("No Tax"));
        assert_eq!(lt.tax, m(0));
        assert_eq!(lt.net, m(9999));
        assert_eq!(lt.gross, m(9999));
    }

    #[test]
    fn multi_rate_document_groups_and_reconciles() {
        let lines = vec![
            TaxLine {
                amount: m(10000),
                rate: TaxRate::new("VAT 20%", 2000, false),
            },
            TaxLine {
                amount: m(5000),
                rate: TaxRate::new("VAT 20%", 2000, false),
            },
            TaxLine {
                amount: m(2000),
                rate: TaxRate::none("No Tax"),
            },
        ];
        let t = compute_document(&lines);
        assert_eq!(t.subtotal, m(17000));
        assert_eq!(t.tax_total, m(3000));
        assert_eq!(t.total, m(20000));
        assert_eq!(t.subtotal + t.tax_total, t.total);
        assert_eq!(t.tax_summary.len(), 2);
        assert_eq!(t.tax_summary[0].taxable_net, m(15000)); // VAT group combined
        assert_eq!(t.tax_summary[0].tax, m(3000));
        assert_eq!(t.tax_summary[1].tax, m(0)); // No Tax group
    }

    #[test]
    fn per_line_rounding_beats_total_level() {
        // Two $1.00 lines @ 12.5%: per-line 0.13 + 0.13 = 0.26 (NOT 0.25 from total-level).
        let rate = TaxRate::new("X 12.5%", 1250, false);
        let lines = vec![
            TaxLine {
                amount: m(100),
                rate: rate.clone(),
            },
            TaxLine {
                amount: m(100),
                rate: rate.clone(),
            },
        ];
        assert_eq!(compute_document(&lines).tax_total, m(26));
    }

    // ---- Property tests (invariants) ----

    proptest! {
        #[test]
        fn total_equals_subtotal_plus_tax(
            amounts in proptest::collection::vec(1i64..100_000_000, 0..20),
            bp in 0i64..3000,
            inclusive in any::<bool>(),
        ) {
            let rate = TaxRate::new("R", bp, inclusive);
            let lines: Vec<TaxLine> =
                amounts.iter().map(|&a| TaxLine { amount: m(a), rate: rate.clone() }).collect();
            let t = compute_document(&lines);
            prop_assert_eq!(t.subtotal + t.tax_total, t.total);
        }

        #[test]
        fn inclusive_line_reconciles_to_entered_amount(amount in 1i64..100_000_000, bp in 1i64..3000) {
            let lt = line_tax(m(amount), &TaxRate::new("R", bp, true));
            prop_assert_eq!(lt.net + lt.tax, lt.gross);
            prop_assert_eq!(lt.gross, m(amount));
        }

        #[test]
        fn exclusive_line_gross_is_net_plus_tax(amount in 1i64..100_000_000, bp in 0i64..3000) {
            let lt = line_tax(m(amount), &TaxRate::new("R", bp, false));
            prop_assert_eq!(lt.net, m(amount));
            prop_assert_eq!(lt.net + lt.tax, lt.gross);
        }
    }
}
