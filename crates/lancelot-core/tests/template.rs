//! New events from a template and copying from another event (R-EVT-3, R-ISO-1, R-ISO-4).

mod common;

use std::path::Path;

use common::*;
use lancelot_core::*;
use tempfile::TempDir;

fn details(name: &str) -> EventDetails {
    EventDetails {
        name: name.into(),
        starts_on: None,
        ends_on: None,
        location: None,
        notes: None,
    }
}

fn line(category: AccountId, description: &str, centre: Option<CostCentreId>) -> BudgetLineDraft {
    BudgetLineDraft {
        category,
        description: description.into(),
        estimate: Estimate::Amount { amount: eur("50") },
        complete: true,
        cost_centre: centre,
    }
}

fn category(book: &Book, kind: AccountKind, name: &str) -> AccountId {
    book.account_by_name(kind, name).unwrap().id
}

/// The example event, closed, as last year's file would be.
fn last_year() -> Book {
    let mut book = new_book();
    mock::build(&mut book);
    let tier = TierDraft {
        name: "Regular".into(),
        price: eur("20"),
        fee_bp: 250,
        quota: Some(100),
        aliases: vec!["Normal".into()],
    };
    book.add_tier(tier).unwrap();
    let open = book.ledger().open_items();
    let overrides = open
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "Last year".into(),
        })
        .collect();
    book.close(overrides, now()).unwrap();
    book
}

#[test]
fn a_template_keeps_the_setup_and_none_of_the_money() {
    let source = last_year();
    let book = Book::from_template(&source, details("Sommerfest 2027")).unwrap();

    assert_eq!(book.event().details.name, "Sommerfest 2027");
    assert!(!book.event().is_closed());
    assert!(book.event().closing_overrides.is_empty());
    assert_eq!(
        book.event().default_reimbursement_account,
        source.event().default_reimbursement_account
    );
    assert_eq!(book.event().drinks, source.event().drinks);

    assert_eq!(book.accounts().count(), source.accounts().count());
    assert_eq!(book.persons().count(), source.persons().count());
    assert_eq!(book.cost_centres().count(), source.cost_centres().count());
    assert_eq!(book.budget_lines().count(), source.budget_lines().count());
    assert_eq!(book.tickets().tiers.len(), 1);

    assert_eq!(book.txns().count(), 0);
    assert_eq!(book.attachments().count(), 0);
    assert_eq!(book.closing_counts().count(), 0);
    assert_eq!(book.reconciliations().count(), 0);
    assert!(book.ledger().result().is_zero());
    assert!(book.budget_lines().all(|b| !b.draft.complete));
    assert!(book.budget().result.actual.is_zero());
    assert!(
        book.drink_products()
            .all(|p| p.draft.taken == 0 && p.draft.returned == 0)
    );
    assert!(
        book.drink_products()
            .zip(source.drink_products())
            .all(|(a, b)| a.draft.unit_price == b.draft.unit_price)
    );
}

#[test]
fn a_template_needs_a_name() {
    let source = new_book();
    let err = Book::from_template(&source, details(" ")).unwrap_err();
    assert_eq!(err.code(), "empty_name");
}

/// Writes a book as an event file by way of a template, and closes it again.
fn written(dir: &TempDir, name: &str, source: &Book) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let (store, report) =
        Store::create_from_template(&path, details(name), source, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    drop(store);
    path
}

#[test]
fn a_template_file_reopens_as_the_same_book() {
    let dir = TempDir::new().unwrap();
    let source = last_year();
    let path = dir.path().join("new.lancelot");
    let (store, report) = Store::create_from_template(
        &path,
        details("Sommerfest 2027"),
        &source,
        StoreOptions::default(),
    )
    .unwrap();
    assert!(report.is_clean(), "{report:?}");
    let book = store.book().clone();
    drop(store);

    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
}

#[test]
fn the_new_event_books_like_any_other() {
    let source = last_year();
    let mut book = Book::from_template(&source, details("Next")).unwrap();
    let verein = role(&book, Role::Verein);
    let venue = category(&book, AccountKind::Expense, "Venue");
    let saved = save(
        &mut book,
        "Hall",
        Entry::Expense {
            payer: verein,
            lines: vec![split(venue, "80")],
        },
    );
    assert!(book.txn(saved).is_some());
    assert_eq!(book.ledger().balance(verein), eur("-80"));
}

#[test]
fn copying_skips_what_is_already_there() {
    let mut source = new_book();
    let venue = category(&source, AccountKind::Expense, "Venue");
    source
        .add_person(
            "Anna",
            Person {
                contact: Some("from the source".into()),
                ..Person::default()
            },
        )
        .unwrap();
    source.add_person("Bob", Person::default()).unwrap();
    source.add_budget_line(line(venue, "Hall", None)).unwrap();
    source.add_budget_line(line(venue, "Stage", None)).unwrap();

    let mut book = new_book();
    let anna = book.add_person("ANNA", Person::default()).unwrap();
    let venue_here = category(&book, AccountKind::Expense, "Venue");
    let mine = BudgetLineDraft {
        complete: false,
        estimate: Estimate::Amount { amount: eur("1") },
        ..line(venue_here, "hall", None)
    };
    book.add_budget_line(mine).unwrap();

    let choice = CopyChoice {
        persons: true,
        budget: true,
    };
    let summary = book.copy_from(&source, choice).unwrap();
    assert_eq!(summary.persons_added, 1);
    assert_eq!(summary.persons_skipped, 1);
    assert_eq!(summary.budget_lines_added, 1);
    assert_eq!(summary.budget_lines_skipped, 1);
    assert_eq!(summary.categories_added, 0);

    // What was here stays as it was.
    assert_eq!(book.person(anna).unwrap().contact, None);
    let hall = book
        .budget_lines()
        .find(|b| b.draft.description == "hall")
        .unwrap();
    assert_eq!(hall.draft.estimate.total().unwrap(), eur("1"));
    let stage = book
        .budget_lines()
        .find(|b| b.draft.description == "Stage")
        .unwrap();
    assert!(!stage.draft.complete, "a new event's plan is not complete");

    // Copying again adds nothing.
    let again = book.copy_from(&source, choice).unwrap();
    assert_eq!(again.persons_added + again.budget_lines_added, 0);
}

#[test]
fn copying_takes_only_what_was_chosen() {
    let mut source = new_book();
    let venue = category(&source, AccountKind::Expense, "Venue");
    source.add_person("Anna", Person::default()).unwrap();
    source.add_budget_line(line(venue, "Hall", None)).unwrap();

    let mut people = new_book();
    let choice = CopyChoice {
        persons: true,
        budget: false,
    };
    people.copy_from(&source, choice).unwrap();
    assert_eq!(people.persons().count(), 1);
    assert_eq!(people.budget_lines().count(), 0);

    let mut budget = new_book();
    let choice = CopyChoice {
        persons: false,
        budget: true,
    };
    budget.copy_from(&source, choice).unwrap();
    assert_eq!(budget.persons().count(), 0);
    assert_eq!(budget.budget_lines().count(), 1);
}

#[test]
fn copying_creates_missing_categories_and_cost_centres_and_follows_roles() {
    let mut source = new_book();
    let stage = source.add_account(AccountKind::Expense, "Stage").unwrap();
    let centre = source.add_cost_centre("Workshop").unwrap();
    source
        .set_category_cost_centre(stage, Some(centre))
        .unwrap();
    source
        .add_budget_line(line(stage, "Boards", Some(centre)))
        .unwrap();
    let pfand = source.account_by_role(Role::Pfand).unwrap().id;
    let drinks = source.drinks_cost_centre().unwrap().id;
    source
        .add_budget_line(line(pfand, "Deposit", Some(drinks)))
        .unwrap();

    let mut book = new_book();
    let pfand_here = book.account_by_role(Role::Pfand).unwrap().id;
    book.rename_account(pfand_here, "Leergut").unwrap();
    let drinks_here = book.drinks_cost_centre().unwrap().id;
    book.rename_cost_centre(drinks_here, "Bar").unwrap();

    let summary = book
        .copy_from(
            &source,
            CopyChoice {
                persons: false,
                budget: true,
            },
        )
        .unwrap();
    assert_eq!(summary.categories_added, 1);
    assert_eq!(summary.cost_centres_added, 1);
    assert_eq!(summary.budget_lines_added, 2);

    let boards = book
        .budget_lines()
        .find(|b| b.draft.description == "Boards")
        .unwrap();
    let new_stage = book.account(boards.draft.category).unwrap();
    assert_eq!(new_stage.name, "Stage");
    assert_eq!(new_stage.cost_centre, boards.draft.cost_centre);
    let deposit = book
        .budget_lines()
        .find(|b| b.draft.description == "Deposit")
        .unwrap();
    assert_eq!(deposit.draft.category, pfand_here);
    assert_eq!(deposit.draft.cost_centre, Some(drinks_here));
}

#[test]
fn a_closed_event_cannot_be_copied_into() {
    let mut closed = last_year();
    let err = closed
        .copy_from(
            &new_book(),
            CopyChoice {
                persons: true,
                budget: true,
            },
        )
        .unwrap_err();
    assert_eq!(err.code(), "event_closed");
}

#[test]
fn reading_another_file_leaves_it_as_it_was() {
    let dir = TempDir::new().unwrap();
    let path = written(&dir, "source.lancelot", &last_year());
    let before = std::fs::read(&path).unwrap();
    let names = |p: &Path| {
        let mut names: Vec<_> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let files = names(&path);

    let source = read_book(&path).unwrap();
    assert_eq!(source.event().details.name, "source.lancelot");
    let mut book = new_book();
    let summary = book
        .copy_from(
            &source,
            CopyChoice {
                persons: true,
                budget: true,
            },
        )
        .unwrap();
    assert!(summary.persons_added > 0);

    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(names(&path), files);
    // And it still opens for writing: nothing kept a lock.
    Store::open(&path, StoreOptions::default()).unwrap();
}

#[test]
fn copied_data_is_written_to_the_file() {
    let dir = TempDir::new().unwrap();
    let source = last_year();
    let path = dir.path().join("new.lancelot");
    let (mut store, _) = Store::create(&path, details("New"), StoreOptions::default()).unwrap();
    store
        .change(|book| {
            book.copy_from(
                &source,
                CopyChoice {
                    persons: true,
                    budget: true,
                },
            )
        })
        .unwrap();
    let book = store.book().clone();
    drop(store);
    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    assert!(reopened.book().budget_lines().count() > 0);
}

#[test]
fn a_file_open_elsewhere_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = written(&dir, "source.lancelot", &new_book());
    let (store, _) = Store::open(&path, StoreOptions::default()).unwrap();
    let err = read_book(&path).err().unwrap();
    assert_eq!(err.code(), "already_open");
    drop(store);
    assert!(read_book(&path).is_ok());
}

#[test]
fn a_file_that_is_no_event_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("notes.lancelot");
    std::fs::write(&path, "just some text, long enough for a header").unwrap();
    let err = read_book(&path).err().unwrap();
    assert_eq!(err.code(), "not_an_event_file");
}
