//! Domain core of Lancelot.
//!
//! All money logic lives here (R-COR-6). The crate has no Tauri dependency, so it can be
//! used from tests, a CLI and the desktop shell alike. See `docs/technical-design.md`.

pub mod amount;
pub mod book;
pub mod budget;
mod csv;
pub mod date;
pub mod entry;
pub mod error;
pub mod export;
pub mod iban;
pub mod integrity;
pub mod ledger;
pub mod model;
pub mod money;
pub mod payout;
pub mod snapshot;
pub mod store;

pub use amount::{AmountError, NumberFormat, Parsed, SignStyle, format_amount, parse_amount};
pub use book::{Book, BookParts, MAX_ATTACHMENT_SIZE, NextIds, Saved, Warning};
pub use budget::{Budget, BudgetLineFigures, CategoryFigures, Figures};
pub use date::{DateError, format_date, parse_date};
pub use entry::{Entry, SettleDirection, SplitLine, TxnDraft, txn_amount};
pub use error::DomainError;
pub use export::transactions_csv;
pub use iban::{BicError, Iban, IbanError, validate_bic, validate_iban};
pub use integrity::{Issue, Report, Severity};
pub use ledger::{
    BlockerOverride, CashStatus, Ledger, MoneyLocation, OpenItem, OpeningCount, PersonEntry,
    PersonStatus, PersonSummary, PersonTotals,
};
pub use model::{
    Account, AccountId, AccountKind, AssetType, Attachment, AttachmentId, BudgetLine,
    BudgetLineDraft, BudgetLineId, ClosingCount, Estimate, Event, EventDetails, Line, Person, Role,
    Txn, TxnId, TxnKind,
};
pub use money::{Cents, MoneyError};
pub use payout::{PayoutRow, Payouts, RepaymentRow, payouts, payouts_csv};
pub use snapshot::{AccountView, EventSnapshot, PersonView, Totals, TxnView};
pub use store::{
    BackupPolicy, ChangeLogEntry, Store, StoreError, StoreOptions, backup_time, event_uuid_of,
    list_backups,
};

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
