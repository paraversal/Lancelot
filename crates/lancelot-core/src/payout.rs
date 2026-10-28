//! The payout and repayment lists (R-PER-4, R-PER-7) and the payout CSV.
//!
//! The payout list is everyone the event owes money to, for the Verein's treasurer, who pays
//! it in one go. The repayment list is everyone who owes the event money; they pay on their
//! own, so their repayments are recorded one by one when the money arrives.

use serde::Serialize;
use ts_rs::TS;

use crate::amount::NumberFormat;
use crate::book::Book;
use crate::model::AccountId;
use crate::money::{Cents, MoneyError, try_sum};

/// The longest remittance text a SEPA transfer carries.
pub const REFERENCE_MAX: usize = 140;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PayoutRow {
    pub person: AccountId,
    pub name: String,
    /// The account holder, or the person's name when none is given.
    pub holder: String,
    /// Normalised (no spaces); `None` while the person has no IBAN.
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub amount: Cents,
    pub reference: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct RepaymentRow {
    pub person: AccountId,
    pub name: String,
    pub amount: Cents,
    /// Where the money should go: the default reimbursement account.
    pub account: AccountId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Payouts {
    /// By name.
    pub payouts: Vec<PayoutRow>,
    pub payout_total: Cents,
    /// What "Mark as paid" records: the payouts that have an IBAN.
    pub payable_total: Cents,
    pub repayments: Vec<RepaymentRow>,
    pub repayment_total: Cents,
}

/// A remittance text in the characters every SEPA bank accepts: German umlauts and ß are
/// spelled out, other characters outside the set are dropped, and it is cut to 140.
pub fn reference(event: &str, person: &str) -> String {
    let mut out = String::new();
    for c in format!("Auslagen {event} {person}").chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'Ä' => out.push_str("Ae"),
            'Ö' => out.push_str("Oe"),
            'Ü' => out.push_str("Ue"),
            'ß' => out.push_str("ss"),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'á' | 'à' | 'â' => out.push('a'),
            'ó' | 'ò' | 'ô' => out.push('o'),
            'í' | 'ì' | 'î' | 'ï' => out.push('i'),
            'ú' | 'ù' | 'û' => out.push('u'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            c if c.is_ascii_alphanumeric() || " /-?:().,'+".contains(c) => out.push(c),
            _ => {}
        }
    }
    let squeezed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    squeezed.chars().take(REFERENCE_MAX).collect()
}

pub fn payouts(book: &Book) -> Result<Payouts, MoneyError> {
    let ledger = book.ledger();
    let event = &book.event().details.name;
    let mut payouts = Vec::new();
    let mut repayments = Vec::new();
    for (account, person) in book.persons() {
        let net = ledger.person(account.id).map(|p| p.net).unwrap_or_default();
        if net.is_positive() {
            payouts.push(PayoutRow {
                person: account.id,
                name: account.name.clone(),
                holder: person
                    .holder
                    .clone()
                    .unwrap_or_else(|| account.name.clone()),
                iban: person.iban.as_ref().map(|i| i.as_str().to_owned()),
                bic: person.bic.clone(),
                amount: net,
                reference: reference(event, &account.name),
            });
        } else if net.is_negative() {
            repayments.push(RepaymentRow {
                person: account.id,
                name: account.name.clone(),
                amount: -net,
                account: book.event().default_reimbursement_account,
            });
        }
    }
    payouts.sort_by_key(|p| p.name.to_lowercase());
    repayments.sort_by_key(|r| r.name.to_lowercase());
    Ok(Payouts {
        payout_total: try_sum(payouts.iter().map(|p| p.amount))?,
        payable_total: try_sum(
            payouts
                .iter()
                .filter(|p| p.iban.is_some())
                .map(|p| p.amount),
        )?,
        repayment_total: try_sum(repayments.iter().map(|r| r.amount))?,
        payouts,
        repayments,
    })
}

/// The payout list as CSV, in the user's number format: `;` and `212,40` (De, what German
/// spreadsheets expect) or `,` and `212.40` (En). Amounts have no thousands separators and
/// IBANs no spaces, so banking tools can import them.
pub fn payouts_csv(list: &Payouts, format: NumberFormat) -> String {
    let (sep, decimal) = match format {
        NumberFormat::De => (';', ','),
        NumberFormat::En => (',', '.'),
    };
    let field = |s: &str| {
        if s.contains([sep, '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    };
    let amount = |c: Cents| {
        let v = c.get();
        format!(
            "{}{}{decimal}{:02}",
            if v < 0 { "-" } else { "" },
            v.unsigned_abs() / 100,
            v.unsigned_abs() % 100
        )
    };
    let mut out = [
        "Name",
        "Account holder",
        "IBAN",
        "BIC",
        "Amount",
        "Reference",
    ]
    .map(field)
    .join(&sep.to_string());
    out.push_str("\r\n");
    for p in &list.payouts {
        let row = [
            field(&p.name),
            field(&p.holder),
            field(p.iban.as_deref().unwrap_or("")),
            field(p.bic.as_deref().unwrap_or("")),
            amount(p.amount),
            field(&p.reference),
        ];
        out.push_str(&row.join(&sep.to_string()));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_use_sepa_characters_only() {
        assert_eq!(
            reference("Sommerfest 2026", "Anna Krüger"),
            "Auslagen Sommerfest 2026 Anna Krueger"
        );
        assert_eq!(
            reference("Straßenfest", "José Núñez"),
            "Auslagen Strassenfest Jose Nunez"
        );
        assert_eq!(reference("Fest  🎉", "Kim"), "Auslagen Fest Kim");
        assert_eq!(
            reference(&"x".repeat(200), "Kim").chars().count(),
            REFERENCE_MAX
        );
    }

    #[test]
    fn csv_follows_the_number_format_and_quotes_when_needed() {
        let list = Payouts {
            payouts: vec![PayoutRow {
                person: AccountId(1),
                name: "Anna; \"Ani\"".into(),
                holder: "Anna Krüger".into(),
                iban: Some("DE89370400440532013000".into()),
                bic: None,
                amount: Cents::new(123_456).unwrap(),
                reference: "Auslagen X Anna".into(),
            }],
            ..Payouts::default()
        };
        assert_eq!(
            payouts_csv(&list, NumberFormat::De),
            "Name;Account holder;IBAN;BIC;Amount;Reference\r\n\"Anna; \"\"Ani\"\"\";Anna Krüger;DE89370400440532013000;;1234,56;Auslagen X Anna\r\n"
        );
        assert!(payouts_csv(&list, NumberFormat::En).contains(",1234.56,"));
    }
}
