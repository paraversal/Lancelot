//! Automatic backups of an event file (R-NF-5).
//!
//! Backups are consistent copies written with `VACUUM INTO` while the file stays open, into
//! `<dir>/<event uuid>/<UTC timestamp>.lancelot`. Only the newest `keep` are kept.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use time::{Duration, OffsetDateTime, UtcOffset};

use super::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupPolicy {
    /// The app's backup folder. Each event gets a subfolder named by its UUID.
    pub dir: PathBuf,
    /// How many backups to keep per event.
    pub keep: usize,
    /// After a change, a new backup is written only if the last one is at least this old.
    pub interval: Duration,
}

impl BackupPolicy {
    /// The defaults: keep 20, at most one every 10 minutes after changes.
    pub fn new(dir: impl Into<PathBuf>) -> BackupPolicy {
        BackupPolicy {
            dir: dir.into(),
            keep: 20,
            interval: Duration::minutes(10),
        }
    }
}

const EXTENSION: &str = "lancelot";

fn stamp(at: OffsetDateTime) -> String {
    let at = at.to_offset(UtcOffset::UTC);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}{:03}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
        at.millisecond()
    )
}

/// Writes a backup now and prunes old ones. Returns the new backup's path.
pub(crate) fn write(
    conn: &Connection,
    policy: &BackupPolicy,
    uuid: &str,
    now: OffsetDateTime,
) -> Result<PathBuf, StoreError> {
    let dir = policy.dir.join(uuid);
    std::fs::create_dir_all(&dir)?;
    let mut path = dir.join(format!("{}.{EXTENSION}", stamp(now)));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{}-{n}.{EXTENSION}", stamp(now)));
        n += 1;
    }
    let target = path
        .to_str()
        .ok_or_else(|| StoreError::corrupt("backup path is not valid UTF-8"))?;
    conn.execute("VACUUM INTO ?1", [target])?;

    for old in list(&policy.dir, uuid)?
        .into_iter()
        .skip(policy.keep.max(1))
    {
        std::fs::remove_file(old)?;
    }
    Ok(path)
}

/// The backups of one event, newest first.
pub fn list(dir: &Path, uuid: &str) -> Result<Vec<PathBuf>, StoreError> {
    let dir = dir.join(uuid);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
        .collect();
    // Timestamps sort lexically; the suffix for same-millisecond backups sorts after.
    paths.sort();
    paths.reverse();
    Ok(paths)
}

/// When a backup was taken, read from its file name. `None` for files Lancelot did not name.
pub fn taken_at(path: &Path) -> Option<OffsetDateTime> {
    let stem = path.file_stem()?.to_str()?;
    let stamp = stem.split('-').next()?;
    let digits = |r: std::ops::Range<usize>| stamp.get(r)?.parse::<u32>().ok();
    if stamp.len() != 19 || stamp.as_bytes()[8] != b'T' || !stamp.ends_with('Z') {
        return None;
    }
    let month = time::Month::try_from(u8::try_from(digits(4..6)?).ok()?).ok()?;
    let date = time::Date::from_calendar_date(
        i32::try_from(digits(0..4)?).ok()?,
        month,
        u8::try_from(digits(6..8)?).ok()?,
    )
    .ok()?;
    let time = time::Time::from_hms_milli(
        u8::try_from(digits(9..11)?).ok()?,
        u8::try_from(digits(11..13)?).ok()?,
        u8::try_from(digits(13..15)?).ok()?,
        u16::try_from(digits(15..18)?).ok()?,
    )
    .ok()?;
    Some(date.with_time(time).assume_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn file_names_carry_the_time() {
        let at = datetime!(2026-06-14 18:05:09.123 UTC);
        let name = format!("{}.lancelot", stamp(at));
        assert_eq!(name, "20260614T180509123Z.lancelot");
        assert_eq!(taken_at(Path::new(&name)), Some(at));
        assert_eq!(
            taken_at(Path::new("20260614T180509123Z-1.lancelot")),
            Some(at)
        );
        assert_eq!(taken_at(Path::new("notes.lancelot")), None);
    }
}
