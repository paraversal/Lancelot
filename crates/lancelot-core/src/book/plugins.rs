//! The book's plugin records (technical design §10).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::{Book, NextIds};
use crate::error::DomainError;
use crate::model::PluginRecordId;
use crate::plugin::{PluginChange, PluginRecord};

/// Far more than a record needs. It only catches a plugin that keeps files in its records.
const MAX_VALUE_SIZE: usize = 64 * 1024;
const MAX_KEY_SIZE: usize = 200;

impl Book {
    pub fn plugin_records(&self) -> impl Iterator<Item = &PluginRecord> {
        self.plugin_records.values()
    }

    /// Applies one plugin's changes together: all of them, or none if one is invalid. Removing a
    /// record that does not exist changes nothing.
    pub fn apply_plugin_changes(
        &mut self,
        plugin: String,
        changes: Vec<PluginChange>,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            check_plugin(&plugin)?;
            for change in changes {
                check_key(&change.key)?;
                let existing = book
                    .plugin_records
                    .values()
                    .find(|r| r.plugin == plugin && r.key == change.key)
                    .map(|r| r.id);
                if change.value.is_null() {
                    if let Some(id) = existing {
                        book.plugin_records.remove(&id);
                    }
                    continue;
                }
                check_value(&change.value)?;
                let id = existing.unwrap_or_else(|| {
                    let id = PluginRecordId(book.next.plugin_record);
                    book.next.plugin_record += 1;
                    id
                });
                book.plugin_records.insert(
                    id,
                    PluginRecord {
                        id,
                        plugin: plugin.clone(),
                        key: change.key,
                        scope: change.scope,
                        value: change.value,
                    },
                );
            }
            Ok(())
        })
    }
}

fn invalid(field: &str) -> DomainError {
    DomainError::InvalidPluginRecord {
        field: field.into(),
    }
}

/// Lower-case letters, digits and hyphens, starting with a letter.
fn check_plugin(plugin: &str) -> Result<(), DomainError> {
    let mut chars = plugin.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(invalid("plugin"))
    }
}

fn check_key(key: &str) -> Result<(), DomainError> {
    if key.is_empty() || key.len() > MAX_KEY_SIZE {
        return Err(invalid("key"));
    }
    Ok(())
}

fn check_value(value: &Value) -> Result<(), DomainError> {
    let size = serde_json::to_string(value)
        .map_err(|_| invalid("value"))?
        .len();
    if value.is_null() || size > MAX_VALUE_SIZE {
        return Err(invalid("value"));
    }
    Ok(())
}

/// Checks the stored records of all plugins, whether or not they are installed.
pub(super) fn check_records(
    records: Vec<PluginRecord>,
    next: &NextIds,
) -> Result<BTreeMap<PluginRecordId, PluginRecord>, DomainError> {
    let mut out = BTreeMap::new();
    let mut keys = BTreeSet::new();
    for r in records {
        let valid = check_plugin(&r.plugin).is_ok()
            && check_key(&r.key).is_ok()
            && check_value(&r.value).is_ok()
            && r.id.0 < next.plugin_record
            && keys.insert((r.plugin.clone(), r.key.clone()));
        if !valid {
            return Err(DomainError::Inconsistent {
                reason: "plugin record with an invalid id, name, key or value".to_owned(),
            });
        }
        out.insert(r.id, r);
    }
    Ok(out)
}
