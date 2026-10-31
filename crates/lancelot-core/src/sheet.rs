//! Reads a ticket sheet (R-TKT-3): `.ods`, `.xlsx` or `.csv`, first sheet, first row as the
//! header, one row per ticket.
//!
//! Only the configured columns are taken from each row (R-TKT-17). Names, e-mail addresses and
//! everything else in the sheet never leave this module. Rows that cannot be read are listed,
//! never dropped silently (R-TKT-4).

use std::io::Cursor;

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::error::DomainError;
use crate::tickets::{BatchTicket, TicketBatch};

/// The sheet's column names (R-TKT-3). Matched ignoring case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TicketColumns {
    pub tier: String,
    pub order: String,
}

impl Default for TicketColumns {
    fn default() -> TicketColumns {
        TicketColumns {
            tier: "Ticket".into(),
            order: "Order ID".into(),
        }
    }
}

/// The configured columns of a sheet, nothing else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Sheet {
    /// The tickets. The hash is the file's.
    pub batch: TicketBatch,
    pub problems: Vec<RowProblem>,
}

/// A row that was not read as a ticket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct RowProblem {
    /// The row number as the spreadsheet shows it, starting at 1 with the header.
    pub row: u32,
    pub problem: Problem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// The tier column is empty.
    NoTier,
    /// The order ID column is empty.
    NoOrder,
    /// A cell holds a spreadsheet error such as `#REF!`.
    CellError,
}

/// Reads the configured columns of a ticket sheet. The format follows the file name's
/// extension; anything that is not `.csv` is tried as a spreadsheet.
pub fn read_sheet(
    filename: &str,
    data: &[u8],
    columns: &TicketColumns,
) -> Result<Sheet, DomainError> {
    let cells = if filename.to_ascii_lowercase().ends_with(".csv") {
        csv_cells(data)?
    } else {
        spreadsheet_cells(data)?
    };
    let (tickets, problems) = pick_columns(cells, columns)?;
    Ok(Sheet {
        batch: TicketBatch {
            source: filename.to_owned(),
            sha256: hex(&Sha256::digest(data)),
            tickets,
        },
        problems,
    })
}

/// A cell as text, or the error it holds.
type Cell = Result<String, ()>;

/// The rows of the first sheet, each with its row number.
fn spreadsheet_cells(data: &[u8]) -> Result<Vec<(u32, Vec<Cell>)>, DomainError> {
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
                other => Ok(other.to_string().trim().to_owned()),
            }));
            (row + 1, out)
        })
        .collect())
}

/// The rows of a CSV file, separated by `;`, `,` or tabs, whichever the header uses most.
/// UTF-8 with or without a byte order mark; anything else is read as Latin-1.
fn csv_cells(data: &[u8]) -> Result<Vec<(u32, Vec<Cell>)>, DomainError> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = match std::str::from_utf8(data) {
        Ok(text) => text.to_owned(),
        Err(_) => data.iter().map(|&b| char::from(b)).collect(),
    };
    let header = text.lines().next().unwrap_or_default();
    let delimiter = b";,\t"
        .iter()
        .copied()
        .max_by_key(|d| header.bytes().filter(|b| b == d).count())
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    // Rows are numbered by record, as a spreadsheet opening the file shows them.
    let mut out = Vec::new();
    for (record, row) in reader.records().zip(1..) {
        let record = record.map_err(|e| DomainError::UnreadableSheet {
            reason: e.to_string(),
        })?;
        out.push((
            row,
            record.iter().map(|c| Ok(c.trim().to_owned())).collect(),
        ));
    }
    Ok(out)
}

/// Finds the configured columns in the header and keeps only those cells of every row.
fn pick_columns(
    cells: Vec<(u32, Vec<Cell>)>,
    columns: &TicketColumns,
) -> Result<(Vec<BatchTicket>, Vec<RowProblem>), DomainError> {
    let empty = |cells: &[Cell]| cells.iter().all(|c| c.as_ref().is_ok_and(|c| c.is_empty()));
    let mut cells = cells.into_iter().filter(|(_, c)| !empty(c));
    let Some((_, header)) = cells.next() else {
        return Err(DomainError::UnreadableSheet {
            reason: "the sheet is empty".into(),
        });
    };
    let find = |name: &str| {
        header
            .iter()
            .position(|h| {
                h.as_ref()
                    .is_ok_and(|h| h.eq_ignore_ascii_case(name.trim()))
            })
            .ok_or_else(|| DomainError::MissingColumn {
                column: name.to_owned(),
            })
    };
    let tier_col = find(&columns.tier)?;
    let order_col = find(&columns.order)?;

    let mut rows = Vec::new();
    let mut problems = Vec::new();
    for (row, cells) in cells {
        let get = |i: usize| cells.get(i).cloned().unwrap_or(Ok(String::new()));
        let problem = match (get(tier_col), get(order_col)) {
            (Err(()), _) | (_, Err(())) => Some(Problem::CellError),
            (Ok(tier), _) if tier.is_empty() => Some(Problem::NoTier),
            (_, Ok(order)) if order.is_empty() => Some(Problem::NoOrder),
            (Ok(tier), Ok(order)) => {
                rows.push(BatchTicket { tier, order });
                None
            }
        };
        if let Some(problem) = problem {
            problems.push(RowProblem { row, problem });
        }
    }
    Ok((rows, problems))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> TicketColumns {
        TicketColumns::default()
    }

    fn read(csv: &str) -> Result<Sheet, DomainError> {
        read_sheet("tickets.csv", csv.as_bytes(), &columns())
    }

    #[test]
    fn reads_only_the_configured_columns() {
        let sheet = read("Name;Ticket;E-Mail;Order ID\nAnna;Regular;a@x.example;4711\n").unwrap();
        assert_eq!(
            sheet.batch.tickets,
            [BatchTicket {
                tier: "Regular".into(),
                order: "4711".into()
            }]
        );
        assert!(sheet.problems.is_empty());
    }

    #[test]
    fn finds_columns_ignoring_case_and_order_and_sniffs_the_delimiter() {
        let sheet = read("order id,TICKET\n 17 , Early Bird \n").unwrap();
        assert_eq!(sheet.batch.tickets[0].tier, "Early Bird");
        assert_eq!(sheet.batch.tickets[0].order, "17");
    }

    #[test]
    fn lists_rows_it_cannot_read_and_skips_blank_ones() {
        let sheet = read("Ticket;Order ID;Name\nRegular;;Anna\n;12;Bob\n;;\n;;Carla\n").unwrap();
        assert!(sheet.batch.tickets.is_empty());
        assert_eq!(
            sheet.problems,
            [
                RowProblem {
                    row: 2,
                    problem: Problem::NoOrder
                },
                RowProblem {
                    row: 3,
                    problem: Problem::NoTier
                },
                RowProblem {
                    row: 5,
                    problem: Problem::NoTier
                },
            ]
        );
    }

    #[test]
    fn a_missing_column_blocks_the_import() {
        assert_eq!(
            read("Ticket;Order\nRegular;1\n"),
            Err(DomainError::MissingColumn {
                column: "Order ID".into()
            })
        );
    }

    #[test]
    fn hashes_the_file() {
        let sheet = read("Ticket;Order ID\n").unwrap();
        assert_eq!(sheet.batch.sha256.len(), 64);
        assert_ne!(
            sheet.batch.sha256,
            read("Ticket;Order ID\n\n").unwrap().batch.sha256
        );
    }

    #[test]
    fn reads_latin_1_and_a_byte_order_mark() {
        let latin1 = b"Ticket;Order ID\nErm\xe4\xdfigt;1\n";
        let sheet = read_sheet("t.csv", latin1, &columns()).unwrap();
        assert_eq!(sheet.batch.tickets[0].tier, "Ermäßigt");
        let bom = read("\u{feff}Ticket;Order ID\nRegular;1\n").unwrap();
        assert_eq!(bom.batch.tickets.len(), 1);
    }

    #[test]
    fn rejects_files_that_are_not_spreadsheets() {
        let e = read_sheet("t.ods", b"not a zip", &columns()).unwrap_err();
        assert_eq!(e.code(), "unreadable_sheet");
    }
}
