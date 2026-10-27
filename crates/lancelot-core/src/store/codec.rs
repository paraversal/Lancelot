//! Conversions between domain values and their SQLite text form.

use serde::Serialize;
use serde::de::DeserializeOwned;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, OffsetDateTime};

use super::StoreError;

/// A unit enum as its serde name, e.g. `AccountKind::Asset` → `asset`.
pub(crate) fn enum_text<T: Serialize>(value: &T) -> Result<String, StoreError> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(StoreError::corrupt(format!("not a unit enum: {other}"))),
    }
}

pub(crate) fn text_enum<T: DeserializeOwned>(text: &str) -> Result<T, StoreError> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|_| StoreError::corrupt(format!("unknown value `{text}`")))
}

pub(crate) fn date_text(date: Date) -> String {
    date.to_string()
}

pub(crate) fn text_date(text: &str) -> Result<Date, StoreError> {
    Date::parse(text, format_description!("[year]-[month]-[day]"))
        .map_err(|_| StoreError::corrupt(format!("invalid date `{text}`")))
}

pub(crate) fn timestamp_text(at: OffsetDateTime) -> Result<String, StoreError> {
    at.to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .map_err(|e| StoreError::corrupt(e.to_string()))
}

pub(crate) fn text_timestamp(text: &str) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::parse(text, &Rfc3339)
        .map_err(|_| StoreError::corrupt(format!("invalid timestamp `{text}`")))
}
