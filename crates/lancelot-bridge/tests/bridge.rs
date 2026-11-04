//! The bridge against the real core: its scenarios, and that it offers what the app offers.

use lancelot_bridge::{Bridge, scenarios};
use serde_json::{Value, json};
use tempfile::TempDir;

fn bridge() -> (TempDir, Bridge) {
    let dir = TempDir::new().unwrap();
    let bridge = Bridge::new(&dir.path().join("bridge")).unwrap();
    (dir, bridge)
}

fn invoke(bridge: &mut Bridge, command: &str, args: Value) -> (u16, Value) {
    let response = bridge.handle(
        "POST",
        &format!("/invoke/{command}"),
        args.to_string().as_bytes(),
    );
    (
        response.status,
        serde_json::from_slice(&response.body).unwrap(),
    )
}

fn reset(bridge: &mut Bridge, scenario: &str) {
    let response = bridge.handle("POST", &format!("/reset/{scenario}"), b"");
    assert_eq!(response.status, 200, "{scenario}");
}

/// Every command the app registers, except what only a window can do.
#[test]
fn it_offers_every_command_of_the_app() {
    let shell = include_str!("../../../app/src-tauri/src/lib.rs");
    let commands: Vec<&str> = shell
        .split("commands::")
        .skip(1)
        .filter_map(|c| c.split(',').next())
        .filter(|c| c.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .filter(|c| *c != "print_window")
        .collect();
    assert!(commands.len() > 60, "{commands:?}");
    let (_dir, mut bridge) = bridge();
    for command in commands {
        let (_, answer) = invoke(&mut bridge, command, json!({}));
        assert_ne!(answer["code"], "unknown_command", "{command}");
    }
}

#[test]
fn it_starts_with_recent_events_and_nothing_open() {
    let (_dir, mut bridge) = bridge();
    assert_eq!(
        invoke(&mut bridge, "event_current", json!({})).1,
        Value::Null
    );
    let (_, recent) = invoke(&mut bridge, "recent_events", json!({}));
    let names: Vec<&str> = recent
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names[0], "Summer Festival 2026");
    assert_eq!(names.len(), 3);
}

#[test]
fn the_example_scenario_is_the_example_event_in_a_real_file() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "example");
    let (_, open) = invoke(&mut bridge, "event_current", json!({}));
    assert_eq!(open["snapshot"]["totals"]["result"], 202_750);
    assert_eq!(open["integrity"]["issues"], json!([]));
}

#[test]
fn every_scenario_loads() {
    let (_dir, mut bridge) = bridge();
    for scenario in scenarios::ALL {
        reset(&mut bridge, scenario);
    }
    assert_eq!(bridge.handle("POST", "/reset/nonsense", b"").status, 400);
}

#[test]
fn the_conflict_scenario_has_an_open_conflict() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "conflict");
    let (_, open) = invoke(&mut bridge, "event_current", json!({}));
    let items = open["snapshot"]["openItems"].as_array().unwrap();
    assert!(
        items.iter().any(|i| i["type"] == "ticketConflicts"),
        "{items:?}"
    );
}

#[test]
fn the_no_order_ids_scenario_has_a_conflict_per_tier() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "no-order-ids");
    let (_, open) = invoke(&mut bridge, "event_current", json!({}));
    let tickets = &open["snapshot"]["tickets"];
    assert_eq!(tickets["orderIds"], json!(false));
    let conflicts = tickets["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0]["order"], "");
    assert_eq!(conflicts[0]["removed"].as_array().unwrap().len(), 2);
}

#[test]
fn the_closed_scenario_is_closed() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "closed");
    let (_, open) = invoke(&mut bridge, "event_current", json!({}));
    assert!(open["snapshot"]["event"]["closedAt"].is_string());
    let (status, error) = invoke(&mut bridge, "person_add", json!({ "name": "Nora" }));
    assert_eq!(
        (status, error["code"].as_str()),
        (400, Some("event_closed"))
    );
}

#[test]
fn the_damaged_and_the_locked_file_are_refused_as_in_the_app() {
    let (_dir, mut bridge) = bridge();
    let (_, recent) = invoke(&mut bridge, "recent_events", json!({}));
    let path = |name: &str| {
        recent
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == name)
            .unwrap()["path"]
            .clone()
    };
    let damaged = path("Winter Festival 2025");
    let locked = path("Summer Festival 2025");
    let (status, error) = invoke(&mut bridge, "event_open", json!({ "path": damaged }));
    assert_eq!((status, error["code"].as_str()), (400, Some("damaged")));
    assert!(error["report"]["issues"].is_array());
    let (status, error) = invoke(&mut bridge, "event_open", json!({ "path": locked }));
    assert_eq!(
        (status, error["code"].as_str()),
        (400, Some("already_open"))
    );
}

#[test]
fn a_wrong_sheet_can_be_taken_back_through_the_commands() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "conflict");
    let (_, open) = invoke(&mut bridge, "event_current", json!({}));
    let last = open["snapshot"]["tickets"]["imports"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["id"]
        .clone();
    let (status, preview) = invoke(&mut bridge, "import_delete_preview", json!({ "id": last }));
    assert_eq!((status, &preview["conflictsAfter"]), (200, &json!(0)));
    let (status, open) = invoke(&mut bridge, "import_delete", json!({ "id": last }));
    assert_eq!(status, 200, "{open}");
    assert_eq!(open["snapshot"]["tickets"]["conflicts"], json!([]));
}

#[test]
fn an_error_keeps_its_code_and_field() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "example");
    let (status, error) = invoke(
        &mut bridge,
        "amount_parse",
        json!({ "input": "1,234", "allowNegative": false }),
    );
    assert_eq!(status, 400);
    assert!(error["code"].is_string() && error["message"].is_string());
}

#[test]
fn a_form_previews_a_fee_or_refund_by_the_core_rule() {
    let (_dir, mut bridge) = bridge();
    let (status, share) = invoke(
        &mut bridge,
        "ticket_share",
        json!({ "price": 1850, "bp": 250, "count": 2 }),
    );
    assert_eq!(status, 200, "{share}");
    assert_eq!(share, json!({ "part": 92, "rest": 3608 }));
}

#[test]
fn a_chosen_sheet_and_receipt_are_real_files() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "example");
    let dialog = |bridge: &mut Bridge, kind: &str| -> Value {
        let response = bridge.handle("POST", &format!("/dialog/{kind}"), b"{}");
        serde_json::from_slice(&response.body).unwrap()
    };
    let sheet = dialog(&mut bridge, "sheet");
    assert!(std::path::Path::new(sheet.as_str().unwrap()).exists());
    let (_, read) = invoke(
        &mut bridge,
        "table_read",
        json!({ "path": sheet, "columns": ["Ticket", "Order ID"] }),
    );
    let tickets: Vec<Value> = read["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!({ "tier": r["cells"][0], "order": r["cells"][1] }))
        .collect();
    let batch = json!({ "source": read["source"], "sha256": read["sha256"], "tickets": tickets });
    let (_, preview) = invoke(
        &mut bridge,
        "tickets_preview",
        json!({ "batch": batch, "date": "2026-06-01" }),
    );
    assert!(preview["sha256"].is_string(), "{preview}");

    let receipts = dialog(&mut bridge, "receipts");
    assert!(std::path::Path::new(receipts[0].as_str().unwrap()).exists());
}

#[test]
fn an_attachment_comes_back_as_its_bytes() {
    let (_dir, mut bridge) = bridge();
    reset(&mut bridge, "example");
    let (_, open) = invoke(&mut bridge, "event_current", json!({}));
    let id = open["snapshot"]["txns"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["attachments"].as_array().unwrap())
        .find(|a| a["mime"] == "image/jpeg")
        .unwrap()["id"]
        .clone();
    let response = bridge.handle("GET", &format!("/attachment/{id}"), b"");
    assert_eq!(
        (response.status, response.mime.as_str()),
        (200, "image/jpeg")
    );
    assert_eq!(&response.body[..3], b"\xFF\xD8\xFF");
    assert_eq!(bridge.handle("GET", "/attachment/99999", b"").status, 404);
}

#[test]
fn an_upload_lands_in_the_bridge_under_its_own_name() {
    let (_dir, mut bridge) = bridge();
    let response = bridge.handle("POST", "/upload?name=../../evil.txt", b"data");
    let path: String = serde_json::from_slice(&response.body).unwrap();
    assert!(
        path.contains("uploads") && path.ends_with("evil.txt"),
        "{path}"
    );
    assert_eq!(std::fs::read(path).unwrap(), b"data");
}
