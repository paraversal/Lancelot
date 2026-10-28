//! Everything the UI shows of an event, in one value (`docs/technical-design.md` §5).
//!
//! The app sends a fresh snapshot after every command, so the UI never patches state and never
//! computes anything about money: every figure in here comes from the [`crate::Ledger`].

use serde::Serialize;
use ts_rs::TS;

use crate::book::Book;
use crate::entry::txn_amount;
use crate::ledger::{
    CashStatus, MoneyLocation, OpenItem, PersonEntry, PersonSummary, PersonTotals,
};
use crate::model::{Account, AccountId, Attachment, ClosingCount, Event, Person, Txn};
use crate::money::Cents;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EventSnapshot {
    pub event: Event,
    /// All accounts and categories, in creation order, each with its displayed balance.
    pub accounts: Vec<AccountView>,
    pub persons: Vec<PersonView>,
    pub person_totals: PersonTotals,
    /// Transactions by date, then by id.
    pub txns: Vec<TxnView>,
    pub totals: Totals,
    pub where_the_money_is: MoneyLocation,
    pub cash: Vec<CashStatus>,
    pub closing_counts: Vec<ClosingCount>,
    pub open_items: Vec<OpenItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AccountView {
    #[serde(flatten)]
    pub account: Account,
    pub balance: Cents,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PersonView {
    pub account: AccountId,
    pub name: String,
    #[serde(flatten)]
    pub details: Person,
    pub summary: PersonSummary,
    /// The running balance, oldest first.
    pub entries: Vec<PersonEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TxnView {
    #[serde(flatten)]
    pub txn: Txn,
    /// What the transaction moves: the receipt total, the transfer or settle amount.
    pub amount: Cents,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Totals {
    pub income: Cents,
    pub expense: Cents,
    pub result: Cents,
}

impl EventSnapshot {
    pub fn new(book: &Book) -> EventSnapshot {
        let ledger = book.ledger();
        let accounts = book
            .accounts()
            .map(|a| AccountView {
                account: a.clone(),
                balance: ledger.balance(a.id),
            })
            .collect();
        let persons = book
            .persons()
            .map(|(a, p)| PersonView {
                account: a.id,
                name: a.name.clone(),
                details: p.clone(),
                summary: ledger.person(a.id).copied().unwrap_or_default(),
                entries: ledger.person_entries(a.id).to_vec(),
            })
            .collect();
        let mut txns: Vec<TxnView> = book
            .txns()
            .map(|t| TxnView {
                amount: txn_amount(&t.lines),
                txn: t.clone(),
                attachments: book.attachments_of(t.id).cloned().collect(),
            })
            .collect();
        txns.sort_by_key(|t| (t.txn.date, t.txn.id));

        EventSnapshot {
            event: book.event().clone(),
            accounts,
            persons,
            person_totals: ledger.person_totals(),
            txns,
            totals: Totals {
                income: ledger.income_total(),
                expense: ledger.expense_total(),
                result: ledger.result(),
            },
            where_the_money_is: ledger.where_the_money_is().clone(),
            cash: ledger.cash().to_vec(),
            closing_counts: book.closing_counts().cloned().collect(),
            open_items: ledger.open_items(),
        }
    }
}
