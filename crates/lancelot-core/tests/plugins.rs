//! Plugin records (technical design §10): stored, logged and copied, never read for their meaning.

mod common;

use common::*;
use lancelot_core::*;
use serde_json::{Value, json};
use tempfile::TempDir;

fn set(key: &str, scope: PluginScope, value: Value) -> PluginChange {
    PluginChange {
        key: key.into(),
        scope,
        value,
    }
}

fn apply(book: &mut Book, changes: Vec<PluginChange>) -> Result<(), DomainError> {
    book.apply_plugin_changes("drinks".into(), changes)
}

fn values(book: &Book) -> Vec<(String, Value)> {
    book.plugin_records()
        .map(|r| (r.key.clone(), r.value.clone()))
        .collect()
}

#[test]
fn records_are_set_replaced_and_removed() {
    let mut book = new_book();
    let setup = PluginScope::Setup;
    apply(
        &mut book,
        vec![
            set("a", setup, json!({ "n": 1 })),
            set("b", setup, json!([1, 2])),
        ],
    )
    .unwrap();
    apply(&mut book, vec![set("a", setup, json!({ "n": 2 }))]).unwrap();
    assert_eq!(
        values(&book),
        [
            ("a".to_owned(), json!({ "n": 2 })),
            ("b".to_owned(), json!([1, 2]))
        ]
    );

    let first = book.plugin_records().next().unwrap().id;
    apply(&mut book, vec![set("a", setup, Value::Null)]).unwrap();
    assert_eq!(values(&book), [("b".to_owned(), json!([1, 2]))]);
    // Removing it again changes nothing, and its id is not used again.
    apply(&mut book, vec![set("a", setup, Value::Null)]).unwrap();
    apply(&mut book, vec![set("a", setup, json!(true))]).unwrap();
    assert!(book.plugin_records().all(|r| r.id != first));
}

#[test]
fn two_plugins_can_use_the_same_key() {
    let mut book = new_book();
    let change = |n: i64| vec![set("a", PluginScope::Records, json!(n))];
    book.apply_plugin_changes("one".into(), change(1)).unwrap();
    book.apply_plugin_changes("two".into(), change(2)).unwrap();
    assert_eq!(book.plugin_records().count(), 2);
}

#[test]
fn an_invalid_change_applies_nothing() {
    let mut book = new_book();
    let good = set("good", PluginScope::Setup, json!(1));
    let err = apply(
        &mut book,
        vec![good.clone(), set("", PluginScope::Setup, json!(2))],
    )
    .unwrap_err();
    assert_eq!(
        (err.code(), err.field()),
        ("invalid_plugin_record", Some("key"))
    );
    let too_large = json!("x".repeat(64 * 1024 + 1));
    let err = apply(
        &mut book,
        vec![good.clone(), set("big", PluginScope::Setup, too_large)],
    )
    .unwrap_err();
    assert_eq!(err.field(), Some("value"));
    let err = book
        .apply_plugin_changes("Not Valid".into(), vec![good])
        .unwrap_err();
    assert_eq!(err.field(), Some("plugin"));
    assert_eq!(book.plugin_records().count(), 0);
}

#[test]
fn a_closed_event_refuses_changes() {
    let mut book = new_book();
    let overrides = book
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "Accepted".into(),
        })
        .collect();
    book.close(overrides, now()).unwrap();
    let err = apply(&mut book, vec![set("a", PluginScope::Setup, json!(1))]).unwrap_err();
    assert_eq!(err.code(), "event_closed");
}

/// Rule 1: a file settles the same with or without any plugin's records.
#[test]
fn records_change_no_figure() {
    let mut book = new_book();
    mock::build(&mut book);
    let before = book.clone();
    apply(
        &mut book,
        vec![set(
            "anything",
            PluginScope::Records,
            json!({ "cost": 99_999 }),
        )],
    )
    .unwrap();
    assert_eq!(book.ledger(), before.ledger());
    assert_eq!(book.budget(), before.budget());
    assert_eq!(book.open_items(), before.open_items());
}

#[test]
fn records_survive_saving_and_are_logged() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("event.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let change = |changes| move |b: &mut Book| b.apply_plugin_changes("drinks".into(), changes);

    store
        .change(change(vec![
            set("a", PluginScope::Setup, json!({ "price": 150 })),
            set("b", PluginScope::Records, json!({ "text": "Größe ☕" })),
        ]))
        .unwrap();
    store
        .change(change(vec![set("a", PluginScope::Setup, json!(2.5))]))
        .unwrap();
    store
        .change(change(vec![set("b", PluginScope::Records, Value::Null)]))
        .unwrap();

    let log = store.change_log(10).unwrap();
    let ops: Vec<_> = log
        .iter()
        .filter(|e| e.entity == "plugin_record")
        .map(|e| e.op.as_str())
        .collect();
    assert_eq!(ops, ["delete", "update", "create", "create"]);

    let book = store.book().clone();
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &book);
    assert_eq!(values(store.book()), [("a".to_owned(), json!(2.5))]);
}

#[test]
fn a_template_copies_only_the_setup() {
    let mut source = new_book();
    apply(
        &mut source,
        vec![
            set("price-list", PluginScope::Setup, json!([1])),
            set("counts", PluginScope::Records, json!([2])),
        ],
    )
    .unwrap();
    let book = Book::from_template(&source, source.event().details.clone()).unwrap();
    assert_eq!(values(&book), [("price-list".to_owned(), json!([1]))]);
}

#[test]
fn copying_from_another_event_leaves_records_alone() {
    let mut source = new_book();
    apply(&mut source, vec![set("a", PluginScope::Setup, json!(1))]).unwrap();
    let mut book = new_book();
    let choice = CopyChoice {
        persons: true,
        budget: true,
    };
    book.copy_from(&source, choice).unwrap();
    assert_eq!(book.plugin_records().count(), 0);
}

#[test]
fn a_file_with_two_records_for_one_key_does_not_load() {
    let mut book = new_book();
    apply(&mut book, vec![set("a", PluginScope::Setup, json!(1))]).unwrap();
    let record = book.plugin_records().next().unwrap().clone();
    let twin = PluginRecord {
        id: PluginRecordId(record.id.0 + 1),
        ..record.clone()
    };
    let mut next = book.next_ids();
    next.plugin_record = twin.id.0 + 1;
    let parts = BookParts {
        event: book.event().clone(),
        ticket_settings: book.ticket_settings(),
        accounts: book.accounts().cloned().collect(),
        persons: Vec::new(),
        txns: Vec::new(),
        attachments: Vec::new(),
        closing_counts: Vec::new(),
        reconciliations: Vec::new(),
        budget_lines: Vec::new(),
        tiers: Vec::new(),
        imports: Vec::new(),
        manual_sales: Vec::new(),
        cancellations: Vec::new(),
        cost_centres: Vec::new(),
        plugin_records: vec![record, twin],
        next,
    };
    assert_eq!(Book::from_parts(parts).unwrap_err().code(), "inconsistent");
}
