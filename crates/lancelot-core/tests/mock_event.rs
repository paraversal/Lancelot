//! The example event from the design mockups (UI.fig), see `common/mock.rs`.

mod common;

use std::collections::BTreeMap;

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
    assert_eq!(total(AccountKind::Income, "General"), eur("50"));
    assert_eq!(total(AccountKind::Expense, "Kitchen"), eur("2.100"));
    assert_eq!(total(AccountKind::Expense, "Venue"), eur("1.800"));
    assert_eq!(total(AccountKind::Expense, "Tech"), eur("610"));
    assert_eq!(total(AccountKind::Expense, "General"), eur("368,50"));
    // The Drinks figure in the mockups is drinks plus deposit.
    let drinks = total(AccountKind::Expense, "Drinks")
        .checked_add(total(AccountKind::Expense, "Deposit"))
        .unwrap();
    assert_eq!(drinks, eur("484"));
    // Drinks result: donations minus drinks cost.
    assert_eq!(eur("812,50").checked_sub(drinks).unwrap(), eur("328,50"));
    let centre = book.cost_centres().next().unwrap().id;
    let pnl = book.budget().cost_centre(centre).unwrap();
    assert_eq!(pnl.result.actual, eur("328,50"));
}

#[test]
fn accounts_and_people_match_the_mockups() {
    let (book, m) = mock_event();
    let l = book.ledger();
    assert_eq!(l.balance(asset(&book, "Verein account")), eur("1.757"));
    assert_eq!(l.balance(asset(&book, "Cash box")), eur("412,90"));

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
    let open = book.open_items();
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
                account: asset(&book, "Cash box"),
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
    assert_eq!(snapshot.open_items, book.open_items());
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

/// Writes the example event as JSON for the UI's tests and the `/design` page
/// (`app/src/lib/api/fixtures`), like ts-rs writes the types. CI checks that it is up to date.
#[test]
fn export_the_snapshot_for_the_ui() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src/lib/api/fixtures");
    let (book, _) = mock_event();
    let mut json = serde_json::to_string_pretty(&EventSnapshot::new(&book)).unwrap();
    json.push('\n');
    std::fs::write(dir.join("example-event.json"), json).unwrap();
}

/// What the app edits is what is stored: a typed-in entry rebuilds exactly its lines.
#[test]
fn every_entry_of_the_example_shows_in_the_form_that_made_it() {
    let (book, _) = mock_event();
    let accounts: BTreeMap<_, _> = book.accounts().map(|a| (a.id, a.clone())).collect();
    let snapshot = EventSnapshot::new(&book);
    assert!(!snapshot.txns.is_empty());
    for view in &snapshot.txns {
        let entry = view
            .entry
            .as_ref()
            .expect("the example's entries have forms");
        assert_eq!(entry.kind(), view.txn.kind);
        assert_eq!(build_lines(entry, &accounts).unwrap(), view.txn.lines);
        assert_eq!(view.who, Some(entry.who()));
    }
}

#[test]
fn running_balances_end_at_the_net_and_the_totals_add_up() {
    let (book, m) = mock_event();
    let l = book.ledger();
    let anna = l.person_entries(m.anna);
    // Groceries (advance), change money (advance), reimbursement.
    let effects: Vec<Cents> = anna.iter().map(|e| e.effect).collect();
    assert_eq!(effects, [eur("312,40"), eur("200"), eur("-300")]);
    assert_eq!(anna.last().unwrap().balance, eur("212,40"));
    for (id, p) in l.persons() {
        assert_eq!(
            l.person_entries(id)
                .last()
                .map(|e| e.balance)
                .unwrap_or_default(),
            p.net
        );
    }
    let t = l.person_totals();
    assert_eq!(
        (t.advanced, t.held, t.settled),
        (eur("837,40"), eur("300"), eur("395"))
    );
    assert_eq!(t.net, eur("142,40"));
    assert_eq!(l.person(m.clara).unwrap().settled, eur("95"));
}

#[test]
fn the_payout_list_and_the_repayment_list() {
    let (book, m) = mock_event();
    let list = payouts(&book).unwrap();
    let rows: Vec<(String, Cents)> = list
        .payouts
        .iter()
        .map(|p| (p.name.clone(), p.amount))
        .collect();
    assert_eq!(
        rows,
        [("Anna".into(), eur("212,40")), ("Malte".into(), eur("110"))]
    );
    assert_eq!(list.payout_total, eur("322,40"));
    // Nobody in the example has an IBAN yet, so nothing is payable.
    assert_eq!(list.payable_total, Cents::ZERO);
    assert_eq!(list.repayments.len(), 1);
    assert_eq!(
        (list.repayments[0].person, list.repayments[0].amount),
        (m.bob, eur("180"))
    );
    assert_eq!(list.payouts[0].reference, "Auslagen Sommerfest 2026 Anna");
}

#[test]
fn the_transactions_csv_has_one_row_per_line() {
    let (book, _) = mock_event();
    let csv = transactions_csv(&book, NumberFormat::De);
    let rows: Vec<Vec<&str>> = csv
        .split_terminator("\r\n")
        .map(|r| r.split(';').collect())
        .collect();
    assert_eq!(
        rows[0],
        [
            "Date",
            "Entry",
            "Type",
            "Description",
            "Counterparty",
            "Account",
            "Account type",
            "Amount",
            "Receipt",
            "Notes",
            "Cost centre"
        ]
    );
    let lines: usize = book.txns().map(|t| t.lines.len()).sum();
    assert_eq!(rows.len() - 1, lines);

    let groceries: Vec<_> = rows.iter().filter(|r| r[3] == "Groceries").collect();
    assert_eq!(groceries.len(), 2);
    assert_eq!(
        groceries
            .iter()
            .map(|r| (r[5], r[6], r[7], r[8]))
            .collect::<Vec<_>>(),
        [
            ("Anna", "Person", "-312,40", "yes"),
            ("Kitchen", "Expense", "312,40", "yes")
        ]
    );
    let cups = rows.iter().find(|r| r[3] == "Cups and ice").unwrap();
    assert_eq!(cups[8], "no");
    let reimbursed = rows.iter().find(|r| r[3] == "Reimbursement Anna").unwrap();
    assert_eq!((reimbursed[2], reimbursed[8]), ("Reimbursement", ""));

    let en = transactions_csv(&book, NumberFormat::En);
    assert!(en.contains(",Kitchen,Expense,312.40,"), "{en}");
    assert!(en.contains("2026-06-13,"));
}

#[test]
fn the_example_budget_forecasts_cautiously() {
    let (book, _) = mock_event();
    let budget = book.budget();
    let figures = |estimate, actual, forecast| Figures {
        estimate: eur(estimate),
        actual: eur(actual),
        forecast: eur(forecast),
    };
    let cat = |kind, name| budget.category(category(&book, kind, name)).unwrap();

    // Complete: the actual. Income under way: the smaller. Expenses under way: the larger.
    assert_eq!(
        cat(AccountKind::Income, "Tickets").figures,
        figures("6.500", "6.527,50", "6.527,50")
    );
    assert_eq!(
        cat(AccountKind::Income, "Drink donations").figures,
        figures("700", "812,50", "700")
    );
    assert_eq!(
        cat(AccountKind::Expense, "Kitchen").figures,
        figures("1.850", "2.100", "2.100")
    );
    assert_eq!(
        cat(AccountKind::Expense, "Mobility").figures,
        figures("150", "0", "150")
    );
    let tech = cat(AccountKind::Expense, "Tech");
    assert_eq!(tech.figures, figures("450", "610", "610"));
    assert_eq!(tech.unbudgeted, eur("110"));

    assert_eq!(budget.income, figures("7.200", "7.390", "7.277,50"));
    assert_eq!(budget.expense, figures("4.895", "5.362,50", "5.512,50"));
    assert_eq!(budget.result, figures("2.305", "2.027,50", "1.765"));
}
