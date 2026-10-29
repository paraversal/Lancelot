//! The event file: one SQLite database per event (R-ISO-1, R-NF-4).
//!
//! A [`Store`] holds the open file and the [`Book`]. Each command is applied to a copy of the
//! book first; the difference is then written in one SQL transaction together with its change
//! log entries and checked; only after the commit does the copy become the book. A failed
//! command leaves both the file and the book as they were. See `docs/technical-design.md` §6.

mod backup;
mod codec;
pub(crate) mod read;
mod write;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, ErrorCode, OpenFlags, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{Date, OffsetDateTime};
use ts_rs::TS;

pub use backup::{BackupPolicy, list as list_backups, taken_at as backup_time};

use crate::book::{Book, Saved};
use crate::entry::TxnDraft;
use crate::error::DomainError;
use crate::integrity::{self, Depth, Report};
use crate::ledger::BlockerOverride;
use crate::model::{
    AccountId, AccountKind, AssetType, AttachmentId, BudgetLineDraft, BudgetLineId, EventDetails,
    Person, TxnId,
};
use crate::money::Cents;
use crate::{APPLICATION_ID, SCHEMA_VERSION};

/// Migrations, in order. Migration `n` (1-based) brings a file to schema version `n`.
const MIGRATIONS: &[&str] = &[include_str!("../../migrations/0001_initial.sql")];

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("this file is not a Lancelot event file")]
    NotAnEventFile,
    #[error(
        "this event file needs a newer Lancelot (schema {found}, this version reads up to {supported})"
    )]
    NewerSchema { found: u32, supported: u32 },
    #[error("this event file is already open")]
    AlreadyOpen,
    #[error("a file with this name already exists")]
    AlreadyExists,
    #[error("the event file is damaged and cannot be opened")]
    Damaged(Report),
    #[error("the change was not saved because the integrity check failed")]
    IntegrityFailed(Report),
    #[error("the event file is damaged: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl StoreError {
    pub(crate) fn corrupt(reason: impl Into<String>) -> StoreError {
        StoreError::Corrupt(reason.into())
    }

    /// Stable identifier for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            StoreError::Domain(e) => e.code(),
            StoreError::NotAnEventFile => "not_an_event_file",
            StoreError::NewerSchema { .. } => "newer_schema",
            StoreError::AlreadyOpen => "already_open",
            StoreError::AlreadyExists => "already_exists",
            StoreError::Damaged(_) => "damaged",
            StoreError::IntegrityFailed(_) => "integrity_failed",
            StoreError::Corrupt(_) => "corrupt",
            StoreError::Sqlite(_) => "sqlite",
            StoreError::Json(_) => "json",
            StoreError::Io(_) => "io",
        }
    }
}

/// Where "now" comes from. Tests pass a clock they control.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

#[derive(Clone)]
pub struct StoreOptions {
    pub backups: Option<BackupPolicy>,
    pub clock: Clock,
}

impl Default for StoreOptions {
    fn default() -> StoreOptions {
        StoreOptions {
            backups: None,
            clock: Arc::new(OffsetDateTime::now_utc),
        }
    }
}

/// One entry of the change log (R-TX-6).
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ChangeLogEntry {
    #[ts(type = "number")]
    pub id: i64,
    pub at: String,
    pub entity: String,
    #[ts(type = "number")]
    pub entity_id: i64,
    pub op: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

pub struct Store {
    conn: Connection,
    path: PathBuf,
    uuid: String,
    book: Book,
    options: StoreOptions,
    last_backup: Option<OffsetDateTime>,
    backup_error: Option<String>,
}

impl Store {
    /// Creates a new event file and opens it. Never overwrites an existing file.
    ///
    /// The file is written next to its target under a temporary name and linked into place,
    /// so a crash never leaves a half-written event file behind.
    pub fn create(
        path: &Path,
        details: EventDetails,
        options: StoreOptions,
    ) -> Result<(Store, Report), StoreError> {
        if path.exists() {
            return Err(StoreError::AlreadyExists);
        }
        let book = Book::new(details)?;
        let uuid = uuid::Uuid::new_v4().hyphenated().to_string();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("event");
        let tmp = path.with_file_name(format!(".{name}.{uuid}.tmp"));

        let written = (|| -> Result<(), StoreError> {
            let mut conn = Connection::open(&tmp)?;
            configure(&conn)?;
            conn.pragma_update(None, "application_id", i64::from(APPLICATION_ID))?;
            migrate(&mut conn, 0)?;
            let now = (options.clock)();
            let tx = conn.transaction()?;
            for (key, value) in [
                ("event_uuid", uuid.clone()),
                ("created_at", codec::timestamp_text(now)?),
                ("created_by", format!("Lancelot {}", crate::VERSION)),
            ] {
                tx.execute(
                    "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )?;
            }
            write::persist(&tx, None, &book, &BTreeMap::new(), now)?;
            tx.commit()?;
            conn.close().map_err(|(_, e)| e)?;
            Ok(())
        })();
        let placed = written.and_then(|()| place(&tmp, path));
        let _ = std::fs::remove_file(&tmp);
        placed?;
        Store::open(path, options)
    }

    /// Opens an event file: checks that it is one, takes the single-writer lock, migrates it if
    /// needed, runs the full integrity check, loads the book and backs it up.
    ///
    /// A file that cannot be loaded is reported as [`StoreError::Damaged`]. A file that loads
    /// but has issues opens, and the issues are in the returned report (R-COR-5).
    pub fn open(path: &Path, options: StoreOptions) -> Result<(Store, Report), StoreError> {
        check_header(path)?;
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::ZERO)?;
        // Take the single-writer lock before anything else touches the file. In exclusive
        // locking mode it is held until the connection closes.
        let locked =
            conn.execute_batch("PRAGMA locking_mode = EXCLUSIVE; BEGIN IMMEDIATE; COMMIT;");
        if let Err(e) = locked {
            return Err(match e.sqlite_error_code() {
                Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
                    StoreError::AlreadyOpen
                }
                _ => e.into(),
            });
        }
        configure(&conn)?;

        let app_id: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if app_id != i64::from(APPLICATION_ID) || version == 0 {
            return Err(StoreError::NotAnEventFile);
        }
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerSchema {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        let uuid = read::event_uuid(&conn)?;

        // Back up before migrating, so the original survives a failed migration. A file of the
        // current version is checked first instead: a damaged file is never backed up, so
        // retrying it can never push the good backups out.
        let now = (options.clock)();
        let mut last_backup = None;
        let mut backup = |conn: &Connection| -> Result<(), StoreError> {
            if let Some(policy) = &options.backups {
                backup::write(conn, policy, &uuid, now)?;
                last_backup = Some(now);
            }
            Ok(())
        };
        if version < SCHEMA_VERSION {
            backup(&conn)?;
            migrate(&mut conn, version)?;
        }

        let report = integrity::check(&conn, Depth::Full)?;
        let book = match read::parts(&conn)
            .map_err(|e| e.to_string())
            .and_then(|p| Book::from_parts(p).map_err(|e| e.to_string()))
        {
            Ok(book) => book,
            Err(_) if report.has_errors() => return Err(StoreError::Damaged(report)),
            Err(reason) => return Err(StoreError::corrupt(reason)),
        };
        if version == SCHEMA_VERSION {
            backup(&conn)?;
        }
        let store = Store {
            conn,
            path: path.to_owned(),
            uuid,
            book,
            options,
            last_backup,
            backup_error: None,
        };
        Ok((store, report))
    }

    // --- reading -------------------------------------------------------------------------

    pub fn book(&self) -> &Book {
        &self.book
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The event's UUID, which names its backup folder.
    pub fn event_uuid(&self) -> &str {
        &self.uuid
    }

    /// The bytes of an attachment.
    pub fn attachment_data(&self, id: AttachmentId) -> Result<Vec<u8>, StoreError> {
        use rusqlite::OptionalExtension;
        self.conn
            .query_row("SELECT data FROM attachment WHERE id = ?1", [id.0], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or(StoreError::Domain(DomainError::UnknownAttachment))
    }

    /// The newest `limit` change log entries, newest first.
    pub fn change_log(&self, limit: usize) -> Result<Vec<ChangeLogEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, at, entity, entity_id, op, before, after FROM change_log ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })?;
        let json = |s: Option<String>| s.map(|s| serde_json::from_str(&s)).transpose();
        let mut out = Vec::new();
        for row in rows {
            let (id, at, entity, entity_id, op, before, after) = row?;
            out.push(ChangeLogEntry {
                id,
                at,
                entity,
                entity_id,
                op,
                before: json(before)?,
                after: json(after)?,
            });
        }
        Ok(out)
    }

    /// The full integrity check, including attachment hashes.
    pub fn check_integrity(&self) -> Result<Report, StoreError> {
        integrity::check(&self.conn, Depth::Full)
    }

    /// The last backup failure after a change, if any. A failed backup never fails the change
    /// itself, which is already saved, but the app should show it.
    pub fn backup_error(&self) -> Option<&str> {
        self.backup_error.as_deref()
    }

    /// Changes where and how backups are written, e.g. after the backup count setting changed.
    pub fn set_backup_policy(&mut self, policy: Option<BackupPolicy>) {
        self.options.backups = policy;
    }

    /// Writes a backup now, regardless of the interval.
    pub fn backup_now(&mut self) -> Result<PathBuf, StoreError> {
        let policy = self
            .options
            .backups
            .clone()
            .ok_or_else(|| StoreError::corrupt("no backup folder configured"))?;
        let now = (self.options.clock)();
        let path = backup::write(&self.conn, &policy, &self.uuid, now)?;
        self.last_backup = Some(now);
        Ok(path)
    }

    // --- commands ------------------------------------------------------------------------

    pub fn save_txn(&mut self, draft: &TxnDraft, id: Option<TxnId>) -> Result<Saved, StoreError> {
        self.apply(&BTreeMap::new(), |b| b.save_txn(draft, id))
    }

    pub fn delete_txn(&mut self, id: TxnId) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.delete_txn(id).map(|_| ()))
    }

    pub fn add_account(&mut self, kind: AccountKind, name: &str) -> Result<AccountId, StoreError> {
        self.apply(&BTreeMap::new(), |b| b.add_account(kind, name))
    }

    pub fn add_asset(
        &mut self,
        name: &str,
        asset_type: AssetType,
    ) -> Result<AccountId, StoreError> {
        self.apply(&BTreeMap::new(), |b| b.add_asset(name, asset_type))
    }

    pub fn set_asset_type(
        &mut self,
        id: AccountId,
        asset_type: AssetType,
    ) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.set_asset_type(id, asset_type))
    }

    pub fn rename_account(&mut self, id: AccountId, name: &str) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.rename_account(id, name))
    }

    pub fn add_person(&mut self, name: &str, person: Person) -> Result<AccountId, StoreError> {
        self.apply(&BTreeMap::new(), |b| b.add_person(name, person))
    }

    pub fn update_person(
        &mut self,
        id: AccountId,
        name: &str,
        person: Person,
    ) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.update_person(id, name, person))
    }

    pub fn add_budget_line(&mut self, draft: BudgetLineDraft) -> Result<BudgetLineId, StoreError> {
        self.apply(&BTreeMap::new(), |b| b.add_budget_line(draft))
    }

    pub fn update_budget_line(
        &mut self,
        id: BudgetLineId,
        draft: BudgetLineDraft,
    ) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.update_budget_line(id, draft))
    }

    pub fn delete_budget_line(&mut self, id: BudgetLineId) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.delete_budget_line(id).map(|_| ()))
    }

    pub fn update_event(&mut self, details: EventDetails) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.update_event(details))
    }

    pub fn set_default_reimbursement_account(&mut self, id: AccountId) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| {
            b.set_default_reimbursement_account(id)
        })
    }

    /// Records every reimbursement on the payout list that has an IBAN, from `account` on
    /// `date`, in one SQL transaction (R-PER-5). Returns how many were recorded.
    pub fn mark_payouts_paid(
        &mut self,
        account: AccountId,
        date: Date,
    ) -> Result<usize, StoreError> {
        let list = crate::payout::payouts(&self.book).map_err(DomainError::from)?;
        self.apply(&BTreeMap::new(), |b| {
            let mut n = 0;
            for p in list.payouts.iter().filter(|p| p.iban.is_some()) {
                let draft = TxnDraft {
                    date,
                    description: "Reimbursement".into(),
                    counterparty: None,
                    notes: Some(p.reference.clone()),
                    entry: crate::entry::Entry::Settle {
                        person: p.person,
                        account,
                        amount: p.amount,
                        direction: crate::entry::SettleDirection::Reimburse,
                    },
                    total: None,
                };
                b.save_txn(&draft, None)?;
                n += 1;
            }
            Ok(n)
        })
    }

    /// Closes the event (R-SET-7). Closing and reopening are in the change log.
    pub fn close(&mut self, overrides: Vec<BlockerOverride>) -> Result<(), StoreError> {
        let now = (self.options.clock)();
        self.apply(&BTreeMap::new(), |b| b.close(overrides, now))
    }

    /// Reopens a closed event (R-EVT-2).
    pub fn reopen(&mut self) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| {
            b.reopen();
            Ok(())
        })
    }

    /// Stores a file inside the event file (R-ATT-1, R-ATT-2).
    pub fn add_attachment(
        &mut self,
        txn: TxnId,
        filename: &str,
        data: &[u8],
    ) -> Result<AttachmentId, StoreError> {
        let now = (self.options.clock)();
        let file = NewFile::new(filename, data, now);
        let blobs = BTreeMap::from([(AttachmentId(self.book.next_ids().attachment), data)]);
        self.apply(&blobs, |b| file.add_to(b, txn))
    }

    /// Saves a transaction together with its new files and the attachments to remove, in one
    /// SQL transaction: a failed save never leaves a receipt without its transaction or the
    /// other way round.
    pub fn save_txn_with_files(
        &mut self,
        draft: &TxnDraft,
        id: Option<TxnId>,
        files: &[(String, Vec<u8>)],
        remove: &[AttachmentId],
    ) -> Result<Saved, StoreError> {
        let now = (self.options.clock)();
        let first = self.book.next_ids().attachment;
        let blobs: BTreeMap<AttachmentId, &[u8]> = files
            .iter()
            .enumerate()
            .map(|(i, (_, data))| (AttachmentId(first + i as i64), data.as_slice()))
            .collect();
        let new: Vec<NewFile> = files
            .iter()
            .map(|(name, data)| NewFile::new(name, data, now))
            .collect();
        self.apply(&blobs, |b| {
            if let Some(txn) = id {
                for a in remove {
                    if b.attachment(*a).map(|x| x.txn) != Some(txn) {
                        return Err(DomainError::UnknownAttachment);
                    }
                }
            }
            let saved = b.save_txn(draft, id)?;
            for a in remove {
                b.remove_attachment(*a)?;
            }
            for file in &new {
                file.add_to(b, saved.id)?;
            }
            Ok(saved)
        })
    }

    pub fn remove_attachment(&mut self, id: AttachmentId) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.remove_attachment(id).map(|_| ()))
    }

    pub fn set_closing_count(
        &mut self,
        account: AccountId,
        date: Date,
        amount: Cents,
    ) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| {
            b.set_closing_count(account, date, amount)
        })
    }

    pub fn clear_closing_count(&mut self, account: AccountId) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.clear_closing_count(account))
    }

    /// Replaces both counts of an asset account in one change, as the count panel saves them.
    /// `None` removes a count.
    pub fn set_counts(
        &mut self,
        account: AccountId,
        opening: Option<(Date, Cents)>,
        closing: Option<(Date, Cents)>,
    ) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| {
            if b.account(account).map(|a| a.kind) != Some(AccountKind::Asset) {
                return Err(DomainError::WrongAccountKind {
                    field: "account".into(),
                });
            }
            match opening {
                Some((date, amount)) if amount.is_positive() => {
                    b.set_opening_count(account, date, amount)?;
                }
                _ => b.clear_opening_count(account)?,
            }
            match closing {
                Some((date, amount)) => b.set_closing_count(account, date, amount),
                None => b.clear_closing_count(account),
            }
        })
    }

    pub fn set_opening_count(
        &mut self,
        account: AccountId,
        date: Date,
        amount: Cents,
    ) -> Result<TxnId, StoreError> {
        self.apply(&BTreeMap::new(), |b| {
            b.set_opening_count(account, date, amount)
        })
    }

    pub fn clear_opening_count(&mut self, account: AccountId) -> Result<(), StoreError> {
        self.apply(&BTreeMap::new(), |b| b.clear_opening_count(account))
    }

    /// Applies a command to a copy of the book, writes the difference and checks it in one SQL
    /// transaction, and keeps the copy only after the commit.
    fn apply<T>(
        &mut self,
        blobs: &BTreeMap<AttachmentId, &[u8]>,
        command: impl FnOnce(&mut Book) -> Result<T, DomainError>,
    ) -> Result<T, StoreError> {
        let mut next = self.book.clone();
        let value = command(&mut next)?;
        if next == self.book {
            return Ok(value);
        }
        let now = (self.options.clock)();
        let tx = self.conn.transaction()?;
        write::persist(&tx, Some(&self.book), &next, blobs, now)?;
        let report = integrity::check(&tx, Depth::Quick)?;
        if report.has_errors() {
            return Err(StoreError::IntegrityFailed(report));
        }
        tx.commit()?;
        self.book = next;
        self.backup_after_change(now);
        Ok(value)
    }

    fn backup_after_change(&mut self, now: OffsetDateTime) {
        let Some(policy) = &self.options.backups else {
            return;
        };
        if self
            .last_backup
            .is_some_and(|last| now - last < policy.interval)
        {
            return;
        }
        match backup::write(&self.conn, policy, &self.uuid, now) {
            Ok(_) => {
                self.last_backup = Some(now);
                self.backup_error = None;
            }
            Err(e) => self.backup_error = Some(e.to_string()),
        }
    }
}

/// Reads the UUID of an event file without opening it as a store, e.g. to list the backups of a
/// file that is damaged. Fails while another store holds the file open.
pub fn event_uuid_of(path: &Path) -> Result<String, StoreError> {
    check_header(path)?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let app_id: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if app_id != i64::from(APPLICATION_ID) {
        return Err(StoreError::NotAnEventFile);
    }
    read::event_uuid(&conn)
}

fn configure(conn: &Connection) -> Result<(), StoreError> {
    // DELETE journal: no -wal/-shm files next to the event file. FULL: durable on every commit.
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL;",
    )?;
    Ok(())
}

fn migrate(conn: &mut Connection, from: u32) -> Result<(), StoreError> {
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(from as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Moves the finished temporary file into place without ever replacing an existing file.
fn place(tmp: &Path, path: &Path) -> Result<(), StoreError> {
    match std::fs::hard_link(tmp, path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(StoreError::AlreadyExists),
        // File systems without hard links (e.g. FAT): fall back to a rename.
        Err(_) if path.exists() => Err(StoreError::AlreadyExists),
        Err(_) => Ok(std::fs::rename(tmp, path)?),
    }
}

/// Rejects anything that is not an SQLite database before SQLite touches it.
fn check_header(path: &Path) -> Result<(), StoreError> {
    let mut header = [0u8; 16];
    let mut file = File::open(path)?;
    if file.read_exact(&mut header).is_err() || &header != b"SQLite format 3\0" {
        return Err(StoreError::NotAnEventFile);
    }
    Ok(())
}

/// A file about to be attached: its details, computed before the book changes.
struct NewFile {
    filename: String,
    mime: &'static str,
    size: u64,
    sha256: String,
    added_at: OffsetDateTime,
}

impl NewFile {
    fn new(filename: &str, data: &[u8], added_at: OffsetDateTime) -> NewFile {
        NewFile {
            filename: filename.to_owned(),
            mime: sniff_mime(data),
            size: data.len() as u64,
            sha256: hex(&Sha256::digest(data)),
            added_at,
        }
    }

    fn add_to(&self, book: &mut Book, txn: TxnId) -> Result<AttachmentId, DomainError> {
        book.add_attachment(
            txn,
            &self.filename,
            self.mime,
            self.size,
            &self.sha256,
            self.added_at,
        )
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The type of an attachment, from its first bytes. Anything else is stored as-is.
fn sniff_mime(data: &[u8]) -> &'static str {
    match data {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [b'%', b'P', b'D', b'F', ..] => "application/pdf",
        [_, _, _, _, b'f', b't', b'y', b'p', brand @ ..]
            if [b"heic", b"heix", b"hevc", b"mif1"]
                .iter()
                .any(|b| brand.starts_with(*b)) =>
        {
            "image/heic"
        }
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_matches_the_migrations() {
        assert_eq!(MIGRATIONS.len(), SCHEMA_VERSION as usize);
    }

    #[test]
    fn sniffs_common_receipt_types() {
        assert_eq!(sniff_mime(b"\x89PNG\r\n\x1a\n...."), "image/png");
        assert_eq!(sniff_mime(b"\xFF\xD8\xFF\xE0...."), "image/jpeg");
        assert_eq!(sniff_mime(b"%PDF-1.7\n"), "application/pdf");
        assert_eq!(sniff_mime(b"\0\0\0\x18ftypheic\0\0"), "image/heic");
        assert_eq!(sniff_mime(b"hello"), "application/octet-stream");
        assert_eq!(sniff_mime(b""), "application/octet-stream");
    }
}
