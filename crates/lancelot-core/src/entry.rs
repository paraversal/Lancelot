//! Typed entry forms (R-TX-3): what the treasurer fills in, turned into balanced lines.
//!
//! The treasurer never chooses accounts by kind. Each form knows which kinds may appear in
//! which place and builds the lines in the direction of the flow table in requirements §4.1:
//! the source line comes first and is negative, the destination lines follow and are positive.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::Date;
use ts_rs::TS;

use crate::error::DomainError;
use crate::model::{
    Account, AccountId, AccountKind, BudgetLineId, Line, Origin, Role, Txn, TxnKind,
};
use crate::money::{Cents, try_sum};

/// Everything a transaction form produces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TxnDraft {
    #[ts(type = "string")]
    pub date: Date,
    pub description: String,
    pub counterparty: Option<String>,
    pub notes: Option<String>,
    pub entry: Entry,
    /// The total on the receipt, when the form has one. The entry's amount (the sum of its
    /// split lines) must equal it, so a split that does not add up cannot be saved.
    #[serde(default)]
    pub total: Option<Cents>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Entry {
    /// Paid by an asset account or a person, for one or more expense categories.
    Expense {
        payer: AccountId,
        lines: Vec<SplitLine>,
    },
    /// Earned in one or more income categories, received by an asset account or a person.
    Income {
        destination: AccountId,
        lines: Vec<SplitLine>,
    },
    /// Money moving into an asset account: between asset accounts, or a person putting their
    /// own money in (an advance). Money going *to* a person is always a [`Entry::Settle`].
    Transfer {
        from: AccountId,
        to: AccountId,
        amount: Cents,
    },
    /// Settles a person's net balance, wholly or partly (R-TX-4). The direction is explicit;
    /// the UI pre-fills it from the person's balance.
    Settle {
        person: AccountId,
        account: AccountId,
        amount: Cents,
        direction: SettleDirection,
    },
    /// Brings an opening count into an asset account (R-ISO-3).
    Opening { account: AccountId, amount: Cents },
}

/// One category line of a split expense or income (R-TX-2). The amount is positive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct SplitLine {
    pub account: AccountId,
    pub amount: Cents,
    pub memo: Option<String>,
    /// Must belong to the line's category (R-BUD-2).
    #[serde(rename = "budgetLine", default)]
    pub budget_line: Option<BudgetLineId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum SettleDirection {
    /// The event pays the person: asset → person.
    Reimburse,
    /// The person pays the event: person → asset.
    Repay,
}

impl Entry {
    pub fn kind(&self) -> TxnKind {
        match self {
            Entry::Expense { .. } => TxnKind::Expense,
            Entry::Income { .. } => TxnKind::Income,
            Entry::Transfer { .. } => TxnKind::Transfer,
            Entry::Settle { .. } => TxnKind::Settle,
            Entry::Opening { .. } => TxnKind::Opening,
        }
    }
}

/// The amount a transaction moves: the sum of its incoming lines. For an expense that is the
/// receipt total, for a transfer or settle its amount.
pub fn txn_amount(lines: &[Line]) -> Cents {
    // Lines are validated and balanced, so the sum stays within the range of one amount.
    try_sum(lines.iter().map(|l| l.amount).filter(|a| a.is_positive())).unwrap_or_default()
}

/// Validates an entry against the event's accounts and returns its balanced lines.
pub fn build_lines(
    entry: &Entry,
    accounts: &BTreeMap<AccountId, Account>,
) -> Result<Vec<Line>, DomainError> {
    use AccountKind::{Asset, Expense, Income, Person};

    let check = |id: AccountId, field: &str, allowed: &[AccountKind]| {
        let account = accounts
            .get(&id)
            .ok_or_else(|| DomainError::UnknownAccount {
                field: field.to_owned(),
            })?;
        if allowed.contains(&account.kind) {
            Ok(())
        } else {
            Err(DomainError::WrongAccountKind {
                field: field.to_owned(),
            })
        }
    };

    let lines = match entry {
        Entry::Expense { payer, lines } => {
            check(*payer, "payer", &[Asset, Person])?;
            let total = check_splits(lines, |id, field| check(id, field, &[Expense]))?;
            let mut out = vec![line(*payer, -total)];
            out.extend(lines.iter().map(|l| split(l, l.amount)));
            out
        }
        Entry::Income { destination, lines } => {
            check(*destination, "destination", &[Asset, Person])?;
            let total = check_splits(lines, |id, field| check(id, field, &[Income]))?;
            let mut out: Vec<Line> = lines.iter().map(|l| split(l, -l.amount)).collect();
            out.push(line(*destination, total));
            out
        }
        Entry::Transfer { from, to, amount } => {
            check(*from, "from", &[Asset, Person])?;
            check(*to, "to", &[Asset])?;
            positive(*amount, "amount")?;
            if from == to {
                return Err(DomainError::SameAccount);
            }
            vec![line(*from, -*amount), line(*to, *amount)]
        }
        Entry::Settle {
            person,
            account,
            amount,
            direction,
        } => {
            check(*person, "person", &[Person])?;
            check(*account, "account", &[Asset])?;
            positive(*amount, "amount")?;
            match direction {
                SettleDirection::Reimburse => {
                    vec![line(*account, -*amount), line(*person, *amount)]
                }
                SettleDirection::Repay => vec![line(*person, -*amount), line(*account, *amount)],
            }
        }
        Entry::Opening { account, amount } => {
            check(*account, "account", &[Asset])?;
            positive(*amount, "amount")?;
            let opening = accounts
                .values()
                .find(|a| a.role == Some(Role::Opening))
                .ok_or_else(|| DomainError::UnknownAccount {
                    field: "account".to_owned(),
                })?;
            vec![line(opening.id, -*amount), line(*account, *amount)]
        }
    };
    Ok(lines)
}

/// Checks a stored transaction: typed-in ones follow [`lines_follow_rules`], and ticket income
/// is an income line, optionally fee lines, and the account that received the money (R-TKT-6).
/// A refund is the opposite: negative income, paid by one account (R-TKT-15).
pub fn txn_follows_rules(txn: &Txn, accounts: &BTreeMap<AccountId, Account>) -> bool {
    if txn.origin == Origin::Manual {
        return lines_follow_rules(txn.kind, &txn.lines, accounts);
    }
    let lines = &txn.lines;
    let kind = |l: &Line| accounts.get(&l.account).map(|a| a.kind);
    if let Origin::TicketRefund { .. } = txn.origin {
        return match lines.as_slice() {
            [income, payer] => {
                txn.kind == TxnKind::Income
                    && kind(income) == Some(AccountKind::Income)
                    && income.amount.is_positive()
                    && matches!(kind(payer), Some(AccountKind::Asset | AccountKind::Person))
                    && payer.amount == -income.amount
            }
            _ => false,
        };
    }
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return false;
    };
    txn.kind == TxnKind::Income
        && lines.len() >= 2
        && kind(first) == Some(AccountKind::Income)
        && first.amount.is_negative()
        && matches!(kind(last), Some(AccountKind::Asset | AccountKind::Person))
        && last.amount.is_positive()
        && lines[1..lines.len() - 1]
            .iter()
            .all(|l| kind(l) == Some(AccountKind::Expense) && l.amount.is_positive())
        && lines
            .iter()
            .map(|l| i128::from(l.amount.get()))
            .sum::<i128>()
            == 0
}

/// Checks stored lines against the shape [`build_lines`] produces for their kind. Used when
/// loading an event file and by the integrity check, so a file edited by hand cannot smuggle
/// in a transaction the forms could never create.
pub fn lines_follow_rules(
    kind: TxnKind,
    lines: &[Line],
    accounts: &BTreeMap<AccountId, Account>,
) -> bool {
    use AccountKind::{Asset, Expense, Income, Opening, Person};

    let kinds: Option<Vec<AccountKind>> = lines
        .iter()
        .map(|l| accounts.get(&l.account).map(|a| a.kind))
        .collect();
    let Some(kinds) = kinds else { return false };
    let sum: i128 = lines.iter().map(|l| i128::from(l.amount.get())).sum();
    if lines.len() < 2 || sum != 0 || lines.iter().any(|l| l.amount.is_zero()) {
        return false;
    }
    if kind != TxnKind::Opening && kinds.contains(&Opening) {
        return false;
    }
    let is = |i: usize, allowed: &[AccountKind], positive: bool| {
        allowed.contains(&kinds[i]) && lines[i].amount.is_positive() == positive
    };
    let last = lines.len() - 1;
    match kind {
        TxnKind::Expense => {
            is(0, &[Asset, Person], false) && (1..=last).all(|i| is(i, &[Expense], true))
        }
        TxnKind::Income => {
            is(last, &[Asset, Person], true) && (0..last).all(|i| is(i, &[Income], false))
        }
        TxnKind::Transfer => {
            lines.len() == 2
                && is(0, &[Asset, Person], false)
                && is(1, &[Asset], true)
                && lines[0].account != lines[1].account
        }
        TxnKind::Settle => {
            lines.len() == 2 && (kinds == [Person, Asset] || kinds == [Asset, Person])
        }
        TxnKind::Opening => lines.len() == 2 && is(0, &[Opening], false) && is(1, &[Asset], true),
    }
}

fn line(account: AccountId, amount: Cents) -> Line {
    Line {
        account,
        amount,
        memo: None,
        budget_line: None,
    }
}

fn split(l: &SplitLine, amount: Cents) -> Line {
    Line {
        account: l.account,
        amount,
        memo: l.memo.clone(),
        budget_line: l.budget_line,
    }
}

fn positive(amount: Cents, field: &str) -> Result<(), DomainError> {
    if amount.is_positive() {
        Ok(())
    } else {
        Err(DomainError::NonPositiveAmount {
            field: field.to_owned(),
        })
    }
}

/// Checks every split line and returns their total.
fn check_splits(
    lines: &[SplitLine],
    check_account: impl Fn(AccountId, &str) -> Result<(), DomainError>,
) -> Result<Cents, DomainError> {
    if lines.is_empty() {
        return Err(DomainError::NoLines);
    }
    for (i, l) in lines.iter().enumerate() {
        check_account(l.account, &format!("lines[{i}].account"))?;
        positive(l.amount, &format!("lines[{i}].amount"))?;
    }
    Ok(try_sum(lines.iter().map(|l| l.amount))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AssetType;

    fn accounts() -> BTreeMap<AccountId, Account> {
        [
            (1, AccountKind::Asset, Some(Role::Verein)),
            (2, AccountKind::Asset, Some(Role::CashBox)),
            (3, AccountKind::Opening, Some(Role::Opening)),
            (4, AccountKind::Income, None),
            (5, AccountKind::Expense, None),
            (6, AccountKind::Expense, None),
            (7, AccountKind::Person, None),
        ]
        .into_iter()
        .map(|(id, kind, role)| {
            let id = AccountId(id);
            (
                id,
                Account {
                    id,
                    kind,
                    name: format!("a{}", id.0),
                    role,
                    asset_type: (kind == AccountKind::Asset).then_some(AssetType::Bank),
                },
            )
        })
        .collect()
    }

    const VEREIN: AccountId = AccountId(1);
    const CASH: AccountId = AccountId(2);
    const TICKETS: AccountId = AccountId(4);
    const FOOD: AccountId = AccountId(5);
    const TECH: AccountId = AccountId(6);
    const ANNA: AccountId = AccountId(7);

    fn c(cents: i64) -> Cents {
        Cents::new(cents).unwrap()
    }

    fn s(account: AccountId, cents: i64) -> SplitLine {
        SplitLine {
            account,
            amount: c(cents),
            memo: None,
            budget_line: None,
        }
    }

    fn amounts(lines: &[Line]) -> Vec<(i64, i64)> {
        lines
            .iter()
            .map(|l| (l.account.0, l.amount.get()))
            .collect()
    }

    #[test]
    fn expense_with_splits_balances() {
        let entry = Entry::Expense {
            payer: ANNA,
            lines: vec![s(FOOD, 28_040), s(TECH, 3_200)],
        };
        let lines = build_lines(&entry, &accounts()).unwrap();
        assert_eq!(amounts(&lines), vec![(7, -31_240), (5, 28_040), (6, 3_200)]);
    }

    #[test]
    fn income_goes_to_destination() {
        let entry = Entry::Income {
            destination: ANNA,
            lines: vec![s(TICKETS, 30_000)],
        };
        let lines = build_lines(&entry, &accounts()).unwrap();
        assert_eq!(amounts(&lines), vec![(4, -30_000), (7, 30_000)]);
    }

    #[test]
    fn settle_direction_decides_the_flow() {
        let reimburse = Entry::Settle {
            person: ANNA,
            account: VEREIN,
            amount: c(21_240),
            direction: SettleDirection::Reimburse,
        };
        assert_eq!(
            amounts(&build_lines(&reimburse, &accounts()).unwrap()),
            vec![(1, -21_240), (7, 21_240)]
        );
        let repay = Entry::Settle {
            person: ANNA,
            account: VEREIN,
            amount: c(18_000),
            direction: SettleDirection::Repay,
        };
        assert_eq!(
            amounts(&build_lines(&repay, &accounts()).unwrap()),
            vec![(7, -18_000), (1, 18_000)]
        );
    }

    #[test]
    fn opening_uses_the_opening_account() {
        let entry = Entry::Opening {
            account: CASH,
            amount: c(8_000),
        };
        assert_eq!(
            amounts(&build_lines(&entry, &accounts()).unwrap()),
            vec![(3, -8_000), (2, 8_000)]
        );
    }

    #[test]
    fn rejects_wrong_kinds_with_the_field() {
        let a = accounts();
        let err = |entry: Entry| build_lines(&entry, &a).unwrap_err();

        assert_eq!(
            err(Entry::Expense {
                payer: TICKETS,
                lines: vec![s(FOOD, 100)]
            }),
            DomainError::WrongAccountKind {
                field: "payer".into()
            }
        );
        assert_eq!(
            err(Entry::Expense {
                payer: ANNA,
                lines: vec![s(FOOD, 100), s(TICKETS, 100)]
            }),
            DomainError::WrongAccountKind {
                field: "lines[1].account".into()
            }
        );
        assert_eq!(
            err(Entry::Transfer {
                from: VEREIN,
                to: ANNA,
                amount: c(100)
            }),
            DomainError::WrongAccountKind { field: "to".into() }
        );
        assert_eq!(
            err(Entry::Settle {
                person: CASH,
                account: VEREIN,
                amount: c(100),
                direction: SettleDirection::Repay
            }),
            DomainError::WrongAccountKind {
                field: "person".into()
            }
        );
        assert_eq!(
            err(Entry::Income {
                destination: AccountId(99),
                lines: vec![s(TICKETS, 100)]
            }),
            DomainError::UnknownAccount {
                field: "destination".into()
            }
        );
    }

    #[test]
    fn rejects_bad_amounts_and_shapes() {
        let a = accounts();
        let err = |entry: Entry| build_lines(&entry, &a).unwrap_err();

        assert_eq!(
            err(Entry::Expense {
                payer: ANNA,
                lines: vec![]
            }),
            DomainError::NoLines
        );
        assert_eq!(
            err(Entry::Expense {
                payer: ANNA,
                lines: vec![s(FOOD, 100), s(FOOD, 0)]
            }),
            DomainError::NonPositiveAmount {
                field: "lines[1].amount".into()
            }
        );
        assert_eq!(
            err(Entry::Transfer {
                from: CASH,
                to: CASH,
                amount: c(100)
            }),
            DomainError::SameAccount
        );
        assert_eq!(
            err(Entry::Opening {
                account: CASH,
                amount: c(-100)
            }),
            DomainError::NonPositiveAmount {
                field: "amount".into()
            }
        );
    }

    #[test]
    fn every_entry_balances() {
        let a = accounts();
        for entry in [
            Entry::Expense {
                payer: CASH,
                lines: vec![s(FOOD, 1), s(TECH, 99_999)],
            },
            Entry::Income {
                destination: VEREIN,
                lines: vec![s(TICKETS, 652_750)],
            },
            Entry::Transfer {
                from: ANNA,
                to: CASH,
                amount: c(20_000),
            },
            Entry::Opening {
                account: CASH,
                amount: c(8_000),
            },
        ] {
            let lines = build_lines(&entry, &a).unwrap();
            assert_eq!(
                try_sum(lines.iter().map(|l| l.amount)).unwrap(),
                Cents::ZERO
            );
        }
    }
}
