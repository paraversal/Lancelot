//! App-wide settings and the recent events list, stored as `settings.json` in the app's data
//! folder (`docs/technical-design.md` §6.4). Event settings live in the event file instead.

use std::path::{Path, PathBuf};

use lancelot_core::NumberFormat;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many recent events the start screen lists.
pub const RECENT_LIMIT: usize = 10;

/// The settings the user edits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub number_format: NumberFormat,
    /// Backups kept per event (R-NF-5).
    pub backup_count: u32,
    /// Reduced motion (`docs/design/motion.md`): `None` follows macOS's "reduce motion",
    /// `Some` overrides it.
    #[serde(default)]
    pub reduced_motion: Option<bool>,
}

impl AppSettings {
    pub const BACKUP_COUNT_RANGE: std::ops::RangeInclusive<u32> = 1..=100;
}

impl Default for AppSettings {
    fn default() -> AppSettings {
        AppSettings {
            number_format: format_for_locale(sys_locale::get_locale().as_deref()),
            backup_count: 20,
            reduced_motion: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    pub path: PathBuf,
    pub name: String,
    /// ISO dates, shown next to the name on the start screen.
    #[serde(default)]
    pub starts_on: Option<String>,
    #[serde(default)]
    pub ends_on: Option<String>,
    /// RFC 3339 UTC.
    pub last_opened: String,
}

/// Everything in `settings.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsFile {
    #[serde(flatten)]
    pub settings: AppSettings,
    pub recent: Vec<RecentEntry>,
}

impl SettingsFile {
    /// Reads the file. A missing or unreadable file gives the defaults, so a damaged settings
    /// file can never keep the app from starting.
    pub fn load(path: &Path) -> SettingsFile {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the file atomically: to a temporary file next to it, then renamed into place.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }

    /// Moves an event to the top of the recent list.
    pub fn remember(&mut self, entry: RecentEntry) {
        self.recent.retain(|r| r.path != entry.path);
        self.recent.insert(0, entry);
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn forget(&mut self, path: &Path) {
        self.recent.retain(|r| r.path != path);
    }
}

/// `1.234,56 €` for languages that write a decimal comma, `€1,234.56` otherwise.
pub fn format_for_locale(locale: Option<&str>) -> NumberFormat {
    const DECIMAL_COMMA: &[&str] = &[
        "de", "fr", "es", "it", "nl", "pt", "da", "sv", "nb", "nn", "no", "fi", "is", "pl", "cs",
        "sk", "sl", "hr", "hu", "ro", "bg", "el", "tr", "ru", "uk", "lt", "lv", "et",
    ];
    let language = locale
        .unwrap_or("en")
        .split(['-', '_'])
        .next()
        .unwrap_or("en")
        .to_ascii_lowercase();
    if DECIMAL_COMMA.contains(&language.as_str()) {
        NumberFormat::De
    } else {
        NumberFormat::En
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str) -> RecentEntry {
        RecentEntry {
            path: PathBuf::from(path),
            name: name.into(),
            starts_on: Some("2026-06-12".into()),
            ends_on: None,
            last_opened: "2026-06-14T18:00:00Z".into(),
        }
    }

    #[test]
    fn locale_decides_the_default_format() {
        assert_eq!(format_for_locale(Some("de-DE")), NumberFormat::De);
        assert_eq!(format_for_locale(Some("fr_CH")), NumberFormat::De);
        assert_eq!(format_for_locale(Some("en-GB")), NumberFormat::En);
        assert_eq!(format_for_locale(Some("ja-JP")), NumberFormat::En);
        assert_eq!(format_for_locale(None), NumberFormat::En);
    }

    #[test]
    fn saves_and_loads() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let mut file = SettingsFile::default();
        file.settings.number_format = NumberFormat::De;
        file.settings.backup_count = 7;
        file.remember(entry("/a.lancelot", "A"));
        file.save(&path).unwrap();
        assert_eq!(SettingsFile::load(&path), file);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn an_older_file_follows_the_systems_motion() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"numberFormat":"de","backupCount":7,"recent":[]}"#,
        )
        .unwrap();
        let file = SettingsFile::load(&path);
        assert_eq!(file.settings.backup_count, 7);
        assert_eq!(file.settings.reduced_motion, None);
    }

    #[test]
    fn a_damaged_file_gives_the_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(SettingsFile::load(&path), SettingsFile::default());
        assert_eq!(
            SettingsFile::load(&dir.path().join("missing.json")),
            SettingsFile::default()
        );
    }

    #[test]
    fn recent_list_is_ordered_unique_and_short() {
        let mut file = SettingsFile::default();
        for i in 0..12 {
            file.remember(entry(&format!("/{i}.lancelot"), &format!("E{i}")));
        }
        file.remember(entry("/5.lancelot", "Renamed"));
        assert_eq!(file.recent.len(), RECENT_LIMIT);
        assert_eq!(file.recent[0].name, "Renamed");
        assert_eq!(
            file.recent
                .iter()
                .filter(|r| r.path == Path::new("/5.lancelot"))
                .count(),
            1
        );
        file.forget(Path::new("/5.lancelot"));
        assert_eq!(file.recent[0].name, "E11");
    }
}
