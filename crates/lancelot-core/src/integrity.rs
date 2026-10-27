//! The integrity check (R-COR-5). It reads the event file directly, without trusting the
//! book, and reports every problem it finds.
//!
//! [`Depth::Full`] runs on open and on demand. [`Depth::Quick`] runs inside every write and
//! leaves out SQLite's page check and the attachment hashes, which grow with the file.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::book::Book;
use crate::entry::lines_follow_rules;
use crate::store::StoreError;
use crate::store::read;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    Quick,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The figures cannot be trusted until this is fixed.
    Error,
    /// The figures are right, but something looks wrong.
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub severity: Severity,
    /// Stable identifier, e.g. `unbalanced` or `attachment_hash`.
    pub code: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn codes(&self) -> Vec<&'static str> {
        self.issues.iter().map(|i| i.code).collect()
    }

    fn error(&mut self, code: &'static str, detail: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Error,
            code,
            detail: detail.into(),
        });
    }

    fn warning(&mut self, code: &'static str, detail: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            code,
            detail: detail.into(),
        });
    }
}

pub fn check(conn: &Connection, depth: Depth) -> Result<Report, StoreError> {
    let mut report = Report::default();

    if depth == Depth::Full {
        let mut stmt = conn.prepare("PRAGMA integrity_check")?;
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for row in rows.into_iter().filter(|r| r != "ok") {
            report.error("sqlite_integrity", row);
        }
    }

    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (table, row) in rows {
        report.error(
            "foreign_key",
            format!("{table} row {row} points to a missing row"),
        );
    }

    // R-COR-2: every transaction sealed, with at least two lines summing to zero.
    let mut stmt = conn.prepare("SELECT id FROM txn WHERE sealed = 0 ORDER BY id")?;
    for id in stmt.query_map([], |r| r.get::<_, i64>(0))? {
        report.error("unsealed", format!("transaction {} is not sealed", id?));
    }
    let mut stmt = conn.prepare(
        "SELECT t.id FROM txn t LEFT JOIN line l ON l.txn_id = t.id
         GROUP BY t.id HAVING COALESCE(SUM(l.amount), 0) <> 0 OR COUNT(l.id) < 2 ORDER BY t.id",
    )?;
    for id in stmt.query_map([], |r| r.get::<_, i64>(0))? {
        report.error(
            "unbalanced",
            format!("transaction {} does not balance", id?),
        );
    }

    // Every transaction has the shape its kind requires (docs/technical-design.md §3.3).
    match (read::accounts(conn), read::txns(conn)) {
        (Ok(accounts), Ok(txns)) => {
            for t in txns
                .iter()
                .filter(|t| !lines_follow_rules(t.kind, &t.lines, &accounts))
            {
                report.error(
                    "malformed_txn",
                    format!(
                        "transaction {} does not follow the rules for its kind",
                        t.id.0
                    ),
                );
            }
        }
        (Err(e), _) | (_, Err(e)) => report.error("load_failed", e.to_string()),
    }

    if depth == Depth::Full {
        let mut stmt = conn.prepare("SELECT id, sha256, size, data FROM attachment ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Vec<u8>>(3)?,
            ))
        })?;
        for row in rows {
            let (id, sha256, size, data) = row?;
            let actual: String = Sha256::digest(&data)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if actual != sha256 || usize::try_from(size).ok() != Some(data.len()) {
                report.error(
                    "attachment_hash",
                    format!("attachment {id} does not match its hash"),
                );
            }
        }
    }

    // Finally the domain view: the file must load into a book.
    match read::parts(conn).and_then(|p| Book::from_parts(p).map_err(StoreError::from)) {
        Ok(book) => over_settled(&book, &mut report),
        Err(e) => {
            if !report.has_errors() {
                report.error("load_failed", e.to_string());
            }
        }
    }

    Ok(report)
}

/// R-COR-5: reimbursements should not exceed what the person was owed, and repayments not
/// what they owed. Allowed, since corrections happen, but worth a look.
fn over_settled(book: &Book, report: &mut Report) {
    let names: BTreeMap<_, _> = book.accounts().map(|a| (a.id, a.name.as_str())).collect();
    for (id, p) in book.ledger().persons() {
        let name = names.get(&id).copied().unwrap_or("?");
        if p.reimbursed.is_positive() && p.net.is_negative() {
            report.warning(
                "over_reimbursed",
                format!("{name} was reimbursed more than they advanced"),
            );
        }
        if p.repaid.is_positive() && p.net.is_positive() {
            report.warning(
                "over_repaid",
                format!("{name} paid back more than they held"),
            );
        }
    }
}
