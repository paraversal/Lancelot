//! Named states to start from, as real event files.

use std::path::Path;

use lancelot_core::{BlockerOverride, EventDetails, Store, StoreError, StoreOptions, TierDraft};
use lancelot_service::Service;
use lancelot_service::api::ApiError;
use rusqlite::Connection;
use time::macros::date;

use crate::common::{eur, mock};

/// The start screen with its recent events; nothing open. The state the bridge starts in.
pub const START: &str = "start";
/// Every scenario, for the error message and the tests.
pub const ALL: &[&str] = &[START, "example", "empty", "conflict", "closed"];

pub const EXAMPLE_FILE: &str = "sommerfest-2026.lancelot";
const DAMAGED_FILE: &str = "winterfest-2025.lancelot";
const LOCKED_FILE: &str = "sommerfest-2025.lancelot";

fn store_error(e: StoreError) -> ApiError {
    ApiError::new("scenario", e.to_string())
}

fn details(name: &str) -> EventDetails {
    EventDetails {
        name: name.to_owned(),
        starts_on: Some(date!(2026 - 06 - 12)),
        ends_on: Some(date!(2026 - 06 - 14)),
        location: None,
        notes: None,
    }
}

fn example(path: &Path, name: &str) -> Result<Store, ApiError> {
    let (mut store, _) =
        Store::create(path, details(name), StoreOptions::default()).map_err(store_error)?;
    mock::build(&mut store);
    Ok(store)
}

fn tier(name: &str, price: &str, fee_bp: u32) -> TierDraft {
    TierDraft {
        name: name.to_owned(),
        price: eur(price),
        fee_bp,
        quota: None,
        aliases: Vec::new(),
    }
}

/// Two imports of the ticket sheet, the second with an order gone: an open conflict.
fn conflict(store: &mut Store) -> Result<(), ApiError> {
    for draft in [
        tier("Early Bird", "25", 0),
        tier("Regular", "32", 250),
        tier("Supporter", "45", 0),
    ] {
        store
            .change(|book| book.add_tier(draft))
            .map_err(store_error)?;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tickets");
    for (file, day) in [("import-1.ods", 1), ("import-2.ods", 8)] {
        let data =
            std::fs::read(fixtures.join(file)).map_err(|e| ApiError::new("io", e.to_string()))?;
        let date = date!(2026 - 06 - 01)
            .replace_day(day)
            .map_err(|e| ApiError::new("scenario", e.to_string()))?;
        store
            .import_tickets(file, &data, date)
            .map_err(store_error)?;
    }
    Ok(())
}

/// The event closed, with every open item accepted.
fn close(store: &mut Store) -> Result<(), ApiError> {
    let overrides = store
        .book()
        .ledger()
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "Accepted for the example".to_owned(),
        })
        .collect();
    store.close(overrides).map_err(store_error)
}

/// A balanced transaction no longer balances: the file does not load.
fn damage(path: &Path) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "UPDATE line SET amount = amount + 1
           WHERE id = (SELECT MIN(id) FROM line WHERE txn_id = (SELECT MIN(id) FROM txn));",
    )
}

/// Fills `dir` and opens what the scenario asks for. Returns the store that keeps a file locked.
pub fn load(service: &mut Service, dir: &Path, scenario: &str) -> Result<Option<Store>, ApiError> {
    if !ALL.contains(&scenario) {
        return Err(ApiError::new(
            "bad_request",
            format!("no such scenario: {scenario}; there are {}", ALL.join(", ")),
        ));
    }

    // Opened once each, so the start screen lists them as recent events.
    let example_path = dir.join(EXAMPLE_FILE);
    let mut main = example(&example_path, "Sommerfest 2026")?;
    match scenario {
        "conflict" => conflict(&mut main)?,
        "closed" => close(&mut main)?,
        _ => {}
    }
    drop(main);
    let damaged_path = dir.join(DAMAGED_FILE);
    drop(example(&damaged_path, "Winterfest 2025")?);
    let locked_path = dir.join(LOCKED_FILE);
    drop(example(&locked_path, "Sommerfest 2025")?);
    for path in [&damaged_path, &locked_path, &example_path] {
        service.open_event(path)?;
    }
    service.close_file();

    damage(&damaged_path).map_err(|e| ApiError::new("scenario", e.to_string()))?;
    let (locked, _) = Store::open(&locked_path, StoreOptions::default()).map_err(store_error)?;

    match scenario {
        START => {}
        "empty" => {
            service.create_event(&dir.join("neues-fest.lancelot"), details("Neues Fest"))?;
        }
        _ => {
            service.open_event(&example_path)?;
        }
    }
    Ok(Some(locked))
}
