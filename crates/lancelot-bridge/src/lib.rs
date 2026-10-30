//! Development only: the real Lancelot commands for the UI in a plain browser, so screens can be
//! clicked through against the real core and a real file. Never part of the app. See
//! `docs/technical-design.md` §5.

pub mod commands;
pub mod scenarios;

// The example event of the core's tests, so there is one in the bridge's files too.
#[path = "../../lancelot-core/tests/common/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use lancelot_core::{AttachmentId, Store};
use lancelot_service::api::ApiError;
use lancelot_service::{AppSettings, Service};
use serde_json::{Value, json};

/// What a request gets back.
pub struct Response {
    pub status: u16,
    pub mime: String,
    pub body: Vec<u8>,
}

impl Response {
    fn json(result: Result<Value, ApiError>) -> Response {
        let (status, value) = match result {
            Ok(value) => (200, value),
            Err(e) => (400, serde_json::to_value(e).unwrap_or(Value::Null)),
        };
        Response {
            status,
            mime: "application/json".to_owned(),
            body: value.to_string().into_bytes(),
        }
    }

    fn not_found() -> Response {
        Response {
            status: 404,
            mime: "text/plain".to_owned(),
            body: b"not found".to_vec(),
        }
    }
}

pub struct Bridge {
    pub service: Service,
    dir: PathBuf,
    /// Holds a file open, so opening it reports it as already open.
    held: Option<Store>,
    next_sheet: usize,
}

impl Bridge {
    /// Everything lives in `dir`, which belongs to the bridge: every reset empties it.
    pub fn new(dir: &Path) -> Result<Bridge, ApiError> {
        let mut bridge = Bridge {
            service: Service::new(dir.join("data")),
            dir: dir.to_owned(),
            held: None,
            next_sheet: 0,
        };
        bridge.reset(scenarios::START)?;
        Ok(bridge)
    }

    /// Starts over with a scenario: new files, new settings, nothing left from before.
    pub fn reset(&mut self, scenario: &str) -> Result<(), ApiError> {
        self.held = None;
        self.service.close_file();
        self.next_sheet = 0;
        if self.dir.exists() {
            fs::remove_dir_all(&self.dir).map_err(io)?;
        }
        for sub in ["data", "events", "saved", "uploads"] {
            fs::create_dir_all(self.dir.join(sub)).map_err(io)?;
        }
        self.service = Service::new(self.dir.join("data"));
        let settings = AppSettings {
            number_format: lancelot_core::NumberFormat::De,
            ..self.service.settings()
        };
        self.service.update_settings(settings)?;
        self.held = scenarios::load(&mut self.service, &self.dir.join("events"), scenario)?;
        Ok(())
    }

    /// The stand-ins for the native dialogs. They never ask, so flows can be clicked through.
    fn dialog(&mut self, kind: &str, name: Option<&str>) -> Result<Value, ApiError> {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        Ok(match kind {
            "open" => json!(self.dir.join("events").join(scenarios::EXAMPLE_FILE)),
            "receipts" => json!([fixtures.join("receipts/receipt.jpg")]),
            // Each sheet in turn, each one later than the last.
            "sheet" => {
                self.next_sheet = self.next_sheet % 3 + 1;
                json!(fixtures.join(format!("tickets/import-{}.ods", self.next_sheet)))
            }
            "save" => json!(unused_path(
                &self.dir.join("saved"),
                name.unwrap_or("saved")
            )),
            _ => {
                return Err(ApiError::new(
                    "bad_request",
                    format!("no such dialog: {kind}"),
                ));
            }
        })
    }

    /// Keeps a file dropped onto the page, since a browser does not say where it came from.
    fn upload(&self, name: &str, data: &[u8]) -> Result<Value, ApiError> {
        let name = Path::new(name)
            .file_name()
            .ok_or_else(|| ApiError::new("bad_request", "no file name"))?;
        let path = unused_path(&self.dir.join("uploads"), &name.to_string_lossy());
        fs::write(&path, data).map_err(io)?;
        Ok(json!(path))
    }

    /// Answers one request. `url` is what comes after the bridge's address.
    pub fn handle(&mut self, method: &str, url: &str, body: &[u8]) -> Response {
        let (path, query) = url.split_once('?').unwrap_or((url, ""));
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let args = || -> Result<Value, ApiError> {
            if body.is_empty() {
                return Ok(Value::Null);
            }
            serde_json::from_slice(body).map_err(|e| ApiError::new("bad_request", e.to_string()))
        };
        match (method, parts.as_slice()) {
            ("POST", ["invoke", command]) => {
                Response::json(args().and_then(|a| commands::run(&mut self.service, command, &a)))
            }
            ("POST", ["reset", scenario]) => {
                Response::json(self.reset(scenario).map(|_| Value::Null))
            }
            ("POST", ["dialog", kind]) => Response::json(
                args().and_then(|a| self.dialog(kind, a.get("name").and_then(Value::as_str))),
            ),
            ("POST", ["upload"]) => {
                let name = query.strip_prefix("name=").unwrap_or("");
                Response::json(self.upload(name, body))
            }
            ("GET", ["attachment", id]) => {
                let found = id
                    .parse()
                    .ok()
                    .and_then(|id| self.service.attachment(AttachmentId(id)).ok());
                match found {
                    Some((mime, body)) => Response {
                        status: 200,
                        mime,
                        body,
                    },
                    None => Response::not_found(),
                }
            }
            _ => Response::not_found(),
        }
    }
}

/// `name` in `dir`, or `stem-2.ext` and so on if that is taken.
fn unused_path(dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    std::iter::once(dir.join(name))
        .chain((2..).map(|n| dir.join(format!("{stem}-{n}.{ext}"))))
        .find(|path| !path.exists())
        .unwrap_or_else(|| dir.join(name))
}

fn io(e: std::io::Error) -> ApiError {
    ApiError::new("io", e.to_string())
}
