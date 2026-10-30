//! Ticket tiers, imports of the ticket sheet and manual sales (requirements §5.6), against the
//! sheets in `fixtures/tickets` (written by `scripts/ticket-fixtures.py`).

mod common;

use std::path::PathBuf;

use common::*;
use lancelot_core::*;
use tempfile::TempDir;

fn fixture(name: &str) -> (String, Vec<u8>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tickets")
        .join(name);
    (name.to_owned(), std::fs::read(path).unwrap())
}

fn sheet(book: &Book, name: &str) -> Sheet {
    let (name, data) = fixture(name);
    read_sheet(&name, &data, book.ticket_columns()).unwrap()
}

fn tier(name: &str, price: &str, fee_bp: u32) -> TierDraft {
    TierDraft {
        name: name.into(),
        price: eur(price),
        fee_bp,
        quota: None,
        aliases: Vec::new(),
    }
}

struct Tiers {
    early: TierId,
    regular: TierId,
    supporter: TierId,
}

fn with_tiers() -> (Book, Tiers) {
    let mut book = new_book();
    let tiers = Tiers {
        early: book.add_tier(tier("Early Bird", "25", 0)).unwrap(),
        regular: book.add_tier(tier("Regular", "32", 250)).unwrap(),
        supporter: book.add_tier(tier("Supporter", "45", 0)).unwrap(),
    };
    (book, tiers)
}

fn import(book: &mut Book, name: &str, day: u8) -> ImportId {
    let sheet = sheet(book, name);
    book.import_tickets(&sheet, june(day), now()).unwrap()
}

/// The generated income of a tier.
fn income_of(book: &Book, tier: TierId) -> Option<&Txn> {
    book.txns()
        .find(|t| t.origin == Origin::TicketImport { tier })
}

fn sold(book: &Book, tier: TierId) -> TicketTotals {
    book.tickets()
        .tiers
        .iter()
        .find(|t| t.tier.id == tier)
        .unwrap()
        .figures
}

#[test]
fn the_three_formats_read_the_same_tickets_and_nothing_else() {
    let (book, _) = with_tiers();
    let ods = sheet(&book, "import-1.ods");
    for other in ["import-1.xlsx", "import-1.csv"] {
        let s = sheet(&book, other);
        assert_eq!(s.rows, ods.rows, "{other}");
        assert_eq!(s.problems, ods.problems, "{other}");
    }
    assert_eq!(ods.rows.len(), 6);
    assert_eq!(ods.rows[0].order, "4701");
    assert_eq!(ods.rows[0].tier, "Early Bird");
    // Names and e-mail addresses are in the sheet but never read (R-TKT-17).
    let debug = format!("{ods:?}");
    assert!(!debug.contains("Gast") && !debug.contains("example.invalid"));
}

#[test]
fn an_import_generates_ticket_income_net_of_fees() {
    let (mut book, t) = with_tiers();
    import(&mut book, "import-1.ods", 1);

    // R-TKT-6: one income per tier to the Verein account; the fee is an expense in it.
    let verein = role(&book, Role::Verein);
    let tickets = role(&book, Role::Tickets);
    let fees = role(&book, Role::TicketFees);
    assert_eq!(book.account(fees).unwrap().name, "Ticket fees");
    let regular = income_of(&book, t.regular).unwrap();
    assert_eq!(regular.kind, TxnKind::Income);
    assert_eq!(regular.date, june(1));
    assert_eq!(regular.description, "3 × Regular");
    let lines: Vec<_> = regular
        .lines
        .iter()
        .map(|l| (l.account, l.amount))
        .collect();
    assert_eq!(
        lines,
        [
            (tickets, -eur("96")),
            (fees, eur("2,40")),
            (verein, eur("93,60"))
        ]
    );
    assert_eq!(income_of(&book, t.early).unwrap().lines.len(), 2);

    let ledger = book.ledger();
    assert_eq!(ledger.balance(verein), eur("188,60"));
    assert_eq!(ledger.balance(tickets), eur("191"));
    assert_eq!(ledger.balance(fees), eur("2,40"));
    assert_eq!(ledger.result(), eur("188,60"));
    assert_eq!(raw_sum(&book), 0);

    let total = book.tickets().total;
    assert_eq!((total.sheet, total.sold), (6, 6));
    assert_eq!(
        (total.gross, total.fees, total.net),
        (eur("191"), eur("2,40"), eur("188,60"))
    );
    assert_eq!(sold(&book, t.regular).sold, 3);
    // Generated income is not a receipt that is missing.
    assert!(ledger.missing_receipts().is_empty());
}

#[test]
fn a_reimport_shows_its_changes_and_replaces_the_income() {
    let (mut book, t) = with_tiers();
    let first = import(&mut book, "import-1.ods", 1);
    let regular_txn = income_of(&book, t.regular).unwrap().id;

    // R-TKT-5: the preview, per tier and per order (R-TKT-12).
    let preview = book.preview_import(&sheet(&book, "import-2.ods"), june(20));
    assert_eq!(preview.previous, Some(first));
    assert!(!preview.same_file);
    assert_eq!(preview.tickets, 8);
    let change = |tier| *preview.tiers.iter().find(|c| c.tier == tier).unwrap();
    let r = change(t.regular);
    assert_eq!((r.before, r.after, r.added, r.removed), (3, 4, 4, 3));
    let s = change(t.supporter);
    assert_eq!((s.before, s.after, s.added, s.removed), (1, 2, 1, 0));
    let e = change(t.early);
    assert_eq!((e.before, e.after, e.added, e.removed), (2, 2, 0, 0));
    let removed: Vec<_> = preview
        .removed
        .iter()
        .map(|o| (o.order.as_str(), o.tier, o.before, o.after))
        .collect();
    assert_eq!(
        removed,
        [("4702", t.regular, 1, 0), ("4711", t.regular, 2, 0)]
    );
    // The row without an order ID is listed; the blank row is not a ticket.
    assert_eq!(
        preview.problems,
        [RowProblem {
            row: 10,
            problem: Problem::NoOrder
        }]
    );

    let second = import(&mut book, "import-2.ods", 8);
    assert_eq!(book.latest_import().unwrap().id, second);
    // Same transaction, new figures and date; removed tickets count as not sold (R-TKT-16).
    let regular = income_of(&book, t.regular).unwrap();
    assert_eq!(regular.id, regular_txn);
    assert_eq!(regular.date, june(8));
    assert_eq!(txn_amount(&regular.lines), eur("128"));
    assert_eq!(sold(&book, t.supporter).gross, eur("90"));
    assert_eq!(
        book.ledger().balance(role(&book, Role::Verein)),
        eur("264,80")
    );

    // Both snapshots stay for the sales over time (R-TKT-8).
    let imports = &book.tickets().imports;
    assert_eq!(
        imports.iter().map(|i| i.tickets).collect::<Vec<_>>(),
        [6, 8]
    );
    assert_eq!(imports[0].import.date, june(1));

    // The same file again is flagged.
    assert!(
        book.preview_import(&sheet(&book, "import-2.ods"), june(20))
            .same_file
    );
}

#[test]
fn unknown_tiers_block_the_import_until_they_are_mapped() {
    let (mut book, t) = with_tiers();
    import(&mut book, "import-2.ods", 8);
    let s = sheet(&book, "import-3.ods");
    let preview = book.preview_import(&s, june(20));
    assert_eq!(preview.unknown.len(), 1);
    assert_eq!(
        (preview.unknown[0].name.as_str(), preview.unknown[0].rows),
        ("Soli-Ticket", 2)
    );

    let before = book.clone();
    let e = book.import_tickets(&s, june(9), now()).unwrap_err();
    assert_eq!(
        e,
        DomainError::UnknownTiers {
            names: vec!["Soli-Ticket".into()]
        }
    );
    assert_eq!(book, before);

    // R-TKT-2: map it as an alias of Supporter.
    let mut supporter = book.tier(t.supporter).unwrap().draft.clone();
    supporter.aliases.push("soli-ticket".into());
    book.update_tier(t.supporter, supporter).unwrap();
    assert!(book.preview_import(&s, june(20)).unknown.is_empty());
    book.import_tickets(&s, june(9), now()).unwrap();
    assert_eq!(sold(&book, t.supporter).sheet, 4);
}

#[test]
fn ticket_income_cannot_be_changed_by_hand() {
    let (mut book, t) = with_tiers();
    import(&mut book, "import-1.ods", 1);
    let id = income_of(&book, t.early).unwrap().id;
    let entry = Entry::Income {
        destination: role(&book, Role::Verein),
        lines: vec![split(role(&book, Role::Tickets), "60")],
    };
    assert_eq!(
        book.save_txn(&draft("x", entry), Some(id)),
        Err(DomainError::GeneratedTxn)
    );
    assert_eq!(book.delete_txn(id), Err(DomainError::GeneratedTxn));
}

#[test]
fn tiers_follow_their_price_and_cannot_be_deleted_once_used() {
    let (mut book, t) = with_tiers();
    import(&mut book, "import-1.ods", 1);
    book.update_tier(t.early, tier("Early Bird", "20", 0))
        .unwrap();
    assert_eq!(
        txn_amount(&income_of(&book, t.early).unwrap().lines),
        eur("40")
    );
    assert_eq!(book.delete_tier(t.early), Err(DomainError::TierInUse));

    let unused = book.add_tier(tier("Crew", "0", 0)).unwrap();
    book.delete_tier(unused).unwrap();
    assert!(book.tier(unused).is_none());

    // A free tier brings no income, but its tickets count.
    let crew = book.add_tier(tier("Crew", "0", 0)).unwrap();
    let mut crew_draft = book.tier(crew).unwrap().draft.clone();
    crew_draft.aliases.push("Early Bird".into());
    assert_eq!(
        book.update_tier(crew, crew_draft).unwrap_err().field(),
        Some("aliases[0]")
    );
}

#[test]
fn manual_sales_are_income_held_by_the_person() {
    let (mut book, _) = with_tiers();
    let reduced = book.add_tier(tier("Reduced", "18", 250)).unwrap();
    let bob = person(&mut book, "Bob");
    let sale = book
        .add_manual_sale(ManualSaleDraft {
            tier: reduced,
            count: 3,
            person: bob,
            date: june(13),
            reference: Some(" Abendkasse Sa ".into()),
        })
        .unwrap();

    // R-TKT-7: Income: Tickets → Bob, without a platform fee.
    let txn = book
        .txns()
        .find(|t| t.origin == Origin::TicketSale { sale })
        .unwrap()
        .clone();
    assert_eq!(txn.description, "3 × Reduced");
    assert_eq!(txn.notes.as_deref(), Some("Abendkasse Sa"));
    assert_eq!(txn.lines.last().unwrap().account, bob);
    assert_eq!(book.ledger().person(bob).unwrap().net, -eur("54"));
    let figures = sold(&book, reduced);
    assert_eq!(
        (figures.manual, figures.sold, figures.fees),
        (3, 3, Cents::ZERO)
    );
    assert_eq!(book.tickets().manual_sales[0].txn, Some(txn.id));
    assert_eq!(book.delete_tier(reduced), Err(DomainError::TierInUse));

    let invalid = ManualSaleDraft {
        tier: reduced,
        count: 0,
        person: bob,
        date: june(13),
        reference: None,
    };
    assert_eq!(
        book.update_manual_sale(sale, invalid.clone())
            .unwrap_err()
            .field(),
        Some("count")
    );
    let not_a_person = ManualSaleDraft {
        count: 1,
        person: role(&book, Role::Verein),
        ..invalid
    };
    assert_eq!(
        book.add_manual_sale(not_a_person).unwrap_err().field(),
        Some("person")
    );

    book.delete_manual_sale(sale).unwrap();
    assert!(book.txns().all(|t| t.origin == Origin::Manual));
    assert_eq!(book.ledger().person(bob).unwrap().net, Cents::ZERO);
}

#[test]
fn the_columns_can_be_renamed() {
    let (mut book, _) = with_tiers();
    book.set_ticket_columns(TicketColumns {
        tier: "Kategorie".into(),
        order: "Order ID".into(),
        sale_date: None,
    })
    .unwrap();
    let (name, data) = fixture("import-1.ods");
    assert_eq!(
        read_sheet(&name, &data, book.ticket_columns()),
        Err(DomainError::MissingColumn {
            column: "Kategorie".into()
        })
    );
    let same = TicketColumns {
        tier: "Ticket".into(),
        order: " ticket ".into(),
        sale_date: None,
    };
    assert_eq!(
        book.set_ticket_columns(same).unwrap_err().field(),
        Some("order")
    );
}

#[test]
fn a_closed_event_takes_no_imports() {
    let (mut book, _) = with_tiers();
    let accepted = book
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "not counted".into(),
        })
        .collect();
    book.close(accepted, now()).unwrap();
    let s = sheet(&book, "import-1.ods");
    assert_eq!(
        book.import_tickets(&s, june(1), now()),
        Err(DomainError::EventClosed)
    );
}

#[test]
fn tickets_survive_saving_and_reopening() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tickets.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let regular = store
        .change(|book| book.add_tier(tier("Regular", "32", 250)))
        .unwrap();
    let mut early = tier("Early Bird", "25", 0);
    early.quota = Some(60);
    early.aliases = vec!["Frühbucher".into()];
    store.change(|book| book.add_tier(early)).unwrap();
    store
        .change(|book| book.add_tier(tier("Supporter", "45", 0)))
        .unwrap();
    let (name, data) = fixture("import-1.ods");
    let first = store.import_tickets(&name, &data, june(1)).unwrap();
    let (name, data) = fixture("import-2.ods");
    let preview = store.preview_ticket_import(&name, &data, june(1)).unwrap();
    assert_eq!(preview.removed.len(), 2);
    store.import_tickets(&name, &data, june(8)).unwrap();
    store
        .change(|book| book.set_import_date(first, june(2)))
        .unwrap();
    let anna = store
        .change(|book| book.add_person("Anna", Person::default()))
        .unwrap();
    store
        .change(|book| {
            book.add_manual_sale(ManualSaleDraft {
                tier: regular,
                count: 2,
                person: anna,
                date: june(12),
                reference: None,
            })
        })
        .unwrap();
    let book = store.book().clone();
    let log = store.change_log(1000).unwrap();
    drop(store);

    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    assert_eq!(
        reopened.book().ticket_imports().next().unwrap().date,
        june(2)
    );

    // The change log records the imports, but no order IDs (R-TKT-18).
    let imports: Vec<_> = log.iter().filter(|e| e.entity == "ticket_import").collect();
    assert_eq!(imports.len(), 3);
    let text = format!("{log:?}");
    assert!(!text.contains("4711"));
}

fn imported_file(dir: &TempDir) -> (std::path::PathBuf, Store) {
    let path = dir.path().join("tickets.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    store
        .change(|book| book.add_tier(tier("Early Bird", "25", 0)))
        .unwrap();
    store
        .change(|book| book.add_tier(tier("Regular", "32", 250)))
        .unwrap();
    store
        .change(|book| book.add_tier(tier("Supporter", "45", 0)))
        .unwrap();
    let (name, data) = fixture("import-1.ods");
    store.import_tickets(&name, &data, june(1)).unwrap();
    (path, store)
}

#[test]
fn other_wording_in_a_generated_entry_is_quietly_brought_in_line_on_open() {
    let dir = TempDir::new().unwrap();
    let (path, store) = imported_file(&dir);
    let expected = store.book().clone();
    drop(store);

    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE txn SET description = replace(description, '×', 'x') WHERE origin = 'ticket_import'",
        [],
    )
    .unwrap();
    drop(conn);

    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert_eq!(store.book(), &expected);
    assert!(report.is_clean(), "{report:?}");
    // It is written, with its change log entries, so the next open finds nothing to do.
    let log = store.change_log(10).unwrap();
    assert!(log.iter().any(|e| e.entity == "txn" && e.op == "update"));
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean());
    assert_eq!(store.book(), &expected);
}

#[test]
fn ticket_income_that_differs_in_money_is_recalculated_with_a_warning() {
    let dir = TempDir::new().unwrap();
    let (path, store) = imported_file(&dir);
    let before = store.book().tickets().total.gross;
    drop(store);

    // What a change to the generator's rounding, or a price set outside the app, comes down to.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE ticket_tier SET price = 3000 WHERE name = 'Regular'",
        [],
    )
    .unwrap();
    drop(conn);

    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert_eq!(report.codes(), ["ticket_income_regenerated"]);
    assert!(!report.has_errors());
    let total = store.book().tickets().total;
    assert_ne!(total.gross, before);
    assert_eq!(
        store
            .book()
            .ledger()
            .balance(role(store.book(), Role::Tickets)),
        total.gross
    );
    drop(store);
    let (_, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "written, so the warning is said once");
}

#[test]
fn reading_another_event_regenerates_in_memory_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let (path, store) = imported_file(&dir);
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE txn SET description = 'other words' WHERE origin = 'ticket_import'",
        [],
    )
    .unwrap();
    drop(conn);

    let book = read_book(&path).unwrap();
    assert!(book.txns().all(|t| t.description != "other words"));
    let conn = rusqlite::Connection::open(&path).unwrap();
    let left: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM txn WHERE description = 'other words'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(left > 0);
}

#[test]
fn a_wrongly_chosen_sheet_can_be_deleted_and_leaves_nothing_behind() {
    let dir = TempDir::new().unwrap();
    let (path, mut store) = imported_file(&dir);
    let clean = store.book().tickets().clone();

    // The right sheet has fewer orders, which reads as tickets gone from the wrong one.
    let (name, data) = fixture("import-2.ods");
    store.import_tickets(&name, &data, june(8)).unwrap();
    assert!(store.book().tickets().conflict_count() > 0);
    let wrong = store.book().ticket_imports().last().unwrap().id;

    let preview = store.book().preview_delete_import(wrong).unwrap();
    assert_eq!(preview.conflicts_after, 0);
    assert_eq!(preview.sold_after, clean.total.sold);
    assert_eq!(preview.net_after, clean.total.net);
    assert!(preview.receipts_lost.is_empty());
    assert!(
        store.book().tickets().conflict_count() > 0,
        "only a preview"
    );

    store.change(|book| book.delete_import(wrong)).unwrap();
    assert_eq!(store.book().tickets().conflict_count(), 0);
    assert_eq!(store.book().tickets().total, clean.total);
    let log = store.change_log(50).unwrap();
    assert!(
        log.iter()
            .any(|e| e.entity == "ticket_import" && e.op == "delete")
    );

    let expected = store.book().clone();
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &expected);
    let conn = rusqlite::Connection::open(&path).unwrap();
    drop(store);
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ticket_import_row WHERE import_id = ?1",
            [wrong.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn an_import_a_cancellation_still_needs_cannot_be_deleted() {
    let (mut book, t) = with_tiers();
    let only = import(&mut book, "import-1.ods", 1);
    book.add_cancellation(CancellationDraft {
        source: CancellationSource::Order {
            order: "4711".into(),
            tier: t.regular,
        },
        count: 1,
        note: None,
        terms: RefundTerms {
            refund_bp: 5000,
            date: june(2),
            payer: None,
        },
    })
    .unwrap();
    assert_eq!(
        book.delete_import(only).unwrap_err().code(),
        "import_in_use"
    );
    assert_eq!(book.ticket_imports().count(), 1, "nothing changed");
    assert_eq!(
        book.delete_import(ImportId(99)).unwrap_err().code(),
        "unknown_import"
    );
}

#[test]
fn the_import_preview_says_which_receipts_would_go() {
    let (mut book, _) = with_tiers();
    let first = import(&mut book, "import-1.ods", 1);
    let income = book
        .txns()
        .find(|t| matches!(t.origin, Origin::TicketImport { .. }))
        .unwrap()
        .id;
    let id = attach(&mut book, income);
    let filename = book.attachment(id).unwrap().filename.clone();

    let preview = book.preview_delete_import(first).unwrap();
    assert_eq!(preview.receipts_lost, std::slice::from_ref(&filename));
    let empty = Sheet {
        filename: "empty.csv".into(),
        sha256: "00".into(),
        rows: Vec::new(),
        problems: Vec::new(),
    };
    let conflicted = book.preview_import(&empty, june(9));
    assert_eq!(conflicted.receipts_lost, [filename], "{conflicted:?}");
    assert!(
        book.preview_import(&sheet(&book, "import-1.ods"), june(9))
            .receipts_lost
            .is_empty()
    );
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    /// Rows of a made-up sheet: (order number, tier index).
    fn rows() -> impl Strategy<Value = Vec<(u8, usize)>> {
        prop::collection::vec((0u8..20, 0usize..3), 0..40)
    }

    fn to_sheet(rows: &[(u8, usize)], n: u32) -> Sheet {
        let names = ["Early Bird", "Regular", "Supporter"];
        Sheet {
            filename: format!("import-{n}.ods"),
            sha256: format!("{n:064x}"),
            rows: rows
                .iter()
                .zip(2..)
                .map(|((order, tier), row)| SheetRow {
                    row,
                    tier: names[*tier].into(),
                    order: order.to_string(),
                })
                .collect(),
            problems: Vec::new(),
        }
    }

    proptest! {
        /// After any series of imports with any prices and fees, the ticket income is exactly
        /// the figures: Verein gets the net, Tickets the gross, fees the rest, and every
        /// balance still sums to zero (R-TKT-6, R-TKT-10, R-COR-2).
        #[test]
        fn ticket_income_matches_the_figures(
            imports in prop::collection::vec(rows(), 1..4),
            prices in prop::collection::vec((0i64..10_000, 0u32..2_000), 3),
        ) {
            let (mut book, t) = with_tiers();
            for (id, (price, fee_bp)) in [t.early, t.regular, t.supporter].into_iter().zip(prices) {
                let mut draft = book.tier(id).unwrap().draft.clone();
                draft.price = Cents::new(price).unwrap();
                draft.fee_bp = fee_bp;
                // Some fees round up to the whole price; those tiers are rejected.
                let _ = book.update_tier(id, draft);
            }
            for (n, rows) in imports.iter().enumerate() {
                let n = u32::try_from(n).unwrap();
                book.import_tickets(&to_sheet(rows, n), june(1), now()).unwrap();
            }

            let total = book.tickets().total;
            let ledger = book.ledger();
            prop_assert_eq!(raw_sum(&book), 0);
            prop_assert_eq!(ledger.balance(role(&book, Role::Verein)), total.net);
            prop_assert_eq!(ledger.balance(role(&book, Role::Tickets)), total.gross);
            let fees = book.account_by_role(Role::TicketFees).map_or(Cents::ZERO, |a| ledger.balance(a.id));
            prop_assert_eq!(fees, total.fees);
            prop_assert_eq!(total.sheet as usize, imports.last().unwrap().len());
            // What a file would hold loads back as the same book.
            let parts = BookParts {
                event: book.event().clone(),
                ticket_columns: book.ticket_columns().clone(),
                drink_settings: book.drink_settings().clone(),
                accounts: book.accounts().cloned().collect(),
                persons: book.persons().map(|(a, p)| (a.id, p.clone())).collect(),
                txns: book.txns().cloned().collect(),
                attachments: Vec::new(),
                closing_counts: Vec::new(),
                reconciliations: Vec::new(),
                budget_lines: Vec::new(),
                tiers: book.tiers().cloned().collect(),
                imports: book.ticket_imports().cloned().collect(),
                manual_sales: Vec::new(),
                cancellations: Vec::new(),
                cost_centres: book.cost_centres().cloned().collect(),
                drink_products: Vec::new(),
                next: book.next_ids(),
            };
            prop_assert_eq!(Book::from_parts(parts), Ok(book));
        }
    }
}

// --- conflicts and cancellations (R-TKT-12 to R-TKT-16) ---------------------------------------

/// A sheet as CSV: each `(order, tier)` is one ticket.
fn csv_sheet(book: &Book, name: &str, rows: &[(&str, &str)]) -> Sheet {
    let mut text = String::from("Order ID,Ticket\n");
    for (order, tier) in rows {
        text.push_str(&format!("{order},{tier}\n"));
    }
    read_sheet(name, text.as_bytes(), book.ticket_columns()).unwrap()
}

fn import_rows(book: &mut Book, day: u8, rows: &[(&str, &str)]) {
    let name = format!("tickets-{day}.csv");
    let sheet = csv_sheet(book, &name, rows);
    book.import_tickets(&sheet, june(day), now()).unwrap();
}

fn terms(refund_bp: u32) -> RefundTerms {
    RefundTerms {
        refund_bp,
        date: june(20),
        payer: None,
    }
}

fn refund(count: u32, refund_bp: u32) -> Resolution {
    Resolution::Refund {
        count,
        terms: terms(refund_bp),
        note: None,
    }
}

fn correction(count: u32) -> Resolution {
    Resolution::Correction { count, note: None }
}

fn conflict_lines(book: &Book) -> Vec<(String, TierId, u32)> {
    book.tickets()
        .conflicts
        .iter()
        .flat_map(|c| {
            c.removed
                .iter()
                .map(move |r| (c.order.clone(), r.tier, r.count))
        })
        .collect()
}

const TWO_REGULAR: &[(&str, &str)] = &[
    ("4711", "Regular"),
    ("4711", "Regular"),
    ("4712", "Regular"),
];
const ONE_LEFT: &[(&str, &str)] = &[("4712", "Regular")];

#[test]
fn a_removal_is_a_conflict_and_not_sold_until_resolved() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    assert!(book.tickets().conflicts.is_empty());

    import_rows(&mut book, 2, ONE_LEFT);
    assert_eq!(conflict_lines(&book), [("4711".into(), t.regular, 2)]);
    let removed = &book.tickets().conflicts[0].removed[0];
    assert_eq!(removed.since, book.latest_import().unwrap().id);

    // R-TKT-16: not sold for now; 2,5 % of 32 € is 0,80 €.
    let figures = sold(&book, t.regular);
    assert_eq!((figures.sheet, figures.sold, figures.unresolved), (1, 1, 2));
    assert_eq!(figures.net, eur("31,20"));
    assert_eq!(
        book.open_items().last(),
        Some(&OpenItem::TicketConflicts { count: 1 })
    );
}

#[test]
fn a_removal_is_found_while_the_tier_grows_and_a_return_clears_it() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(
        &mut book,
        2,
        &[
            ("4712", "Regular"),
            ("4800", "Regular"),
            ("4801", "Regular"),
        ],
    );
    assert_eq!(conflict_lines(&book), [("4711".into(), t.regular, 2)]);

    // One of them comes back: only the other is still missing.
    import_rows(
        &mut book,
        3,
        &[
            ("4711", "Regular"),
            ("4712", "Regular"),
            ("4800", "Regular"),
            ("4801", "Regular"),
        ],
    );
    assert_eq!(conflict_lines(&book), [("4711".into(), t.regular, 1)]);
    import_rows(
        &mut book,
        4,
        &[
            ("4711", "Regular"),
            ("4711", "Regular"),
            ("4712", "Regular"),
            ("4800", "Regular"),
            ("4801", "Regular"),
        ],
    );
    assert!(conflict_lines(&book).is_empty());
}

#[test]
fn a_refund_keeps_the_ticket_sold_and_books_negative_income() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);
    book.resolve_conflict("4711", t.regular, refund(2, 5000))
        .unwrap();
    assert!(conflict_lines(&book).is_empty());

    // R-TKT-15: sold = sheet + cancelled; fees stay on the sold count.
    let f = sold(&book, t.regular);
    assert_eq!((f.sheet, f.cancelled, f.sold, f.unresolved), (1, 2, 3, 0));
    assert_eq!(
        (f.gross, f.fees, f.refunds, f.net),
        (eur("96"), eur("2,40"), eur("32"), eur("61,60"))
    );

    let verein = role(&book, Role::Verein);
    let refunds = role(&book, Role::TicketRefunds);
    assert_eq!(book.account(refunds).unwrap().name, "Ticket refunds");
    assert_eq!(book.account(refunds).unwrap().kind, AccountKind::Income);
    // Negative income, shown as such.
    assert_eq!(book.ledger().balance(refunds), -eur("32"));
    assert_eq!(book.ledger().balance(verein), eur("61,60"));
    assert_eq!(book.ledger().result(), eur("61,60"));
    assert_eq!(raw_sum(&book), 0);

    let view = &book.tickets().cancellations[0];
    assert_eq!((view.per_ticket, view.refund), (eur("16"), eur("32")));
    assert_eq!(view.payer, Some(verein));
    let txn = book.txn(view.txn.unwrap()).unwrap();
    assert_eq!(txn.date, june(20));
    assert_eq!(txn.description, "Refund 2 × Regular");
    assert_eq!(txn.kind, TxnKind::Refund);
    // Generated: not to be changed by hand, and not a missing receipt.
    assert_eq!(book.delete_txn(txn.id), Err(DomainError::GeneratedTxn));
    assert!(book.ledger().missing_receipts().is_empty());
}

#[test]
fn a_refund_is_its_own_kind_and_shows_who_pays_it() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);
    book.resolve_conflict("4711", t.regular, refund(2, 5000))
        .unwrap();
    let snapshot = EventSnapshot::new(&book);
    let view = snapshot
        .txns
        .iter()
        .find(|v| matches!(v.txn.origin, Origin::TicketRefund { .. }))
        .unwrap();
    assert_eq!(view.txn.kind, TxnKind::Refund);
    assert_eq!(view.flow, Flow::Out);
    assert!(!view.needs_receipt);
    assert_eq!(view.who, Some(role(&book, Role::Verein)));
    // Ticket income stays income, and shows with its fee among the splits.
    let income = snapshot
        .txns
        .iter()
        .find(|v| matches!(v.txn.origin, Origin::TicketImport { .. }))
        .unwrap();
    assert_eq!(income.txn.kind, TxnKind::Income);
    let Some(Entry::Income { destination, lines }) = &income.entry else {
        panic!("{:?}", income.entry);
    };
    assert_eq!(*destination, role(&book, Role::Verein));
    assert_eq!(lines[0].amount, income.amount);
    assert_eq!(lines.len(), 2);
}

#[test]
fn a_share_rounds_on_each_ticket_like_the_tier() {
    let share = |price, bp, count| {
        let s = ticket_share(Cents::new(price).unwrap(), bp, count).unwrap();
        (s.part.get(), s.rest.get())
    };
    assert_eq!(share(25, 5_000, 1), (13, 12));
    assert_eq!(share(3_200, 250, 1), (80, 3_120));
    assert_eq!(share(1_850, 250, 1), (46, 1_804)); // 46,25 ct
    assert_eq!(share(1_000, 25, 1), (3, 997)); // 2,5 ct
    assert_eq!(share(1_850, 250, 3), (138, 5_412));
    assert_eq!(share(3_200, 0, 4), (0, 12_800));
}

#[test]
fn the_per_ticket_refund_is_rounded_then_multiplied() {
    let (mut book, _) = with_tiers();
    let cheap = book.add_tier(tier("Cheap", "0,25", 0)).unwrap();
    import_rows(
        &mut book,
        1,
        &[("1", "Cheap"), ("1", "Cheap"), ("1", "Cheap")],
    );
    import_rows(&mut book, 2, &[("2", "Regular")]);
    // 50 % of 0,25 € is 0,125 €, so 0,13 € per ticket: 0,39 €, not 0,38 €.
    book.resolve_conflict("1", cheap, refund(3, 5000)).unwrap();
    assert_eq!(sold(&book, cheap).refunds, eur("0,39"));
}

#[test]
fn a_correction_leaves_the_ticket_out_and_a_refund_of_nothing_books_nothing() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);

    // Wholly or split into parts (R-TKT-13).
    book.resolve_conflict("4711", t.regular, correction(1))
        .unwrap();
    assert_eq!(conflict_lines(&book), [("4711".into(), t.regular, 1)]);
    let f = sold(&book, t.regular);
    assert_eq!((f.sold, f.unresolved, f.refunds), (1, 1, Cents::ZERO));

    // 0 %: cancelled, and the event keeps the whole price.
    book.resolve_conflict("4711", t.regular, refund(1, 0))
        .unwrap();
    let f = sold(&book, t.regular);
    assert_eq!((f.cancelled, f.sold, f.unresolved), (1, 2, 0));
    assert_eq!(f.gross, eur("64"));
    assert_eq!(f.refunds, Cents::ZERO);
    assert_eq!(book.tickets().cancellations[1].txn, None);
    assert!(book.account_by_role(Role::TicketRefunds).is_none());

    assert_eq!(
        book.resolve_conflict("4711", t.regular, correction(1)),
        Err(DomainError::TooManyTickets)
    );
}

#[test]
fn deleting_a_resolution_brings_the_conflict_back() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);
    let before = book.ledger().result();
    let id = book
        .resolve_conflict("4711", t.regular, refund(2, 5000))
        .unwrap();
    assert_ne!(book.ledger().result(), before);

    book.delete_cancellation(id).unwrap();
    assert_eq!(conflict_lines(&book), [("4711".into(), t.regular, 2)]);
    assert_eq!(book.ledger().result(), before);
}

#[test]
fn a_manual_cancellation_counts_once_and_is_matched_when_the_order_leaves() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    let draft = |count, order: &str| CancellationDraft {
        source: CancellationSource::Order {
            order: order.into(),
            tier: t.regular,
        },
        count,
        terms: terms(5000),
        note: Some(" after the deadline ".into()),
    };
    // Only orders the sheet had, and no more tickets than they had.
    assert_eq!(
        book.add_cancellation(draft(1, "9999")),
        Err(DomainError::UnknownOrder)
    );
    assert_eq!(
        book.add_cancellation(draft(3, "4711")),
        Err(DomainError::TooManyTickets)
    );
    let id = book.add_cancellation(draft(1, " 4711 ")).unwrap();

    // The order stays in the sheet: still sold, with a refund, and no conflict.
    let f = sold(&book, t.regular);
    assert_eq!((f.sheet, f.cancelled, f.sold), (3, 0, 3));
    assert_eq!(f.refunds, eur("16"));
    assert!(book.tickets().conflicts.is_empty());
    assert_eq!(
        book.tickets().cancellations[0].cancellation.note.as_deref(),
        Some("after the deadline")
    );

    // It leaves: the conflict offers the cancellation and only needs confirming (R-TKT-14).
    import_rows(&mut book, 2, ONE_LEFT);
    let removed = &book.tickets().conflicts[0].removed[0];
    assert_eq!(removed.count, 2);
    assert_eq!(
        removed.matches,
        [ConflictMatch {
            cancellation: id,
            count: 1,
            refund_bp: 5000
        }]
    );
    let confirm = Resolution::Match {
        cancellation: id,
        count: 1,
    };
    book.resolve_conflict("4711", t.regular, confirm.clone())
        .unwrap();
    assert_eq!(conflict_lines(&book), [("4711".into(), t.regular, 1)]);
    // Not counted twice.
    let f = sold(&book, t.regular);
    assert_eq!((f.sheet, f.cancelled, f.sold, f.unresolved), (1, 1, 2, 1));
    assert_eq!(f.refunds, eur("16"));
    assert_eq!(
        book.resolve_conflict("4711", t.regular, confirm),
        Err(DomainError::TooManyTickets)
    );
}

#[test]
fn a_cancelled_ticket_that_returns_counts_as_in_the_sheet_again() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);
    book.resolve_conflict("4711", t.regular, refund(2, 10000))
        .unwrap();
    assert_eq!(sold(&book, t.regular).sold, 3);

    import_rows(&mut book, 3, TWO_REGULAR);
    let f = sold(&book, t.regular);
    assert_eq!((f.sheet, f.cancelled, f.sold, f.unresolved), (3, 0, 3, 0));
    assert_eq!(f.refunds, eur("64"));
    assert_eq!(book.tickets().cancellations[0].gone_now, 0);
    assert!(book.tickets().conflicts.is_empty());
}

#[test]
fn a_changed_tier_shows_the_addition_with_the_removal() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, &[("4711", "Regular"), ("4712", "Supporter")]);
    import_rows(
        &mut book,
        2,
        &[("4711", "Supporter"), ("4712", "Supporter")],
    );
    let c = &book.tickets().conflicts[0];
    assert_eq!(c.order, "4711");
    assert_eq!(c.removed[0].tier, t.regular);
    assert_eq!((c.added[0].tier, c.added[0].count), (t.supporter, 1));

    book.resolve_conflict("4711", t.regular, correction(1))
        .unwrap();
    assert_eq!(sold(&book, t.regular).sold, 0);
    assert_eq!(sold(&book, t.supporter).sold, 2);
}

#[test]
fn a_refund_on_a_manual_sale_comes_out_of_what_the_person_holds() {
    let (mut book, t) = with_tiers();
    let anna = book.add_person("Anna", Person::default()).unwrap();
    let sale = book
        .add_manual_sale(ManualSaleDraft {
            tier: t.regular,
            count: 2,
            person: anna,
            date: june(1),
            reference: None,
        })
        .unwrap();
    let draft = |count, payer| CancellationDraft {
        source: CancellationSource::Sale { sale },
        count,
        terms: RefundTerms {
            refund_bp: 10000,
            date: june(2),
            payer,
        },
        note: None,
    };
    assert_eq!(
        book.add_cancellation(draft(3, None)),
        Err(DomainError::TooManyTickets)
    );
    let id = book.add_cancellation(draft(1, None)).unwrap();

    let f = sold(&book, t.regular);
    assert_eq!((f.manual, f.sold), (2, 2));
    assert_eq!(
        (f.gross, f.refunds, f.net),
        (eur("64"), eur("32"), eur("32"))
    );
    let p = book.ledger().person(anna).unwrap();
    assert_eq!(
        (p.held, p.advanced, p.net),
        (eur("32"), Cents::ZERO, -eur("32"))
    );
    assert_eq!(raw_sum(&book), 0);

    // The sale cannot go or shrink below its cancellations.
    assert_eq!(
        book.delete_manual_sale(sale),
        Err(DomainError::SaleHasCancellations)
    );

    // Overridden: the cash box pays, and Anna still holds the whole sale.
    let cash = role(&book, Role::CashBox);
    book.update_cancellation(
        id,
        CancellationEdit {
            count: 1,
            terms: RefundTerms {
                refund_bp: 10000,
                date: june(2),
                payer: Some(cash),
            },
            note: None,
        },
    )
    .unwrap();
    assert_eq!(book.ledger().person(anna).unwrap().held, eur("64"));
    assert_eq!(book.ledger().balance(cash), -eur("32"));
    book.delete_cancellation(id).unwrap();
    book.delete_manual_sale(sale).unwrap();
}

#[test]
fn a_person_paying_more_than_they_hold_has_advanced_the_rest() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);
    let anna = book.add_person("Anna", Person::default()).unwrap();
    book.resolve_conflict(
        "4711",
        t.regular,
        Resolution::Refund {
            count: 1,
            terms: RefundTerms {
                refund_bp: 10000,
                date: june(3),
                payer: Some(anna),
            },
            note: None,
        },
    )
    .unwrap();
    let p = book.ledger().person(anna).unwrap();
    assert_eq!(
        (p.held, p.advanced, p.net),
        (Cents::ZERO, eur("32"), eur("32"))
    );
}

#[test]
fn conflicts_keep_the_event_from_closing_until_accepted() {
    let (mut book, t) = with_tiers();
    import_rows(&mut book, 1, TWO_REGULAR);
    import_rows(&mut book, 2, ONE_LEFT);
    let cash = role(&book, Role::CashBox);
    book.set_closing_count(cash, june(20), Cents::ZERO).unwrap();
    let item = OpenItem::TicketConflicts { count: 1 };
    assert!(matches!(
        book.close(Vec::new(), now()),
        Err(DomainError::ClosingBlocked { blockers }) if blockers == [item.clone()]
    ));
    book.close(
        vec![BlockerOverride {
            item,
            note: "the Verein is checking".into(),
        }],
        now(),
    )
    .unwrap();
    assert_eq!(
        book.resolve_conflict("4711", t.regular, correction(2)),
        Err(DomainError::EventClosed)
    );
}

#[test]
fn cancellations_survive_saving_and_stay_out_of_the_change_log() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tickets.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let regular = store
        .change(|book| book.add_tier(tier("Regular", "32", 250)))
        .unwrap();
    let rows = |rows: &[(&str, &str)]| {
        let mut text = String::from("Order ID,Ticket\n");
        for (order, tier) in rows {
            text.push_str(&format!("{order},{tier}\n"));
        }
        text.into_bytes()
    };
    store
        .import_tickets("a.csv", &rows(TWO_REGULAR), june(1))
        .unwrap();
    store
        .import_tickets("b.csv", &rows(ONE_LEFT), june(2))
        .unwrap();
    let id = store
        .change(|book| {
            book.resolve_conflict(
                "4711",
                regular,
                Resolution::Refund {
                    count: 1,
                    terms: terms(5000),
                    note: Some("called in".into()),
                },
            )
        })
        .unwrap();
    store
        .change(|book| book.resolve_conflict("4711", regular, correction(1)))
        .unwrap();
    let cash = store.book().account_by_role(Role::CashBox).unwrap().id;
    store
        .change(|book| {
            book.update_cancellation(
                id,
                CancellationEdit {
                    count: 1,
                    terms: RefundTerms {
                        refund_bp: 2500,
                        date: june(4),
                        payer: Some(cash),
                    },
                    note: None,
                },
            )
        })
        .unwrap();
    let book = store.book().clone();
    let log = store.change_log(1000).unwrap();
    drop(store);

    let (reopened, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(reopened.book(), &book);
    assert_eq!(reopened.book().cancellations().count(), 2);

    // The change log records them, but no order IDs (R-TKT-18).
    assert!(log.iter().any(|e| e.entity == "ticket_cancellation"));
    assert!(!format!("{log:?}").contains("4711"));
}

#[test]
fn a_file_with_a_cancellation_of_tickets_it_never_had_does_not_open() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tickets.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();
    let regular = store
        .change(|book| book.add_tier(tier("Regular", "32", 0)))
        .unwrap();
    let data = b"Order ID,Ticket\n4711,Regular\n";
    store.import_tickets("a.csv", data, june(1)).unwrap();
    store
        .change(|book| {
            book.add_cancellation(CancellationDraft {
                source: CancellationSource::Order {
                    order: "4711".into(),
                    tier: regular,
                },
                count: 1,
                terms: terms(0),
                note: None,
            })
        })
        .unwrap();
    drop(store);

    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("UPDATE ticket_cancellation SET order_id = '1234'", [])
        .unwrap();
    drop(conn);
    assert!(Store::open(&path, StoreOptions::default()).is_err());
}

mod conflict_properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Whatever the sheets and however conflicts are resolved: the books balance, the figures
        /// match the ledger, nothing is resolved twice and a file loads back as the same book.
        #[test]
        fn money_follows_the_tickets(
            sheets in prop::collection::vec(prop::collection::vec((0..4u8, 0..2u8), 0..8), 1..5),
            choices in prop::collection::vec((0..3u8, 0..=10000u32), 0..12),
        ) {
            let (mut book, _) = with_tiers();
            let mut choices = choices.into_iter();
            for (i, rows) in sheets.iter().enumerate() {
                let rows: Vec<(String, &str)> = rows
                    .iter()
                    .map(|(order, tier)| {
                        (format!("O{order}"), if *tier == 0 { "Regular" } else { "Supporter" })
                    })
                    .collect();
                let rows: Vec<(&str, &str)> = rows.iter().map(|(o, t)| (o.as_str(), *t)).collect();
                import_rows(&mut book, i as u8 + 1, &rows);

                let open: Vec<_> = conflict_lines(&book);
                for (order, tier, count) in open {
                    let Some((kind, bp)) = choices.next() else { break };
                    let resolution = match kind {
                        0 => refund(count, bp),
                        1 => correction(count),
                        _ => refund(1, bp),
                    };
                    book.resolve_conflict(&order, tier, resolution).unwrap();
                }
            }

            let total = book.tickets().total;
            let ledger = book.ledger();
            let of = |role: Role| book
                .account_by_role(role)
                .map_or(Cents::ZERO, |a| ledger.balance(a.id));
            prop_assert_eq!(raw_sum(&book), 0);
            prop_assert_eq!(of(Role::Verein), total.net);
            prop_assert_eq!(of(Role::Tickets), total.gross);
            prop_assert_eq!(of(Role::TicketFees), total.fees);
            prop_assert_eq!(of(Role::TicketRefunds), -total.refunds);
            prop_assert_eq!(total.sold, total.sheet + total.cancelled);
            let open: u32 = conflict_lines(&book).iter().map(|(_, _, n)| n).sum();
            prop_assert_eq!(open, total.unresolved);

            let parts = BookParts {
                event: book.event().clone(),
                ticket_columns: book.ticket_columns().clone(),
                drink_settings: book.drink_settings().clone(),
                accounts: book.accounts().cloned().collect(),
                persons: book.persons().map(|(a, p)| (a.id, p.clone())).collect(),
                txns: book.txns().cloned().collect(),
                attachments: Vec::new(),
                closing_counts: Vec::new(),
                reconciliations: Vec::new(),
                budget_lines: Vec::new(),
                tiers: book.tiers().cloned().collect(),
                imports: book.ticket_imports().cloned().collect(),
                manual_sales: Vec::new(),
                cancellations: book.cancellations().cloned().collect(),
                cost_centres: book.cost_centres().cloned().collect(),
                drink_products: Vec::new(),
                next: book.next_ids(),
            };
            prop_assert_eq!(Book::from_parts(parts), Ok(book));
        }
    }
}
