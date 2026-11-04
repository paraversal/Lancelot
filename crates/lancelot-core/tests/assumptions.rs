//! Assumptions: the numbers a budget is planned on, which budget lines scale with (R-BUD-6).

mod common;

use common::*;
use lancelot_core::*;
use tempfile::TempDir;

fn scaled(
    category: AccountId,
    description: &str,
    quantity: u32,
    price: &str,
    per: Vec<AssumptionId>,
) -> BudgetLineDraft {
    BudgetLineDraft {
        cost_centre: None,
        category,
        description: description.into(),
        estimate: Estimate::Quantity {
            quantity,
            unit_price: eur(price),
            per,
        },
        complete: false,
    }
}

fn estimate(book: &Book, line: BudgetLineId) -> Cents {
    book.budget().line(line).unwrap().estimate
}

#[test]
fn a_line_follows_the_assumptions_it_scales_with() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let guests = book.add_assumption("Guests", 250).unwrap();
    let days = book.add_assumption("Days", 3).unwrap();
    let breakfast = book
        .add_budget_line(scaled(kitchen, "Breakfast", 1, "4,50", vec![guests, days]))
        .unwrap();
    let shirts = book
        .add_budget_line(scaled(kitchen, "Shirts", 2, "10", vec![guests]))
        .unwrap();
    assert_eq!(estimate(&book, breakfast), eur("3.375"));
    assert_eq!(estimate(&book, shirts), eur("5.000"));
    assert_eq!(book.lines_scaling_with(guests), 2);
    assert_eq!(book.lines_scaling_with(days), 1);

    book.update_assumption(guests, "Guests", 300).unwrap();
    assert_eq!(estimate(&book, breakfast), eur("4.050"));
    assert_eq!(estimate(&book, shirts), eur("6.000"));
    assert_eq!(book.budget().expense.estimate, eur("10.050"));

    // No guests yet: the lines plan nothing, and nothing breaks.
    book.update_assumption(guests, "Guests", 0).unwrap();
    assert_eq!(estimate(&book, breakfast), Cents::ZERO);
}

#[test]
fn the_forecast_stays_cautious_when_an_assumption_falls() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let guests = book.add_assumption("Guests", 200).unwrap();
    let food = book
        .add_budget_line(scaled(kitchen, "Food", 1, "10", vec![guests]))
        .unwrap();
    let payer = asset(&book, "Bank account");
    save(
        &mut book,
        "Groceries",
        Entry::Expense {
            payer,
            lines: vec![SplitLine {
                budget_line: Some(food),
                cost_centre: None,
                ..split(kitchen, "1.500")
            }],
        },
    );
    // Fewer guests lower the plan, but what was spent is spent.
    book.update_assumption(guests, "Guests", 100).unwrap();
    let figures = book.budget().line(food).unwrap();
    assert_eq!(figures.estimate, eur("1.000"));
    assert_eq!(figures.forecast, eur("1.500"));
}

#[test]
fn names_are_needed_and_unique() {
    let mut book = new_book();
    book.add_assumption(" Guests ", 1).unwrap();
    assert_eq!(book.assumptions().next().unwrap().name, "Guests");
    assert_eq!(
        book.add_assumption("guests", 2).unwrap_err().code(),
        "duplicate_name"
    );
    assert_eq!(
        book.add_assumption("  ", 2).unwrap_err().code(),
        "empty_name"
    );
    let days = book.add_assumption("Days", 2).unwrap();
    assert_eq!(
        book.update_assumption(days, "GUESTS", 2)
            .unwrap_err()
            .code(),
        "duplicate_name"
    );
}

#[test]
fn a_line_names_only_assumptions_that_exist_and_each_once() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let guests = book.add_assumption("Guests", 10).unwrap();
    let twice = scaled(kitchen, "Food", 1, "1", vec![guests, guests]);
    let error = book.add_budget_line(twice).unwrap_err();
    assert_eq!(error.code(), "unknown_assumption");
    assert_eq!(error.field(), Some("estimate.per[1]"));
    let unknown = scaled(kitchen, "Food", 1, "1", vec![AssumptionId(99)]);
    assert_eq!(
        book.add_budget_line(unknown).unwrap_err().field(),
        Some("estimate.per[0]")
    );
}

#[test]
fn an_assumption_in_use_stays() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let guests = book.add_assumption("Guests", 10).unwrap();
    let food = book
        .add_budget_line(scaled(kitchen, "Food", 1, "1", vec![guests]))
        .unwrap();
    assert_eq!(
        book.delete_assumption(guests).unwrap_err(),
        DomainError::AssumptionInUse
    );
    book.delete_budget_line(food).unwrap();
    assert_eq!(book.delete_assumption(guests).unwrap().name, "Guests");
    assert_eq!(book.assumptions().count(), 0);
    assert_eq!(
        book.delete_assumption(guests).unwrap_err().code(),
        "unknown_assumption"
    );
}

#[test]
fn a_value_that_would_overflow_a_line_is_refused() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let guests = book.add_assumption("Guests", 10).unwrap();
    book.add_budget_line(scaled(kitchen, "Food", 1_000_000, "1.000", vec![guests]))
        .unwrap();
    let before = book.clone();
    assert_eq!(
        book.update_assumption(guests, "Guests", u32::MAX)
            .unwrap_err()
            .code(),
        "overflow"
    );
    assert_eq!(book, before);
}

#[test]
fn a_closed_event_keeps_its_assumptions() {
    let mut book = new_book();
    let guests = book.add_assumption("Guests", 10).unwrap();
    let overrides = book
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "Closed for the test".into(),
        })
        .collect();
    book.close(overrides, now()).unwrap();
    assert_eq!(
        book.update_assumption(guests, "Guests", 20).unwrap_err(),
        DomainError::EventClosed
    );
    assert_eq!(
        book.add_assumption("Days", 1).unwrap_err(),
        DomainError::EventClosed
    );
}

#[test]
fn assumptions_survive_saving_and_reopening() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("assumptions.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) =
        Store::create(&path, details, &two_accounts(), StoreOptions::default()).unwrap();
    let kitchen = expense(store.book(), "Kitchen");
    let (guests, days) = store
        .change(|b| {
            Ok::<_, DomainError>((
                b.add_assumption("Guests", 250)?,
                b.add_assumption("Days", 3)?,
            ))
        })
        .unwrap();
    // The order of the assumptions is kept, and a line that changes drops the ones it left.
    let breakfast = store
        .change(|b| b.add_budget_line(scaled(kitchen, "Breakfast", 1, "4,50", vec![days, guests])))
        .unwrap();
    store
        .change(|b| {
            b.update_budget_line(
                breakfast,
                scaled(kitchen, "Breakfast", 2, "4,50", vec![guests]),
            )
        })
        .unwrap();
    let gone = store
        .change(|b| b.add_budget_line(scaled(kitchen, "Gone", 1, "1", vec![days])))
        .unwrap();
    store.change(|b| b.delete_budget_line(gone)).unwrap();
    store
        .change(|b| b.update_assumption(guests, "Guests", 300))
        .unwrap();

    let book = store.book().clone();
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &book);
    assert_eq!(estimate(store.book(), breakfast), eur("2.700"));

    let log = store.change_log(50).unwrap();
    let ops: Vec<_> = log
        .iter()
        .filter(|e| e.entity == "assumption")
        .map(|e| e.op.as_str())
        .collect();
    assert_eq!(ops, ["update", "create", "create"]);
}

#[test]
fn a_template_brings_its_assumptions_and_a_copy_matches_them_by_name() {
    let mut source = new_book();
    let kitchen = expense(&source, "Kitchen");
    let guests = source.add_assumption("Guests", 250).unwrap();
    let helpers = source.add_assumption("Helpers", 40).unwrap();
    source
        .add_budget_line(scaled(kitchen, "Food", 1, "10", vec![guests]))
        .unwrap();
    source
        .add_budget_line(scaled(kitchen, "Shirts", 1, "12", vec![helpers]))
        .unwrap();

    let details = source.event().details.clone();
    let next_year = Book::from_template(&source, details).unwrap();
    let names: Vec<_> = next_year
        .assumptions()
        .map(|a| (a.name.as_str(), a.value))
        .collect();
    assert_eq!(names, [("Guests", 250), ("Helpers", 40)]);
    assert_eq!(next_year.budget().expense.estimate, eur("2.980"));

    // Here, "guests" already exists with its own value, which stays.
    let mut book = new_book();
    let here = book.add_assumption("guests", 100).unwrap();
    let summary = book
        .copy_from(
            &source,
            CopyChoice {
                persons: false,
                budget: true,
            },
        )
        .unwrap();
    assert_eq!(summary.assumptions_added, 1);
    let names: Vec<_> = book
        .assumptions()
        .map(|a| (a.name.as_str(), a.value))
        .collect();
    assert_eq!(names, [("guests", 100), ("Helpers", 40)]);
    let food = book
        .budget_lines()
        .find(|b| b.draft.description == "Food")
        .unwrap();
    assert!(matches!(&food.draft.estimate, Estimate::Quantity { per, .. } if per == &[here]));
    assert_eq!(estimate(&book, food.id), eur("1.000"));
}
