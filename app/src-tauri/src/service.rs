//! What the commands do, without Tauri, so it can be tested on its own.
//!
//! One window holds at most one open event. Opening or creating another closes the current
//! one first, which releases its lock.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use lancelot_core::{
    AccountId, AccountKind, BackupPolicy, EventDetails, EventSnapshot, Parsed, Person, Report,
    Store, StoreOptions, TxnDraft, TxnId, backup_time, event_uuid_of, format_date, list_backups,
    parse_amount, parse_date,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::api::{Added, ApiError, BackupInfo, OpenEvent, ParsedDate, RecentEvent, SavedTxn};
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

    /// Runs a command on the open store and returns the new state.
    fn change(&mut self, command: impl FnOnce(&mut Store) -> Result<()>) -> Result<OpenEvent> {
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

    pub fn update_event(&mut self, details: EventDetails) -> Result<OpenEvent> {
        self.change(|s| Ok(s.update_event(details)?))
    }

    pub fn set_default_reimbursement_account(&mut self, account: AccountId) -> Result<OpenEvent> {
        self.change(|s| Ok(s.set_default_reimbursement_account(account)?))
    }

    pub fn save_txn(&mut self, draft: &TxnDraft, id: Option<TxnId>) -> Result<SavedTxn> {
        let mut saved = None;
        let event = self.change(|s| {
            saved = Some(s.save_txn(draft, id)?);
            Ok(())
        })?;
        let saved = saved.ok_or_else(ApiError::no_event)?;
        Ok(SavedTxn {
            event,
            id: saved.id,
            warnings: saved.warnings,
        })
    }

    pub fn delete_txn(&mut self, id: TxnId) -> Result<OpenEvent> {
        self.change(|s| Ok(s.delete_txn(id)?))
    }

    /// Adds a category (income or expense) or an asset account.
    pub fn add_account(&mut self, kind: AccountKind, name: &str) -> Result<Added> {
        let mut id = None;
        let event = self.change(|s| {
            id = Some(s.add_account(kind, name)?);
            Ok(())
        })?;
        Ok(Added {
            event,
            id: id.ok_or_else(ApiError::no_event)?,
        })
    }

    pub fn rename_account(&mut self, id: AccountId, name: &str) -> Result<OpenEvent> {
        self.change(|s| Ok(s.rename_account(id, name)?))
    }

    /// Adds a person by name; bank details follow on the People screen (M4).
    pub fn add_person(&mut self, name: &str) -> Result<Added> {
        let mut id = None;
        let event = self.change(|s| {
            id = Some(s.add_person(name, Person::default())?);
            Ok(())
        })?;
        Ok(Added {
            event,
            id: id.ok_or_else(ApiError::no_event)?,
        })
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
    use lancelot_core::NumberFormat;
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
        let renamed = s.update_event(dated).unwrap();
        assert_eq!(renamed.revision, 2);
        assert_eq!(
            s.recent_events()[0].starts_on.as_deref(),
            Some("2026-06-12")
        );
        s.close_file();
        assert_eq!(
            s.update_event(details("x")).unwrap_err().code,
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
        s.update_event(details("After")).unwrap();

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

        let anna = s.add_person("Anna").unwrap().id;
        let mobility = s.add_account(AccountKind::Expense, "Transport").unwrap().id;
        s.rename_account(mobility, "Fuel").unwrap();
        assert_eq!(s.add_person("anna").unwrap_err().code, "duplicate_name");

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
                    },
                    SplitLine {
                        account: mobility,
                        amount: cents(6_350),
                        memo: None,
                    },
                ],
            },
            total: Some(cents(31_240)),
        };
        let saved = s.save_txn(&draft, None).unwrap();
        let txn = &saved.event.snapshot.txns[0];
        assert_eq!((txn.amount.get(), txn.txn.lines.len()), (31_240, 3));
        let person = &saved.event.snapshot.persons[0];
        assert_eq!(person.summary.net.get(), 31_240);

        let mut wrong = draft.clone();
        wrong.total = Some(cents(1));
        let err = s.save_txn(&wrong, Some(saved.id)).unwrap_err();
        assert_eq!(
            (err.code.as_str(), err.field.as_deref()),
            ("split_mismatch", Some("lines"))
        );

        let after = s.delete_txn(saved.id).unwrap();
        assert!(after.snapshot.txns.is_empty());
    }
}
