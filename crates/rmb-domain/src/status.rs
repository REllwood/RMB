//! Document status state machines + payment-derived status and balance.
//!
//! Legal transitions are enforced here (the data layer calls `can_transition_to` before
//! persisting a status change). Issued invoices are immutable: the only ways out are payment
//! (Issued → PartPaid → Paid) or Void.

use serde::{Deserialize, Serialize};

use crate::money::Money;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    Draft,
    Sent,
    Accepted,
    Declined,
    Expired,
    Converted,
}

impl QuoteStatus {
    pub fn can_transition_to(self, to: QuoteStatus) -> bool {
        use QuoteStatus::*;
        matches!(
            (self, to),
            (Draft, Sent)
                | (Draft, Accepted)
                | (Draft, Declined)
                | (Sent, Draft)
                | (Sent, Accepted)
                | (Sent, Declined)
                | (Sent, Expired)
                | (Accepted, Converted)
                | (Accepted, Declined)
        )
    }

    pub fn is_terminal(self) -> bool {
        use QuoteStatus::*;
        matches!(self, Declined | Expired | Converted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Draft,
    Issued,
    PartPaid,
    Paid,
    Void,
}

impl InvoiceStatus {
    pub fn can_transition_to(self, to: InvoiceStatus) -> bool {
        use InvoiceStatus::*;
        matches!(
            (self, to),
            (Draft, Issued)
                | (Issued, PartPaid)
                | (Issued, Paid)
                | (Issued, Void)
                | (PartPaid, Paid)
                | (PartPaid, Void)
        )
    }

    pub fn is_editable(self) -> bool {
        // Only drafts may be edited; issued invoices are immutable (void + reissue to change).
        matches!(self, InvoiceStatus::Draft)
    }
}

/// Derive the payment-based status of an *issued* invoice from its total and amount paid.
/// (Draft and Void are explicit states handled before calling this.)
pub fn payment_status(total: Money, paid: Money) -> InvoiceStatus {
    if !paid.is_positive() {
        InvoiceStatus::Issued
    } else if paid >= total {
        InvoiceStatus::Paid
    } else {
        InvoiceStatus::PartPaid
    }
}

/// Outstanding balance = total − paid.
pub fn outstanding(total: Money, paid: Money) -> Money {
    total - paid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_legal_transitions() {
        assert!(QuoteStatus::Draft.can_transition_to(QuoteStatus::Sent));
        assert!(QuoteStatus::Sent.can_transition_to(QuoteStatus::Accepted));
        assert!(QuoteStatus::Accepted.can_transition_to(QuoteStatus::Converted));
        assert!(!QuoteStatus::Declined.can_transition_to(QuoteStatus::Sent));
        assert!(QuoteStatus::Converted.is_terminal());
    }

    #[test]
    fn invoice_legal_transitions() {
        assert!(InvoiceStatus::Draft.can_transition_to(InvoiceStatus::Issued));
        assert!(InvoiceStatus::Issued.can_transition_to(InvoiceStatus::Void));
        assert!(!InvoiceStatus::Paid.can_transition_to(InvoiceStatus::Issued));
        assert!(!InvoiceStatus::Issued.can_transition_to(InvoiceStatus::Draft));
        assert!(InvoiceStatus::Draft.is_editable());
        assert!(!InvoiceStatus::Issued.is_editable());
    }

    #[test]
    fn payment_derived_status_and_balance() {
        let total = Money::from_minor(10000);
        assert_eq!(payment_status(total, Money::ZERO), InvoiceStatus::Issued);
        assert_eq!(
            payment_status(total, Money::from_minor(5000)),
            InvoiceStatus::PartPaid
        );
        assert_eq!(
            payment_status(total, Money::from_minor(10000)),
            InvoiceStatus::Paid
        );
        assert_eq!(
            payment_status(total, Money::from_minor(12000)),
            InvoiceStatus::Paid
        );
        assert_eq!(
            outstanding(total, Money::from_minor(3000)),
            Money::from_minor(7000)
        );
    }
}
