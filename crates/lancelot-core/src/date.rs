//! Date input and display (R-NF-6, dates).
//!
//! Dates are shown as `31.12.2026` (De) or `2026-12-31` (En). Input accepts both regardless of
//! the setting, because the separators make it unambiguous:
//!
//! - `D.M.YYYY` or `D.M.YY` (two-digit years are 20YY), with or without leading zeros;
//! - `YYYY-MM-DD`.
//!
//! Spaces around the input are ignored. The test cases live in `fixtures/date-input.json`.

use serde::Serialize;
use thiserror::Error;
use time::{Date, Month};

use crate::amount::NumberFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, Serialize)]
pub enum DateError {
    #[error("no date entered")]
    Empty,
    #[error("enter a date as 31.12.2026 or 2026-12-31")]
    Format,
    #[error("this day does not exist")]
    NoSuchDay,
}

impl DateError {
    /// Stable identifier for the UI and the fixture file.
    pub fn code(self) -> &'static str {
        match self {
            DateError::Empty => "empty",
            DateError::Format => "format",
            DateError::NoSuchDay => "no_such_day",
        }
    }
}

pub fn format_date(date: Date, format: NumberFormat) -> String {
    let (y, m, d) = (date.year(), u8::from(date.month()), date.day());
    match format {
        NumberFormat::De => format!("{d:02}.{m:02}.{y:04}"),
        NumberFormat::En => format!("{y:04}-{m:02}-{d:02}"),
    }
}

pub fn parse_date(input: &str) -> Result<Date, DateError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(DateError::Empty);
    }
    let numbers = |sep: char| -> Option<Vec<u32>> {
        let parts: Vec<&str> = input.split(sep).collect();
        let ok = parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.len() <= 4 && p.bytes().all(|b| b.is_ascii_digit()));
        ok.then(|| parts.iter().filter_map(|p| p.parse().ok()).collect())
    };
    let parts: Vec<&str> = input.split(['.', '-']).collect();
    let (year, month, day) = if let Some(n) = numbers('.') {
        let (d, m, y) = (n[0], n[1], n[2]);
        let year = match parts[2].len() {
            2 => 2000 + y,
            4 => y,
            _ => return Err(DateError::Format),
        };
        if parts[0].len() > 2 || parts[1].len() > 2 {
            return Err(DateError::Format);
        }
        (year, m, d)
    } else if let Some(n) = numbers('-') {
        if parts[0].len() != 4 || parts[1].len() > 2 || parts[2].len() > 2 {
            return Err(DateError::Format);
        }
        (n[0], n[1], n[2])
    } else {
        return Err(DateError::Format);
    };

    let month = u8::try_from(month)
        .ok()
        .and_then(|m| Month::try_from(m).ok())
        .ok_or(DateError::NoSuchDay)?;
    let day = u8::try_from(day).map_err(|_| DateError::NoSuchDay)?;
    let year = i32::try_from(year).map_err(|_| DateError::Format)?;
    Date::from_calendar_date(year, month, day).map_err(|_| DateError::NoSuchDay)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use time::macros::date;

    #[test]
    fn formats_both_styles() {
        assert_eq!(
            format_date(date!(2026 - 06 - 03), NumberFormat::De),
            "03.06.2026"
        );
        assert_eq!(
            format_date(date!(2026 - 06 - 03), NumberFormat::En),
            "2026-06-03"
        );
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Case {
        input: String,
        date: Option<String>,
        error: Option<String>,
    }

    #[test]
    fn fixture_cases() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../../fixtures/date-input.json")).unwrap();
        assert!(cases.len() >= 20);
        for case in cases {
            let got = parse_date(&case.input);
            match (case.date, case.error) {
                (Some(iso), None) => {
                    let d = got.unwrap_or_else(|e| panic!("{:?}: unexpected {e:?}", case.input));
                    assert_eq!(format_date(d, NumberFormat::En), iso, "{:?}", case.input);
                }
                (None, Some(code)) => assert_eq!(got.unwrap_err().code(), code, "{:?}", case.input),
                _ => panic!(
                    "{:?}: a case needs exactly one of `date` and `error`",
                    case.input
                ),
            }
        }
    }

    #[test]
    fn formatted_dates_parse_back() {
        let mut d = date!(2024 - 01 - 01);
        while d < date!(2026 - 12 - 31) {
            for f in [NumberFormat::De, NumberFormat::En] {
                assert_eq!(parse_date(&format_date(d, f)), Ok(d));
            }
            d = d.next_day().unwrap();
        }
    }
}
