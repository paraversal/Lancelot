//! Property tests over random sequences of entries, edits and deletions (R-COR-6).

mod common;

use common::world::{Ids, apply, op, spec};
use common::*;
use lancelot_core::*;
use proptest::prelude::*;

fn world() -> (Book, Ids) {
    let mut book = new_book();
    for name in ["Anna", "Bob", "Malte"] {
        person(&mut book, name);
    }
    let ids = Ids::of(&book);
    (book, ids)
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
        let (mut book, ids) = world();
        for op in &ops {
            apply(&ids, &mut book, op);
            check_invariants(&book)?;
        }
        // Deleting everything brings every balance back to zero.
        let txns: Vec<TxnId> = book.txns().map(|t| t.id).collect();
        for id in txns {
            book.delete_txn(id).unwrap();
        }
        for account in book.accounts() {
            prop_assert_eq!(book.ledger().raw_balance(account.id), Cents::ZERO);
        }
    }

    #[test]
    fn editing_equals_deleting_and_adding(ops in prop::collection::vec(op(), 1..20), first in spec(), second in spec()) {
        let (mut book, ids) = world();
        for op in &ops {
            apply(&ids, &mut book, op);
        }
        let Ok(saved) = book.save_txn(&draft("p", ids.entry(&first)), None) else { return Ok(()) };

        let mut edited = book.clone();
        let edit = edited.save_txn(&draft("p", ids.entry(&second)), Some(saved.id));

        let mut replaced = book.clone();
        replaced.delete_txn(saved.id).unwrap();
        let add = replaced.save_txn(&draft("p", ids.entry(&second)), None);

        prop_assert_eq!(edit.is_ok(), add.is_ok());
        if edit.is_ok() {
            // Everything but the id of the edited transaction is the same.
            let figures = |b: &Book| {
                let l = b.ledger();
                let balances: Vec<_> = b.accounts().map(|a| l.balance(a.id)).collect();
                let persons: Vec<_> = l.persons().map(|(id, p)| (id, *p)).collect();
                (balances, persons, l.where_the_money_is().clone(), l.cash().to_vec(), l.missing_receipts().len())
            };
            prop_assert_eq!(figures(&edited), figures(&replaced));
        }
    }
}
