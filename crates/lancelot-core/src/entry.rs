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
    Account, AccountId, AccountKind, BudgetLineId, CostCentreId, Flow, Line, Origin, Txn, TxnKind,
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
    /// Money from any account to any other: assets, people and categories (not the opening
    /// account). From A to B: A is owed the amount and B owes it.
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
    /// Must belong to the line's category (R-BUD-2).
    #[serde(rename = "budgetLine", default)]
    pub budget_line: Option<BudgetLineId>,
    /// Must match the budget line's, if there is one (R-CC-1).
    #[serde(rename = "costCentre", default)]
    pub cost_centre: Option<CostCentreId>,
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

    /// Who paid or received: the payer, the destination, the source, the person settling, the
    /// account counted.
    pub fn who(&self) -> AccountId {
        match self {
            Entry::Expense { payer, .. } => *payer,
            Entry::Income { destination, .. } => *destination,
            Entry::Transfer { from, .. } => *from,
            Entry::Settle { person, .. } => *person,
            Entry::Opening { account, .. } => *account,
        }
    }
}

/// The form a stored transaction shows in: the reverse of [`build_lines`] for typed-in entries,
/// `None` for lines no form can show. Generated income and refunds show as income, with their fee
/// lines among the splits and the refund's payer as the destination; they cannot be saved.
pub fn entry_of(txn: &Txn, accounts: &BTreeMap<AccountId, Account>) -> Option<Entry> {
    let entry = shown_as(txn, accounts)?;
    let same = |lines: Vec<Line>| {
        let key = |l: &Line| (l.account, l.amount);
        lines.iter().map(key).eq(txn.lines.iter().map(key))
    };
    let shows_it = txn.origin.is_generated() || build_lines(&entry, accounts).is_ok_and(same);
    shows_it.then_some(entry)
}

fn shown_as(txn: &Txn, accounts: &BTreeMap<AccountId, Account>) -> Option<Entry> {
    let kind = |l: &Line| accounts.get(&l.account).map(|a| a.kind);
    let splits = |lines: &[Line]| {
        lines
            .iter()
            .map(|l| SplitLine {
                account: l.account,
                amount: l.amount.abs(),
                budget_line: l.budget_line,
                cost_centre: l.cost_centre,
            })
            .collect()
    };
    Some(match (txn.kind, txn.lines.as_slice()) {
        (TxnKind::Expense, [payer, lines @ ..]) => Entry::Expense {
            payer: payer.account,
            lines: splits(lines),
        },
        (TxnKind::Income, [lines @ .., destination]) => Entry::Income {
            destination: destination.account,
            lines: splits(lines),
        },
        (TxnKind::Refund, [lines @ .., payer]) => Entry::Income {
            destination: payer.account,
            lines: splits(lines),
        },
        (TxnKind::Transfer, [from, to]) => Entry::Transfer {
            from: from.account,
            to: to.account,
            amount: to.amount,
        },
        (TxnKind::Settle, [out, into]) => {
            let (person, account, direction) = if kind(into) == Some(AccountKind::Person) {
                (into, out, SettleDirection::Reimburse)
            } else {
                (out, into, SettleDirection::Repay)
            };
            Entry::Settle {
                person: person.account,
                account: account.account,
                amount: into.amount,
                direction,
            }
        }
        (TxnKind::Opening, [_, account]) => Entry::Opening {
            account: account.account,
            amount: account.amount,
        },
        _ => return None,
    })
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
            check(*from, "from", &[Asset, Person, Income, Expense])?;
            check(*to, "to", &[Asset, Person, Income, Expense])?;
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
                .find(|a| a.kind == AccountKind::Opening)
                .ok_or_else(|| DomainError::UnknownAccount {
                    field: "account".to_owned(),
                })?;
            vec![line(opening.id, -*amount), line(*account, *amount)]
        }
    };
    Ok(lines)
}

/// Checks a stored transaction. The kind is only a label: what matters is that its lines balance,
/// refer to known accounts and keep the opening account to opening counts (R-COR-2). A refund comes
/// only from a cancellation (R-TKT-15).
pub fn txn_follows_rules(txn: &Txn, accounts: &BTreeMap<AccountId, Account>) -> bool {
    lines_follow_rules(txn.kind, &txn.lines, accounts)
        && (txn.kind == TxnKind::Refund) == matches!(txn.origin, Origin::TicketRefund { .. })
}

/// The part of [`txn_follows_rules`] that depends on the lines. Used when loading an event file and
/// by the integrity check.
pub fn lines_follow_rules(
    kind: TxnKind,
    lines: &[Line],
    accounts: &BTreeMap<AccountId, Account>,
) -> bool {
    use AccountKind::{Asset, Opening};

    let kinds: Option<Vec<AccountKind>> = lines
        .iter()
        .map(|l| accounts.get(&l.account).map(|a| a.kind))
        .collect();
    let Some(kinds) = kinds else { return false };
    let sum: i128 = lines.iter().map(|l| i128::from(l.amount.get())).sum();
    if lines.len() < 2 || sum != 0 || lines.iter().any(|l| l.amount.is_zero()) {
        return false;
    }
    if kind == TxnKind::Opening {
        return kinds == [Opening, Asset] && lines[1].amount.is_positive();
    }
    !kinds.contains(&Opening)
}

/// Which way the lines move the result: what they do to the income and expense categories.
pub fn flow_of(lines: &[Line], accounts: &BTreeMap<AccountId, Account>) -> Flow {
    let effect: i128 = lines
        .iter()
        .filter(|l| {
            accounts
                .get(&l.account)
                .is_some_and(|a| matches!(a.kind, AccountKind::Income | AccountKind::Expense))
        })
        .map(|l| -i128::from(l.amount.get()))
        .sum();
    match effect.signum() {
        1 => Flow::In,
        -1 => Flow::Out,
        _ => Flow::Moved,
    }
}

fn line(account: AccountId, amount: Cents) -> Line {
    Line {
        account,
        amount,
        budget_line: None,
        cost_centre: None,
    }
}

fn split(l: &SplitLine, amount: Cents) -> Line {
    Line {
        account: l.account,
        amount,
        budget_line: l.budget_line,
        cost_centre: l.cost_centre,
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
    use crate::model::{AssetType, TxnId};

    fn accounts() -> BTreeMap<AccountId, Account> {
        [
            (1, AccountKind::Asset),
            (2, AccountKind::Asset),
            (3, AccountKind::Opening),
            (4, AccountKind::Income),
            (5, AccountKind::Expense),
            (6, AccountKind::Expense),
            (7, AccountKind::Person),
        ]
        .into_iter()
        .map(|(id, kind)| {
            let id = AccountId(id);
            (
                id,
                Account {
                    id,
                    kind,
                    name: format!("a{}", id.0),
                    role: None,
                    asset_type: (kind == AccountKind::Asset).then_some(AssetType::Bank),
                    net_movement: false,
                    cost_centre: None,
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
            budget_line: None,
            cost_centre: None,
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
                to: AccountId(3),
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
    fn a_transfer_goes_from_any_account_to_any_other() {
        let a = accounts();
        for (from, to) in [
            (ANNA, AccountId(8)),
            (VEREIN, ANNA),
            (CASH, FOOD),
            (TICKETS, FOOD),
        ] {
            let mut a = a.clone();
            a.insert(
                AccountId(8),
                Account {
                    id: AccountId(8),
                    kind: AccountKind::Person,
                    name: "a8".into(),
                    role: None,
                    asset_type: None,
                    net_movement: false,
                    cost_centre: None,
                },
            );
            let entry = Entry::Transfer {
                from,
                to,
                amount: c(5_000),
            };
            let lines = build_lines(&entry, &a).unwrap();
            assert_eq!(
                amounts(&lines),
                vec![(from.0, -5_000), (to.0, 5_000)],
                "{from:?} to {to:?}"
            );
            assert!(lines_follow_rules(TxnKind::Transfer, &lines, &a));
        }
    }

    #[test]
    fn the_flow_follows_what_the_lines_do_to_the_result() {
        let a = accounts();
        let flow = |from, to| {
            let entry = Entry::Transfer {
                from,
                to,
                amount: c(100),
            };
            flow_of(&build_lines(&entry, &a).unwrap(), &a)
        };
        assert_eq!(flow(CASH, ANNA), Flow::Moved);
        assert_eq!(flow(CASH, FOOD), Flow::Out);
        assert_eq!(flow(TICKETS, CASH), Flow::In);
        assert_eq!(flow(FOOD, CASH), Flow::In);
        assert_eq!(flow(TICKETS, FOOD), Flow::Moved);
    }

    #[test]
    fn the_opening_account_stays_with_opening_counts() {
        let a = accounts();
        let opening = build_lines(
            &Entry::Opening {
                account: CASH,
                amount: c(100),
            },
            &a,
        )
        .unwrap();
        assert!(lines_follow_rules(TxnKind::Opening, &opening, &a));
        assert!(!lines_follow_rules(TxnKind::Transfer, &opening, &a));
        let transfer = build_lines(
            &Entry::Transfer {
                from: VEREIN,
                to: CASH,
                amount: c(100),
            },
            &a,
        )
        .unwrap();
        assert!(!lines_follow_rules(TxnKind::Opening, &transfer, &a));
    }

    #[test]
    fn only_tickets_make_a_refund() {
        let lines = build_lines(
            &Entry::Income {
                destination: CASH,
                lines: vec![s(TICKETS, 100)],
            },
            &accounts(),
        )
        .unwrap();
        let txn = |kind, origin| Txn {
            id: TxnId(1),
            kind,
            date: time::macros::date!(2026 - 06 - 13),
            description: String::new(),
            counterparty: None,
            notes: None,
            lines: lines.clone(),
            origin,
        };
        let refund = Origin::TicketRefund {
            cancellation: crate::model::CancellationId(1),
        };
        let a = accounts();
        assert!(txn_follows_rules(&txn(TxnKind::Income, Origin::Manual), &a));
        assert!(!txn_follows_rules(
            &txn(TxnKind::Refund, Origin::Manual),
            &a
        ));
        assert!(txn_follows_rules(&txn(TxnKind::Refund, refund), &a));
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
