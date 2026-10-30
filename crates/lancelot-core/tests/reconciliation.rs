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
        .change(|book| book.add_reconciliation(verein, june(10), eur("-1")))
        .unwrap();
    let b = store
        .change(|book| book.add_reconciliation(verein, june(11), eur("2")))
        .unwrap();
    // One checkpoint takes the date another one just left.
    store
        .change(|book| book.update_reconciliation(a, june(12), eur("-1")))
        .unwrap();
    store
        .change(|book| book.update_reconciliation(b, june(10), eur("2")))
        .unwrap();
    store.change(|book| book.delete_reconciliation(a)).unwrap();
    let c = store
        .change(|book| book.add_reconciliation(verein, june(11), eur("3")))
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

fn tiers(book: &mut Book) {
    for (name, price, fee_bp) in [("Early Bird", "25", 0), ("Regular", "32", 250)] {
        book.add_tier(TierDraft {
            name: name.into(),
            price: eur(price),
            fee_bp,
            quota: None,
            aliases: Vec::new(),
        })
        .unwrap();
    }
}

/// A sheet: (order number, whether the ticket is Regular).
fn sheet(rows: &[(u8, bool)], n: usize) -> Sheet {
    Sheet {
        filename: format!("sheet-{n}.csv"),
        sha256: format!("{n}"),
        rows: rows
            .iter()
            .enumerate()
            .map(|(i, (order, regular))| SheetRow {
                row: u32::try_from(i).unwrap() + 2,
                order: order.to_string(),
                tier: if *regular { "Regular" } else { "Early Bird" }.to_owned(),
            })
            .collect(),
        problems: Vec::new(),
    }
}

#[test]
fn a_re_import_leaves_an_earlier_checkpoint_as_it_was() {
    let mut book = new_book();
    tiers(&mut book);
    let verein = role(&book, Role::Verein);
    book.import_tickets(&sheet(&[(1, false), (2, false)], 1), june(1), now())
        .unwrap();
    let id = book.add_reconciliation(verein, june(5), eur("50")).unwrap();
    assert!(status(&book, id).difference.is_zero());

    book.import_tickets(
        &sheet(&[(1, false), (2, false), (3, false)], 2),
        june(10),
        now(),
    )
    .unwrap();
    let r = status(&book, id);
    assert_eq!((r.computed, r.from_sheet), (eur("50"), eur("50")));
    assert!(r.difference.is_zero(), "still what the statement showed");
    assert_eq!(book.ledger().balance(verein), eur("75"));

    // A statement after the new sheet sees it, and one before any sheet sees nothing.
    let later = book
        .add_reconciliation(verein, june(12), eur("75"))
        .unwrap();
    assert!(status(&book, later).difference.is_zero());
    let earlier = book.add_reconciliation(verein, june(1).previous_day().unwrap(), Cents::ZERO);
    assert!(status(&book, earlier.unwrap()).difference.is_zero());
}

#[test]
fn a_closing_count_and_a_statement_of_the_same_figure_agree() {
    let mut book = new_book();
    tiers(&mut book);
    let verein = role(&book, Role::Verein);
    book.set_opening_count(verein, june(1), eur("5000"))
        .unwrap();
    let kitchen = expense(&book, "Kitchen");
    let entry = Entry::Expense {
        payer: verein,
        lines: vec![split(kitchen, "100")],
    };
    book.save_txn(&draft("Food", entry), None).unwrap();

    let id = book
        .add_reconciliation(verein, june(30), eur("-100"))
        .unwrap();
    book.set_closing_count(verein, june(30), eur("-100"))
        .unwrap();
    assert!(status(&book, id).difference.is_zero());
    let cash = book.ledger().cash();
    let count = cash.iter().find(|c| c.account == verein).unwrap();
    assert_eq!(count.difference, Some(Cents::ZERO));
}

#[test]
fn a_negative_count_survives_the_file_and_a_negative_opening_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("count.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let verein = role(store.book(), Role::Verein);
    let cash = role(store.book(), Role::CashBox);
    store
        .change(|book| book.set_counts(verein, None, Some((june(30), eur("-100")))))
        .unwrap();
    assert_eq!(
        store
            .change(|book| book.set_counts(cash, Some((june(1), eur("-5"))), None))
            .unwrap_err()
            .code(),
        "negative_amount"
    );
    let expected = store.book().clone();
    drop(store);
    let (store, _) = Store::open(&path, StoreOptions::default()).unwrap();
    assert_eq!(store.book(), &expected);
}

mod properties {
    use proptest::prelude::*;

    use super::*;

    fn csv(rows: &[(u8, bool)]) -> String {
        let mut csv = String::from("Order ID;Ticket\n");
        for (order, regular) in rows {
            let tier = if *regular { "Regular" } else { "Early Bird" };
            csv.push_str(&format!("{order};{tier}\n"));
        }
        csv
    }

    fn rows() -> impl Strategy<Value = Vec<(u8, bool)>> {
        prop::collection::vec((0u8..8, any::<bool>()), 0..12)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// Later sheets never change what an earlier statement compares with, a statement after
        /// the last sheet compares with the books, and deleting a sheet takes back all of it.
        #[test]
        fn sheets_and_checkpoints_get_along(
            sheets in prop::collection::vec((rows(), 1u8..28), 1..5),
            checkpoint_day in 1u8..28,
        ) {
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("p.lancelot");
            let details = new_book().event().details.clone();
            let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
            for (name, price, fee_bp) in [("Early Bird", "25", 0), ("Regular", "32", 250)] {
                store.change(|book| book.add_tier(TierDraft {
                    name: name.into(),
                    price: eur(price),
                    fee_bp,
                    quota: None,
                    aliases: Vec::new(),
                })).unwrap();
            }
            let verein = role(store.book(), Role::Verein);
            let id = store.change(|book| book.add_reconciliation(verein, june(checkpoint_day), Cents::ZERO)).unwrap();

            for (n, (rows, day)) in sheets.iter().enumerate() {
                let before = store.book().clone();
                let figure = status(&before, id).computed;
                let import = store
                    .import_tickets(&format!("sheet-{n}.csv"), csv(rows).as_bytes(), june(*day))
                    .unwrap();
                if june(*day) > june(checkpoint_day) {
                    prop_assert_eq!(status(store.book(), id).computed, figure);
                }
                prop_assert_eq!(raw_sum(store.book()), 0);

                // Taking every other sheet back gives the figures it had.
                if n % 2 == 1 {
                    store.change(|book| book.delete_import(import)).unwrap();
                    prop_assert_eq!(status(store.book(), id).computed, figure);
                    prop_assert_eq!(store.book().tickets().total, before.tickets().total);
                    prop_assert_eq!(
                        store.book().ledger().balance(verein),
                        before.ledger().balance(verein)
                    );
                }
            }

            // A statement after every sheet compares with the whole balance.
            let final_id = store.change(|book| book.add_reconciliation(verein, june(28), Cents::ZERO)).unwrap();
            prop_assert_eq!(
                status(store.book(), final_id).computed,
                store.book().ledger().balance(verein)
            );

            let expected = store.book().clone();
            drop(store);
            let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
            prop_assert!(!report.has_errors(), "{:?}", report);
            prop_assert_eq!(store.book(), &expected);
        }
    }
}
