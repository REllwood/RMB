//! The shared quote/invoice/job line model and document-level totals (reusing the tax engine).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::money::Money;
use crate::tax::{compute_document, round_half_up, DocumentTotals, TaxLine, TaxRate};

/// A line on a quote / invoice / job. `quantity` may be fractional (e.g. 2.5 hours);
/// `line_amount = round(unit_price × quantity)`. Whether that amount is net or tax-inclusive
/// is governed by `tax_rate.inclusive`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentLine {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Money,
    pub tax_rate: TaxRate,
}

impl DocumentLine {
    pub fn new(
        description: impl Into<String>,
        quantity: Decimal,
        unit_price: Money,
        tax_rate: TaxRate,
    ) -> Self {
        Self {
            description: description.into(),
            quantity,
            unit_price,
            tax_rate,
        }
    }

    /// The line's taxable amount, `unit_price × quantity` rounded to the minor unit.
    pub fn line_amount(&self) -> Money {
        Money::from_minor(round_half_up(
            Decimal::from(self.unit_price.minor()) * self.quantity,
        ))
    }
}

/// Compute document totals for a set of lines (per-line tax, summed, grouped by rate).
pub fn total_lines(lines: &[DocumentLine]) -> DocumentTotals {
    let tax_lines: Vec<TaxLine> = lines
        .iter()
        .map(|l| TaxLine {
            amount: l.line_amount(),
            rate: l.tax_rate.clone(),
        })
        .collect();
    compute_document(&tax_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_quantity_line_amount() {
        let l = DocumentLine::new(
            "widget",
            Decimal::from(3),
            Money::from_minor(1000),
            TaxRate::new("VAT 20%", 2000, false),
        );
        assert_eq!(l.line_amount(), Money::from_minor(3000));
    }

    #[test]
    fn fractional_quantity_rounds_half_up() {
        // 2.5 × $0.99 = $2.475 → $2.48
        let l = DocumentLine::new(
            "cable (m)",
            Decimal::new(25, 1),
            Money::from_minor(99),
            TaxRate::none("No Tax"),
        );
        assert_eq!(l.line_amount(), Money::from_minor(248));
    }

    #[test]
    fn total_lines_uses_tax_engine() {
        let lines = vec![DocumentLine::new(
            "labour",
            Decimal::from(1),
            Money::from_minor(10000),
            TaxRate::new("VAT 20%", 2000, false),
        )];
        let t = total_lines(&lines);
        assert_eq!(t.subtotal, Money::from_minor(10000));
        assert_eq!(t.tax_total, Money::from_minor(2000));
        assert_eq!(t.total, Money::from_minor(12000));
    }
}
