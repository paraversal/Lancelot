//! Property tests over random sequences of entries, edits and deletions (R-COR-6).

mod common;

use common::*;
use lancelot_core::*;
use proptest::prelude::*;

struct World {
    book: Book,
    assets: Vec<AccountId>,
    persons: Vec<AccountId>,
    incomes: Vec<AccountId>,
    expenses: Vec<AccountId>,
}

fn world() -> World {
    let mut book = new_book();
    let persons = ["Anna", "Bob", "Malte"]
        .map(|n| person(&mut book, n))
        .to_vec();
    let of = |kind| {
        book.accounts()
            .filter(|a| a.kind == kind)
            .map(|a| a.id)
            .collect::<Vec<_>>()
    };
    World {
        assets: of(AccountKind::Asset),
        incomes: of(AccountKind::Income),
        expenses: of(AccountKind::Expense),
        persons,
        book,
    }
}

/// An entry described by indices, so proptest can shrink it.
#[derive(Clone, Debug)]
struct Spec {
    kind: u8,
    a: usize,
    b: usize,
    amounts: Vec<i64>,
    flag: bool,
}

#[derive(Clone, Debug)]
enum Op {
    Add(Spec),
    Edit(usize, Spec),
    Delete(usize),
}

fn spec() -> impl Strategy<Value = Spec> {
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

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => spec().prop_map(Op::Add),
        1 => (0usize..50, spec()).prop_map(|(i, s)| Op::Edit(i, s)),
        1 => (0usize..50).prop_map(Op::Delete),
    ]
}

impl World {
    fn entry(&self, s: &Spec) -> Entry {
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

    fn apply(&mut self, op: &Op) {
        let ids: Vec<TxnId> = self.book.txns().map(|t| t.id).collect();
        // Invalid entries (e.g. a transfer to the same account) are rejected and change nothing.
        let _ = match op {
            Op::Add(s) => self
                .book
                .save_txn(&draft("p", self.entry(s)), None)
                .map(|_| ()),
            Op::Edit(i, s) if !ids.is_empty() => self
                .book
                .save_txn(&draft("p", self.entry(s)), Some(ids[i % ids.len()]))
                .map(|_| ()),
            Op::Delete(i) if !ids.is_empty() => {
                self.book.delete_txn(ids[i % ids.len()]).map(|_| ())
            }
            _ => Ok(()),
        };
    }
}

fn check_invariants(book: &Book) -> Result<(), TestCaseError> {
    let ledger = book.ledger();
    prop_assert_eq!(raw_sum(book), 0, "all raw balances sum to zero");
    for txn in book.txns() {
        prop_assert_eq!(
            txn.lines.iter().map(|l| l.amount.get()).sum::<i64>(),
            0,
            "every transaction balances"
        );
    }
    for (id, p) in ledger.persons() {
        prop_assert_eq!(
            p.net,
            ledger.balance(id),
            "net balance is the displayed person balance"
        );
        prop_assert_eq!(p.net, -ledger.raw_balance(id));
    }
    prop_assert_eq!(
        ledger.result(),
        ledger
            .income_total()
            .checked_sub(ledger.expense_total())
            .unwrap()
    );
    prop_assert_eq!(
        ledger.where_the_money_is().total,
        ledger.result(),
        "where the money is adds up to the result"
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn invariants_hold_after_every_operation(ops in prop::collection::vec(op(), 1..40)) {
        let mut w = world();
        for op in &ops {
            w.apply(op);
            check_invariants(&w.book)?;
        }
        // Deleting everything brings every balance back to zero.
        let ids: Vec<TxnId> = w.book.txns().map(|t| t.id).collect();
        for id in ids {
            w.book.delete_txn(id).unwrap();
        }
        for account in w.book.accounts() {
            prop_assert_eq!(w.book.ledger().raw_balance(account.id), Cents::ZERO);
        }
    }

    #[test]
    fn editing_equals_deleting_and_adding(ops in prop::collection::vec(op(), 1..20), first in spec(), second in spec()) {
        let mut w = world();
        for op in &ops {
            w.apply(op);
        }
        let Ok(saved) = w.book.save_txn(&draft("p", w.entry(&first)), None) else { return Ok(()) };

        let mut edited = w.book.clone();
        let edit = edited.save_txn(&draft("p", w.entry(&second)), Some(saved.id));

        let mut replaced = w.book.clone();
        replaced.delete_txn(saved.id).unwrap();
        let add = replaced.save_txn(&draft("p", w.entry(&second)), None);

        prop_assert_eq!(edit.is_ok(), add.is_ok());
        if edit.is_ok() {
            prop_assert_eq!(edited.ledger(), replaced.ledger());
        }
    }
}
