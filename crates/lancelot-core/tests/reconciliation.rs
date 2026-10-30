//! Reconciliation checkpoints (R-ACC-4, R-ISO-2, R-SET-5).

mod common;

use common::*;
use lancelot_core::*;
use tempfile::TempDir;

/// Income into the account on the given day.
fn income_on(book: &mut Book, account: AccountId, day: u8, value: &str) {
    let category = book
        .account_by_name(AccountKind::Income, "General")
        .unwrap()
        .id;
    let entry = Entry::Income {
        destination: account,
        lines: vec![split(category, value)],
    };
    let draft = TxnDraft {
        date: june(day),
        ..draft("Income", entry)
    };
    book.save_txn(&draft, None).unwrap();
}

fn status(book: &Book, id: ReconciliationId) -> ReconciliationStatus {
    *book
        .ledger()
        .reconciliations()
        .iter()
        .find(|r| r.id == id)
        .unwrap()
}

#[test]
fn only_lines_up_to_the_date_count() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    income_on(&mut book, verein, 10, "100");
    income_on(&mut book, verein, 11, "20");
    income_on(&mut book, verein, 12, "3");

    let id = book
        .add_reconciliation(verein, june(11), eur("120"))
        .unwrap();
    let r = status(&book, id);
    assert_eq!(r.computed, eur("120"));
    assert!(r.difference.is_zero());
    assert_eq!(book.ledger().balance(verein), eur("123"));
}

#[test]
fn the_verein_account_compares_its_net_movement() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    book.set_opening_count(verein, june(1), eur("5000"))
        .unwrap();
    income_on(&mut book, verein, 10, "100");

    let id = book
        .add_reconciliation(verein, june(11), eur("100"))
        .unwrap();
    let r = status(&book, id);
    assert!(r.movement);
    assert_eq!(r.computed, eur("100"));
    assert!(r.difference.is_zero());
}

#[test]
fn another_account_compares_its_balance_with_the_opening_count() {
    let mut book = new_book();
    let other = book.add_asset("Paypal", AssetType::Bank).unwrap();
    book.set_opening_count(other, june(1), eur("50")).unwrap();
    income_on(&mut book, other, 10, "100");

    let id = book
        .add_reconciliation(other, june(11), eur("150"))
        .unwrap();
    let r = status(&book, id);
    assert!(!r.movement);
    assert_eq!(r.computed, eur("150"));
    assert!(r.difference.is_zero());
}

#[test]
fn a_movement_can_be_negative() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    let venue = book
        .account_by_name(AccountKind::Expense, "Venue")
        .unwrap()
        .id;
    save(
        &mut book,
        "Hall",
        Entry::Expense {
            payer: verein,
            lines: vec![split(venue, "80")],
        },
    );

    let id = book
        .add_reconciliation(verein, june(30), eur("-80"))
        .unwrap();
    assert!(status(&book, id).difference.is_zero());
}

#[test]
fn a_mismatch_is_an_open_item_until_the_books_agree() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    let id = book
        .add_reconciliation(verein, june(30), eur("100"))
        .unwrap();
    let expected = OpenItem::ReconciliationMismatch {
        checkpoint: id,
        account: verein,
        difference: eur("100"),
    };
    assert!(book.ledger().open_items().contains(&expected));

    income_on(&mut book, verein, 10, "100");
    assert!(
        !book
            .ledger()
            .open_items()
            .iter()
            .any(|i| matches!(i, OpenItem::ReconciliationMismatch { .. }))
    );
}

#[test]
fn an_earlier_mismatch_stays_open_after_a_later_match() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    income_on(&mut book, verein, 20, "100");
    book.add_reconciliation(verein, june(10), eur("5")).unwrap();
    book.add_reconciliation(verein, june(30), eur("100"))
        .unwrap();
    let mismatches = book
        .ledger()
        .open_items()
        .into_iter()
        .filter(|i| matches!(i, OpenItem::ReconciliationMismatch { .. }))
        .count();
    assert_eq!(mismatches, 1);
}

#[test]
fn a_mismatch_blocks_closing_unless_overridden() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    let cash = role(&book, Role::CashBox);
    book.set_closing_count(cash, june(30), Cents::ZERO).unwrap();
    book.add_reconciliation(verein, june(30), eur("1")).unwrap();

    let err = book.close(vec![], now()).unwrap_err();
    let DomainError::ClosingBlocked { blockers } = err else {
        panic!("{err:?}");
    };
    assert_eq!(blockers.len(), 1);

    let overrides = vec![BlockerOverride {
        item: blockers[0].clone(),
        note: "The bank booked it a day late".into(),
    }];
    book.close(overrides, now()).unwrap();
    assert!(book.event().is_closed());
    assert_eq!(
        book.add_reconciliation(verein, june(29), Cents::ZERO)
            .unwrap_err()
            .code(),
        "event_closed"
    );
}

#[test]
fn one_checkpoint_per_account_and_date() {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    let cash = role(&book, Role::CashBox);
    let first = book.add_reconciliation(verein, june(10), eur("1")).unwrap();
    let second = book.add_reconciliation(verein, june(11), eur("2")).unwrap();
    book.add_reconciliation(cash, june(10), eur("3")).unwrap();

    let duplicate = |e: DomainError| e.code();
    assert_eq!(
        book.add_reconciliation(verein, june(10), eur("9"))
            .map_err(duplicate),
        Err("duplicate_checkpoint")
    );
    assert_eq!(
        book.update_reconciliation(second, june(10), eur("9"))
            .map_err(duplicate),
        Err("duplicate_checkpoint")
    );
    // Its own date is not a conflict.
    book.update_reconciliation(first, june(10), eur("9"))
        .unwrap();
}

#[test]
fn only_an_asset_account_has_checkpoints() {
    let mut book = new_book();
    let venue = book
        .account_by_name(AccountKind::Expense, "Venue")
        .unwrap()
        .id;
    assert_eq!(
        book.add_reconciliation(venue, june(10), eur("1"))
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
    assert_eq!(
        book.delete_reconciliation(ReconciliationId(99))
            .unwrap_err()
            .code(),
        "unknown_reconciliation"
    );
}

#[test]
fn checkpoints_survive_the_file_and_are_logged() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("e.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let verein = role(store.book(), Role::Verein);

    let a = store
        .add_reconciliation(verein, june(10), eur("-1"))
        .unwrap();
    let b = store
        .add_reconciliation(verein, june(11), eur("2"))
        .unwrap();
    // One checkpoint takes the date another one just left.
    store.update_reconciliation(a, june(12), eur("-1")).unwrap();
    store.update_reconciliation(b, june(10), eur("2")).unwrap();
    store.delete_reconciliation(a).unwrap();
    let c = store
        .add_reconciliation(verein, june(11), eur("3"))
        .unwrap();
    assert_ne!(c, a, "ids are never reused");

    let book = store.book().clone();
    let log = store.change_log(10).unwrap();
    assert!(
        log.iter()
            .any(|e| e.entity == "reconciliation" && e.op == "delete" && e.entity_id == a.0)
    );
    drop(store);

    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    assert_eq!(reopened.book().reconciliations().count(), 2);
}
