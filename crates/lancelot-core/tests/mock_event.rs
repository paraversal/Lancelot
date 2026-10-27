//! The example event from the design mockups (UI.fig), rebuilt from individual transactions.
//! Only the totals come from the mockups; the transactions behind them are made up.

mod common;

use common::*;
use lancelot_core::*;

struct Mock {
    book: Book,
    anna: AccountId,
    malte: AccountId,
    bob: AccountId,
    clara: AccountId,
}

fn mock_event() -> Mock {
    let mut book = new_book();
    let verein = role(&book, Role::Verein);
    let cash = role(&book, Role::CashBox);
    let [tickets, donations, other_in] =
        ["Tickets", "Drink donations", "Other"].map(|n| income(&book, n));
    let [food, venue, drinks, pfand, tech, other] =
        ["Food", "Venue", "Drinks", "Pfand", "Tech", "Other"].map(|n| expense(&book, n));
    let [anna, malte, bob, clara] = ["Anna", "Malte", "Bob", "Clara"].map(|n| person(&mut book, n));
    let b = &mut book;

    // Income
    save(
        b,
        "Ticket sheet",
        Entry::Income {
            destination: verein,
            lines: vec![split(tickets, "6.227,50")],
        },
    );
    save(
        b,
        "Tickets, paid to Bob",
        Entry::Income {
            destination: bob,
            lines: vec![split(tickets, "300")],
        },
    );
    save(
        b,
        "Drink donations",
        Entry::Income {
            destination: cash,
            lines: vec![split(donations, "812,50")],
        },
    );
    save(
        b,
        "Donation box",
        Entry::Income {
            destination: cash,
            lines: vec![split(other_in, "50")],
        },
    );

    // Paid by the Verein
    save(
        b,
        "Hall rent",
        Entry::Expense {
            payer: verein,
            lines: vec![split(venue, "1.800")],
        },
    );
    save(
        b,
        "Hoffmann drinks",
        Entry::Expense {
            payer: verein,
            lines: vec![split(drinks, "420"), split(pfand, "32")],
        },
    );
    save(
        b,
        "PA rental",
        Entry::Expense {
            payer: verein,
            lines: vec![split(tech, "500")],
        },
    );
    save(
        b,
        "Catering",
        Entry::Expense {
            payer: verein,
            lines: vec![split(food, "1.098,50")],
        },
    );
    save(
        b,
        "Event insurance",
        Entry::Expense {
            payer: verein,
            lines: vec![split(other, "225")],
        },
    );

    // Paid from the cash box
    save(
        b,
        "Großmarkt",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "342,75"), split(other, "48,50")],
        },
    );
    save(
        b,
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "226,35")],
        },
    );
    save(
        b,
        "Cups and ice",
        Entry::Expense {
            payer: cash,
            lines: vec![split(drinks, "32")],
        },
    );

    // Advances and settling
    save(
        b,
        "Groceries",
        Entry::Expense {
            payer: anna,
            lines: vec![split(food, "312,40")],
        },
    );
    save(
        b,
        "Change money",
        Entry::Transfer {
            from: anna,
            to: cash,
            amount: eur("200"),
        },
    );
    save(
        b,
        "Reimbursement Anna",
        Entry::Settle {
            person: anna,
            account: verein,
            amount: eur("300"),
            direction: SettleDirection::Reimburse,
        },
    );
    save(
        b,
        "Cables",
        Entry::Expense {
            payer: malte,
            lines: vec![split(tech, "110")],
        },
    );
    save(
        b,
        "Snacks",
        Entry::Expense {
            payer: bob,
            lines: vec![split(food, "120")],
        },
    );
    save(
        b,
        "Decoration",
        Entry::Expense {
            payer: clara,
            lines: vec![split(other, "95")],
        },
    );
    save(
        b,
        "Reimbursement Clara",
        Entry::Settle {
            person: clara,
            account: verein,
            amount: eur("95"),
            direction: SettleDirection::Reimburse,
        },
    );

    Mock {
        book,
        anna,
        malte,
        bob,
        clara,
    }
}

#[test]
fn result_and_totals_match_the_mockups() {
    let m = mock_event();
    let l = m.book.ledger();
    assert_eq!(l.result(), eur("2.027,50"));
    assert_eq!(l.income_total(), eur("7.390"));
    assert_eq!(l.expense_total(), eur("5.362,50"));

    let total = |kind, name| l.balance(category(&m.book, kind, name));
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
    let m = mock_event();
    let l = m.book.ledger();
    assert_eq!(l.balance(role(&m.book, Role::Verein)), eur("1.757"));
    assert_eq!(l.balance(role(&m.book, Role::CashBox)), eur("412,90"));

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
    let m = mock_event();
    let w = m.book.ledger().where_the_money_is();
    assert_eq!(w.held_by_people, eur("180"));
    assert_eq!(w.owed_to_people, eur("322,40"));
    assert_eq!(w.opening, Cents::ZERO);
    assert_eq!(w.total, eur("2.027,50"));
    assert_eq!(raw_sum(&m.book), 0);
}

#[test]
fn open_items_are_the_unsettled_people() {
    let m = mock_event();
    let open = m.book.ledger().open_items();
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
        ]
    );
}
