//! Reads a table from a file: `.ods`, `.xlsx` or `.csv`, first sheet, for a plugin to make sense of
//! (technical design §10).
//!
//! Only the columns asked for are taken from each row (R-TKT-17). Names, e-mail addresses and
//! everything else in the file never leave this module. A row that holds a spreadsheet error is
//! listed, never dropped silently (R-TKT-4).

use std::io::Cursor;

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::amount::NumberFormat;
use crate::error::DomainError;
use crate::tickets::{BatchTicket, TicketBatch};

/// The columns asked for of a file, nothing else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Table {
    /// The file's name.
    pub source: String,
    /// SHA-256 of the file, lower-case hex.
    pub sha256: String,
    /// The rows below the header. Blank rows are left out.
    pub rows: Vec<TableRow>,
    /// The numbers of the rows with a spreadsheet error, such as `#REF!`, in a column asked for.
    pub errors: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TableRow {
    /// The row number as the spreadsheet shows it, starting at 1.
    pub row: u32,
    /// In the order of the columns asked for, trimmed. An empty cell is an empty string. A number
    /// is written in `format`, so the amount parser reads it as it stands; a date is ISO.
    pub cells: Vec<String>,
}

/// Reads these columns of a file. The format follows the file name's extension; anything that is
/// not `.csv` is tried as a spreadsheet. The header is the first row holding all the columns, so
/// a bank's lines above it don't matter; names are matched ignoring case.
pub fn read_table(
    filename: &str,
    data: &[u8],
    columns: &[String],
    format: NumberFormat,
) -> Result<Table, DomainError> {
    let cells = if filename.to_ascii_lowercase().ends_with(".csv") {
        csv_cells(data)?
    } else {
        spreadsheet_cells(data, format)?
    };
    let (rows, errors) = pick_columns(cells, columns)?;
    Ok(Table {
        source: filename.to_owned(),
        sha256: hex(&Sha256::digest(data)),
        rows,
        errors,
    })
}

impl TicketBatch {
    /// The tickets of a table read with the tier and order columns, without the rows missing
    /// either. The app's Ticket sheet plugin does this itself and shows those rows; this is for
    /// the example events and tests, which have no one to show them to.
    pub fn from_table(table: Table) -> TicketBatch {
        TicketBatch {
            source: table.source,
            sha256: table.sha256,
            tickets: table
                .rows
                .into_iter()
                .filter_map(|r| match <[String; 2]>::try_from(r.cells) {
                    Ok([tier, order]) if !tier.is_empty() && !order.is_empty() => {
                        Some(BatchTicket { tier, order })
                    }
                    _ => None,
                })
                .collect(),
        }
    }
}

/// A cell as text, or the error it holds.
type Cell = Result<String, ()>;

/// The rows of the first sheet, each with its row number.
fn spreadsheet_cells(
    data: &[u8],
    format: NumberFormat,
) -> Result<Vec<(u32, Vec<Cell>)>, DomainError> {
    let unreadable = |e: calamine::Error| DomainError::UnreadableSheet {
        reason: e.to_string(),
    };
    let mut book = open_workbook_auto_from_rs(Cursor::new(data)).map_err(unreadable)?;
    let range = book
        .worksheet_range_at(0)
        .ok_or(DomainError::UnreadableSheet {
            reason: "the file has no sheets".into(),
        })?
        .map_err(unreadable)?;
    let first = range.start().map_or(0, |(row, _)| row);
    let leading = range.start().map_or(0, |(_, col)| col as usize);
    Ok(range
        .rows()
        .zip(first..)
        .map(|(cells, row)| {
            // Cells left of the used range are empty, so column positions stay as in the sheet.
            let mut out = vec![Ok(String::new()); leading];
            out.extend(cells.iter().map(|c| match c {
                Data::Error(_) => Err(()),
                Data::Empty => Ok(String::new()),
                Data::Float(number) => Ok(match format {
                    NumberFormat::De => number.to_string().replace('.', ","),
                    NumberFormat::En => number.to_string(),
                }),
                Data::DateTime(date) if date.is_datetime() => {
                    let (y, m, d, hour, minute, second, _) = date.to_ymd_hms_milli();
                    Ok(if (hour, minute, second) == (0, 0, 0) {
                        format!("{y:04}-{m:02}-{d:02}")
                    } else {
                        format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}")
                    })
                }
                other => Ok(other.to_string().trim().to_owned()),
            }));
            (row + 1, out)
        })
        .collect())
}

/// The rows of a CSV file, separated by `;`, `,` or tabs, whichever the first lines use most.
/// UTF-8 with or without a byte order mark; anything else is read as Latin-1.
fn csv_cells(data: &[u8]) -> Result<Vec<(u32, Vec<Cell>)>, DomainError> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = match std::str::from_utf8(data) {
        Ok(text) => text.to_owned(),
        Err(_) => data.iter().map(|&b| char::from(b)).collect(),
    };
    let delimiter = b";,\t"
        .iter()
        .copied()
        .max_by_key(|d| {
            text.lines()
                .take(20)
                .map(|line| line.bytes().filter(|b| b == d).count())
                .sum::<usize>()
        })
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    // Rows are numbered by line, since the reader skips empty ones and a spreadsheet shows them.
    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| DomainError::UnreadableSheet {
            reason: e.to_string(),
        })?;
        let row = record.position().map_or(0, |p| p.line() as u32);
        out.push((
            row,
            record.iter().map(|c| Ok(c.trim().to_owned())).collect(),
        ));
    }
    Ok(out)
}

/// Finds the header and keeps only the cells of these columns from the rows below it.
fn pick_columns(
    cells: Vec<(u32, Vec<Cell>)>,
    columns: &[String],
) -> Result<(Vec<TableRow>, Vec<u32>), DomainError> {
    let blank = |cells: &[Cell]| cells.iter().all(|c| c.as_ref().is_ok_and(|c| c.is_empty()));
    let cells: Vec<_> = cells.into_iter().filter(|(_, c)| !blank(c)).collect();
    let Some((_, first)) = cells.first() else {
        return Err(DomainError::UnreadableSheet {
            reason: "the sheet is empty".into(),
        });
    };
    let find = |header: &[Cell], name: &str| {
        header.iter().position(|h| {
            h.as_ref()
                .is_ok_and(|h| h.eq_ignore_ascii_case(name.trim()))
        })
    };
    let holds_all = |header: &[Cell]| columns.iter().all(|c| find(header, c).is_some());
    let Some(at) = cells.iter().position(|(_, c)| holds_all(c)) else {
        let missing = columns.iter().find(|c| find(first, c).is_none());
        return Err(DomainError::MissingColumn {
            column: missing.cloned().unwrap_or_default(),
        });
    };
    let positions: Vec<usize> = columns
        .iter()
        .filter_map(|c| find(&cells[at].1, c))
        .collect();

    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (row, cells) in &cells[at + 1..] {
        let picked: Result<Vec<String>, ()> = positions
            .iter()
            .map(|&i| cells.get(i).cloned().unwrap_or(Ok(String::new())))
            .collect();
        match picked {
            Ok(cells) => rows.push(TableRow { row: *row, cells }),
            Err(()) => errors.push(*row),
        }
    }
    Ok((rows, errors))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<String> {
        vec!["Ticket".into(), "Order ID".into()]
    }

    fn read(csv: &str) -> Result<Table, DomainError> {
        read_table("tickets.csv", csv.as_bytes(), &columns(), NumberFormat::De)
    }

    fn cells(table: &Table) -> Vec<Vec<&str>> {
        table
            .rows
            .iter()
            .map(|r| r.cells.iter().map(String::as_str).collect())
            .collect()
    }

    #[test]
    fn reads_only_the_columns_asked_for_in_the_order_asked_for() {
        let table = read("Name;Order ID;E-Mail;Ticket\nAnna;4711;a@x.example;Regular\n").unwrap();
        assert_eq!(cells(&table), [["Regular", "4711"]]);
        assert_eq!(table.source, "tickets.csv");
        assert!(table.errors.is_empty());
    }

    #[test]
    fn finds_columns_ignoring_case_and_sniffs_the_delimiter() {
        let table = read("order id,TICKET\n 17 , Early Bird \n").unwrap();
        assert_eq!(cells(&table), [["Early Bird", "17"]]);
    }

    #[test]
    fn numbers_rows_and_keeps_empty_cells_but_not_blank_rows() {
        let table = read("Ticket;Order ID;Name\nRegular;;Anna\n;12;Bob\n;;\n;;Carla\n").unwrap();
        assert_eq!(
            table.rows.iter().map(|r| r.row).collect::<Vec<_>>(),
            [2, 3, 5]
        );
        assert_eq!(cells(&table), [["Regular", ""], ["", "12"], ["", ""]]);
    }

    #[test]
    fn finds_the_header_below_lines_that_are_not_the_table() {
        let bank = "Konto;DE12\nZeitraum;Oktober\n\nTicket;Order ID\nRegular;1\n";
        let table = read(bank).unwrap();
        assert_eq!(cells(&table), [["Regular", "1"]]);
        assert_eq!(table.rows[0].row, 5);
    }

    #[test]
    fn a_missing_column_is_named() {
        assert_eq!(
            read("Ticket;Order\nRegular;1\n"),
            Err(DomainError::MissingColumn {
                column: "Order ID".into()
            })
        );
    }

    #[test]
    fn hashes_the_file() {
        let table = read("Ticket;Order ID\n").unwrap();
        assert_eq!(table.sha256.len(), 64);
        assert_ne!(table.sha256, read("Ticket;Order ID\n\n").unwrap().sha256);
    }

    #[test]
    fn reads_latin_1_and_a_byte_order_mark() {
        let latin1 = b"Ticket;Order ID\nErm\xe4\xdfigt;1\n";
        let table = read_table("t.csv", latin1, &columns(), NumberFormat::De).unwrap();
        assert_eq!(cells(&table), [["Ermäßigt", "1"]]);
        let bom = read("\u{feff}Ticket;Order ID\nRegular;1\n").unwrap();
        assert_eq!(bom.rows.len(), 1);
    }

    #[test]
    fn rejects_files_that_are_not_spreadsheets() {
        let e = read_table("t.ods", b"not a zip", &columns(), NumberFormat::De).unwrap_err();
        assert_eq!(e.code(), "unreadable_sheet");
    }

    #[test]
    fn a_batch_leaves_out_rows_missing_a_tier_or_an_order() {
        let table = read("Ticket;Order ID\nRegular;1\nRegular;\n;2\n").unwrap();
        let batch = TicketBatch::from_table(table);
        assert_eq!(batch.source, "tickets.csv");
        assert_eq!(
            batch.tickets,
            [BatchTicket {
                tier: "Regular".into(),
                order: "1".into()
            }]
        );
    }
}
