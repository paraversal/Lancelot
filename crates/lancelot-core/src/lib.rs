//! Domain core of Lancelot.
//!
//! All money logic lives here (R-COR-6). The crate has no Tauri dependency, so it can be
//! used from tests, a CLI and the desktop shell alike. See `docs/technical-design.md`.

/// Schema version written to `PRAGMA user_version` of every event file.
pub const SCHEMA_VERSION: u32 = 1;

/// SQLite `application_id` of an event file: `LNCL`.
pub const APPLICATION_ID: u32 = u32::from_be_bytes(*b"LNCL");

/// Version of this crate, shown in the app.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_id_spells_lncl() {
        assert_eq!(APPLICATION_ID, 0x4C4E_434C);
    }
}
