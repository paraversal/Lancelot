//! Errors of the domain layer. Each has a stable [`DomainError::code`] and, where it concerns
//! one input, a [`DomainError::field`] path such as `lines[2].amount`, so the UI can mark it.

use thiserror::Error;

use crate::ledger::OpenItem;
use crate::money::MoneyError;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("unknown account")]
    UnknownAccount { field: String },
    #[error("this account cannot be used here")]
    WrongAccountKind { field: String },
    #[error("amount must be greater than zero")]
    NonPositiveAmount { field: String },
    #[error("at least one line is needed")]
    NoLines,
    #[error("source and destination must be different accounts")]
    SameAccount,
    #[error("the event is closed")]
    EventClosed,
    #[error("the event still has open items")]
    ClosingBlocked { blockers: Vec<OpenItem> },
    #[error("an override needs a note")]
    MissingNote,
    #[error("unknown transaction")]
    UnknownTxn,
    #[error("unknown person")]
    UnknownPerson,
    #[error("a name is needed")]
    EmptyName { field: String },
    #[error("this name is already used")]
    DuplicateName { field: String },
    #[error("invalid BIC")]
    InvalidBic,
    #[error("the lines do not add up to the total")]
    SplitMismatch,
    #[error("amount must not be negative")]
    NegativeAmount { field: String },
    #[error("unknown attachment")]
    UnknownAttachment,
    #[error("an attachment can have at most 50 MB")]
    AttachmentTooLarge,
    #[error("transaction {txn:?} does not follow the rules for its kind")]
    MalformedTxn { txn: crate::model::TxnId },
    #[error("unknown budget line")]
    UnknownBudgetLine,
    #[error("the budget line belongs to another category")]
    WrongBudgetLine { field: String },
    #[error("transactions are linked to this budget line, so its category cannot change")]
    BudgetLineInUse,
    #[error("unknown cost centre")]
    UnknownCostCentre { field: String },
    #[error("this line has the cost centre of its budget line")]
    WrongCostCentre { field: String },
    #[error("the Drinks cost centre cannot be deleted")]
    CostCentreProtected,
    #[error("unknown product")]
    UnknownDrinkProduct,
    #[error("unknown checkpoint")]
    UnknownReconciliation,
    #[error("this account already has a checkpoint on this date")]
    DuplicateCheckpoint { field: String },
    #[error("more units returned than taken")]
    TooManyReturned,
    #[error("unknown ticket tier")]
    UnknownTier,
    #[error("tickets of this tier were imported or sold, so it cannot be deleted")]
    TierInUse,
    #[error("a cancellation still needs the tickets of this import: delete it first")]
    ImportInUse,
    #[error("this account is in use: entries, budget lines or settings refer to it")]
    AccountInUse,
    #[error("a cash count cannot be below zero")]
    NegativeCashCount,
    #[error("the fee must be lower than the price")]
    FeeTooHigh,
    #[error("unknown ticket import")]
    UnknownImport,
    #[error("unknown manual sale")]
    UnknownManualSale,
    #[error("unknown cancellation")]
    UnknownCancellation,
    #[error("no such order with tickets of this tier in the sheet")]
    UnknownOrder,
    #[error("that is more tickets than are left to cancel")]
    TooManyTickets,
    #[error("the refund must be between 0 and 100 %")]
    RefundTooHigh,
    #[error("this sale has cancellations: delete them first")]
    SaleHasCancellations,
    #[error("a correction cannot be edited: delete it and resolve the conflict again")]
    CorrectionNotEditable,
    #[error("this transaction is generated from tickets and cannot be changed by hand")]
    GeneratedTxn,
    #[error("the ticket sheet cannot be read: {reason}")]
    UnreadableSheet { reason: String },
    #[error("the ticket sheet has no column `{column}`")]
    MissingColumn { column: String },
    #[error("the ticket sheet has tiers that match no tier: {}", names.join(", "))]
    UnknownTiers { names: Vec<String> },
    #[error("inconsistent event data: {reason}")]
    Inconsistent { reason: String },
    #[error(transparent)]
    Money(#[from] MoneyError),
}

impl DomainError {
    /// Stable identifier for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            DomainError::UnknownAccount { .. } => "unknown_account",
            DomainError::WrongAccountKind { .. } => "wrong_account_kind",
            DomainError::NonPositiveAmount { .. } => "non_positive_amount",
            DomainError::NoLines => "no_lines",
            DomainError::SameAccount => "same_account",
            DomainError::EventClosed => "event_closed",
            DomainError::ClosingBlocked { .. } => "closing_blocked",
            DomainError::MissingNote => "missing_note",
            DomainError::UnknownTxn => "unknown_txn",
            DomainError::UnknownPerson => "unknown_person",
            DomainError::EmptyName { .. } => "empty_name",
            DomainError::DuplicateName { .. } => "duplicate_name",
            DomainError::InvalidBic => "invalid_bic",
            DomainError::NegativeAmount { .. } => "negative_amount",
            DomainError::SplitMismatch => "split_mismatch",
            DomainError::UnknownAttachment => "unknown_attachment",
            DomainError::AttachmentTooLarge => "attachment_too_large",
            DomainError::MalformedTxn { .. } => "malformed_txn",
            DomainError::UnknownBudgetLine => "unknown_budget_line",
            DomainError::WrongBudgetLine { .. } => "wrong_budget_line",
            DomainError::BudgetLineInUse => "budget_line_in_use",
            DomainError::UnknownCostCentre { .. } => "unknown_cost_centre",
            DomainError::WrongCostCentre { .. } => "wrong_cost_centre",
            DomainError::CostCentreProtected => "cost_centre_protected",
            DomainError::UnknownDrinkProduct => "unknown_drink_product",
            DomainError::UnknownReconciliation => "unknown_reconciliation",
            DomainError::DuplicateCheckpoint { .. } => "duplicate_checkpoint",
            DomainError::TooManyReturned => "too_many_returned",
            DomainError::UnknownTier => "unknown_tier",
            DomainError::TierInUse => "tier_in_use",
            DomainError::ImportInUse => "import_in_use",
            DomainError::AccountInUse => "account_in_use",
            DomainError::NegativeCashCount => "negative_cash_count",
            DomainError::FeeTooHigh => "fee_too_high",
            DomainError::UnknownImport => "unknown_import",
            DomainError::UnknownManualSale => "unknown_manual_sale",
            DomainError::UnknownCancellation => "unknown_cancellation",
            DomainError::UnknownOrder => "unknown_order",
            DomainError::TooManyTickets => "too_many_tickets",
            DomainError::RefundTooHigh => "refund_too_high",
            DomainError::SaleHasCancellations => "sale_has_cancellations",
            DomainError::CorrectionNotEditable => "correction_not_editable",
            DomainError::GeneratedTxn => "generated_txn",
            DomainError::UnreadableSheet { .. } => "unreadable_sheet",
            DomainError::MissingColumn { .. } => "missing_column",
            DomainError::UnknownTiers { .. } => "unknown_tiers",
            DomainError::Inconsistent { .. } => "inconsistent",
            DomainError::Money(e) => e.code(),
        }
    }

    /// The input this error belongs to, if any.
    pub fn field(&self) -> Option<&str> {
        match self {
            DomainError::UnknownAccount { field }
            | DomainError::WrongAccountKind { field }
            | DomainError::NonPositiveAmount { field }
            | DomainError::NegativeAmount { field }
            | DomainError::EmptyName { field }
            | DomainError::DuplicateName { field }
            | DomainError::WrongBudgetLine { field }
            | DomainError::UnknownCostCentre { field }
            | DomainError::WrongCostCentre { field }
            | DomainError::DuplicateCheckpoint { field } => Some(field),
            DomainError::TooManyReturned => Some("returned"),
            DomainError::BudgetLineInUse => Some("category"),
            DomainError::SameAccount => Some("to"),
            DomainError::InvalidBic => Some("bic"),
            DomainError::SplitMismatch => Some("lines"),
            DomainError::FeeTooHigh => Some("feeBp"),
            DomainError::UnknownTier => Some("tier"),
            DomainError::UnknownOrder => Some("order"),
            DomainError::TooManyTickets => Some("count"),
            DomainError::RefundTooHigh => Some("refundBp"),
            _ => None,
        }
    }
}
