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
            | DomainError::WrongBudgetLine { field } => Some(field),
            DomainError::BudgetLineInUse => Some("category"),
            DomainError::SameAccount => Some("to"),
            DomainError::InvalidBic => Some("bic"),
            DomainError::SplitMismatch => Some("lines"),
            _ => None,
        }
    }
}
