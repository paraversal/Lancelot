//! What crosses the IPC boundary besides the core's own types (`docs/technical-design.md` §5).

use lancelot_core::{
    AccountId, AmountError, Cents, DateError, DomainError, EventSnapshot, Report, StoreError,
    TxnId, Warning,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Every command fails with this. `code` is stable; `message` is for people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    /// The input the error belongs to, e.g. `name` or `lines[1].amount`.
    pub field: Option<String>,
    /// For a damaged event file: what the integrity check found.
    pub report: Option<Report>,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> ApiError {
        ApiError {
            code: code.to_owned(),
            message: message.into(),
            field: None,
            report: None,
        }
    }

    pub fn no_event() -> ApiError {
        ApiError::new("no_event_open", "no event is open")
    }
}

impl From<DomainError> for ApiError {
    fn from(e: DomainError) -> ApiError {
        ApiError {
            field: e.field().map(str::to_owned),
            ..ApiError::new(e.code(), e.to_string())
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> ApiError {
        let base = ApiError::new(e.code(), e.to_string());
        match e {
            StoreError::Domain(d) => d.into(),
            StoreError::Damaged(report) | StoreError::IntegrityFailed(report) => ApiError {
                report: Some(report),
                ..base
            },
            _ => base,
        }
    }
}

impl From<lancelot_core::IbanError> for ApiError {
    fn from(e: lancelot_core::IbanError) -> ApiError {
        ApiError::new(e.code(), e.to_string())
    }
}

impl From<AmountError> for ApiError {
    fn from(e: AmountError) -> ApiError {
        ApiError::new(e.code(), e.to_string())
    }
}

impl From<DateError> for ApiError {
    fn from(e: DateError) -> ApiError {
        ApiError::new(e.code(), e.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> ApiError {
        ApiError::new("io", e.to_string())
    }
}

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    #[ts(type = "number")]
    pub schema_version: u32,
}

/// The open event as the UI sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OpenEvent {
    /// Increases with every change, so the UI can tell snapshots apart.
    #[ts(type = "number")]
    pub revision: u64,
    pub path: String,
    pub file_name: String,
    pub snapshot: EventSnapshot,
    /// From opening the file or the last "Check again" (R-COR-5).
    pub integrity: Report,
    /// The last failed backup after a change, if any.
    pub backup_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RecentEvent {
    pub path: String,
    pub name: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub last_opened: String,
    /// False when the file was moved or deleted since.
    pub exists: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    /// RFC 3339 UTC, from the file name.
    pub taken_at: Option<String>,
    #[ts(type = "number")]
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ParsedDate {
    /// ISO `YYYY-MM-DD`.
    pub date: String,
}

/// After saving a transaction: the new state, the transaction's id and any warnings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct SavedTxn {
    pub event: OpenEvent,
    pub id: TxnId,
    pub warnings: Vec<Warning>,
}

/// After adding an account, a category or a person: the new state and its id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Added {
    pub event: OpenEvent,
    pub id: AccountId,
}

/// A checked IBAN: as stored and as shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CheckedIban {
    pub iban: String,
    pub formatted: String,
}

/// An opening or closing count as the count panel sends it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
pub struct CountInput {
    /// Any format `date_parse` accepts.
    pub date: String,
    pub amount: Cents,
}
