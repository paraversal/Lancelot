//! What the commands do, without Tauri, so it can be tested on its own.
//!
//! One window holds at most one open event. Opening or creating another closes the current
//! one first, which releases its lock.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use lancelot_core::{
    AccountId, AttachmentId, BackupPolicy, BlockerOverride, Book, Cents, CopyChoice, CopySummary,
    DomainError, EventDetails, EventSnapshot, ImportDeletion, ImportId, ImportPreview,
    MAX_ATTACHMENT_SIZE, Parsed, Report, Store, StoreError, StoreOptions, TemplateSummary,
    TicketShare, TxnDraft, TxnId, backup_time, event_uuid_of, format_date, list_backups,
    parse_amount, parse_date, read_book, ticket_share,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::api::{
    Added, ApiError, BackupInfo, CheckedIban, CountInput, OpenEvent, ParsedDate, RecentEvent,
    SavedTxn,
};
use crate::settings::{AppSettings, RecentEntry, SettingsFile};

pub struct Service {
    data_dir: PathBuf,
    file: SettingsFile,
    open: Option<Opened>,
}

struct Opened {
    store: Store,
    revision: u64,
    integrity: Report,
}

type Result<T> = std::result::Result<T, ApiError>;

fn recent_entry(store: &Store) -> RecentEntry {
    let d = &store.book().event().details;
    let iso =
        |date: Option<time::Date>| date.map(|d| format_date(d, lancelot_core::NumberFormat::En));
    RecentEntry {
        path: store.path().to_owned(),
        name: d.name.clone(),
        starts_on: iso(d.starts_on),
        ends_on: iso(d.ends_on),
        last_opened: now(),
    }
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// A ticket sheet's name and bytes. Sheets are small; anything the size of an attachment
/// limit is not one.
fn read_sheet_file(path: &Path) -> Result<(String, Vec<u8>)> {
    if std::fs::metadata(path)?.len() > MAX_ATTACHMENT_SIZE {
        return Err(ApiError::new(
            "sheet_too_large",
            "this file is too large for a ticket sheet",
        ));
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sheet".into());
    Ok((name, std::fs::read(path)?))
}

impl Service {
    /// `data_dir` holds `settings.json` and the `backups` folder.
    pub fn new(data_dir: PathBuf) -> Service {
        let file = SettingsFile::load(&data_dir.join("settings.json"));
        Service {
            data_dir,
            file,
            open: None,
        }
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    fn backup_policy(&self) -> BackupPolicy {
        let mut policy = BackupPolicy::new(self.backups_dir());
        policy.keep = self.file.settings.backup_count as usize;
        policy
    }

    // --- settings ------------------------------------------------------------------------

    pub fn settings(&self) -> AppSettings {
        self.file.settings.clone()
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> Result<AppSettings> {
        if !AppSettings::BACKUP_COUNT_RANGE.contains(&settings.backup_count) {
            let mut e = ApiError::new("invalid_setting", "keep between 1 and 100 backups");
            e.field = Some("backupCount".into());
            return Err(e);
        }
        self.file.settings = settings;
        self.file.save(&self.settings_path())?;
        let policy = self.backup_policy();
        if let Some(open) = &mut self.open {
            open.store.set_backup_policy(Some(policy));
        }
        Ok(self.settings())
    }

    pub fn recent_events(&self) -> Vec<RecentEvent> {
        self.file
            .recent
            .iter()
            .map(|r| RecentEvent {
                path: r.path.display().to_string(),
                name: r.name.clone(),
                starts_on: r.starts_on.clone(),
                ends_on: r.ends_on.clone(),
                last_opened: r.last_opened.clone(),
                exists: r.path.is_file(),
            })
            .collect()
    }

    pub fn forget_recent(&mut self, path: &Path) -> Result<Vec<RecentEvent>> {
        self.file.forget(path);
        self.file.save(&self.settings_path())?;
        Ok(self.recent_events())
    }

    // --- opening and closing -------------------------------------------------------------

    fn options(&self) -> StoreOptions {
        StoreOptions {
            backups: Some(self.backup_policy()),
            ..StoreOptions::default()
        }
    }

    pub fn create_event(&mut self, path: &Path, details: EventDetails) -> Result<OpenEvent> {
        self.close_file();
        let (store, integrity) = Store::create(path, details, self.options())?;
        self.adopt(store, integrity)
    }

    /// A new event with the setup of another one (R-EVT-3).
    pub fn create_event_from(
        &mut self,
        path: &Path,
        details: EventDetails,
        template: &Path,
    ) -> Result<OpenEvent> {
        let template = self.other_event(template)?;
        self.close_file();
        let (store, integrity) =
            Store::create_from_template(path, details, &template, self.options())?;
        self.adopt(store, integrity)
    }

    /// Reads another event, which may be the file open here.
    fn other_event(&self, path: &Path) -> Result<Book> {
        match &self.open {
            Some(o) if o.store.path() == path => Ok(o.store.book().clone()),
            _ => Ok(read_book(path)?),
        }
    }

    pub fn template_preview(&self, path: &Path) -> Result<TemplateSummary> {
        Ok(self.other_event(path)?.template_summary())
    }

    /// What copying everything from another event would add to the open one; changes nothing.
    pub fn copy_preview(&self, path: &Path) -> Result<CopySummary> {
        let source = self.other_event(path)?;
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        let choice = CopyChoice {
            persons: true,
            budget: true,
        };
        let summary = open.store.book().clone().copy_from(&source, choice);
        summary.map_err(|e| StoreError::from(e).into())
    }

    /// Copies persons and budget lines from another event (R-ISO-4).
    pub fn copy_from_event(&mut self, path: &Path, choice: CopyChoice) -> Result<OpenEvent> {
        let source = self.other_event(path)?;
        self.change(|b| b.copy_from(&source, choice))
    }

    pub fn open_event(&mut self, path: &Path) -> Result<OpenEvent> {
        if self.open.as_ref().is_some_and(|o| o.store.path() == path) {
            return self.current().ok_or_else(ApiError::no_event);
        }
        self.close_file();
        let (store, integrity) = Store::open(path, self.options())?;
        self.adopt(store, integrity)
    }

    fn adopt(&mut self, store: Store, integrity: Report) -> Result<OpenEvent> {
        self.file.remember(recent_entry(&store));
        self.file.save(&self.settings_path())?;
        self.open = Some(Opened {
            store,
            revision: 1,
            integrity,
        });
        self.current().ok_or_else(ApiError::no_event)
    }

    pub fn close_file(&mut self) {
        self.open = None;
    }

    pub fn current(&self) -> Option<OpenEvent> {
        let o = self.open.as_ref()?;
        let path = o.store.path();
        Some(OpenEvent {
            revision: o.revision,
            path: path.display().to_string(),
            file_name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            snapshot: EventSnapshot::new(o.store.book()),
            integrity: o.integrity.clone(),
            backup_error: o.store.backup_error().map(str::to_owned),
        })
    }

    /// Applies a command to the open book and returns the new state.
    pub fn change<T>(
        &mut self,
        command: impl FnOnce(&mut Book) -> std::result::Result<T, DomainError>,
    ) -> Result<OpenEvent> {
        self.on_store(|s| {
            s.change(command)?;
            Ok(())
        })
    }

    /// [`Service::change`] for a command that adds an account, whose id the UI needs.
    pub fn add(
        &mut self,
        command: impl FnOnce(&mut Book) -> std::result::Result<AccountId, DomainError>,
    ) -> Result<Added> {
        let mut id = None;
        let event = self.on_store(|s| {
            id = Some(s.change(command)?);
            Ok(())
        })?;
        Ok(Added {
            event,
            id: id.ok_or_else(ApiError::no_event)?,
        })
    }

    /// Runs a command on the open store and returns the new state.
    fn on_store(&mut self, command: impl FnOnce(&mut Store) -> Result<()>) -> Result<OpenEvent> {
        let open = self.open.as_mut().ok_or_else(ApiError::no_event)?;
        command(&mut open.store)?;
        open.revision += 1;
        // Keep the start screen's name and dates in step with the event.
        let entry = recent_entry(&open.store);
        let stale = self.file.recent.first().is_some_and(|r| {
            r.path == entry.path
                && (r.name != entry.name
                    || r.starts_on != entry.starts_on
                    || r.ends_on != entry.ends_on)
        });
        if stale {
            self.file.remember(entry);
            self.file.save(&self.settings_path())?;
        }
        self.current().ok_or_else(ApiError::no_event)
    }

    // --- event commands ------------------------------------------------------------------

    /// Saves a transaction with the files to attach (paths, e.g. dropped onto the window) and
    /// the attachments to remove, all in one change.
    pub fn save_txn(
        &mut self,
        draft: &TxnDraft,
        id: Option<TxnId>,
        files: &[PathBuf],
        remove: &[AttachmentId],
    ) -> Result<SavedTxn> {
        let mut read = Vec::with_capacity(files.len());
        for path in files {
            if std::fs::metadata(path)?.len() > MAX_ATTACHMENT_SIZE {
                return Err(DomainError::AttachmentTooLarge.into());
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            read.push((name, std::fs::read(path)?));
        }
        let mut saved = None;
        let event = self.on_store(|s| {
            saved = Some(s.save_txn_with_files(draft, id, &read, remove)?);
            Ok(())
        })?;
        let saved = saved.ok_or_else(ApiError::no_event)?;
        Ok(SavedTxn {
            event,
            id: saved.id,
            warnings: saved.warnings,
        })
    }

    /// An attachment's type and bytes, for the preview protocol.
    pub fn attachment(&self, id: AttachmentId) -> Result<(String, Vec<u8>)> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        let meta = open
            .store
            .book()
            .attachment(id)
            .ok_or(DomainError::UnknownAttachment)?;
        Ok((meta.mime.clone(), open.store.attachment_data(id)?))
    }

    /// Writes an attachment to a temporary file under its own name, for opening it in its
    /// default app (R-ATT-3). Returns the path.
    pub fn attachment_temp(&self, id: AttachmentId) -> Result<PathBuf> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        let meta = open
            .store
            .book()
            .attachment(id)
            .ok_or(DomainError::UnknownAttachment)?;
        let dir = std::env::temp_dir().join("lancelot").join(id.0.to_string());
        std::fs::create_dir_all(&dir)?;
        // Only the file name: a stored name can never point outside the folder.
        let name = Path::new(&meta.filename)
            .file_name()
            .map(|n| n.to_owned())
            .unwrap_or_else(|| "file".into());
        let path = dir.join(name);
        std::fs::write(&path, open.store.attachment_data(id)?)?;
        Ok(path)
    }

    /// Exports an attachment to a path the user chose in the save dialog.
    pub fn export_attachment(&self, id: AttachmentId, target: &Path) -> Result<()> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        std::fs::write(target, open.store.attachment_data(id)?)?;
        Ok(())
    }

    /// Replaces an asset account's opening and closing counts (R-ISO-3).
    pub fn set_counts(
        &mut self,
        account: AccountId,
        opening: Option<CountInput>,
        closing: Option<CountInput>,
    ) -> Result<OpenEvent> {
        let parse = |c: Option<CountInput>, field: &str| {
            c.map(|c| {
                parse_date(&c.date)
                    .map(|d| (d, c.amount))
                    .map_err(|e| ApiError {
                        field: Some(format!("{field}.date")),
                        ..ApiError::from(e)
                    })
            })
            .transpose()
        };
        let (opening, closing) = (parse(opening, "opening")?, parse(closing, "closing")?);
        self.change(|b| b.set_counts(account, opening, closing))
    }

    /// Reads a ticket sheet from disk and shows what importing it would change (R-TKT-5).
    pub fn preview_tickets(&self, path: &Path, date: time::Date) -> Result<ImportPreview> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        let (name, data) = read_sheet_file(path)?;
        Ok(open.store.preview_ticket_import(&name, &data, date)?)
    }

    /// Imports a ticket sheet as the snapshot of `date`. `sha256` is the file the preview
    /// showed: a file changed since is not imported unseen.
    pub fn import_tickets(
        &mut self,
        path: &Path,
        date: time::Date,
        sha256: &str,
    ) -> Result<OpenEvent> {
        let (name, data) = read_sheet_file(path)?;
        self.on_store(|s| {
            if s.preview_ticket_import(&name, &data, date)?.sha256 != sha256 {
                return Err(ApiError::new(
                    "file_changed",
                    "the file changed since it was read; read it again",
                ));
            }
            s.import_tickets(&name, &data, date)?;
            Ok(())
        })
    }

    /// What deleting an import would change; changes nothing.
    pub fn preview_delete_import(&self, id: ImportId) -> Result<ImportDeletion> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        Ok(open.store.book().preview_delete_import(id)?)
    }

    /// Checks an IBAN as it is typed (R-PER-2).
    pub fn check_iban(&self, input: &str) -> Result<CheckedIban> {
        let iban = lancelot_core::validate_iban(input)?;
        Ok(CheckedIban {
            formatted: iban.formatted(),
            iban: iban.as_str().to_owned(),
        })
    }

    /// Writes the payout list as CSV in the user's number format (R-PER-4). Starts with a byte
    /// order mark, so spreadsheet apps read umlauts correctly.
    pub fn export_payouts_csv(&self, target: &Path) -> Result<()> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        let list = lancelot_core::payouts(open.store.book()).map_err(DomainError::from)?;
        let csv = lancelot_core::payouts_csv(&list, self.file.settings.number_format);
        std::fs::write(target, format!("\u{feff}{csv}"))?;
        Ok(())
    }

    /// Every line of every transaction as CSV (R-SET-6), with a BOM so spreadsheets read the
    /// umlauts.
    pub fn export_transactions_csv(&self, target: &Path) -> Result<()> {
        let open = self.open.as_ref().ok_or_else(ApiError::no_event)?;
        let csv =
            lancelot_core::transactions_csv(open.store.book(), self.file.settings.number_format);
        std::fs::write(target, format!("\u{feff}{csv}"))?;
        Ok(())
    }

    /// Closes the event (R-SET-7). Every open item must be resolved or come with a note.
    pub fn close_event(&mut self, overrides: Vec<BlockerOverride>) -> Result<OpenEvent> {
        self.on_store(|s| Ok(s.close(overrides)?))
    }

    pub fn check_integrity(&mut self) -> Result<OpenEvent> {
        let open = self.open.as_mut().ok_or_else(ApiError::no_event)?;
        open.integrity = open.store.check_integrity()?;
        self.current().ok_or_else(ApiError::no_event)
    }

    // --- backups -------------------------------------------------------------------------

    /// The backups of an event file, newest first. Works for the open event and for a file
    /// that cannot be opened, e.g. a damaged one.
    pub fn backups(&self, event: &Path) -> Result<Vec<BackupInfo>> {
        let uuid = match &self.open {
            Some(o) if o.store.path() == event => o.store.event_uuid().to_owned(),
            _ => event_uuid_of(event)?,
        };
        let paths = list_backups(&self.backups_dir(), &uuid)?;
        Ok(paths
            .into_iter()
            .map(|p| BackupInfo {
                taken_at: backup_time(&p).and_then(|t| t.format(&Rfc3339).ok()),
                size: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
                path: p.display().to_string(),
            })
            .collect())
    }

    /// Copies a backup of `event` to `target` (never over an existing file) and opens the copy.
    pub fn restore_backup(
        &mut self,
        event: &Path,
        backup: &Path,
        target: &Path,
    ) -> Result<OpenEvent> {
        let known = self
            .backups(event)?
            .into_iter()
            .any(|b| Path::new(&b.path) == backup);
        if !known {
            return Err(ApiError::new(
                "not_a_backup",
                "this file is not a backup of this event",
            ));
        }
        let mut from = std::fs::File::open(backup)?;
        let mut to = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    ApiError::new("already_exists", "a file with this name already exists")
                } else {
                    e.into()
                }
            })?;
        std::io::copy(&mut from, &mut to)?;
        to.sync_all()?;
        drop(to);
        self.open_event(target)
    }

    // --- input ---------------------------------------------------------------------------

    pub fn parse_amount(&self, input: &str, allow_negative: bool) -> Result<Parsed> {
        Ok(parse_amount(
            input,
            self.file.settings.number_format,
            allow_negative,
        )?)
    }

    /// The fee or refund a form is about to save, by the core's rule.
    pub fn ticket_share(&self, price: Cents, bp: u32, count: u32) -> Result<TicketShare> {
        ticket_share(price, bp, count).map_err(|e| DomainError::from(e).into())
    }

    pub fn parse_date(&self, input: &str) -> Result<ParsedDate> {
        let date = parse_date(input)?;
        Ok(ParsedDate {
            date: format_date(date, lancelot_core::NumberFormat::En),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lancelot_core::{
        AccountKind, AssetType, CancellationDraft, CancellationEdit, CancellationSource,
        ManualSaleDraft, NumberFormat, OpenItem, Person, RefundTerms, Resolution, TicketColumns,
        TierDraft,
    };
    use tempfile::TempDir;

    fn details(name: &str) -> EventDetails {
        EventDetails {
            name: name.into(),
            starts_on: None,
            ends_on: None,
            location: None,
            notes: None,
        }
    }

    fn service(dir: &TempDir) -> Service {
        Service::new(dir.path().join("data"))
    }

    #[test]
    fn starts_from_a_template_and_copies_from_another_event() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        let old = dir.path().join("old.lancelot");
        s.create_event(&old, details("Old")).unwrap();
        s.add(|b| b.add_person("Kim", Person::default())).unwrap();
        s.close_file();

        let summary = s.template_preview(&old).unwrap();
        assert_eq!((summary.name.as_str(), summary.persons), ("Old", 1));

        let new = dir.path().join("new.lancelot");
        let open = s.create_event_from(&new, details("New"), &old).unwrap();
        assert_eq!(open.snapshot.event.details.name, "New");
        assert_eq!(open.snapshot.persons.len(), 1);
        s.close_file();

        let third = dir.path().join("third.lancelot");
        s.create_event(&third, details("Third")).unwrap();
        let preview = s.copy_preview(&old).unwrap();
        assert_eq!(preview.persons_added, 1);
        assert_eq!(s.current().unwrap().snapshot.persons.len(), 0);

        let choice = CopyChoice {
            persons: true,
            budget: false,
        };
        let copied = s.copy_from_event(&old, choice).unwrap();
        assert_eq!(copied.snapshot.persons[0].name, "Kim");
        assert_eq!(s.copy_preview(&old).unwrap().persons_skipped, 1);

        // The file open here is read from memory: its lock would refuse a second reader.
        assert_eq!(s.template_preview(&third).unwrap().name, "Third");
    }

    #[test]
    fn reconciles_a_statement() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        let open = s
            .create_event(&dir.path().join("e.lancelot"), details("E"))
            .unwrap();
        let verein = open.snapshot.event.default_reimbursement_account;
        let amount = lancelot_core::parse_amount("12,00", lancelot_core::NumberFormat::De, true)
            .unwrap()
            .cents;

        let on = |day| time::Date::from_calendar_date(2026, time::Month::June, day).unwrap();
        let open = s
            .change(|b| b.add_reconciliation(verein, on(30), amount))
            .unwrap();
        let r = open.snapshot.reconciliations[0];
        assert_eq!((r.movement, r.difference), (true, amount));
        let err = s
            .change(|b| b.add_reconciliation(verein, on(30), amount))
            .unwrap_err();
        assert_eq!(err.code, "duplicate_checkpoint");

        let open = s
            .change(|b| b.update_reconciliation(r.id, on(29), lancelot_core::Cents::ZERO))
            .unwrap();
        assert!(open.snapshot.reconciliations[0].difference.is_zero());
        let open = s.change(|b| b.delete_reconciliation(r.id)).unwrap();
        assert!(open.snapshot.reconciliations.is_empty());
    }

    #[test]
    fn creates_opens_and_remembers_events() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sommerfest.lancelot");
        let mut s = service(&dir);
        assert!(s.current().is_none());

        let open = s.create_event(&path, details("Sommerfest")).unwrap();
        assert_eq!(
            (open.revision, open.file_name.as_str()),
            (1, "sommerfest.lancelot")
        );
        assert_eq!(open.snapshot.event.details.name, "Sommerfest");
        assert!(open.integrity.is_clean());

        let mut dated = details("Sommerfest 2026");
        dated.starts_on = Some(time::macros::date!(2026 - 06 - 12));
        let renamed = s.change(|b| b.update_event(dated)).unwrap();
        assert_eq!(renamed.revision, 2);
        assert_eq!(
            s.recent_events()[0].starts_on.as_deref(),
            Some("2026-06-12")
        );
        s.close_file();
        assert_eq!(
            s.change(|b| b.update_event(details("x"))).unwrap_err().code,
            "no_event_open"
        );

        // A new service (the app restarted) remembers the event under its new name.
        let mut s = service(&dir);
        let recent = s.recent_events();
        assert_eq!(
            (recent[0].name.as_str(), recent[0].exists),
            ("Sommerfest 2026", true)
        );
        let open = s.open_event(Path::new(&recent[0].path)).unwrap();
        assert_eq!(open.snapshot.event.details.name, "Sommerfest 2026");
    }

    #[test]
    fn missing_recent_files_are_marked_and_can_be_forgotten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("old.lancelot");
        let mut s = service(&dir);
        s.create_event(&path, details("Old")).unwrap();
        s.close_file();
        std::fs::remove_file(&path).unwrap();
        assert!(!s.recent_events()[0].exists);
        assert!(s.forget_recent(&path).unwrap().is_empty());
        assert_eq!(s.open_event(&path).unwrap_err().code, "io");
    }

    #[test]
    fn errors_carry_codes_and_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("e.lancelot");
        let mut s = service(&dir);
        let err = s.create_event(&path, details("  ")).unwrap_err();
        assert_eq!(
            (err.code.as_str(), err.field.as_deref()),
            ("empty_name", Some("name"))
        );
        s.create_event(&path, details("E")).unwrap();
        assert_eq!(
            s.create_event(&path, details("E")).unwrap_err().code,
            "already_exists"
        );

        // Another app instance on the same file.
        let mut other = service(&dir);
        s.open_event(&path).unwrap();
        assert_eq!(other.open_event(&path).unwrap_err().code, "already_open");
    }

    #[test]
    fn settings_persist_and_drive_input() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        let mut settings = s.settings();
        settings.number_format = NumberFormat::De;
        settings.backup_count = 5;
        s.update_settings(settings.clone()).unwrap();
        assert_eq!(service(&dir).settings(), settings);

        assert_eq!(s.parse_amount("1.234", false).unwrap().cents.get(), 123_400);
        settings.number_format = NumberFormat::En;
        s.update_settings(settings.clone()).unwrap();
        assert_eq!(
            s.parse_amount("1.234", false).unwrap_err().code,
            "too_many_decimals"
        );
        assert_eq!(s.parse_date("13.6.26").unwrap().date, "2026-06-13");

        settings.backup_count = 0;
        let err = s.update_settings(settings).unwrap_err();
        assert_eq!(
            (err.code.as_str(), err.field.as_deref()),
            ("invalid_setting", Some("backupCount"))
        );
    }

    #[test]
    fn restores_a_backup_as_a_copy() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("e.lancelot");
        let mut s = service(&dir);
        s.create_event(&path, details("Before")).unwrap();
        let backups = s.backups(&path).unwrap();
        assert_eq!(backups.len(), 1);
        assert!(backups[0].taken_at.is_some());
        s.change(|b| b.update_event(details("After"))).unwrap();

        let backup = PathBuf::from(&backups[0].path);
        let copy = dir.path().join("restored.lancelot");
        assert_eq!(
            s.restore_backup(&path, &backup, &path).unwrap_err().code,
            "already_exists"
        );
        assert_eq!(
            s.restore_backup(&path, &path, &copy).unwrap_err().code,
            "not_a_backup"
        );

        let restored = s.restore_backup(&path, &backup, &copy).unwrap();
        assert_eq!(restored.snapshot.event.details.name, "Before");
        assert_eq!(restored.file_name, "restored.lancelot");
        // The original is untouched and free again.
        s.close_file();
        assert_eq!(
            s.open_event(&path).unwrap().snapshot.event.details.name,
            "After"
        );
    }

    #[test]
    fn a_damaged_file_can_be_restored_from_its_backups() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("damaged.lancelot");
        let mut s = service(&dir);
        s.create_event(&path, details("Winterfest")).unwrap();
        s.close_file();

        // Break the file behind Lancelot's back: an empty transaction that is sealed.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF; INSERT INTO line (txn_id, position, account_id, amount) VALUES (99, 0, 1, 5);",
        )
        .unwrap();
        drop(conn);
        let err = s.open_event(&path).unwrap_err();
        assert_eq!(err.code, "damaged");
        assert!(err.report.is_some());

        let backups = s.backups(&path).unwrap();
        assert_eq!(backups.len(), 1);
        let copy = dir.path().join("restored.lancelot");
        let open = s
            .restore_backup(&path, Path::new(&backups[0].path), &copy)
            .unwrap();
        assert_eq!(open.snapshot.event.details.name, "Winterfest");
    }

    #[test]
    fn enters_and_edits_transactions() {
        use lancelot_core::{Entry, SplitLine};
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("e.lancelot");
        let mut s = service(&dir);
        let open = s.create_event(&path, details("E")).unwrap();
        let category = |name: &str| {
            open.snapshot
                .accounts
                .iter()
                .find(|a| a.account.name == name)
                .unwrap()
                .account
                .id
        };
        let kitchen = category("Kitchen");

        let anna = s
            .add(|b| b.add_person("Anna", Person::default()))
            .unwrap()
            .id;
        let mobility = s
            .add(|b| b.add_account(AccountKind::Expense, "Transport"))
            .unwrap()
            .id;
        s.change(|b| b.rename_account(mobility, "Fuel")).unwrap();
        assert_eq!(
            s.add(|b| b.add_person("anna", Person::default()))
                .unwrap_err()
                .code,
            "duplicate_name"
        );

        let cents = |c: i64| lancelot_core::Cents::new(c).unwrap();
        let draft = TxnDraft {
            date: time::macros::date!(2026 - 06 - 13),
            description: "Metro".into(),
            counterparty: None,
            notes: None,
            entry: Entry::Expense {
                payer: anna,
                lines: vec![
                    SplitLine {
                        account: kitchen,
                        amount: cents(24_890),
                        memo: None,
                        budget_line: None,
                        cost_centre: None,
                    },
                    SplitLine {
                        account: mobility,
                        amount: cents(6_350),
                        memo: None,
                        budget_line: None,
                        cost_centre: None,
                    },
                ],
            },
            total: Some(cents(31_240)),
        };
        let saved = s.save_txn(&draft, None, &[], &[]).unwrap();
        let txn = &saved.event.snapshot.txns[0];
        assert_eq!((txn.amount.get(), txn.txn.lines.len()), (31_240, 3));
        let person = &saved.event.snapshot.persons[0];
        assert_eq!(person.summary.net.get(), 31_240);

        let mut wrong = draft.clone();
        wrong.total = Some(cents(1));
        let err = s.save_txn(&wrong, Some(saved.id), &[], &[]).unwrap_err();
        assert_eq!(
            (err.code.as_str(), err.field.as_deref()),
            ("split_mismatch", Some("lines"))
        );

        let after = s.change(|b| b.delete_txn(saved.id)).unwrap();
        assert!(after.snapshot.txns.is_empty());
    }

    #[test]
    fn attaches_files_from_disk_and_hands_them_out() {
        use lancelot_core::{Entry, SplitLine};
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("e.lancelot");
        let mut s = service(&dir);
        let open = s.create_event(&path, details("E")).unwrap();
        let id_of = |name: &str| {
            open.snapshot
                .accounts
                .iter()
                .find(|a| a.account.name == name)
                .unwrap()
                .account
                .id
        };
        let receipt = dir.path().join("metro.pdf");
        std::fs::write(&receipt, b"%PDF-1.7 receipt").unwrap();
        let draft = TxnDraft {
            date: time::macros::date!(2026 - 06 - 13),
            description: "Metro".into(),
            counterparty: None,
            notes: None,
            entry: Entry::Expense {
                payer: id_of("Cash box"),
                lines: vec![SplitLine {
                    account: id_of("Kitchen"),
                    amount: lancelot_core::Cents::new(500).unwrap(),
                    memo: None,
                    budget_line: None,
                    cost_centre: None,
                }],
            },
            total: None,
        };
        let saved = s
            .save_txn(&draft, None, std::slice::from_ref(&receipt), &[])
            .unwrap();
        let attachment = &saved.event.snapshot.txns[0].attachments[0];
        assert_eq!(
            (attachment.filename.as_str(), attachment.mime.as_str()),
            ("metro.pdf", "application/pdf")
        );

        let (mime, bytes) = s.attachment(attachment.id).unwrap();
        assert_eq!(
            (mime.as_str(), bytes.as_slice()),
            ("application/pdf", b"%PDF-1.7 receipt".as_slice())
        );
        let temp = s.attachment_temp(attachment.id).unwrap();
        assert_eq!(temp.file_name().unwrap(), "metro.pdf");
        let export = dir.path().join("copy.pdf");
        s.export_attachment(attachment.id, &export).unwrap();
        assert_eq!(std::fs::read(export).unwrap(), b"%PDF-1.7 receipt");

        let missing = dir.path().join("gone.pdf");
        assert_eq!(
            s.save_txn(&draft, Some(saved.id), &[missing], &[])
                .unwrap_err()
                .code,
            "io"
        );
        let cleared = s
            .save_txn(&draft, Some(saved.id), &[], &[attachment.id])
            .unwrap();
        assert!(cleared.event.snapshot.txns[0].attachments.is_empty());
    }

    #[test]
    fn updates_people_and_checks_ibans() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        s.create_event(&dir.path().join("e.lancelot"), details("E"))
            .unwrap();
        let anna = s
            .add(|b| b.add_person("Anna", Person::default()))
            .unwrap()
            .id;

        let checked = s.check_iban("de89 3704 0044 0532 0130 00").unwrap();
        assert_eq!(checked.formatted, "DE89 3704 0044 0532 0130 00");
        assert_eq!(
            s.check_iban("DE88370400440532013000").unwrap_err().code,
            "bad_checksum"
        );

        let person = Person {
            holder: Some("Anna Krüger".into()),
            iban: Some(lancelot_core::validate_iban(&checked.iban).unwrap()),
            ..Person::default()
        };
        let open = s
            .change(|b| b.update_person(anna, "Anna Krüger", person))
            .unwrap();
        let p = &open.snapshot.persons[0];
        assert_eq!(
            (p.name.as_str(), p.details.iban.as_ref().map(|i| i.as_str())),
            ("Anna Krüger", Some("DE89370400440532013000"))
        );
    }

    #[test]
    fn adds_edits_and_deletes_budget_lines() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        let open = s
            .create_event(&dir.path().join("e.lancelot"), details("E"))
            .unwrap();
        let kitchen = open
            .snapshot
            .accounts
            .iter()
            .find(|a| a.account.name == "Kitchen")
            .unwrap()
            .account
            .id;
        let draft = |amount| lancelot_core::BudgetLineDraft {
            cost_centre: None,
            category: kitchen,
            description: "Food".into(),
            estimate: lancelot_core::Estimate::Amount {
                amount: lancelot_core::Cents::new(amount).unwrap(),
            },
            complete: false,
        };

        let open = s.change(|b| b.add_budget_line(draft(100_000))).unwrap();
        let id = open.snapshot.budget_lines[0].id;
        assert_eq!(open.snapshot.budget.expense.forecast.get(), 100_000);
        let open = s
            .change(|b| b.update_budget_line(id, draft(120_000)))
            .unwrap();
        assert_eq!(open.snapshot.budget.expense.estimate.get(), 120_000);
        let err = s
            .change(|b| {
                b.add_budget_line(lancelot_core::BudgetLineDraft {
                    cost_centre: None,
                    description: " ".into(),
                    ..draft(1)
                })
            })
            .unwrap_err();
        assert_eq!(
            (err.code.as_str(), err.field.as_deref()),
            ("empty_name", Some("description"))
        );
        let open = s.change(|b| b.delete_budget_line(id)).unwrap();
        assert!(open.snapshot.budget_lines.is_empty());
    }

    #[test]
    fn imports_ticket_sheets_and_records_manual_sales() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        s.create_event(&dir.path().join("e.lancelot"), details("E"))
            .unwrap();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tickets");
        let tier = |name: &str, price| TierDraft {
            name: name.into(),
            price: lancelot_core::Cents::new(price).unwrap(),
            fee_bp: 0,
            quota: None,
            aliases: Vec::new(),
        };
        s.change(|b| b.add_tier(tier("Early Bird", 2_500))).unwrap();

        // Regular is not a tier yet: the preview names it and the import is refused.
        let june = |d| time::Date::from_calendar_date(2026, time::Month::June, d).unwrap();
        let sheet = fixtures.join("import-1.ods");
        let preview = s.preview_tickets(&sheet, june(1)).unwrap();
        assert_eq!(preview.unknown[0].name, "Regular");
        let err = s
            .import_tickets(&sheet, june(1), &preview.sha256)
            .unwrap_err();
        assert_eq!(err.code, "unknown_tiers");

        let open = s.change(|b| b.add_tier(tier("Regular", 3_200))).unwrap();
        let regular = open.snapshot.tickets.tiers[1].tier.id;
        s.change(|b| b.add_tier(tier("Supporter", 4_500))).unwrap();
        let preview = s.preview_tickets(&sheet, june(1)).unwrap();
        assert!(preview.unknown.is_empty());
        // A file that is not the one previewed is not imported.
        let err = s.import_tickets(&sheet, june(1), "0000").unwrap_err();
        assert_eq!(err.code, "file_changed");
        let open = s.import_tickets(&sheet, june(1), &preview.sha256).unwrap();
        let tickets = &open.snapshot.tickets;
        assert_eq!(tickets.imports.len(), 1);
        assert_eq!(tickets.total.sheet, preview.tickets);
        assert!(
            open.snapshot
                .txns
                .iter()
                .all(|t| t.txn.origin.is_generated())
        );
        let err = s.change(|b| b.delete_tier(regular)).unwrap_err();
        assert_eq!(err.code, "tier_in_use");

        let import = tickets.imports[0].import.id;
        let open = s.change(|b| b.set_import_date(import, june(2))).unwrap();
        assert_eq!(open.snapshot.tickets.imports[0].import.date, june(2));
        assert!(open.snapshot.txns.iter().all(|t| t.txn.date == june(2)));

        let bob = s
            .add(|b| b.add_person("Bob", Person::default()))
            .unwrap()
            .id;
        let sale = |count| ManualSaleDraft {
            tier: regular,
            count,
            person: bob,
            date: june(13),
            reference: Some(" Abendkasse ".into()),
        };
        let open = s.change(|b| b.add_manual_sale(sale(2))).unwrap();
        let view = &open.snapshot.tickets.manual_sales[0];
        assert_eq!(view.gross.get(), 6_400);
        assert_eq!(view.sale.draft.reference.as_deref(), Some("Abendkasse"));
        let id = view.sale.id;
        let open = s.change(|b| b.update_manual_sale(id, sale(3))).unwrap();
        assert_eq!(open.snapshot.tickets.total.manual, 3);
        let open = s.change(|b| b.delete_manual_sale(id)).unwrap();
        assert!(open.snapshot.tickets.manual_sales.is_empty());

        let open = s
            .change(|b| {
                b.set_ticket_columns(TicketColumns {
                    tier: "Kategorie".into(),
                    order: "Order ID".into(),
                    sale_date: None,
                })
            })
            .unwrap();
        assert_eq!(open.snapshot.ticket_columns.tier, "Kategorie");
        assert!(s.preview_tickets(&sheet, june(9)).is_err());
    }

    #[test]
    fn resolves_ticket_conflicts_and_records_cancellations() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        s.create_event(&dir.path().join("e.lancelot"), details("E"))
            .unwrap();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tickets");
        let tier = |name: &str, price| TierDraft {
            name: name.into(),
            price: lancelot_core::Cents::new(price).unwrap(),
            fee_bp: 0,
            quota: None,
            aliases: Vec::new(),
        };
        s.change(|b| b.add_tier(tier("Early Bird", 2_500))).unwrap();
        let open = s.change(|b| b.add_tier(tier("Regular", 3_200))).unwrap();
        let regular = open.snapshot.tickets.tiers[1].tier.id;
        s.change(|b| b.add_tier(tier("Supporter", 4_500))).unwrap();
        let june = |d| time::Date::from_calendar_date(2026, time::Month::June, d).unwrap();
        let import = |s: &mut Service, name: &str, day| {
            let path = fixtures.join(name);
            let sha = s.preview_tickets(&path, june(day)).unwrap().sha256;
            s.import_tickets(&path, june(day), &sha).unwrap()
        };
        import(&mut s, "import-1.ods", 1);
        let terms = RefundTerms {
            refund_bp: 5_000,
            date: june(3),
            payer: None,
        };

        // Recorded while the order is still in the sheet.
        let open = s
            .change(|b| {
                b.add_cancellation(CancellationDraft {
                    source: CancellationSource::Order {
                        order: "4711".into(),
                        tier: regular,
                    },
                    count: 1,
                    terms,
                    note: None,
                })
            })
            .unwrap();
        let id = open.snapshot.tickets.cancellations[0].cancellation.id;
        assert!(open.snapshot.tickets.conflicts.is_empty());
        let err = s
            .change(|b| {
                b.add_cancellation(CancellationDraft {
                    source: CancellationSource::Order {
                        order: "nope".into(),
                        tier: regular,
                    },
                    count: 1,
                    terms,
                    note: None,
                })
            })
            .unwrap_err();
        assert_eq!(err.code, "unknown_order");
        assert_eq!(err.field.as_deref(), Some("order"));

        // Orders 4702 and 4711 leave; the cancellation waits on 4711.
        let open = import(&mut s, "import-2.ods", 2);
        let conflicts = &open.snapshot.tickets.conflicts;
        assert_eq!(conflicts[1].order, "4711");
        assert_eq!(conflicts[1].removed[0].matches[0].cancellation, id);
        assert!(
            open.snapshot
                .open_items
                .contains(&OpenItem::TicketConflicts { count: 2 })
        );

        let open = s
            .change(|b| {
                b.resolve_conflict(
                    "4711",
                    regular,
                    Resolution::Match {
                        cancellation: id,
                        count: 1,
                    },
                )
            })
            .unwrap();
        assert_eq!(open.snapshot.tickets.total.cancelled, 1);
        let open = s
            .change(|b| {
                b.resolve_conflict(
                    "4711",
                    regular,
                    Resolution::Correction {
                        count: 1,
                        note: None,
                    },
                )
            })
            .unwrap();
        let correction = open.snapshot.tickets.cancellations[1].cancellation.id;
        let open = s
            .change(|b| {
                b.resolve_conflict(
                    "4702",
                    regular,
                    Resolution::Refund {
                        count: 1,
                        terms,
                        note: Some("called in".into()),
                    },
                )
            })
            .unwrap();
        assert!(open.snapshot.tickets.conflicts.is_empty());
        assert_eq!(open.snapshot.tickets.total.refunds.get(), 3_200);

        let err = s
            .change(|b| {
                b.update_cancellation(
                    correction,
                    CancellationEdit {
                        count: 1,
                        terms,
                        note: None,
                    },
                )
            })
            .unwrap_err();
        assert_eq!(err.code, "correction_not_editable");
        let open = s
            .change(|b| {
                b.update_cancellation(
                    id,
                    CancellationEdit {
                        count: 1,
                        terms: RefundTerms {
                            refund_bp: 10_000,
                            ..terms
                        },
                        note: None,
                    },
                )
            })
            .unwrap();
        assert_eq!(open.snapshot.tickets.total.refunds.get(), 4_800);
        let open = s.change(|b| b.delete_cancellation(correction)).unwrap();
        assert_eq!(open.snapshot.tickets.conflicts.len(), 1);
    }

    #[test]
    fn exports_the_payout_list_and_marks_it_paid() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        let mut settings = s.settings();
        settings.number_format = lancelot_core::NumberFormat::De;
        s.update_settings(settings).unwrap();
        let open = s
            .create_event(&dir.path().join("e.lancelot"), details("Fest"))
            .unwrap();
        let id_of = |name: &str| {
            open.snapshot
                .accounts
                .iter()
                .find(|a| a.account.name == name)
                .unwrap()
                .account
                .id
        };
        let (verein, kitchen) = (id_of("Verein account"), id_of("Kitchen"));
        let anna = s
            .add(|b| b.add_person("Anna Krüger", Person::default()))
            .unwrap()
            .id;
        let person = Person {
            iban: Some(lancelot_core::validate_iban("DE89370400440532013000").unwrap()),
            ..Person::default()
        };
        s.change(|b| b.update_person(anna, "Anna Krüger", person))
            .unwrap();
        let draft = TxnDraft {
            date: time::macros::date!(2026 - 06 - 13),
            description: "Metro".into(),
            counterparty: None,
            notes: None,
            entry: lancelot_core::Entry::Expense {
                payer: anna,
                lines: vec![lancelot_core::SplitLine {
                    account: kitchen,
                    amount: lancelot_core::Cents::new(21_240).unwrap(),
                    memo: None,
                    budget_line: None,
                    cost_centre: None,
                }],
            },
            total: None,
        };
        s.save_txn(&draft, None, &[], &[]).unwrap();

        let target = dir.path().join("payouts.csv");
        s.export_payouts_csv(&target).unwrap();
        let csv = std::fs::read_to_string(&target).unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert!(
            csv.contains(
                "Anna Krüger;Anna Krüger;DE89370400440532013000;;212,40;Auslagen Fest Anna Krueger"
            ),
            "{csv}"
        );

        let paid = s
            .change(|b| b.mark_payouts_paid(verein, time::macros::date!(2026 - 06 - 20)))
            .unwrap();
        assert!(paid.snapshot.payouts.payouts.is_empty());
        assert_eq!(paid.snapshot.persons[0].summary.net.get(), 0);
    }

    #[test]
    fn adds_cash_accounts_and_saves_both_counts_at_once() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        s.create_event(&dir.path().join("e.lancelot"), details("Fest"))
            .unwrap();
        let bar = s.add(|b| b.add_asset("Bar till", AssetType::Cash)).unwrap();
        let account = bar
            .event
            .snapshot
            .accounts
            .iter()
            .find(|a| a.account.id == bar.id)
            .unwrap();
        assert_eq!(account.account.asset_type, Some(AssetType::Cash));

        let count = |date: &str, cents| CountInput {
            date: date.into(),
            amount: lancelot_core::Cents::new(cents).unwrap(),
        };
        let open = s
            .set_counts(
                bar.id,
                Some(count("12.06.2026", 5_000)),
                Some(count("2026-06-14", 4_800)),
            )
            .unwrap();
        let status = open
            .snapshot
            .cash
            .iter()
            .find(|c| c.account == bar.id)
            .unwrap();
        assert_eq!(status.opening.map(|o| o.amount.get()), Some(5_000));
        assert_eq!(status.difference.map(|d| d.get()), Some(-200));

        let err = s
            .set_counts(bar.id, Some(count("someday", 5_000)), None)
            .unwrap_err();
        assert_eq!(err.field.as_deref(), Some("opening.date"));

        let open = s.set_counts(bar.id, None, None).unwrap();
        let status = open
            .snapshot
            .cash
            .iter()
            .find(|c| c.account == bar.id)
            .unwrap();
        assert_eq!((status.opening, status.counted), (None, None));
        assert!(open.snapshot.txns.is_empty());

        let open = s
            .change(|b| b.set_asset_type(bar.id, AssetType::Bank))
            .unwrap();
        assert!(open.snapshot.cash.iter().all(|c| c.account != bar.id));
    }

    #[test]
    fn closes_with_notes_reopens_and_exports_the_transactions() {
        let dir = TempDir::new().unwrap();
        let mut s = service(&dir);
        let open = s
            .create_event(&dir.path().join("e.lancelot"), details("Fest"))
            .unwrap();
        let cash = open
            .snapshot
            .accounts
            .iter()
            .find(|a| a.account.name == "Cash box")
            .unwrap()
            .account
            .id;

        let err = s.close_event(vec![]).unwrap_err();
        assert_eq!(err.code, "closing_blocked");
        let item = lancelot_core::OpenItem::ClosingCountMissing { account: cash };
        let accepted = BlockerOverride {
            item,
            note: "Nobody used the box".into(),
        };
        let closed = s.close_event(vec![accepted.clone()]).unwrap();
        assert!(closed.snapshot.event.closed_at.is_some());
        assert_eq!(closed.snapshot.event.closing_overrides, [accepted]);
        assert_eq!(
            s.add(|b| b.add_person("Kim", Person::default()))
                .unwrap_err()
                .code,
            "event_closed"
        );

        let target = dir.path().join("entries.csv");
        s.export_transactions_csv(&target).unwrap();
        let csv = std::fs::read_to_string(&target).unwrap();
        assert!(csv.starts_with("\u{feff}Date,Entry,Type,"), "{csv}");

        let reopened = s.change(|b| b.reopen()).unwrap();
        assert!(reopened.snapshot.event.closed_at.is_none());
        s.add(|b| b.add_person("Kim", Person::default())).unwrap();
    }
}
