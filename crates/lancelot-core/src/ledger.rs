//! Everything derived from the transactions (R-COR-4): balances, person summaries, totals,
//! the result and where the money is. Nothing here is stored; it is recomputed on every change.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use time::Date;

use crate::model::{
    Account, AccountId, AccountKind, AssetType, ClosingCount, Origin, Reconciliation,
    ReconciliationId, Role, Txn, TxnId, TxnKind,
};
use crate::money::{Cents, MoneyError};

/// Per-person figures for the People view (R-PER-3, R-TX-5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PersonSummary {
    /// Paid out of the person's own pocket for the event (expenses, money put into the cash box).
    pub advanced: Cents,
    /// Event money the person received (e.g. tickets paid to their private account).
    pub held: Cents,
    /// Paid to the person by the event.
    pub reimbursed: Cents,
    /// Paid back to the event by the person.
    pub repaid: Cents,
    /// Settled in either direction: `reimbursed + repaid`.
    pub settled: Cents,
    /// `advanced − held − reimbursed + repaid`. Positive: the event owes the person.
    pub net: Cents,
    pub status: PersonStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum PersonStatus {
    /// The event owes the person.
    Owed,
    /// The person owes the event.
    Owes,
    #[default]
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AccountBalance {
    pub account: AccountId,
    pub balance: Cents,
}

/// The result, broken down by where the money sits. `total` always equals the result:
/// `assets + held_by_people − owed_to_people − opening`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MoneyLocation {
    pub assets: Vec<AccountBalance>,
    pub held_by_people: Cents,
    pub owed_to_people: Cents,
    /// Opening counts brought in. They are not part of the event's result.
    pub opening: Cents,
    pub total: Cents,
}

/// The counts of an asset account against its computed balance (R-ISO-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CashStatus {
    pub account: AccountId,
    pub asset_type: AssetType,
    /// What was counted at the start: the account's opening transactions.
    pub opening: Option<OpeningCount>,
    pub computed: Cents,
    pub counted: Option<Cents>,
    /// `counted − computed`: positive when there is more money than the books say.
    pub difference: Option<Cents>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OpeningCount {
    /// The first opening transaction; the one the count panel edits.
    pub txn: TxnId,
    #[ts(type = "string")]
    pub date: Date,
    /// Of all the account's opening transactions.
    pub amount: Cents,
}

/// A statement against the books on its date (R-ACC-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationStatus {
    pub id: ReconciliationId,
    pub account: AccountId,
    #[ts(type = "string")]
    pub date: Date,
    pub amount: Cents,
    /// What the books say by the date: see [`book_figure`].
    pub computed: Cents,
    /// The part of `computed` that the ticket sheets known by the date brought in.
    pub from_sheet: Cents,
    /// `amount − computed`: positive when the statement shows more than the books.
    pub difference: Cents,
    /// The Verein account: `amount` and `computed` are net movements, without opening counts.
    pub movement: bool,
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

/// What the books say an asset account should show.
#[derive(Clone, Copy)]
struct BookFigure {
    total: Cents,
    from_sheet: Cents,
}

/// What a closing count or a statement is compared with, up to and including `until`.
///
/// The Verein account shows only the event's net movement, without opening counts (R-ISO-2).
/// It also takes the ticket income from the sheets known by then: the generated entry stands for
/// income that came in over time but carries the date of the latest import, so its date says
/// nothing about when the money arrived.
fn book_figure(
    account: &Account,
    txns: &BTreeMap<TxnId, Txn>,
    until: Option<Date>,
    sheet_income: &[(Date, Cents)],
) -> Result<BookFigure, MoneyError> {
    let movement = account.role == Some(Role::Verein);
    let from_sheet = if movement {
        sheet_income
            .iter()
            .rfind(|(date, _)| until.is_none_or(|d| *date <= d))
            .map_or(Cents::ZERO, |(_, income)| *income)
    } else {
        Cents::ZERO
    };
    let total = txns
        .values()
        .filter(|t| until.is_none_or(|d| t.date <= d))
        .filter(|t| {
            !(movement
                && (t.kind == TxnKind::Opening || matches!(t.origin, Origin::TicketImport { .. })))
        })
        .flat_map(|t| &t.lines)
        .filter(|l| l.account == account.id)
        .try_fold(from_sheet, |acc, l| acc.checked_add(l.amount))?;
    Ok(BookFigure { total, from_sheet })
}

/// What [`Ledger::compute`] needs from the book.
pub struct LedgerInput<'a> {
    pub accounts: &'a BTreeMap<AccountId, Account>,
    pub txns: &'a BTreeMap<TxnId, Txn>,
    pub closing_counts: &'a BTreeMap<AccountId, ClosingCount>,
    pub reconciliations: &'a BTreeMap<ReconciliationId, Reconciliation>,
    /// Transactions that have at least one attachment.
    pub with_attachments: &'a BTreeSet<TxnId>,
    /// Unresolved ticket conflicts.
    pub ticket_conflicts: u32,
    /// What the sheets had brought in by each import's date, oldest first.
    pub sheet_income: &'a [(Date, Cents)],
}

/// An open item explicitly accepted when closing the event (R-SET-7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct BlockerOverride {
    pub item: OpenItem,
    pub note: String,
}

/// All derived figures of an event.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    raw: BTreeMap<AccountId, Cents>,
    displayed: BTreeMap<AccountId, Cents>,
    persons: BTreeMap<AccountId, PersonSummary>,
    income_total: Cents,
    expense_total: Cents,
    result: Cents,
    money: MoneyLocation,
    cash: Vec<CashStatus>,
    reconciliations: Vec<ReconciliationStatus>,
    missing_receipts: Vec<TxnId>,
    person_entries: BTreeMap<AccountId, Vec<PersonEntry>>,
    person_totals: PersonTotals,
    ticket_conflicts: u32,
}

/// One line of a person's running balance (R-PER-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PersonEntry {
    pub txn: TxnId,
    /// What the transaction does to the person's net balance: positive when the event owes
    /// them more (an advance), negative when less (money held, a reimbursement).
    pub effect: Cents,
    /// The net balance after this transaction.
    pub balance: Cents,
}

/// The totals row of the People table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PersonTotals {
    pub advanced: Cents,
    pub held: Cents,
    pub settled: Cents,
    pub net: Cents,
}

impl Ledger {
    /// Computes every figure. Fails only if a sum leaves the valid amount range.
    pub fn compute(input: LedgerInput<'_>) -> Result<Ledger, MoneyError> {
        let LedgerInput {
            accounts,
            txns,
            closing_counts,
            reconciliations,
            with_attachments,
            ticket_conflicts,
            sheet_income,
        } = input;
        let mut raw: BTreeMap<AccountId, Cents> =
            accounts.keys().map(|id| (*id, Cents::ZERO)).collect();
        let mut persons: BTreeMap<AccountId, PersonSummary> = accounts
            .values()
            .filter(|a| a.kind == AccountKind::Person)
            .map(|a| (a.id, PersonSummary::default()))
            .collect();
        let mut refunded: BTreeMap<AccountId, Cents> = BTreeMap::new();

        for txn in txns.values() {
            for line in &txn.lines {
                let balance = raw.entry(line.account).or_default();
                *balance = balance.checked_add(line.amount)?;

                if let Some(p) = persons.get_mut(&line.account) {
                    if let Origin::TicketRefund { .. } = txn.origin {
                        let sum = refunded.entry(line.account).or_default();
                        *sum = sum.checked_sub(line.amount)?;
                        continue;
                    }
                    let (field, amount) = match (txn.kind, line.amount.is_positive()) {
                        (TxnKind::Settle, true) => (&mut p.reimbursed, line.amount),
                        (TxnKind::Settle, false) => (&mut p.repaid, -line.amount),
                        (_, true) => (&mut p.held, line.amount),
                        (_, false) => (&mut p.advanced, -line.amount),
                    };
                    *field = field.checked_add(amount)?;
                }
            }
        }

        // A refund comes out of the ticket money a person holds (R-TKT-15); beyond that it is
        // their own money, an advance.
        for (id, refund) in refunded {
            if let Some(p) = persons.get_mut(&id) {
                let from_held = refund.min(p.held);
                p.held = p.held.checked_sub(from_held)?;
                p.advanced = p.advanced.checked_add(refund.checked_sub(from_held)?)?;
            }
        }

        for p in persons.values_mut() {
            p.settled = p.reimbursed.checked_add(p.repaid)?;
            p.net = p
                .advanced
                .checked_sub(p.held)?
                .checked_sub(p.reimbursed)?
                .checked_add(p.repaid)?;
            p.status = match p.net.signum() {
                1 => PersonStatus::Owed,
                -1 => PersonStatus::Owes,
                _ => PersonStatus::Settled,
            };
        }

        // Running balances, in date order (then by id), and the totals row.
        let mut ordered: Vec<&Txn> = txns.values().collect();
        ordered.sort_by_key(|t| (t.date, t.id));
        let mut person_entries: BTreeMap<AccountId, Vec<PersonEntry>> = BTreeMap::new();
        for txn in ordered {
            for line in txn
                .lines
                .iter()
                .filter(|l| persons.contains_key(&l.account))
            {
                let entries = person_entries.entry(line.account).or_default();
                let before = entries.last().map(|e| e.balance).unwrap_or_default();
                let effect = -line.amount;
                entries.push(PersonEntry {
                    txn: txn.id,
                    effect,
                    balance: before.checked_add(effect)?,
                });
            }
        }
        let mut person_totals = PersonTotals::default();
        for p in persons.values() {
            person_totals.advanced = person_totals.advanced.checked_add(p.advanced)?;
            person_totals.held = person_totals.held.checked_add(p.held)?;
            person_totals.settled = person_totals.settled.checked_add(p.settled)?;
            person_totals.net = person_totals.net.checked_add(p.net)?;
        }

        let kind_of = |id: &AccountId| accounts.get(id).map(|a| a.kind);
        let mut displayed = BTreeMap::new();
        for (id, c) in &raw {
            if let Some(kind) = kind_of(id) {
                displayed.insert(*id, c.checked_mul(kind.display_sign())?);
            }
        }

        let total_of = |kind: AccountKind| -> Result<Cents, MoneyError> {
            displayed
                .iter()
                .filter(|(id, _)| kind_of(id) == Some(kind))
                .try_fold(Cents::ZERO, |acc, (_, c)| acc.checked_add(*c))
        };
        let income_total = total_of(AccountKind::Income)?;
        let expense_total = total_of(AccountKind::Expense)?;
        let result = income_total.checked_sub(expense_total)?;

        let assets: Vec<AccountBalance> = displayed
            .iter()
            .filter(|(id, _)| kind_of(id) == Some(AccountKind::Asset))
            .map(|(id, c)| AccountBalance {
                account: *id,
                balance: *c,
            })
            .collect();
        let mut held_by_people = Cents::ZERO;
        let mut owed_to_people = Cents::ZERO;
        for p in persons.values() {
            if p.net.is_negative() {
                held_by_people = held_by_people.checked_sub(p.net)?;
            } else {
                owed_to_people = owed_to_people.checked_add(p.net)?;
            }
        }
        let opening = total_of(AccountKind::Opening)?;
        let total = assets
            .iter()
            .try_fold(Cents::ZERO, |acc, a| acc.checked_add(a.balance))?
            .checked_add(held_by_people)?
            .checked_sub(owed_to_people)?
            .checked_sub(opening)?;

        let mut openings: BTreeMap<AccountId, OpeningCount> = BTreeMap::new();
        for t in txns.values().filter(|t| t.kind == TxnKind::Opening) {
            for l in t
                .lines
                .iter()
                .filter(|l| kind_of(&l.account) == Some(AccountKind::Asset))
            {
                match openings.get_mut(&l.account) {
                    Some(o) => o.amount = o.amount.checked_add(l.amount)?,
                    None => {
                        openings.insert(
                            l.account,
                            OpeningCount {
                                txn: t.id,
                                date: t.date,
                                amount: l.amount,
                            },
                        );
                    }
                }
            }
        }

        let mut cash = Vec::new();
        for a in accounts.values() {
            let Some(asset_type) = a.asset_type.filter(|_| a.kind == AccountKind::Asset) else {
                continue;
            };
            let counted = closing_counts.get(&a.id).map(|c| c.amount);
            let opening = openings.get(&a.id).copied();
            if asset_type == AssetType::Bank && counted.is_none() && opening.is_none() {
                continue;
            }
            let figure = book_figure(a, txns, None, sheet_income)?;
            let computed = figure.total;
            let difference = counted.map(|c| c.checked_sub(computed)).transpose()?;
            cash.push(CashStatus {
                account: a.id,
                asset_type,
                opening,
                computed,
                counted,
                difference,
            });
        }

        let mut checkpoints = Vec::new();
        for r in reconciliations.values() {
            let Some(account) = accounts.get(&r.account) else {
                continue;
            };
            let movement = account.role == Some(Role::Verein);
            let figure = book_figure(account, txns, Some(r.date), sheet_income)?;
            let computed = figure.total;
            checkpoints.push(ReconciliationStatus {
                id: r.id,
                account: r.account,
                date: r.date,
                amount: r.amount,
                computed,
                from_sheet: figure.from_sheet,
                difference: r.amount.checked_sub(computed)?,
                movement,
            });
        }
        checkpoints.sort_by_key(|c| (c.date, c.id));

        let missing_receipts = txns
            .values()
            .filter(|t| t.kind == TxnKind::Expense && !with_attachments.contains(&t.id))
            .map(|t| t.id)
            .collect();

        Ok(Ledger {
            raw,
            displayed,
            persons,
            income_total,
            expense_total,
            result,
            money: MoneyLocation {
                assets,
                held_by_people,
                owed_to_people,
                opening,
                total,
            },
            cash,
            reconciliations: checkpoints,
            missing_receipts,
            person_entries,
            person_totals,
            ticket_conflicts,
        })
    }

    /// Sum of the account's lines. All raw balances together are always zero.
    pub fn raw_balance(&self, account: AccountId) -> Cents {
        self.raw.get(&account).copied().unwrap_or_default()
    }

    /// The balance as shown to the user (`docs/technical-design.md` §3.1).
    pub fn balance(&self, account: AccountId) -> Cents {
        self.displayed.get(&account).copied().unwrap_or_default()
    }

    pub fn person(&self, account: AccountId) -> Option<&PersonSummary> {
        self.persons.get(&account)
    }

    /// A person's transactions with their running balance, oldest first.
    pub fn person_entries(&self, person: AccountId) -> &[PersonEntry] {
        self.person_entries
            .get(&person)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn person_totals(&self) -> PersonTotals {
        self.person_totals
    }

    pub fn persons(&self) -> impl Iterator<Item = (AccountId, &PersonSummary)> {
        self.persons.iter().map(|(id, p)| (*id, p))
    }

    pub fn income_total(&self) -> Cents {
        self.income_total
    }

    pub fn expense_total(&self) -> Cents {
        self.expense_total
    }

    /// Income minus expenses (R-SET-1).
    pub fn result(&self) -> Cents {
        self.result
    }

    pub fn where_the_money_is(&self) -> &MoneyLocation {
        &self.money
    }

    /// Counts against computed balances: every cash account, plus every bank account that
    /// has an opening or a closing count.
    pub fn cash(&self) -> &[CashStatus] {
        &self.cash
    }

    /// Every checkpoint against the books, by date (R-ACC-4).
    pub fn reconciliations(&self) -> &[ReconciliationStatus] {
        &self.reconciliations
    }

    /// Expense transactions without an attachment (R-ATT-4).
    pub fn missing_receipts(&self) -> &[TxnId] {
        &self.missing_receipts
    }

    /// People first, then receipts, then cash, then statements, then tickets.
    pub fn open_items(&self) -> Vec<OpenItem> {
        let people = self
            .persons
            .iter()
            .filter(|(_, p)| !p.net.is_zero())
            .map(|(id, p)| OpenItem::PersonBalance {
                person: *id,
                net: p.net,
            });
        let receipts = self
            .missing_receipts
            .iter()
            .map(|txn| OpenItem::MissingReceipt { txn: *txn });
        let cash = self.cash.iter().filter_map(|c| match c.difference {
            None if c.asset_type == AssetType::Cash => {
                Some(OpenItem::ClosingCountMissing { account: c.account })
            }
            None => None,
            Some(d) if !d.is_zero() => Some(OpenItem::CashDifference {
                account: c.account,
                difference: d,
            }),
            Some(_) => None,
        });
        let statements = self
            .reconciliations
            .iter()
            .filter(|r| !r.difference.is_zero())
            .map(|r| OpenItem::ReconciliationMismatch {
                checkpoint: r.id,
                account: r.account,
                difference: r.difference,
            });
        let tickets = (self.ticket_conflicts > 0).then_some(OpenItem::TicketConflicts {
            count: self.ticket_conflicts,
        });
        people
            .chain(receipts)
            .chain(cash)
            .chain(statements)
            .chain(tickets)
            .collect()
    }
}
