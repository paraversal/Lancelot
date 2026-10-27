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
    #[error("a closed event can only be reopened to Settling")]
    InvalidStateChange,
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
            DomainError::InvalidStateChange => "invalid_state_change",
            DomainError::MissingNote => "missing_note",
            DomainError::UnknownTxn => "unknown_txn",
            DomainError::UnknownPerson => "unknown_person",
            DomainError::EmptyName { .. } => "empty_name",
            DomainError::DuplicateName { .. } => "duplicate_name",
            DomainError::InvalidBic => "invalid_bic",
            DomainError::Money(e) => e.code(),
        }
    }

    /// The input this error belongs to, if any.
    pub fn field(&self) -> Option<&str> {
        match self {
            DomainError::UnknownAccount { field }
            | DomainError::WrongAccountKind { field }
            | DomainError::NonPositiveAmount { field }
            | DomainError::EmptyName { field }
            | DomainError::DuplicateName { field } => Some(field),
            DomainError::SameAccount => Some("to"),
            DomainError::InvalidBic => Some("bic"),
            _ => None,
        }
    }
}
