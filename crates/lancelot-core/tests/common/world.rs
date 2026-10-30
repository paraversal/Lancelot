//! Random sequences of entries, edits and deletions for the property tests.

use lancelot_core::*;
use proptest::prelude::*;

use super::draft;

/// The accounts random entries are drawn from.
pub struct Ids {
    pub assets: Vec<AccountId>,
    pub persons: Vec<AccountId>,
    pub incomes: Vec<AccountId>,
    pub expenses: Vec<AccountId>,
}

/// An entry described by indices, so proptest can shrink it.
#[derive(Clone, Debug)]
pub struct Spec {
    pub kind: u8,
    pub a: usize,
    pub b: usize,
    pub amounts: Vec<i64>,
    pub flag: bool,
}

#[derive(Clone, Debug)]
pub enum Op {
    Add(Spec),
    Edit(usize, Spec),
    Delete(usize),
}

pub fn spec() -> impl Strategy<Value = Spec> {
    (
        0u8..5,
        0usize..10,
        0usize..10,
        prop::collection::vec(1i64..2_000_000, 1..4),
        any::<bool>(),
    )
        .prop_map(|(kind, a, b, amounts, flag)| Spec {
            kind,
            a,
            b,
            amounts,
            flag,
        })
}

pub fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => spec().prop_map(Op::Add),
        1 => (0usize..50, spec()).prop_map(|(i, s)| Op::Edit(i, s)),
        1 => (0usize..50).prop_map(Op::Delete),
    ]
}

/// What an [`Op`] means for the current transactions.
pub enum Action {
    Save(TxnDraft, Option<TxnId>),
    Delete(TxnId),
    Nothing,
}

impl Ids {
    pub fn of(book: &Book) -> Ids {
        let of = |kind| {
            book.accounts()
                .filter(|a| a.kind == kind)
                .map(|a| a.id)
                .collect::<Vec<_>>()
        };
        Ids {
            assets: of(AccountKind::Asset),
            persons: of(AccountKind::Person),
            incomes: of(AccountKind::Income),
            expenses: of(AccountKind::Expense),
        }
    }

    pub fn entry(&self, s: &Spec) -> Entry {
        let pick = |list: &[AccountId], i: usize| list[i % list.len()];
        let payer = if s.flag {
            pick(&self.persons, s.a)
        } else {
            pick(&self.assets, s.a)
        };
        let lines = |cats: &[AccountId]| {
            s.amounts
                .iter()
                .enumerate()
                .map(|(i, c)| SplitLine {
                    account: pick(cats, s.b + i),
                    amount: Cents::new(*c).unwrap(),
                    memo: None,
                    budget_line: None,
                    cost_centre: None,
                })
                .collect()
        };
        let amount = Cents::new(s.amounts[0]).unwrap();
        match s.kind {
            0 => Entry::Expense {
                payer,
                lines: lines(&self.expenses),
            },
            1 => Entry::Income {
                destination: payer,
                lines: lines(&self.incomes),
            },
            2 => Entry::Transfer {
                from: payer,
                to: pick(&self.assets, s.b),
                amount,
            },
            3 => Entry::Settle {
                person: pick(&self.persons, s.a),
                account: pick(&self.assets, s.b),
                amount,
                direction: if s.flag {
                    SettleDirection::Reimburse
                } else {
                    SettleDirection::Repay
                },
            },
            _ => Entry::Opening {
                account: pick(&self.assets, s.a),
                amount,
            },
        }
    }

    pub fn action(&self, book: &Book, op: &Op) -> Action {
        let ids: Vec<TxnId> = book.txns().map(|t| t.id).collect();
        match op {
            Op::Add(s) => Action::Save(draft("p", self.entry(s)), None),
            Op::Edit(i, s) if !ids.is_empty() => {
                Action::Save(draft("p", self.entry(s)), Some(ids[i % ids.len()]))
            }
            Op::Delete(i) if !ids.is_empty() => Action::Delete(ids[i % ids.len()]),
            _ => Action::Nothing,
        }
    }
}

/// Applies an op to a book. Invalid entries (e.g. a transfer to the same account) are
/// rejected and change nothing.
pub fn apply(ids: &Ids, book: &mut Book, op: &Op) {
    let _ = match ids.action(book, op) {
        Action::Save(d, id) => book.save_txn(&d, id).map(|_| ()),
        Action::Delete(id) => book.delete_txn(id).map(|_| ()),
        Action::Nothing => Ok(()),
    };
}
