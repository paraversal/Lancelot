//! Domain core of Lancelot.
//!
//! All money logic lives here (R-COR-6). The crate has no Tauri dependency, so it can be
//! used from tests, a CLI and the desktop shell alike. See `docs/technical-design.md`.

pub mod amount;
pub mod book;
pub mod entry;
pub mod error;
pub mod iban;
pub mod ledger;
pub mod model;
pub mod money;

pub use amount::{AmountError, NumberFormat, Parsed, SignStyle, format_amount, parse_amount};
pub use book::{Book, Saved, Warning};
pub use entry::{Entry, SettleDirection, SplitLine, TxnDraft};
pub use error::DomainError;
pub use iban::{BicError, Iban, IbanError, validate_bic, validate_iban};
pub use ledger::{BlockerOverride, Ledger, MoneyLocation, OpenItem, PersonStatus, PersonSummary};
pub use model::{
    Account, AccountId, AccountKind, Event, EventDetails, EventState, Line, Person, Role, Txn,
    TxnId, TxnKind,
};
pub use money::{Cents, MoneyError};

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
