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
pub mod plugin;
pub mod sales;
pub mod sheet;
pub mod snapshot;
pub mod store;
pub mod tickets;

pub use amount::{
    AmountError, NumberFormat, Parsed, ParsedSum, SignStyle, format_amount, parse_amount, parse_sum,
};
pub use book::{
    Book, BookParts, CopyChoice, CopySummary, MAX_ATTACHMENT_SIZE, NextIds, Saved, TemplateSummary,
    Warning,
};
pub use budget::{Budget, BudgetLineFigures, CategoryFigures, CostCentreFigures, Figures};
pub use date::{DateError, format_date, parse_date};
pub use entry::{Entry, SettleDirection, SplitLine, TxnDraft, build_lines, entry_of, txn_amount};
pub use error::DomainError;
pub use export::transactions_csv;
pub use iban::{BicError, Iban, IbanError, validate_bic, validate_iban};
pub use integrity::{Issue, Report, Severity};
pub use ledger::{
    CashStatus, Ledger, MoneyLocation, OpeningCount, PersonEntry, PersonStatus, PersonSummary,
    PersonTotals, ReconciliationStatus,
};
pub use model::{
    Account, AccountId, AccountKind, AssetType, Attachment, AttachmentId, BlockerOverride,
    BudgetLine, BudgetLineDraft, BudgetLineId, CancellationId, ClosingCount, CostCentre,
    CostCentreId, Estimate, Event, EventDetails, Flow, ImportId, Line, ManualSaleId, OpenItem,
    Origin, Person, PluginRecordId, Reconciliation, ReconciliationId, Role, TierId, Txn, TxnId,
    TxnKind,
};
pub use money::{Cents, MoneyError};
pub use payout::{PayoutRow, Payouts, RepaymentRow, payouts, payouts_csv};
pub use plugin::{PluginChange, PluginRecord, PluginScope};
pub use sales::{Profitability, SalesPoint, TierBreakEven, TierSales};
pub use sheet::{Table, TableRow, read_table};
pub use snapshot::{AccountView, EventSnapshot, PersonView, Totals, TxnView};
pub use store::{
    BackupPolicy, ChangeLogEntry, Store, StoreError, StoreOptions, backup_time, event_uuid_of,
    list_backups, read_book,
};
pub use tickets::{
    BatchTicket, Cancellation, CancellationDraft, CancellationEdit, CancellationSource,
    CancellationView, ConflictMatch, ImportDeletion, ImportPreview, ManualSale, ManualSaleDraft,
    OrderConflict, RefundTerms, RemovedTickets, Resolution, TicketBatch, TicketImport,
    TicketSettings, TicketShare, TicketTotals, Tickets, Tier, TierDraft, ticket_share,
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
