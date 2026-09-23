//! Document status state machines + payment-derived status and balance.
//!
//! Legal transitions are defined here and checked by the data layer before it persists a status
//! change. Issued invoices are immutable: their status only moves with payments (recorded or
//! removed) or to Void.

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

    pub fn as_db(self) -> &'static str {
        match self {
            QuoteStatus::Draft => "draft",
            QuoteStatus::Sent => "sent",
            QuoteStatus::Accepted => "accepted",
            QuoteStatus::Declined => "declined",
            QuoteStatus::Expired => "expired",
            QuoteStatus::Converted => "converted",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(QuoteStatus::Draft),
            "sent" => Some(QuoteStatus::Sent),
            "accepted" => Some(QuoteStatus::Accepted),
            "declined" => Some(QuoteStatus::Declined),
            "expired" => Some(QuoteStatus::Expired),
            "converted" => Some(QuoteStatus::Converted),
            _ => None,
        }
    }
}

/// Lifecycle of a job. `Invoiced` is reached **only** when the system bills the job
/// (`invoice_from_job`); it is never set or unset by hand, because re-opening a billed job would
/// expose its time/materials to double-invoicing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Open,
    InProgress,
    Done,
    Invoiced,
}

impl JobStatus {
    /// Legal **manual** transitions: freely among Open/InProgress/Done. Transitions into or out of
    /// `Invoiced` are system-only and rejected here.
    pub fn can_transition_to(self, to: JobStatus) -> bool {
        use JobStatus::*;
        self != to
            && matches!(self, Open | InProgress | Done)
            && matches!(to, Open | InProgress | Done)
    }

    pub fn as_db(self) -> &'static str {
        match self {
            JobStatus::Open => "open",
            JobStatus::InProgress => "in_progress",
            JobStatus::Done => "done",
            JobStatus::Invoiced => "invoiced",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "open" => Some(JobStatus::Open),
            "in_progress" => Some(JobStatus::InProgress),
            "done" => Some(JobStatus::Done),
            "invoiced" => Some(JobStatus::Invoiced),
            _ => None,
        }
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
    /// Draft → Issued on issue (Paid when the total is zero). Payment-derived statuses move both
    /// ways as payments are recorded or removed. Void needs no payments against the invoice, so it
    /// is reachable from Issued, or from Paid for a zero total.
    pub fn can_transition_to(self, to: InvoiceStatus) -> bool {
        use InvoiceStatus::*;
        matches!(
            (self, to),
            (Draft, Issued)
                | (Draft, Paid)
                | (Issued, PartPaid)
                | (Issued, Paid)
                | (Issued, Void)
                | (PartPaid, Issued)
                | (PartPaid, Paid)
                | (Paid, Issued)
                | (Paid, PartPaid)
                | (Paid, Void)
        )
    }

    pub fn is_editable(self) -> bool {
        // Only drafts may be edited; issued invoices are immutable (void + reissue to change).
        matches!(self, InvoiceStatus::Draft)
    }

    pub fn as_db(self) -> &'static str {
        match self {
            InvoiceStatus::Draft => "draft",
            InvoiceStatus::Issued => "issued",
            InvoiceStatus::PartPaid => "part_paid",
            InvoiceStatus::Paid => "paid",
            InvoiceStatus::Void => "void",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(InvoiceStatus::Draft),
            "issued" => Some(InvoiceStatus::Issued),
            "part_paid" => Some(InvoiceStatus::PartPaid),
            "paid" => Some(InvoiceStatus::Paid),
            "void" => Some(InvoiceStatus::Void),
            _ => None,
        }
    }
}

/// Derive the payment-based status of an *issued* invoice from its total and amount paid.
/// (Draft and Void are explicit states handled before calling this.)
pub fn payment_status(total: Money, paid: Money) -> InvoiceStatus {
    if !total.is_positive() {
        // A zero (or non-positive) total is considered settled on issue.
        InvoiceStatus::Paid
    } else if !paid.is_positive() {
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
        assert!(InvoiceStatus::Draft.can_transition_to(InvoiceStatus::Paid)); // zero total
        assert!(InvoiceStatus::Issued.can_transition_to(InvoiceStatus::Void));
        assert!(InvoiceStatus::Paid.can_transition_to(InvoiceStatus::Issued)); // payment removed
        assert!(!InvoiceStatus::PartPaid.can_transition_to(InvoiceStatus::Void)); // has payments
        assert!(!InvoiceStatus::Issued.can_transition_to(InvoiceStatus::Draft));
        assert!(!InvoiceStatus::Void.can_transition_to(InvoiceStatus::Issued));
        assert!(InvoiceStatus::Draft.is_editable());
        assert!(!InvoiceStatus::Issued.is_editable());
    }

    #[test]
    fn job_status_transitions() {
        assert!(JobStatus::Open.can_transition_to(JobStatus::InProgress));
        assert!(JobStatus::InProgress.can_transition_to(JobStatus::Done));
        assert!(JobStatus::Done.can_transition_to(JobStatus::Open));
        // Invoiced is system-only: not reachable or leavable by hand.
        assert!(!JobStatus::Done.can_transition_to(JobStatus::Invoiced));
        assert!(!JobStatus::Invoiced.can_transition_to(JobStatus::Open));
        assert!(!JobStatus::Open.can_transition_to(JobStatus::Open)); // no-op rejected
        assert_eq!(JobStatus::from_db("nonsense"), None);
        assert_eq!(JobStatus::InProgress.as_db(), "in_progress");
    }

    #[test]
    fn payment_derived_status_and_balance() {
        let total = Money::from_minor(10000);
        assert_eq!(payment_status(total, Money::ZERO), InvoiceStatus::Issued);
        assert_eq!(
            payment_status(Money::ZERO, Money::ZERO),
            InvoiceStatus::Paid
        ); // zero-total settled
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
