//! The frozen event file in `fixtures/events`, written by schema version 1.
//! Once the first real event is entered it is never rewritten: a file an earlier version wrote must
//! keep opening with the same figures, so every schema change has to be a migration. Until then the
//! schema is edited in place and this file is written again (technical design §3.2).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::mock;
use common::*;
use lancelot_core::*;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const SAMPLE: &str = "sample-v1.lancelot";
/// A regenerated sample would hide a schema edited in place, so its bytes are pinned too.
const SAMPLE_SHA256: &str = "dd2ff016e1782967149cf2b14a6b34683dd05cae3721a119d38a8d09320c4d55";

fn sample_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/events")
        .join(name)
}

/// A working copy: opening a file may write to it.
fn copy_of_sample(dir: &TempDir) -> PathBuf {
    let path = dir.path().join(SAMPLE);
    std::fs::copy(sample_path(SAMPLE), &path).unwrap();
    path
}

fn options() -> StoreOptions {
    StoreOptions {
        backups: None,
        clock: Arc::new(now),
    }
}

fn opened(path: &Path) -> Store {
    let (store, report) = Store::open(path, options()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    store
}

fn read_only(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn strings(conn: &Connection, sql: &str) -> Vec<String> {
    let mut statement = conn.prepare(sql).unwrap();
    statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Everything that defines the format of a file, in a stable order.
fn schema(conn: &Connection) -> Vec<String> {
    strings(
        conn,
        "SELECT type || ' ' || name || ': ' || COALESCE(sql, '') FROM sqlite_schema
         ORDER BY type, name",
    )
}

fn tier_id(book: &Book, name: &str) -> TierId {
    book.tiers().find(|t| t.draft.name == name).unwrap().id
}

fn tier_figures(book: &Book, name: &str) -> TicketTotals {
    let tier = tier_id(book, name);
    let tickets = book.tickets();
    tickets
        .tiers
        .iter()
        .find(|t| t.tier.id == tier)
        .unwrap()
        .figures
}

/// Writes a sample of the current schema version, next to the older ones. This made the frozen file:
///
/// `cargo test -p lancelot-core --test sample_file -- --ignored write_the_sample`
#[test]
#[ignore = "writes a new frozen file, and refuses to replace one"]
fn write_the_sample() {
    let path = sample_path(&format!("sample-v{SCHEMA_VERSION}.lancelot"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (mut store, _) = Store::create(
        &path,
        EventDetails {
            currency: Default::default(),
            name: "Summer Festival 2026".into(),
            starts_on: Some(june(12)),
            ends_on: Some(june(14)),
            location: Some("Riverside Park".into()),
            notes: Some("The sample of the event file format.".into()),
        },
        &two_accounts(),
        options(),
    )
    .unwrap();
    let m = mock::build(&mut store);

    store
        .change(|b| {
            b.update_person(
                m.anna,
                "Anna",
                Person {
                    holder: Some("Anna Beispiel".into()),
                    iban: Some(validate_iban("DE89370400440532013000").unwrap()),
                    bic: Some("COBADEFFXXX".into()),
                    other_method: None,
                    contact: Some("anna@example.invalid".into()),
                    notes: Some("Pays for the kitchen".into()),
                },
            )?;
            b.update_person(
                m.malte,
                "Malte",
                Person {
                    other_method: Some("PayPal: malte@example.invalid".into()),
                    ..Person::default()
                },
            )?;
            b.add_cost_centre("Workshop")?;
            let float = b.add_asset("Bar float", AssetType::Cash)?;
            b.set_opening_count(float, june(12), eur("100"))?;
            for (name, price, fee_bp, quota, aliases) in [
                ("Early Bird", "25", 0, Some(100), vec![]),
                ("Regular", "32", 250, None, vec!["Normal".to_owned()]),
                ("Supporter", "45", 0, None, vec![]),
            ] {
                b.add_tier(TierDraft {
                    name: name.into(),
                    price: eur(price),
                    fee_bp,
                    quota,
                    aliases,
                })?;
            }
            Ok::<_, DomainError>(())
        })
        .unwrap();

    // The second sheet loses order 4711 and upgrades order 4702: two conflicts.
    for (file, day) in [("import-1.ods", 1), ("import-2.ods", 8)] {
        let data = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/tickets")
                .join(file),
        )
        .unwrap();
        store
            .import_tickets(&batch(file, &data), june(day))
            .unwrap();
    }
    store
        .change(|b| {
            let regular = tier_id(b, "Regular");
            let terms = |refund_bp| RefundTerms {
                refund_bp,
                date: june(20),
                payer: None,
            };
            b.resolve_conflict(
                "4711",
                regular,
                Resolution::Refund {
                    count: 2,
                    terms: terms(5000),
                    note: Some("Could not come".into()),
                },
            )?;
            b.resolve_conflict(
                "4702",
                regular,
                Resolution::Correction {
                    count: 1,
                    note: Some("Upgraded".into()),
                },
            )?;
            let cash = asset(b, "Cash box");
            let sale = b.add_manual_sale(ManualSaleDraft {
                tier: regular,
                count: 3,
                destination: cash,
                date: june(13),
                reference: Some("At the door".into()),
            })?;
            b.add_cancellation(CancellationDraft {
                source: CancellationSource::Sale { sale },
                count: 1,
                terms: terms(10_000),
                note: None,
            })?;
            let bank = asset(b, "Bank account");
            let movement = b.ledger().balance(bank);
            let date = time::macros::date!(2026 - 07 - 01);
            b.add_reconciliation(bank, date, movement)?;
            Ok::<_, DomainError>(())
        })
        .unwrap();

    let overrides = store
        .book()
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "Accepted for the sample".to_owned(),
        })
        .collect();
    store.close(overrides).unwrap();
}

#[test]
fn the_sample_is_the_frozen_file() {
    let bytes = std::fs::read(sample_path(SAMPLE)).unwrap();
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hash, SAMPLE_SHA256);
}

#[test]
fn the_sample_holds_something_of_every_kind() {
    let conn = read_only(&sample_path(SAMPLE));
    for table in strings(&conn, "SELECT name FROM sqlite_schema WHERE type = 'table'") {
        let rows: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert!(rows > 0, "{table} is empty");
    }
    assert_eq!(
        strings(&conn, "SELECT DISTINCT kind FROM txn ORDER BY kind"),
        [
            "expense", "income", "opening", "refund", "settle", "transfer"
        ]
    );
    assert_eq!(
        strings(&conn, "SELECT DISTINCT origin FROM txn ORDER BY origin"),
        ["manual", "ticket_import", "ticket_refund", "ticket_sale"]
    );
}

/// Fails when `0001_initial.sql` is edited instead of adding a migration.
#[test]
fn a_new_file_has_the_schema_of_the_migrated_sample() {
    let dir = TempDir::new().unwrap();
    let migrated = copy_of_sample(&dir);
    drop(opened(&migrated));
    let new = dir.path().join("new.lancelot");
    drop(
        Store::create(
            &new,
            new_book().event().details.clone(),
            &two_accounts(),
            options(),
        )
        .unwrap(),
    );

    let (migrated, new) = (read_only(&migrated), read_only(&new));
    assert_eq!(schema(&migrated), schema(&new));
    let version: u32 = migrated
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}

/// The figures are written out, so that they stay what the file said when it was frozen.
#[test]
fn the_sample_opens_with_the_figures_it_was_frozen_with() {
    let dir = TempDir::new().unwrap();
    let store = opened(&copy_of_sample(&dir));
    let book = store.book();
    let l = book.ledger();

    assert_eq!(
        book.event().details.location.as_deref(),
        Some("Riverside Park")
    );
    assert_eq!(book.event().closed_at, Some(now()));
    assert_eq!(book.event().closing_overrides.len(), 7);

    assert_eq!(l.result(), eur("2.386,70"));
    assert_eq!(l.income_total(), eur("7.754"));
    assert_eq!(l.expense_total(), eur("5.367,30"));
    assert_eq!(raw_sum(book), 0);
    assert_eq!(l.balance(asset(book, "Bank account")), eur("2.052,20"));
    assert_eq!(l.balance(asset(book, "Cash box")), eur("476,90"));
    assert_eq!(l.balance(asset(book, "Bar float")), eur("100"));
    let w = l.where_the_money_is();
    assert_eq!(
        (w.held_by_people, w.owed_to_people, w.opening, w.total),
        (eur("180"), eur("322,40"), eur("100"), eur("2.386,70"))
    );

    // Sold 9 Regular: 4 on the sheet, 2 cancelled, 3 at the door, 1 of those cancelled.
    let regular = tier_figures(book, "Regular");
    assert_eq!(
        (regular.sheet, regular.cancelled, regular.manual),
        (4, 2, 3)
    );
    assert_eq!(
        (regular.sold, regular.places, regular.unresolved),
        (9, 6, 0)
    );
    assert_eq!(
        (regular.gross, regular.fees, regular.refunds, regular.net),
        (eur("288"), eur("4,80"), eur("64"), eur("219,20"))
    );
    assert_eq!(tier_figures(book, "Early Bird").net, eur("50"));
    assert_eq!(tier_figures(book, "Supporter").net, eur("90"));
    let tier = book.tier(tier_id(book, "Regular")).unwrap();
    assert_eq!(tier.draft.aliases, ["Normal"]);
    let tickets = book.tickets();
    assert_eq!(tickets.imports.len(), 2);
    assert_eq!(tickets.manual_sales.len(), 1);
    assert!(tickets.conflicts.is_empty());
    let refunds: Vec<_> = tickets.cancellations.iter().map(|c| c.refund).collect();
    assert_eq!(refunds, [eur("32"), Cents::ZERO, eur("32")]);

    let bank = asset(book, "Bank account");
    let checkpoints: Vec<_> = book
        .reconciliations()
        .map(|r| (r.account, r.amount))
        .collect();
    assert_eq!(checkpoints, [(bank, eur("1.757")), (bank, eur("2.052,20"))]);
    assert_eq!(
        book.closing_count(asset(book, "Cash box"))
            .map(|c| c.amount),
        Some(eur("410"))
    );

    assert_eq!(book.plugin_records().count(), 11);
    assert!(book.plugin_records().all(|r| r.plugin == "drinks"));
    assert_eq!(book.budget().result.estimate, eur("2.305"));

    let anna = book
        .account_by_name(AccountKind::Person, "Anna")
        .unwrap()
        .id;
    let iban = book.person(anna).unwrap().iban.as_ref().unwrap();
    assert_eq!(iban.as_str(), "DE89370400440532013000");

    assert_eq!(store.change_log(10_000).unwrap().len(), 104);

    assert_eq!(book.attachments().count(), 11);
    let receipts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/receipts");
    for attachment in book.attachments() {
        assert_eq!(
            store.attachment_data(attachment.id).unwrap(),
            std::fs::read(receipts.join(&attachment.filename)).unwrap()
        );
    }

    let json = serde_json::to_value(EventSnapshot::new(book)).unwrap();
    assert_eq!(json["totals"]["result"], 238_670);
    assert_eq!(
        read_book(&sample_path(SAMPLE)).unwrap().ledger().result(),
        l.result()
    );
}

#[test]
fn the_sample_can_be_worked_on() {
    let dir = TempDir::new().unwrap();
    let path = copy_of_sample(&dir);
    let mut store = opened(&path);
    let before = store.book().ledger().result();
    let cash = asset(store.book(), "Cash box");
    let kitchen = expense(store.book(), "Kitchen");
    store.change(|b| b.reopen()).unwrap();
    store
        .change(|b| {
            b.save_txn(
                &draft(
                    "Late receipt",
                    Entry::Expense {
                        payer: cash,
                        lines: vec![split(kitchen, "10")],
                    },
                ),
                None,
            )
        })
        .unwrap();
    let book = store.book().clone();
    drop(store);

    let store = opened(&path);
    assert_eq!(store.book(), &book);
    assert_eq!(
        store.book().ledger().result(),
        before.checked_sub(eur("10")).unwrap()
    );
}
