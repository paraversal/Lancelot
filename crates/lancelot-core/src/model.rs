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

/// Identifies a budget line within one event (R-BUD-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct BudgetLineId(#[ts(type = "number")] pub i64);

/// Identifies a ticket tier within one event (R-TKT-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct TierId(#[ts(type = "number")] pub i64);

/// Identifies an import of the ticket sheet (R-TKT-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct ImportId(#[ts(type = "number")] pub i64);

/// Identifies a ticket cancellation or correction (R-TKT-13, R-TKT-14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct CancellationId(#[ts(type = "number")] pub i64);

/// Identifies a manual ticket sale (R-TKT-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct ManualSaleId(#[ts(type = "number")] pub i64);

/// Identifies a cost centre within one event (R-CC-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct CostCentreId(#[ts(type = "number")] pub i64);

/// Identifies a commission product within one event (R-DRK-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct DrinkProductId(#[ts(type = "number")] pub i64);

/// Identifies a reconciliation checkpoint within one event (R-ACC-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(transparent)]
pub struct ReconciliationId(#[ts(type = "number")] pub i64);

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
    /// The income category of ticket sales (R-TKT-6).
    Tickets,
    /// The expense category of platform fees, made when a tier first has a fee (R-TKT-6).
    TicketFees,
    /// The income category of refunds, made with the first refund (R-TKT-15).
    TicketRefunds,
    /// The expense category of Pfand on crates that could not be returned (R-DRK-2).
    Pfand,
}

/// Cost centres that exist in every event and that the app refers to directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum CostCentreRole {
    Drinks,
}

/// A sub-area of an event with its own profit and loss (R-CC-1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct CostCentre {
    pub id: CostCentreId,
    pub name: String,
    pub role: Option<CostCentreRole>,
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
    /// For a category: the cost centre a new line of it starts with.
    pub cost_centre: Option<CostCentreId>,
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
    /// A ticket refund: negative income, always generated (R-TKT-15).
    Refund,
}

/// Which way a transaction moves the event's money.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Flow {
    In,
    Out,
    /// Between the event's own accounts and people.
    Moved,
}

impl TxnKind {
    pub fn flow(self) -> Flow {
        match self {
            TxnKind::Income => Flow::In,
            TxnKind::Expense | TxnKind::Refund => Flow::Out,
            TxnKind::Transfer | TxnKind::Settle | TxnKind::Opening => Flow::Moved,
        }
    }

    /// Whether the transaction should have a receipt attached (R-ATT-4).
    pub fn needs_receipt(self) -> bool {
        self == TxnKind::Expense
    }
}

/// One leg of a transaction. Money leaving an account is negative, money arriving positive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct Line {
    pub account: AccountId,
    pub amount: Cents,
    pub memo: Option<String>,
    /// The budget line this category line counts towards (R-BUD-2). Only income and expense
    /// lines of the budget line's own category can have one.
    #[serde(rename = "budgetLine", default)]
    pub budget_line: Option<BudgetLineId>,
    /// Only income and expense lines can have one, and a line linked to a budget line has that
    /// line's (R-CC-1).
    #[serde(rename = "costCentre", default)]
    pub cost_centre: Option<CostCentreId>,
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
    /// Typed in, or generated from tickets. Generated transactions are read-only (R-TKT-6).
    #[serde(default)]
    pub origin: Origin,
}

/// Where a transaction comes from.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS, Deserialize,
)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Origin {
    #[default]
    Manual,
    /// The ticket income of one tier from the latest import of the sheet.
    TicketImport { tier: TierId },
    /// A manual ticket sale.
    TicketSale { sale: ManualSaleId },
    /// The refund of a cancellation: negative income (R-TKT-15).
    TicketRefund { cancellation: CancellationId },
}

impl Origin {
    pub fn is_generated(self) -> bool {
        self != Origin::Manual
    }
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
    pub closing_overrides: Vec<BlockerOverride>,
}

/// Something that keeps the event from being closed (R-SET-5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OpenItem {
    /// A person the event owes money to, or who owes the event.
    PersonBalance { person: AccountId, net: Cents },
    /// An expense without an attachment (R-ATT-4).
    MissingReceipt { txn: TxnId },
    /// A cash account has no closing count yet.
    ClosingCountMissing { account: AccountId },
    /// The closing count differs from the computed balance.
    CashDifference {
        account: AccountId,
        difference: Cents,
    },
    /// A statement differs from the books on its date (R-ACC-4).
    ReconciliationMismatch {
        checkpoint: ReconciliationId,
        account: AccountId,
        difference: Cents,
    },
    /// Tickets removed from the sheet that have no resolution yet (R-TKT-16), per order and tier.
    TicketConflicts { count: u32 },
}

/// An open item explicitly accepted when closing the event (R-SET-7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct BlockerOverride {
    pub item: OpenItem,
    pub note: String,
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

/// "The statement shows this on this date" for an asset account (R-ACC-4). For the Verein account
/// the amount is the event's net movement up to the date, since its real balance is never entered
/// (R-ISO-2). Either can be negative.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct Reconciliation {
    pub id: ReconciliationId,
    pub account: AccountId,
    #[ts(type = "string")]
    pub date: Date,
    pub amount: Cents,
}

/// A planned income or expense (R-BUD-1). Planning data only: it never affects a balance
/// (R-BUD-5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct BudgetLine {
    pub id: BudgetLineId,
    #[serde(flatten)]
    pub draft: BudgetLineDraft,
}

/// What the budget form fills in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct BudgetLineDraft {
    /// An income or expense category.
    pub category: AccountId,
    pub description: String,
    pub estimate: Estimate,
    /// Nothing more is expected: the forecast uses the actual amount (R-BUD-4).
    pub complete: bool,
    #[serde(rename = "costCentre", default)]
    pub cost_centre: Option<CostCentreId>,
}

/// A budget line's estimate, always positive or zero, whether it is an income or an expense.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Estimate {
    Amount {
        amount: Cents,
    },
    /// Quantity × unit price, e.g. 40 crates at 12,50 €. Exact, so nothing is rounded.
    Quantity {
        quantity: u32,
        #[serde(rename = "unitPrice")]
        unit_price: Cents,
    },
}

impl Estimate {
    pub fn total(self) -> Result<Cents, crate::money::MoneyError> {
        match self {
            Estimate::Amount { amount } => Ok(amount),
            Estimate::Quantity {
                quantity,
                unit_price,
            } => unit_price.checked_mul(i64::from(quantity)),
        }
    }
}
