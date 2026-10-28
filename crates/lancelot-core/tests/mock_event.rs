//! The example event from the design mockups (UI.fig), see `common/mock.rs`.

mod common;

use common::mock::{self, Mock};
use common::*;
use lancelot_core::*;

fn mock_event() -> (Book, Mock) {
    let mut book = new_book();
    let mock = mock::build(&mut book);
    (book, mock)
}

#[test]
fn result_and_totals_match_the_mockups() {
    let (book, _) = mock_event();
    let l = book.ledger();
    assert_eq!(l.result(), eur("2.027,50"));
    assert_eq!(l.income_total(), eur("7.390"));
    assert_eq!(l.expense_total(), eur("5.362,50"));

    let total = |kind, name| l.balance(category(&book, kind, name));
    assert_eq!(total(AccountKind::Income, "Tickets"), eur("6.527,50"));
    assert_eq!(total(AccountKind::Income, "Drink donations"), eur("812,50"));
    assert_eq!(total(AccountKind::Income, "Other"), eur("50"));
    assert_eq!(total(AccountKind::Expense, "Food"), eur("2.100"));
    assert_eq!(total(AccountKind::Expense, "Venue"), eur("1.800"));
    assert_eq!(total(AccountKind::Expense, "Tech"), eur("610"));
    assert_eq!(total(AccountKind::Expense, "Other"), eur("368,50"));
    // The Drinks figure in the mockups is drinks plus Pfand.
    let drinks = total(AccountKind::Expense, "Drinks")
        .checked_add(total(AccountKind::Expense, "Pfand"))
        .unwrap();
    assert_eq!(drinks, eur("484"));
    // Drinks result: donations minus drinks cost.
    assert_eq!(eur("812,50").checked_sub(drinks).unwrap(), eur("328,50"));
}

#[test]
fn accounts_and_people_match_the_mockups() {
    let (book, m) = mock_event();
    let l = book.ledger();
    assert_eq!(l.balance(role(&book, Role::Verein)), eur("1.757"));
    assert_eq!(l.balance(role(&book, Role::CashBox)), eur("412,90"));

    let person = |id| *l.person(id).unwrap();
    assert_eq!(
        (person(m.anna).net, person(m.anna).status),
        (eur("212,40"), PersonStatus::Owed)
    );
    assert_eq!(
        (person(m.malte).net, person(m.malte).status),
        (eur("110"), PersonStatus::Owed)
    );
    assert_eq!(
        (person(m.bob).net, person(m.bob).status),
        (eur("-180"), PersonStatus::Owes)
    );
    assert_eq!(person(m.clara).status, PersonStatus::Settled);
    assert_eq!(person(m.anna).advanced, eur("512,40"));
    assert_eq!(person(m.anna).reimbursed, eur("300"));
}

#[test]
fn where_the_money_is_reconciles_to_the_result() {
    let (book, _) = mock_event();
    let w = book.ledger().where_the_money_is();
    assert_eq!(w.held_by_people, eur("180"));
    assert_eq!(w.owed_to_people, eur("322,40"));
    assert_eq!(w.opening, Cents::ZERO);
    assert_eq!(w.total, eur("2.027,50"));
    assert_eq!(raw_sum(&book), 0);
}

#[test]
fn open_items_are_the_unsettled_people_a_receipt_and_the_cash_box() {
    let (book, m) = mock_event();
    let open = book.ledger().open_items();
    assert_eq!(
        open,
        vec![
            OpenItem::PersonBalance {
                person: m.anna,
                net: eur("212,40")
            },
            OpenItem::PersonBalance {
                person: m.malte,
                net: eur("110")
            },
            OpenItem::PersonBalance {
                person: m.bob,
                net: eur("-180")
            },
            OpenItem::MissingReceipt { txn: m.cups },
            OpenItem::CashDifference {
                account: role(&book, Role::CashBox),
                difference: eur("-2,90")
            },
        ]
    );
}

#[test]
fn the_snapshot_carries_every_figure_in_ui_form() {
    let (book, m) = mock_event();
    let snapshot = EventSnapshot::new(&book);
    assert_eq!(snapshot.totals.result, eur("2.027,50"));
    assert_eq!(snapshot.persons.len(), 4);
    assert_eq!(snapshot.open_items, book.ledger().open_items());
    assert!(
        snapshot
            .txns
            .windows(2)
            .all(|w| (w[0].txn.date, w[0].txn.id) <= (w[1].txn.date, w[1].txn.id))
    );

    // The JSON the UI receives: camelCase, amounts as integer cents, dates as ISO text.
    let json = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(json["totals"]["result"], 202_750);
    assert_eq!(json["whereTheMoneyIs"]["owedToPeople"], 32_240);
    assert_eq!(json["event"]["name"], "Sommerfest 2026");
    assert_eq!(json["event"]["startsOn"], "2026-06-12");
    assert_eq!(json["txns"][0]["date"], "2026-06-13");
    let anna = json["persons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["account"] == m.anna.0)
        .unwrap();
    assert_eq!(anna["name"], "Anna");
    assert_eq!(anna["summary"]["net"], 21_240);
    assert_eq!(anna["summary"]["status"], "owed");
    assert_eq!(json["openItems"][3]["type"], "missingReceipt");
}
