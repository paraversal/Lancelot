//! Everything the UI shows of an event, in one value (`docs/technical-design.md` §5).
//!
//! The app sends a fresh snapshot after every command, so the UI never patches state and never
//! computes anything about money: every figure in here comes from the [`crate::Ledger`].

use serde::Serialize;
use ts_rs::TS;

use crate::book::Book;
use crate::budget::Budget;
use crate::entry::{Entry, entry_of, flow_of, txn_amount};
use crate::ledger::{
    CashStatus, MoneyLocation, PersonEntry, PersonSummary, PersonTotals, ReconciliationStatus,
};
use crate::model::{
    Account, AccountId, Assumption, Attachment, BudgetLine, ClosingCount, CostCentre, Event, Flow,
    OpenItem, Person, Txn,
};
use crate::money::Cents;
use crate::payout::Payouts;
use crate::plugin::PluginRecord;
use crate::sales::Profitability;
use crate::tickets::{TicketSettings, Tickets};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EventSnapshot {
    pub event: Event,
    /// Whether the event sells tickets, and where the sheet's money arrives.
    pub ticket_settings: TicketSettings,
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
    /// Statements against the books, by date (R-ACC-4).
    pub reconciliations: Vec<ReconciliationStatus>,
    pub open_items: Vec<OpenItem>,
    /// The payout and repayment lists (R-PER-4, R-PER-7).
    pub payouts: Payouts,
    /// In creation order.
    pub budget_lines: Vec<BudgetLine>,
    /// In creation order (R-BUD-6).
    pub assumptions: Vec<Assumption>,
    /// Plan vs actual and the forecast (R-BUD-3, R-BUD-4).
    pub budget: Budget,
    /// Tiers, imports and manual sales with their figures (R-TKT-1…7).
    pub tickets: Tickets,
    /// The projected result and break-even (R-TKT-9), once there is a budget and while the
    /// event is open.
    pub profitability: Option<Profitability>,
    /// In creation order (R-CC-2).
    pub cost_centres: Vec<CostCentre>,
    /// What plugins keep in the file, in creation order (technical design §10).
    pub plugin_records: Vec<PluginRecord>,
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
#[serde(rename_all = "camelCase")]
pub struct TxnView {
    #[serde(flatten)]
    pub txn: Txn,
    /// What the transaction moves: the receipt total, the transfer or settle amount.
    pub amount: Cents,
    pub attachments: Vec<Attachment>,
    /// What the form shows. Generated entries show in the same form, read-only.
    pub entry: Option<Entry>,
    /// Who paid or received: the payer, the destination, the source, the person settling.
    pub who: Option<AccountId>,
    pub flow: Flow,
    pub needs_receipt: bool,
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
            .map(|t| {
                let entry = entry_of(t, book.account_map());
                TxnView {
                    amount: txn_amount(&t.lines),
                    txn: t.clone(),
                    attachments: book.attachments_of(t.id).cloned().collect(),
                    who: entry.as_ref().map(Entry::who),
                    entry,
                    flow: flow_of(&t.lines, book.account_map()),
                    needs_receipt: t.kind.needs_receipt(),
                }
            })
            .collect();
        txns.sort_by_key(|t| (t.txn.date, t.txn.id));

        EventSnapshot {
            event: book.event().clone(),
            ticket_settings: book.ticket_settings(),
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
            reconciliations: ledger.reconciliations().to_vec(),
            open_items: book.open_items(),
            payouts: book.payouts().clone(),
            budget_lines: book.budget_lines().cloned().collect(),
            assumptions: book.assumptions().cloned().collect(),
            budget: book.budget().clone(),
            tickets: book.tickets().clone(),
            profitability: book.profitability(),
            cost_centres: book.cost_centres().cloned().collect(),
            plugin_records: book.plugin_records().cloned().collect(),
        }
    }
}
