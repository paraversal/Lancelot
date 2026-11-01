//! What plugins keep in the event file (technical design §10). The core checks the shape and stores
//! the value; only the plugin knows what it means, and no figure of the core depends on it.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::model::PluginRecordId;

/// What a template does with a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum PluginScope {
    /// Copied by a template, like a price list.
    Setup,
    /// Belongs to this event only, like counts.
    Records,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct PluginRecord {
    pub id: PluginRecordId,
    pub plugin: String,
    pub key: String,
    pub scope: PluginScope,
    #[ts(type = "unknown")]
    pub value: Value,
}

/// Sets a record, or removes it when `value` is null.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct PluginChange {
    pub key: String,
    pub scope: PluginScope,
    #[ts(type = "unknown")]
    pub value: Value,
}
