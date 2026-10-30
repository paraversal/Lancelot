//! The raw transactions as CSV (R-SET-6), for spreadsheets and the Verein's own books.

use crate::amount::NumberFormat;
use crate::book::Book;
use crate::csv::Csv;
use crate::date::format_date;
use crate::model::{AccountKind, Txn, TxnKind};

/// One row per line of every transaction, by date and then entry number. Amounts keep the
/// ledger's sign: money leaving an account is negative, so each entry's rows sum to zero.
pub fn transactions_csv(book: &Book, format: NumberFormat) -> String {
    let mut csv = Csv::new(format);
    csv.header(&[
        "Date",
        "Entry",
        "Type",
        "Description",
        "Counterparty",
        "Account",
        "Account type",
        "Amount",
        "Memo",
        "Receipt",
        "Notes",
        "Cost centre",
    ]);
    let mut txns: Vec<&Txn> = book.txns().collect();
    txns.sort_by_key(|t| (t.date, t.id));
    for t in txns {
        let receipt = match (t.kind.needs_receipt(), book.attachments_of(t.id).next()) {
            (true, Some(_)) => "yes",
            (true, None) => "no",
            (false, _) => "",
        };
        for line in &t.lines {
            let account = book.account(line.account);
            let row = [
                format_date(t.date, format),
                t.id.0.to_string(),
                csv.text(type_label(book, t)),
                csv.text(&t.description),
                csv.text(t.counterparty.as_deref().unwrap_or("")),
                csv.text(account.map(|a| a.name.as_str()).unwrap_or("")),
                csv.text(account.map(|a| kind_label(a.kind)).unwrap_or("")),
                csv.amount(line.amount),
                csv.text(line.memo.as_deref().unwrap_or("")),
                receipt.to_owned(),
                csv.text(t.notes.as_deref().unwrap_or("")),
                csv.text(
                    line.cost_centre
                        .and_then(|c| book.cost_centre(c))
                        .map(|c| c.name.as_str())
                        .unwrap_or(""),
                ),
            ];
            csv.row(&row);
        }
    }
    csv.finish()
}

fn type_label(book: &Book, t: &Txn) -> &'static str {
    match t.kind {
        TxnKind::Expense => "Expense",
        TxnKind::Income => "Income",
        TxnKind::Transfer => "Transfer",
        TxnKind::Opening => "Opening count",
        TxnKind::Refund => "Refund",
        // A reimbursement pays the person: their line is positive.
        TxnKind::Settle => {
            let to_person = t.lines.iter().any(|l| {
                l.amount.is_positive()
                    && book.account(l.account).map(|a| a.kind) == Some(AccountKind::Person)
            });
            if to_person {
                "Reimbursement"
            } else {
                "Repayment"
            }
        }
    }
}

fn kind_label(kind: AccountKind) -> &'static str {
    match kind {
        AccountKind::Asset => "Account",
        AccountKind::Person => "Person",
        AccountKind::Income => "Income",
        AccountKind::Expense => "Expense",
        AccountKind::Opening => "Opening",
    }
}
