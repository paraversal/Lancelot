//! The event file: round trips, bad files, locking, the database's own rules, tampering,
//! the change log and backups.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::mock::{self, Target};
use common::world::{Action, Ids, op};
use common::*;
use lancelot_core::*;
use proptest::prelude::*;
use rusqlite::Connection;
use tempfile::TempDir;
use time::{Duration, OffsetDateTime};

fn details() -> EventDetails {
    new_book().event().details.clone()
}

fn create(dir: &TempDir) -> (Store, PathBuf) {
    let path = dir.path().join("sommerfest.lancelot");
    let (store, report) =
        Store::create(&path, details(), &two_accounts(), StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    (store, path)
}

fn open(path: &Path) -> (Store, Report) {
    Store::open(path, StoreOptions::default()).unwrap()
}

/// Opens the file next to the store, bypassing all of Lancelot's rules.
fn raw(path: &Path) -> Connection {
    Connection::open(path).unwrap()
}

#[test]
fn the_currency_and_the_other_method_are_kept_in_the_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("dollars.lancelot");
    let mut in_dollars = details();
    in_dollars.currency = Currency::Usd;
    let (mut store, _) =
        Store::create(&path, in_dollars, &two_accounts(), StoreOptions::default()).unwrap();
    let cash = Person {
        other_method: Some("Cash".into()),
        ..Person::default()
    };
    let anna = store.change(|b| b.add_person("Anna", cash)).unwrap();
    drop(store);
    let (reopened, report) = open(&path);
    assert!(report.is_clean());
    assert_eq!(reopened.book().event().details.currency, Currency::Usd);
    let person = reopened.book().person(anna).unwrap();
    assert_eq!(person.other_method.as_deref(), Some("Cash"));
}

#[test]
fn a_new_file_reopens_as_the_same_book() {
    let dir = TempDir::new().unwrap();
    let (store, path) = create(&dir);
    let book = store.book().clone();
    drop(store);
    let (reopened, report) = open(&path);
    assert!(report.is_clean());
    assert_eq!(reopened.book(), &book);
    assert_eq!(reopened.book().event().details.name, "Sommerfest 2026");
}

#[test]
fn the_example_event_survives_saving_and_reopening() {
    // The exit criterion of M1.
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let m = mock::build(&mut store);
    let book = store.book().clone();
    drop(store);

    let (store, report) = open(&path);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &book);
    let l = store.book().ledger();
    assert_eq!(l.result(), eur("2.027,50"));
    assert_eq!(
        l.balance(asset(store.book(), "Verein account")),
        eur("1.757")
    );
    assert_eq!(l.balance(asset(store.book(), "Cash box")), eur("412,90"));
    assert_eq!(l.person(m.anna).unwrap().net, eur("212,40"));
    assert_eq!(l.person(m.malte).unwrap().net, eur("110"));
    assert_eq!(l.person(m.bob).unwrap().net, eur("-180"));
    assert_eq!(l.missing_receipts(), [m.cups]);
}

#[test]
fn attachments_are_stored_inside_the_file() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let txn = store.save(
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    let pdf = b"%PDF-1.7\n receipt".to_vec();
    let id = store.add_attachment(txn, " bakery.pdf ", &pdf).unwrap();

    let a = store.book().attachment(id).unwrap().clone();
    assert_eq!(
        (a.filename.as_str(), a.mime.as_str(), a.size),
        ("bakery.pdf", "application/pdf", pdf.len() as u64)
    );
    assert_eq!(a.sha256.len(), 64);
    drop(store);

    let (mut store, _) = open(&path);
    assert_eq!(store.attachment_data(id).unwrap(), pdf);
    store.change(|book| book.remove_attachment(id)).unwrap();
    assert_eq!(
        store.attachment_data(id).unwrap_err().code(),
        "unknown_attachment"
    );
}

#[test]
fn deleting_a_transaction_deletes_its_attachments() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let txn = store.save(
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    store.attach(txn, false);
    store.change(|book| book.delete_txn(txn)).unwrap();
    drop(store);
    let count: i64 = raw(&path)
        .query_row("SELECT COUNT(*) FROM attachment", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn failed_commands_change_nothing() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let before = store.book().clone();
    let food = expense(store.book(), "Kitchen");
    let err = store.change(|book| {
        book.save_txn(
            &draft(
                "x",
                Entry::Expense {
                    payer: food,
                    lines: vec![split(food, "1")],
                },
            ),
            None,
        )
    });
    assert_eq!(err.unwrap_err().code(), "wrong_account_kind");
    assert_eq!(store.book(), &before);
    drop(store);
    assert_eq!(open(&path).0.book(), &before);
}

#[test]
fn never_overwrites_an_existing_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("notes.lancelot");
    std::fs::write(&path, "important").unwrap();
    let err = Store::create(&path, details(), &two_accounts(), StoreOptions::default())
        .err()
        .unwrap();
    assert_eq!(err.code(), "already_exists");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "important");
    // No temporary files are left behind.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn rejects_files_that_are_not_event_files() {
    let dir = TempDir::new().unwrap();
    let text = dir.path().join("text.lancelot");
    std::fs::write(&text, "just some text, long enough for a header").unwrap();
    assert_eq!(
        Store::open(&text, StoreOptions::default())
            .err()
            .unwrap()
            .code(),
        "not_an_event_file"
    );

    let empty = dir.path().join("empty.lancelot");
    std::fs::write(&empty, "").unwrap();
    assert_eq!(
        Store::open(&empty, StoreOptions::default())
            .err()
            .unwrap()
            .code(),
        "not_an_event_file"
    );

    let foreign = dir.path().join("foreign.sqlite");
    raw(&foreign)
        .execute_batch("CREATE TABLE t (x INTEGER);")
        .unwrap();
    assert_eq!(
        Store::open(&foreign, StoreOptions::default())
            .err()
            .unwrap()
            .code(),
        "not_an_event_file"
    );
}

#[test]
fn refuses_files_from_a_newer_version() {
    let dir = TempDir::new().unwrap();
    let (store, path) = create(&dir);
    drop(store);
    raw(&path).pragma_update(None, "user_version", 99).unwrap();
    match Store::open(&path, StoreOptions::default()) {
        Err(StoreError::NewerSchema {
            found: 99,
            supported,
        }) => assert_eq!(supported, SCHEMA_VERSION),
        other => panic!("unexpected {:?}", other.err()),
    }
}

#[test]
fn a_file_can_only_be_open_once() {
    let dir = TempDir::new().unwrap();
    let (store, path) = create(&dir);
    assert_eq!(
        Store::open(&path, StoreOptions::default())
            .err()
            .unwrap()
            .code(),
        "already_open"
    );
    drop(store);
    assert!(Store::open(&path, StoreOptions::default()).is_ok());
}

#[test]
fn leaves_no_journal_files_behind() {
    let dir = TempDir::new().unwrap();
    let (mut store, _) = create(&dir);
    store
        .change(|book| book.add_person("Anna", Person::default()))
        .unwrap();
    drop(store);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["sommerfest.lancelot"]);
}

#[test]
fn a_tampered_transaction_is_reported_and_the_file_not_loaded() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let txn = store.save(
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    drop(store);

    let conn = raw(&path);
    conn.execute(
        "UPDATE line SET amount = 999 WHERE txn_id = ?1 AND position = 1",
        [txn.0],
    )
    .unwrap();
    drop(conn);

    match Store::open(&path, StoreOptions::default()) {
        Err(StoreError::Damaged(report)) => {
            let codes = report.codes();
            assert!(codes.contains(&"unbalanced"), "{codes:?}");
        }
        other => panic!("unexpected {:?}", other.err()),
    }
}

#[test]
fn a_transaction_with_the_wrong_shape_is_reported() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let txn = store.save(
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    drop(store);

    // Balanced, but the opening account belongs to opening counts alone.
    let opening = category(&new_book(), AccountKind::Opening, "Opening balance");
    let conn = raw(&path);
    conn.execute(
        "UPDATE line SET account_id = ?2 WHERE txn_id = ?1 AND position = 0",
        [txn.0, opening.0],
    )
    .unwrap();
    drop(conn);

    // The file is sound and its contents fail the rules: that is not called damage.
    match Store::open(&path, StoreOptions::default()) {
        Err(StoreError::Inconsistent(report)) => {
            assert!(report.codes().contains(&"malformed_txn"), "{report:?}")
        }
        other => panic!("unexpected {:?}", other.err()),
    }
}

#[test]
fn a_tampered_attachment_is_reported_but_the_file_opens() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let txn = store.save(
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    store.attach(txn, false);
    assert!(store.check_integrity().unwrap().is_clean());
    drop(store);

    raw(&path)
        .execute("UPDATE attachment SET data = x'00'", [])
        .unwrap();
    let (store, report) = open(&path);
    assert_eq!(report.codes(), ["attachment_hash"]);
    assert_eq!(
        store.check_integrity().unwrap().codes(),
        ["attachment_hash"]
    );
}

#[test]
fn overpaying_shows_as_a_warning() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let verein = asset(store.book(), "Verein account");
    let anna = store.person("Anna");
    store.save(
        "Too much",
        Entry::Settle {
            person: anna,
            account: verein,
            amount: eur("10"),
            direction: SettleDirection::Reimburse,
        },
    );
    drop(store);
    let (_, report) = open(&path);
    assert_eq!(report.codes(), ["over_reimbursed"]);
    assert!(!report.has_errors());
}

#[test]
fn every_change_is_in_the_change_log() {
    let dir = TempDir::new().unwrap();
    let (mut store, _) = create(&dir);
    let (cash, food, tech) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
        expense(store.book(), "Tech"),
    );
    let txn = store.save(
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    store
        .change(|book| {
            book.save_txn(
                &draft(
                    "Cables",
                    Entry::Expense {
                        payer: cash,
                        lines: vec![split(tech, "35")],
                    },
                ),
                Some(txn),
            )
        })
        .unwrap();
    store.change(|book| book.delete_txn(txn)).unwrap();
    store
        .change(|book| book.set_closing_count(cash, june(14), Cents::ZERO))
        .unwrap();
    store.close(vec![]).unwrap();
    store.change(|book| book.reopen()).unwrap();

    let log = store.change_log(100).unwrap();
    let ops: Vec<(&str, &str)> = log
        .iter()
        .take(6)
        .map(|e| (e.entity.as_str(), e.op.as_str()))
        .collect();
    assert_eq!(
        ops,
        [
            ("event", "reopen"), // R-EVT-2
            ("event", "close"),
            ("closing_count", "create"),
            ("txn", "delete"),
            ("txn", "update"),
            ("txn", "create"),
        ]
    );
    let update = &log[4];
    assert_eq!(update.before.as_ref().unwrap()["description"], "Bakery");
    assert_eq!(update.after.as_ref().unwrap()["description"], "Cables");
    assert_eq!(log[3].after, None);
    assert_eq!(
        log[0].after.as_ref().unwrap()["closedAt"],
        serde_json::Value::Null
    );
    assert!(log[1].after.as_ref().unwrap()["closedAt"].is_string());
    // Creating the file logged the event and its default accounts.
    assert!(log.iter().any(|e| e.entity == "event" && e.op == "create"));
}

#[test]
fn ids_are_not_reused_after_reopening() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let entry = Entry::Expense {
        payer: cash,
        lines: vec![split(food, "20")],
    };
    let first = store.save("a", entry.clone());
    store.change(|book| book.delete_txn(first)).unwrap();
    drop(store);
    let (mut store, _) = open(&path);
    assert!(store.save("b", entry) > first);
}

/// A clock the test moves forward by hand.
fn clock() -> (Arc<Mutex<OffsetDateTime>>, StoreOptions, TempDir) {
    let now = Arc::new(Mutex::new(now()));
    let shared = now.clone();
    let backups = TempDir::new().unwrap();
    let mut policy = BackupPolicy::new(backups.path());
    policy.keep = 3;
    let options = StoreOptions {
        backups: Some(policy),
        clock: Arc::new(move || *shared.lock().unwrap()),
    };
    (now, options, backups)
}

#[test]
fn backups_on_open_after_the_interval_and_pruned() {
    let dir = TempDir::new().unwrap();
    let (now, options, backups) = clock();
    let path = dir.path().join("sommerfest.lancelot");
    let (mut store, _) = Store::create(&path, details(), &two_accounts(), options).unwrap();
    let uuid = store.event_uuid().to_owned();
    let count = || list_backups(backups.path(), &uuid).unwrap().len();
    assert_eq!(count(), 1, "a backup on open");

    let advance = |minutes| *now.lock().unwrap() += Duration::minutes(minutes);
    advance(1);
    store
        .change(|book| book.add_person("Anna", Person::default()))
        .unwrap();
    assert_eq!(count(), 1, "within the interval");

    advance(10);
    store
        .change(|book| book.add_person("Bob", Person::default()))
        .unwrap();
    assert_eq!(count(), 2, "after the interval");

    for name in ["Clara", "Malte", "Nina"] {
        advance(11);
        store
            .change(|book| book.add_person(name, Person::default()))
            .unwrap();
    }
    assert_eq!(count(), 3, "only the newest are kept");
    assert!(store.backup_error().is_none());

    // The newest backup is a complete event file with everything up to the last backup.
    let newest = list_backups(backups.path(), &uuid).unwrap()[0].clone();
    drop(store);
    let (copy, report) = Store::open(&newest, StoreOptions::default()).unwrap();
    assert!(report.is_clean());
    assert_eq!(copy.book().persons().count(), 5);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Whatever happens, the file holds exactly the book, and reopening gives it back.
    #[test]
    fn the_file_always_matches_the_book(ops in prop::collection::vec(op(), 1..25)) {
        let dir = TempDir::new().unwrap();
        let (mut store, path) = create(&dir);
        let mut book = store.book().clone();
        for name in ["Anna", "Bob", "Malte"] {
            Target::person(&mut store, name);
            Target::person(&mut book, name);
        }
        let ids = Ids::of(&book);
        for op in &ops {
            match ids.action(&book, op) {
                Action::Save(d, id) => prop_assert_eq!(store.change(|book| book.save_txn(&d, id)).is_ok(), book.save_txn(&d, id).is_ok()),
                Action::Delete(id) => prop_assert_eq!(store.change(|book| book.delete_txn(id)).is_ok(), book.delete_txn(id).is_ok()),
                Action::Nothing => {}
            }
            prop_assert_eq!(store.book(), &book);
        }
        drop(store);
        let (reopened, report) = open(&path);
        prop_assert!(!report.has_errors(), "{:?}", report);
        prop_assert_eq!(reopened.book(), &book);
    }
}

#[test]
fn a_transaction_and_its_files_are_saved_together() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, kitchen) = (
        asset(store.book(), "Cash box"),
        expense(store.book(), "Kitchen"),
    );
    let entry = Entry::Expense {
        payer: cash,
        lines: vec![split(kitchen, "20")],
    };
    let jpg = b"\xFF\xD8\xFF\xE0 photo".to_vec();
    let pdf = b"%PDF-1.7 scan".to_vec();

    let saved = store
        .save_txn_with_files(
            &draft("Bakery", entry.clone()),
            None,
            &[("a.jpg".into(), jpg.clone()), ("b.pdf".into(), pdf.clone())],
            &[],
        )
        .unwrap();
    let ids: Vec<AttachmentId> = store
        .book()
        .attachments_of(saved.id)
        .map(|a| a.id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(store.attachment_data(ids[0]).unwrap(), jpg);
    assert_eq!(
        store.book().attachment(ids[1]).unwrap().mime,
        "application/pdf"
    );

    // Edit: drop the photo, add another scan.
    store
        .save_txn_with_files(
            &draft("Bakery", entry.clone()),
            Some(saved.id),
            &[("c.pdf".into(), pdf.clone())],
            &[ids[0]],
        )
        .unwrap();
    let names: Vec<String> = store
        .book()
        .attachments_of(saved.id)
        .map(|a| a.filename.clone())
        .collect();
    assert_eq!(names, ["b.pdf", "c.pdf"]);

    // A save that fails writes none of its files.
    let before = store.book().clone();
    let mut wrong = draft("Bakery", entry.clone());
    wrong.total = Some(eur("99"));
    let err = store
        .save_txn_with_files(
            &wrong,
            Some(saved.id),
            &[("d.pdf".into(), pdf.clone())],
            &[],
        )
        .unwrap_err();
    assert_eq!(err.code(), "split_mismatch");
    assert_eq!(store.book(), &before);

    // Attachments of another transaction cannot be removed through this one.
    let other = store
        .save_txn_with_files(&draft("Other", entry), None, &[], &[])
        .unwrap();
    let foreign = store.book().attachments_of(saved.id).next().unwrap().id;
    assert_eq!(
        store
            .save_txn_with_files(
                &draft(
                    "Other",
                    Entry::Expense {
                        payer: cash,
                        lines: vec![split(kitchen, "20")]
                    }
                ),
                Some(other.id),
                &[],
                &[foreign]
            )
            .unwrap_err()
            .code(),
        "unknown_attachment"
    );
    drop(store);
    let (store, report) = open(&path);
    assert!(store.check_integrity().unwrap().is_clean() && report.is_clean());
    assert_eq!(store.book().attachments().count(), 2);
}

#[test]
fn marking_the_payout_list_as_paid_settles_everyone_with_an_iban() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let m = mock::build(&mut store);
    let iban = |s: &str| Person {
        iban: Some(validate_iban(s).unwrap()),
        ..Person::default()
    };
    store
        .change(|book| book.update_person(m.anna, "Anna", iban("DE89370400440532013000")))
        .unwrap();
    store
        .change(|book| book.update_person(m.malte, "Malte", iban("DE02120300000000202051")))
        .unwrap();
    let verein = asset(store.book(), "Verein account");

    let n = store
        .change(|book| book.mark_payouts_paid(verein, june(20), &[m.anna, m.malte]))
        .unwrap();
    assert_eq!(n, 2);
    let l = store.book().ledger();
    assert_eq!(l.person(m.anna).unwrap().net, Cents::ZERO);
    assert_eq!(l.person(m.malte).unwrap().net, Cents::ZERO);
    assert_eq!(
        l.person(m.bob).unwrap().net,
        eur("-180"),
        "repayments are not touched"
    );
    assert_eq!(
        l.balance(verein),
        eur("1.757").checked_sub(eur("322,40")).unwrap()
    );
    assert!(payouts(store.book()).unwrap().payouts.is_empty());
    drop(store);
    assert!(open(&path).1.is_clean());
}

#[test]
fn asset_types_and_counts_survive_reopening() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let bar = store
        .change(|book| book.add_asset("Cash – Bar", AssetType::Cash))
        .unwrap();
    let verein = asset(store.book(), "Verein account");
    store
        .change(|book| book.set_asset_type(verein, AssetType::Cash))
        .unwrap();
    store
        .change(|book| book.set_opening_count(bar, june(12), eur("40")))
        .unwrap();
    store
        .change(|book| book.set_opening_count(bar, june(12), eur("50")))
        .unwrap();
    let book = store.book().clone();
    drop(store);
    let (mut reopened, report) = open(&path);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    reopened
        .change(|book| book.clear_opening_count(bar))
        .unwrap();
    assert_eq!(reopened.book().txns().count(), 0);
}

#[test]
fn the_database_refuses_an_asset_without_a_type() {
    let dir = TempDir::new().unwrap();
    let (store, path) = create(&dir);
    drop(store);
    let conn = raw(&path);
    for sql in [
        "UPDATE account SET asset_type = NULL WHERE name = 'Cash box'",
        "UPDATE account SET asset_type = 'bank' WHERE kind = 'opening'",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn a_closed_event_reopens_closed_and_read_only() {
    let (at, options, _backups) = clock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("e.lancelot");
    let (mut store, _) = Store::create(&path, details(), &two_accounts(), options.clone()).unwrap();
    let cash = asset(store.book(), "Cash box");
    store
        .change(|book| book.set_closing_count(cash, june(14), Cents::ZERO))
        .unwrap();
    store.close(vec![]).unwrap();
    let closed_at = *at.lock().unwrap();
    drop(store);

    let (mut store, report) = Store::open(&path, options).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book().event().closed_at, Some(closed_at));
    assert_eq!(
        store
            .change(|book| book.add_person("Kim", Person::default()))
            .unwrap_err()
            .code(),
        "event_closed"
    );
    store.change(|book| book.reopen()).unwrap();
    assert!(!store.book().event().is_closed());
    store
        .change(|book| book.add_person("Kim", Person::default()))
        .unwrap();
}

#[test]
fn a_mistyped_person_or_category_can_go_while_nothing_refers_to_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) =
        Store::create(&path, details, &two_accounts(), StoreOptions::default()).unwrap();
    let typo = store
        .change(|book| book.add_person("Aña", Person::default()))
        .unwrap();
    let category = store
        .change(|book| book.add_account(AccountKind::Expense, "Kichten"))
        .unwrap();
    let used = store
        .change(|book| book.add_person("Anna", Person::default()))
        .unwrap();
    let entry = Entry::Expense {
        payer: used,
        lines: vec![split(category, "10")],
    };
    let txn = store
        .change(|book| book.save_txn(&draft("Food", entry), None))
        .unwrap()
        .id;

    assert_eq!(
        store
            .change(|book| book.delete_account(used))
            .unwrap_err()
            .code(),
        "account_in_use"
    );
    assert_eq!(
        store
            .change(|book| book.delete_account(category))
            .unwrap_err()
            .code(),
        "account_in_use"
    );
    let verein = asset(store.book(), "Verein account");
    assert_eq!(
        store
            .change(|book| book.delete_account(verein))
            .unwrap_err()
            .code(),
        "account_in_use"
    );
    store.change(|book| book.delete_account(typo)).unwrap();
    assert!(store.book().account(typo).is_none());
    assert_eq!(
        store.book().persons().filter(|(a, _)| a.id == typo).count(),
        0
    );

    store.change(|book| book.delete_txn(txn)).unwrap();
    store.change(|book| book.delete_account(category)).unwrap();
    let log = store.change_log(50).unwrap();
    assert!(
        log.iter()
            .any(|e| e.entity == "account" && e.op == "delete")
    );

    let expected = store.book().clone();
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &expected);
}

/// A write that reaches the file wrong, here by a trigger, is rolled back and never kept.
#[test]
fn a_write_is_read_back_before_it_counts() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("e.lancelot");
    let (store, _) =
        Store::create(&path, details(), &two_accounts(), StoreOptions::default()).unwrap();
    drop(store);
    // The schema is read when a connection opens, so the trigger goes in before the store does.
    let extra = raw(&path);
    extra
        .execute_batch(
            "CREATE TRIGGER wrong_name AFTER INSERT ON account BEGIN
               UPDATE account SET name = 'Someone else' WHERE id = NEW.id;
             END;",
        )
        .unwrap();
    drop(extra);
    let (mut store, _) = Store::open(&path, StoreOptions::default()).unwrap();
    let before = store.book().clone();
    let error = store
        .change(|b| b.add_person("Anna", Person::default()))
        .unwrap_err();
    assert!(matches!(error, StoreError::IntegrityFailed(_)), "{error:?}");
    assert_eq!(store.book(), &before);
    drop(store);
    // Nothing of it reached the file.
    let (store, _) = open(&path);
    assert_eq!(store.book().persons().count(), 0);
}

#[test]
fn a_generated_entry_written_wrong_is_caught_although_loading_regenerates_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("e.lancelot");
    let (store, _) =
        Store::create(&path, details(), &two_accounts(), StoreOptions::default()).unwrap();
    drop(store);
    let extra = raw(&path);
    extra
        .execute_batch(
            "CREATE TRIGGER wrong_wording AFTER INSERT ON txn
             WHEN NEW.origin <> 'manual' BEGIN
               UPDATE txn SET description = 'Something else' WHERE id = NEW.id;
             END;",
        )
        .unwrap();
    drop(extra);
    let (mut store, _) = Store::open(&path, StoreOptions::default()).unwrap();
    let anna = store
        .change(|b| b.add_person("Anna", Person::default()))
        .unwrap();
    let tier = store
        .change(|b| {
            b.add_tier(TierDraft {
                name: "Regular".into(),
                price: eur("10,00"),
                fee_bp: 0,
                quota: None,
                aliases: Vec::new(),
            })
        })
        .unwrap();
    let error = store
        .change(|b| {
            b.add_manual_sale(ManualSaleDraft {
                tier,
                count: 1,
                destination: anna,
                date: june(13),
                reference: None,
            })
        })
        .unwrap_err();
    assert!(matches!(error, StoreError::IntegrityFailed(_)), "{error:?}");
}

#[test]
fn a_foreign_key_left_dangling_stops_the_commit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("e.lancelot");
    let (store, _) =
        Store::create(&path, details(), &two_accounts(), StoreOptions::default()).unwrap();
    drop(store);
    let extra = raw(&path);
    extra
        .execute_batch(
            "CREATE TRIGGER dangling AFTER INSERT ON account
             WHEN NEW.kind = 'person' BEGIN
               UPDATE account SET cost_centre_id = 999 WHERE id = NEW.id;
             END;",
        )
        .unwrap();
    drop(extra);
    let (mut store, _) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(
        store
            .change(|b| b.add_person("Anna", Person::default()))
            .is_err()
    );
    assert_eq!(store.book().persons().count(), 0);
}

#[test]
fn the_settings_on_the_event_row_are_saved_and_logged() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    store
        .change(|book| book.set_tickets_enabled(false))
        .unwrap();
    let log = store.change_log(10).unwrap();
    let last = |entity: &str| log.iter().find(|e| e.entity == entity).unwrap();
    assert_eq!(last("ticket_settings").op, "update");
    assert_eq!(
        last("ticket_settings").after.as_ref().unwrap()["enabled"],
        false
    );
    drop(store);

    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert!(!reopened.book().ticket_settings().enabled);
}

#[test]
fn deleting_an_import_is_logged_once() {
    let dir = TempDir::new().unwrap();
    let (mut store, _) = create(&dir);
    let tier = |name: &str, price| TierDraft {
        name: name.into(),
        price: eur(price),
        fee_bp: 0,
        quota: None,
        aliases: Vec::new(),
    };
    for (name, price) in [
        ("Early Bird", "25,00"),
        ("Regular", "32,00"),
        ("Supporter", "45,00"),
    ] {
        store.change(|b| b.add_tier(tier(name, price))).unwrap();
    }
    let sheet = include_bytes!("../../../fixtures/tickets/import-1.ods");
    let import = store
        .import_tickets(&batch("import-1.ods", sheet), june(1))
        .unwrap();
    store.change(|b| b.delete_import(import)).unwrap();
    let log = store.change_log(100).unwrap();
    let deletions = log
        .iter()
        .filter(|e| e.entity == "ticket_import" && e.op == "delete")
        .count();
    assert_eq!(deletions, 1);
}

#[test]
fn every_kind_of_transfer_and_setting_survives_reopening() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let cash = asset(store.book(), "Cash box");
    let food = expense(store.book(), "Kitchen");
    let anna = store
        .change(|b| b.add_person("Anna", Person::default()))
        .unwrap();
    store.save(
        "To Anna",
        Entry::Transfer {
            from: cash,
            to: anna,
            amount: eur("20"),
        },
    );
    store.save(
        "Onto a category",
        Entry::Transfer {
            from: anna,
            to: food,
            amount: eur("5"),
        },
    );
    store.change(|b| b.set_net_movement(cash, true)).unwrap();
    store.change(|b| b.set_ticket_account(Some(cash))).unwrap();
    let centre = store.change(|b| b.add_cost_centre("Gone")).unwrap();
    store.change(|b| b.delete_cost_centre(centre)).unwrap();
    store.change(|b| b.set_tickets_enabled(false)).unwrap();
    let book = store.book().clone();
    let log = store.change_log(1000).unwrap();
    drop(store);

    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    assert!(log.iter().any(|e| e.entity == "ticket_settings"));
}

#[test]
fn a_new_file_has_no_triggers_and_lines_are_checked_on_opening() {
    let dir = TempDir::new().unwrap();
    let (store, path) = create(&dir);
    drop(store);
    let conn = raw(&path);
    let triggers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(triggers, 0);
}

#[test]
fn an_event_with_a_single_cash_account_opens_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e.lancelot");
    let till = [AccountDraft {
        name: "Till".into(),
        asset_type: AssetType::Cash,
    }];
    drop(Store::create(&path, details(), &till, StoreOptions::default()).unwrap());

    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(!report.has_errors());
    let id = asset(store.book(), "Till");
    assert_eq!(store.book().event().default_reimbursement_account, id);
    assert!(
        store
            .book()
            .account_by_name(AccountKind::Asset, "Cash box")
            .is_none()
    );
}
