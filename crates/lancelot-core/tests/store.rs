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
    let (store, report) = Store::create(&path, details(), StoreOptions::default()).unwrap();
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
    assert_eq!(l.balance(role(store.book(), Role::Verein)), eur("1.757"));
    assert_eq!(l.balance(role(store.book(), Role::CashBox)), eur("412,90"));
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
        role(store.book(), Role::CashBox),
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
    store.remove_attachment(id).unwrap();
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
        role(store.book(), Role::CashBox),
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
    store.delete_txn(txn).unwrap();
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
    let err = store.save_txn(
        &draft(
            "x",
            Entry::Expense {
                payer: food,
                lines: vec![split(food, "1")],
            },
        ),
        None,
    );
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
    let err = Store::create(&path, details(), StoreOptions::default())
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
    store.add_person("Anna", Person::default()).unwrap();
    drop(store);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["sommerfest.lancelot"]);
}

#[test]
fn the_database_refuses_unbalanced_transactions() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        role(store.book(), Role::CashBox),
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
    // Lines of a sealed transaction cannot change.
    let err = conn
        .execute("UPDATE line SET amount = 999 WHERE txn_id = ?1", [txn.0])
        .unwrap_err();
    assert!(err.to_string().contains("sealed"), "{err}");
    let err = conn
        .execute("DELETE FROM line WHERE txn_id = ?1", [txn.0])
        .unwrap_err();
    assert!(err.to_string().contains("sealed"), "{err}");
    // An unbalanced transaction cannot be sealed.
    conn.execute("UPDATE txn SET sealed = 0 WHERE id = ?1", [txn.0])
        .unwrap();
    conn.execute(
        "UPDATE line SET amount = 999 WHERE txn_id = ?1 AND position = 1",
        [txn.0],
    )
    .unwrap();
    let err = conn
        .execute("UPDATE txn SET sealed = 1 WHERE id = ?1", [txn.0])
        .unwrap_err();
    assert!(err.to_string().contains("unbalanced"), "{err}");
    // Nor can one be inserted sealed.
    let err = conn
        .execute(
            "INSERT INTO txn (id, kind, date, sealed) VALUES (99, 'expense', '2026-06-13', 1)",
            [],
        )
        .unwrap_err();
    assert!(err.to_string().contains("unsealed"), "{err}");
}

#[test]
fn a_tampered_transaction_is_reported_and_the_file_not_loaded() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        role(store.book(), Role::CashBox),
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
    conn.execute("UPDATE txn SET sealed = 0 WHERE id = ?1", [txn.0])
        .unwrap();
    conn.execute(
        "UPDATE line SET amount = 999 WHERE txn_id = ?1 AND position = 1",
        [txn.0],
    )
    .unwrap();
    drop(conn);

    match Store::open(&path, StoreOptions::default()) {
        Err(StoreError::Damaged(report)) => {
            let codes = report.codes();
            assert!(
                codes.contains(&"unsealed") && codes.contains(&"unbalanced"),
                "{codes:?}"
            );
        }
        other => panic!("unexpected {:?}", other.err()),
    }
}

#[test]
fn a_transaction_with_the_wrong_shape_is_reported() {
    let dir = TempDir::new().unwrap();
    let (mut store, path) = create(&dir);
    let (cash, food) = (
        role(store.book(), Role::CashBox),
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

    // Balanced, but now it books money from Tickets into Kitchen, which no form can create.
    let tickets = income(&new_book(), "Tickets");
    let conn = raw(&path);
    conn.execute("UPDATE txn SET sealed = 0 WHERE id = ?1", [txn.0])
        .unwrap();
    conn.execute(
        "UPDATE line SET account_id = ?2 WHERE txn_id = ?1 AND position = 0",
        [txn.0, tickets.0],
    )
    .unwrap();
    conn.execute("UPDATE txn SET sealed = 1 WHERE id = ?1", [txn.0])
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
        role(store.book(), Role::CashBox),
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
    let verein = role(store.book(), Role::Verein);
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
        role(store.book(), Role::CashBox),
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
        .save_txn(
            &draft(
                "Cables",
                Entry::Expense {
                    payer: cash,
                    lines: vec![split(tech, "35")],
                },
            ),
            Some(txn),
        )
        .unwrap();
    store.delete_txn(txn).unwrap();
    store
        .set_closing_count(cash, june(14), Cents::ZERO)
        .unwrap();
    store.close(vec![]).unwrap();
    store.reopen().unwrap();

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
        role(store.book(), Role::CashBox),
        expense(store.book(), "Kitchen"),
    );
    let entry = Entry::Expense {
        payer: cash,
        lines: vec![split(food, "20")],
    };
    let first = store.save("a", entry.clone());
    store.delete_txn(first).unwrap();
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
    let (mut store, _) = Store::create(&path, details(), options).unwrap();
    let uuid = store.event_uuid().to_owned();
    let count = || list_backups(backups.path(), &uuid).unwrap().len();
    assert_eq!(count(), 1, "a backup on open");

    let advance = |minutes| *now.lock().unwrap() += Duration::minutes(minutes);
    advance(1);
    store.add_person("Anna", Person::default()).unwrap();
    assert_eq!(count(), 1, "within the interval");

    advance(10);
    store.add_person("Bob", Person::default()).unwrap();
    assert_eq!(count(), 2, "after the interval");

    for name in ["Clara", "Malte", "Nina"] {
        advance(11);
        store.add_person(name, Person::default()).unwrap();
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
                Action::Save(d, id) => prop_assert_eq!(store.save_txn(&d, id).is_ok(), book.save_txn(&d, id).is_ok()),
                Action::Delete(id) => prop_assert_eq!(store.delete_txn(id).is_ok(), book.delete_txn(id).is_ok()),
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
        role(store.book(), Role::CashBox),
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
        .update_person(m.anna, "Anna", iban("DE89370400440532013000"))
        .unwrap();
    store
        .update_person(m.malte, "Malte", iban("DE02120300000000202051"))
        .unwrap();
    let verein = role(store.book(), Role::Verein);

    let n = store.mark_payouts_paid(verein, june(20)).unwrap();
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
    let bar = store.add_asset("Cash – Bar", AssetType::Cash).unwrap();
    let verein = role(store.book(), Role::Verein);
    store.set_asset_type(verein, AssetType::Cash).unwrap();
    store.set_opening_count(bar, june(12), eur("40")).unwrap();
    store.set_opening_count(bar, june(12), eur("50")).unwrap();
    let book = store.book().clone();
    drop(store);
    let (mut reopened, report) = open(&path);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    reopened.clear_opening_count(bar).unwrap();
    assert_eq!(reopened.book().txns().count(), 0);
}

#[test]
fn the_database_refuses_an_asset_without_a_type() {
    let dir = TempDir::new().unwrap();
    let (store, path) = create(&dir);
    drop(store);
    let conn = raw(&path);
    for sql in [
        "UPDATE account SET asset_type = NULL WHERE role = 'cash_box'",
        "UPDATE account SET asset_type = 'bank' WHERE role = 'opening'",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn a_closed_event_reopens_closed_and_read_only() {
    let (at, options, _backups) = clock();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("e.lancelot");
    let (mut store, _) = Store::create(&path, details(), options.clone()).unwrap();
    let cash = role(store.book(), Role::CashBox);
    store
        .set_closing_count(cash, june(14), Cents::ZERO)
        .unwrap();
    store.close(vec![]).unwrap();
    let closed_at = *at.lock().unwrap();
    drop(store);

    let (mut store, report) = Store::open(&path, options).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book().event().closed_at, Some(closed_at));
    assert_eq!(
        store
            .add_person("Kim", Person::default())
            .unwrap_err()
            .code(),
        "event_closed"
    );
    store.reopen().unwrap();
    assert!(!store.book().event().is_closed());
    store.add_person("Kim", Person::default()).unwrap();
}

#[test]
fn a_mistyped_person_or_category_can_go_while_nothing_refers_to_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let typo = store.add_person("Aña", Person::default()).unwrap();
    let category = store.add_account(AccountKind::Expense, "Kichten").unwrap();
    let used = store.add_person("Anna", Person::default()).unwrap();
    let entry = Entry::Expense {
        payer: used,
        lines: vec![split(category, "10")],
    };
    let txn = store.save_txn(&draft("Food", entry), None).unwrap().id;

    assert_eq!(
        store.delete_account(used).unwrap_err().code(),
        "account_in_use"
    );
    assert_eq!(
        store.delete_account(category).unwrap_err().code(),
        "account_in_use"
    );
    let verein = role(store.book(), Role::Verein);
    assert_eq!(
        store.delete_account(verein).unwrap_err().code(),
        "account_in_use"
    );
    store.delete_account(typo).unwrap();
    assert!(store.book().account(typo).is_none());
    assert_eq!(
        store.book().persons().filter(|(a, _)| a.id == typo).count(),
        0
    );

    store.delete_txn(txn).unwrap();
    store.delete_account(category).unwrap();
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
