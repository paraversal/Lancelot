//! Everything derived from the transactions (R-COR-4): balances, person summaries, totals,
//! the result and where the money is. Nothing here is stored; it is recomputed on every change.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Account, AccountId, AccountKind, Txn, TxnKind};
use crate::money::{Cents, MoneyError};

/// Per-person figures for the People view (R-PER-3, R-TX-5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
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
    /// `advanced − held − reimbursed + repaid`. Positive: the event owes the person.
    pub net: Cents,
    pub status: PersonStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonStatus {
    /// The event owes the person.
    Owed,
    /// The person owes the event.
    Owes,
    #[default]
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct AccountBalance {
    pub account: AccountId,
    pub balance: Cents,
}

/// The result, broken down by where the money sits. `total` always equals the result:
/// `assets + held_by_people − owed_to_people − opening`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoneyLocation {
    pub assets: Vec<AccountBalance>,
    pub held_by_people: Cents,
    pub owed_to_people: Cents,
    /// Opening counts brought in. They are not part of the event's result.
    pub opening: Cents,
    pub total: Cents,
}

/// Something that keeps the event from being closed (R-SET-5). Missing receipts and the cash
/// box closing count join in M1c, reconciliation and ticket conflicts in phase 2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OpenItem {
    PersonBalance { person: AccountId, net: Cents },
}

/// An open item explicitly accepted when closing the event (R-SET-7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl Ledger {
    /// Computes every figure. Fails only if a sum leaves the valid amount range.
    pub fn compute<'a>(
        accounts: &BTreeMap<AccountId, Account>,
        txns: impl IntoIterator<Item = &'a Txn>,
    ) -> Result<Ledger, MoneyError> {
        let mut raw: BTreeMap<AccountId, Cents> =
            accounts.keys().map(|id| (*id, Cents::ZERO)).collect();
        let mut persons: BTreeMap<AccountId, PersonSummary> = accounts
            .values()
            .filter(|a| a.kind == AccountKind::Person)
            .map(|a| (a.id, PersonSummary::default()))
            .collect();

        for txn in txns {
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

    pub fn open_items(&self) -> Vec<OpenItem> {
        self.persons
            .iter()
            .filter(|(_, p)| !p.net.is_zero())
            .map(|(id, p)| OpenItem::PersonBalance {
                person: *id,
                net: p.net,
            })
            .collect()
    }
}
