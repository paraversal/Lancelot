//! Everything derived from the transactions (R-COR-4): balances, person summaries, totals,
//! the result and where the money is. Nothing here is stored; it is recomputed on every change.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{Account, AccountId, AccountKind, ClosingCount, Role, Txn, TxnId, TxnKind};
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

/// The closing count of an asset account against its computed balance (R-ISO-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CashStatus {
    pub account: AccountId,
    pub computed: Cents,
    pub counted: Option<Cents>,
    /// `counted − computed`: positive when there is more money than the books say.
    pub difference: Option<Cents>,
}

/// Something that keeps the event from being closed (R-SET-5). Reconciliation mismatches and
/// ticket conflicts join in phase 2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OpenItem {
    /// A person the event owes money to, or who owes the event.
    PersonBalance { person: AccountId, net: Cents },
    /// An expense without an attachment (R-ATT-4).
    MissingReceipt { txn: TxnId },
    /// The cash box has no closing count yet.
    ClosingCountMissing { account: AccountId },
    /// The closing count differs from the computed balance.
    CashDifference {
        account: AccountId,
        difference: Cents,
    },
}

/// What [`Ledger::compute`] needs from the book.
pub struct LedgerInput<'a> {
    pub accounts: &'a BTreeMap<AccountId, Account>,
    pub txns: &'a BTreeMap<TxnId, Txn>,
    pub closing_counts: &'a BTreeMap<AccountId, ClosingCount>,
    /// Transactions that have at least one attachment.
    pub with_attachments: &'a BTreeSet<TxnId>,
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
    missing_receipts: Vec<TxnId>,
    person_entries: BTreeMap<AccountId, Vec<PersonEntry>>,
    person_totals: PersonTotals,
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
            with_attachments,
        } = input;
        let mut raw: BTreeMap<AccountId, Cents> =
            accounts.keys().map(|id| (*id, Cents::ZERO)).collect();
        let mut persons: BTreeMap<AccountId, PersonSummary> = accounts
            .values()
            .filter(|a| a.kind == AccountKind::Person)
            .map(|a| (a.id, PersonSummary::default()))
            .collect();

        for txn in txns.values() {
            for line in &txn.lines {
                let balance = raw.entry(line.account).or_default();
                *balance = balance.checked_add(line.amount)?;

                if let Some(p) = persons.get_mut(&line.account) {
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

        let mut cash = Vec::new();
        for a in accounts.values().filter(|a| a.kind == AccountKind::Asset) {
            let counted = closing_counts.get(&a.id).map(|c| c.amount);
            if counted.is_none() && a.role != Some(Role::CashBox) {
                continue;
            }
            let computed = displayed.get(&a.id).copied().unwrap_or_default();
            let difference = counted.map(|c| c.checked_sub(computed)).transpose()?;
            cash.push(CashStatus {
                account: a.id,
                computed,
                counted,
                difference,
            });
        }

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
            missing_receipts,
            person_entries,
            person_totals,
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

    /// Closing counts against computed balances: the cash box, plus every asset account
    /// that has a closing count.
    pub fn cash(&self) -> &[CashStatus] {
        &self.cash
    }

    /// Expense transactions without an attachment (R-ATT-4).
    pub fn missing_receipts(&self) -> &[TxnId] {
        &self.missing_receipts
    }

    /// People first, then receipts, then cash.
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
            None => Some(OpenItem::ClosingCountMissing { account: c.account }),
            Some(d) if !d.is_zero() => Some(OpenItem::CashDifference {
                account: c.account,
                difference: d,
            }),
            Some(_) => None,
        });
        people.chain(receipts).chain(cash).collect()
    }
}
