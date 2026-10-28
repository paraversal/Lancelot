//! CSV in the user's number format, the way German and English spreadsheets open it: `;` and
//! `212,40` (De) or `,` and `212.40` (En). Amounts have no thousands separators, lines end in
//! CRLF, and fields are quoted only when they need it.

use crate::amount::NumberFormat;
use crate::money::Cents;

pub(crate) struct Csv {
    sep: char,
    decimal: char,
    out: String,
}

impl Csv {
    pub(crate) fn new(format: NumberFormat) -> Csv {
        let (sep, decimal) = match format {
            NumberFormat::De => (';', ','),
            NumberFormat::En => (',', '.'),
        };
        Csv {
            sep,
            decimal,
            out: String::new(),
        }
    }

    pub(crate) fn text(&self, s: &str) -> String {
        if s.contains([self.sep, '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    }

    pub(crate) fn amount(&self, c: Cents) -> String {
        let v = c.get();
        format!(
            "{}{}{}{:02}",
            if v < 0 { "-" } else { "" },
            v.unsigned_abs() / 100,
            self.decimal,
            v.unsigned_abs() % 100
        )
    }

    /// Appends a row of fields that are already quoted or formatted.
    pub(crate) fn row(&mut self, fields: &[String]) {
        self.out.push_str(&fields.join(&self.sep.to_string()));
        self.out.push_str("\r\n");
    }

    pub(crate) fn header(&mut self, names: &[&str]) {
        let fields: Vec<String> = names.iter().map(|n| self.text(n)).collect();
        self.row(&fields);
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }
}
