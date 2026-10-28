//! The event's data: accounts, persons, transactions and the event itself.
//!
//! These are plain values. The rules that keep them consistent live in [`crate::entry`] and
//! [`crate::book`]; everything derived from them lives in [`crate::ledger`].

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use ts_rs::TS;

use crate::iban::Iban;
use crate::money::Cents;

/// Identifies an account within one event. Becomes the SQLite row id in the event file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct AccountId(#[ts(type = "number")] pub i64);

/// Identifies a transaction within one event. Becomes the SQLite row id in the event file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct TxnId(#[ts(type = "number")] pub i64);

/// Identifies an attachment within one event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct AttachmentId(#[ts(type = "number")] pub i64);

/// See requirements §4.1 and the sign convention in `docs/technical-design.md` §3.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    /// Event money sitting somewhere: the Verein account, the cash box.
    Asset,
    /// Someone who advanced or holds event money. Positive displayed balance: the event owes them.
    Person,
    Income,
    Expense,
    /// The counterpart of opening counts.
    Opening,
}

impl AccountKind {
    /// Factor that turns a raw balance (sum of lines) into the displayed balance.
    pub fn display_sign(self) -> i64 {
        match self {
            AccountKind::Asset | AccountKind::Expense => 1,
            AccountKind::Person | AccountKind::Income | AccountKind::Opening => -1,
        }
    }
}

/// System accounts that exist in every event and that the app refers to directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Verein,
    CashBox,
    Opening,
}

/// What an asset account holds. Cash accounts are counted at the start and the end (R-ISO-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Bank,
    Cash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: AccountId,
    pub kind: AccountKind,
    pub name: String,
    pub role: Option<Role>,
    /// Set exactly for asset accounts.
    pub asset_type: Option<AssetType>,
}

/// Bank and contact details of a person. The person's name is the name of their account.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub holder: Option<String>,
    pub iban: Option<Iban>,
    pub bic: Option<String>,
    pub contact: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum TxnKind {
    Expense,
    Income,
    Transfer,
    /// A reimbursement (asset → person) or a repayment (person → asset).
    Settle,
    /// An opening count, e.g. the cash box at the start.
    Opening,
}

/// One leg of a transaction. Money leaving an account is negative, money arriving positive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct Line {
    pub account: AccountId,
    pub amount: Cents,
    pub memo: Option<String>,
}

/// A balanced transaction: its lines always sum to zero (R-COR-2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct Txn {
    pub id: TxnId,
    pub kind: TxnKind,
    #[ts(type = "string")]
    pub date: Date,
    pub description: String,
    pub counterparty: Option<String>,
    pub notes: Option<String>,
    pub lines: Vec<Line>,
}

impl Event {
    pub fn is_closed(&self) -> bool {
        self.closed_at.is_some()
    }
}

/// The editable details of an event (R-EVT-1, R-EVT-4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EventDetails {
    pub name: String,
    #[ts(type = "string | null")]
    pub starts_on: Option<Date>,
    #[ts(type = "string | null")]
    pub ends_on: Option<Date>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    #[serde(flatten)]
    pub details: EventDetails,
    /// When the event was closed. A closed event is read-only until it is reopened (R-EVT-2).
    #[serde(with = "time::serde::rfc3339::option")]
    #[ts(type = "string | null")]
    pub closed_at: Option<OffsetDateTime>,
    /// Where reimbursements come from by default. Initially the Verein account.
    pub default_reimbursement_account: AccountId,
    /// Open items that were explicitly accepted when the event was closed (R-SET-7).
    pub closing_overrides: Vec<crate::ledger::BlockerOverride>,
}

/// A file attached to a transaction, e.g. a receipt (R-ATT-1). The bytes live only in the
/// event file; the book keeps the details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: AttachmentId,
    pub txn: TxnId,
    pub filename: String,
    pub mime: String,
    #[ts(type = "number")]
    pub size: u64,
    /// SHA-256 of the bytes, lower-case hex.
    pub sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    #[ts(type = "string")]
    pub added_at: OffsetDateTime,
}

/// The amount counted in an asset account at the end, e.g. a cash box (R-ISO-3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct ClosingCount {
    pub account: AccountId,
    #[ts(type = "string")]
    pub date: Date,
    pub amount: Cents,
}
