//! Lenient amount input and display formatting (R-NF-8, R-NF-6).
//!
//! Parsing accepts both `1.234,56` and `1,234.56` regardless of the user's setting, as long as
//! the input is unambiguous. The rules, in order:
//!
//! 1. Spaces and currency symbols are ignored. One leading `-`, `−` or `+` is allowed.
//! 2. The last `.` or `,` followed by 1–2 digits is the decimal separator.
//! 3. Every other separator is a thousands separator. It must be the other character, and the
//!    groups must be correct: 1–3 digits first, then exactly 3.
//! 4. More than two decimals are rejected; nothing is rounded.
//! 5. A single separator followed by exactly three digits (`1.234`, `1,234`) is ambiguous and
//!    read according to the setting. If the setting makes it the thousands separator, the
//!    amount is accepted and flagged as ambiguous. If the setting makes it the decimal
//!    separator, it means three decimals and is rejected: for a German user, `1,234` is far more
//!    likely a typo of `1,23` or `12,34` than a thousand of anything.
//!
//! The test cases live in `fixtures/amount-input.json`.
//!
//! Amount fields also add up what is typed into them (`1,29+1,49`, see [`parse_sum`]). Only the
//! fields do: a cell of a file that reads `10-12` is not a sum, so everything else stays with
//! [`parse_amount`]. The test cases live in `fixtures/amount-sum.json`.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::money::{Cents, Currency, MAX_AMOUNT};

/// The app-wide number and date format (R-NF-6). Dates are formatted elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum NumberFormat {
    /// `1.234,56 €` · `31.12.2026`
    De,
    /// `€1,234.56` · `2026-12-31`
    En,
}

impl NumberFormat {
    pub fn decimal_separator(self) -> char {
        match self {
            NumberFormat::De => ',',
            NumberFormat::En => '.',
        }
    }

    pub fn thousands_separator(self) -> char {
        match self {
            NumberFormat::De => '.',
            NumberFormat::En => ',',
        }
    }
}

/// How the sign of an amount is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum SignStyle {
    /// `−12,50 €` for negative amounts, nothing for positive ones.
    Plain,
    /// `+12,50 €` and `−12,50 €`, e.g. income and expenses in the ledger. Zero has no sign.
    Always,
    /// `12,50 €` without a sign, e.g. in "owes 12,50 €".
    Magnitude,
}

/// A successfully parsed amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Parsed {
    pub cents: Cents,
    /// The input was read according to the number format setting (rule 5), so the UI should
    /// point out how it was read.
    pub ambiguous: bool,
}

/// The total of an amount field's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ParsedSum {
    pub cents: Cents,
    /// At least one term was read according to the number format setting.
    pub ambiguous: bool,
    /// How many amounts were added up; 1 for a plain amount.
    pub terms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum AmountError {
    #[error("no amount entered")]
    Empty,
    #[error("use only digits, a comma or point, and + or −")]
    InvalidCharacter,
    #[error("thousands separators must form groups of three")]
    BadGrouping,
    #[error("at most two decimals are allowed")]
    TooManyDecimals,
    #[error("a sign is only allowed at the start")]
    MisplacedSign,
    #[error("negative amounts are not allowed here")]
    NegativeNotAllowed,
    #[error("amount is too large")]
    TooLarge,
    #[error("a sum needs a number between its signs")]
    MissingTerm,
}

impl AmountError {
    /// Stable identifier for the UI and the fixture file.
    pub fn code(self) -> &'static str {
        match self {
            AmountError::Empty => "empty",
            AmountError::InvalidCharacter => "invalid_character",
            AmountError::BadGrouping => "bad_grouping",
            AmountError::TooManyDecimals => "too_many_decimals",
            AmountError::MisplacedSign => "misplaced_sign",
            AmountError::NegativeNotAllowed => "negative_not_allowed",
            AmountError::TooLarge => "too_large",
            AmountError::MissingTerm => "missing_term",
        }
    }
}

const MINUS: char = '−'; // U+2212, used for display

fn is_separator(c: char) -> bool {
    c == '.' || c == ','
}

fn is_sign(c: char) -> bool {
    c == '-' || c == MINUS || c == '+'
}

/// Any currency's symbol, not only the event's: a pasted amount can come from anywhere.
fn without_spaces_and_symbols(input: &str) -> String {
    let mut text = input.to_owned();
    for currency in Currency::ALL {
        text = text.replace(currency.symbol(), "");
    }
    text.retain(|c| !c.is_whitespace());
    text
}

/// Parses user input into cents. See the module documentation for the rules.
pub fn parse_amount(
    input: &str,
    format: NumberFormat,
    allow_negative: bool,
) -> Result<Parsed, AmountError> {
    let cleaned = without_spaces_and_symbols(input);

    let (negative, body) = match cleaned.chars().next() {
        Some(c) if is_sign(c) => (c != '+', &cleaned[c.len_utf8()..]),
        _ => (false, cleaned.as_str()),
    };
    if body.is_empty() {
        return Err(AmountError::Empty);
    }
    if body.chars().any(is_sign) {
        return Err(AmountError::MisplacedSign);
    }
    if !body.chars().all(|c| c.is_ascii_digit() || is_separator(c)) {
        return Err(AmountError::InvalidCharacter);
    }

    let (integer, decimals, ambiguous) = split(body, format)?;
    let cents = to_cents(integer, decimals)?;

    if negative && cents != 0 && !allow_negative {
        return Err(AmountError::NegativeNotAllowed);
    }
    let cents = if negative { -cents } else { cents };
    let cents = Cents::new(cents).map_err(|_| AmountError::TooLarge)?;
    Ok(Parsed { cents, ambiguous })
}

/// Adds up what was typed into an amount field, each term read by [`parse_amount`]. One sign at
/// the end is the next term still being typed and is ignored. `allow_negative` applies to the
/// total only, so `10-2` is fine in a field that takes no negative amounts.
pub fn parse_sum(
    input: &str,
    format: NumberFormat,
    allow_negative: bool,
) -> Result<ParsedSum, AmountError> {
    let cleaned = without_spaces_and_symbols(input);
    let body = cleaned.strip_suffix(is_sign).unwrap_or(&cleaned);

    let mut starts: Vec<usize> = body
        .char_indices()
        .skip(1)
        .filter(|(_, c)| is_sign(*c))
        .map(|(i, _)| i)
        .collect();
    starts.insert(0, 0);
    let terms = starts.len();

    let mut total = 0_i64;
    let mut ambiguous = false;
    for (n, start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(body.len());
        let parsed = parse_amount(&body[*start..end], format, true).map_err(|e| match e {
            AmountError::Empty if terms > 1 => AmountError::MissingTerm,
            e => e,
        })?;
        total = total
            .checked_add(parsed.cents.get())
            .ok_or(AmountError::TooLarge)?;
        ambiguous |= parsed.ambiguous;
    }

    if total < 0 && !allow_negative {
        return Err(AmountError::NegativeNotAllowed);
    }
    Ok(ParsedSum {
        cents: Cents::new(total).map_err(|_| AmountError::TooLarge)?,
        ambiguous,
        terms: terms as u32,
    })
}

/// Splits `body` (digits and separators only) into its integer digits, its decimal digits and
/// the ambiguity flag. The integer digits are returned without thousands separators.
fn split(body: &str, format: NumberFormat) -> Result<(String, &str, bool), AmountError> {
    let Some(last) = body.rfind(is_separator) else {
        return Ok((body.to_owned(), "", false));
    };
    let sep = body[last..].chars().next().unwrap_or('.');
    let head = &body[..last];
    let tail = &body[last + 1..];
    let separator_count = body.chars().filter(|c| is_separator(*c)).count();

    match tail.len() {
        0 => Err(AmountError::BadGrouping),
        1 | 2 => {
            let thousands = if sep == '.' { ',' } else { '.' };
            Ok((ungroup(head, thousands, true)?, tail, false))
        }
        3 if separator_count == 1 => {
            if sep == format.thousands_separator() {
                Ok((ungroup(body, sep, false)?, "", true))
            } else {
                Err(AmountError::TooManyDecimals)
            }
        }
        // Several separators: if they are all the same character they are thousands
        // separators (`1.234.567`); if the last one differs it is a decimal separator with
        // three decimals (`1.234,567`).
        3 if head.contains(|c| is_separator(c) && c != sep) => Err(AmountError::TooManyDecimals),
        3 => Ok((ungroup(body, sep, false)?, "", false)),
        _ => Err(AmountError::TooManyDecimals),
    }
}

/// Removes thousands separators from `s`, checking the grouping. `s` may contain only
/// `thousands` as separator. An empty integer part is allowed when decimals follow (`,50`).
fn ungroup(s: &str, thousands: char, has_decimals: bool) -> Result<String, AmountError> {
    if s.chars().any(|c| is_separator(c) && c != thousands) {
        return Err(AmountError::BadGrouping);
    }
    let groups: Vec<&str> = s.split(thousands).collect();
    if groups.len() == 1 {
        if s.is_empty() && !has_decimals {
            return Err(AmountError::BadGrouping);
        }
        return Ok(s.to_owned());
    }
    let first_ok = (1..=3).contains(&groups[0].len());
    let rest_ok = groups[1..].iter().all(|g| g.len() == 3);
    if first_ok && rest_ok {
        Ok(groups.concat())
    } else {
        Err(AmountError::BadGrouping)
    }
}

fn to_cents(integer: String, decimals: &str) -> Result<i64, AmountError> {
    let integer = integer.trim_start_matches('0');
    // Cheap early exit so the parse below cannot overflow; the exact limit is checked after.
    if integer.len() > 13 {
        return Err(AmountError::TooLarge);
    }
    let units: i64 = if integer.is_empty() {
        0
    } else {
        integer.parse().map_err(|_| AmountError::TooLarge)?
    };
    let cents: i64 = match decimals.len() {
        0 => 0,
        1 => {
            decimals
                .parse::<i64>()
                .map_err(|_| AmountError::InvalidCharacter)?
                * 10
        }
        _ => decimals
            .parse()
            .map_err(|_| AmountError::InvalidCharacter)?,
    };
    let total = units
        .checked_mul(100)
        .and_then(|e| e.checked_add(cents))
        .ok_or(AmountError::TooLarge)?;
    if total > MAX_AMOUNT.get() {
        return Err(AmountError::TooLarge);
    }
    Ok(total)
}

/// Formats cents for display: `1.234,56 €` (De) or `€1,234.56` (En). The symbol's place follows
/// the number format, and a symbol of letters keeps a space: `CHF 1,234.56`.
pub fn format_amount(
    amount: Cents,
    format: NumberFormat,
    sign: SignStyle,
    currency: Currency,
) -> String {
    let value = amount.get();
    let magnitude = value.unsigned_abs();
    let units = (magnitude / 100).to_string();
    let cents = magnitude % 100;

    let mut grouped = String::with_capacity(units.len() + units.len() / 3);
    for (i, digit) in units.chars().enumerate() {
        if i > 0 && (units.len() - i).is_multiple_of(3) {
            grouped.push(format.thousands_separator());
        }
        grouped.push(digit);
    }

    let sign = match sign {
        SignStyle::Magnitude => "",
        _ if value < 0 => "−",
        SignStyle::Always if value > 0 => "+",
        _ => "",
    };
    let decimal = format.decimal_separator();
    let symbol = currency.symbol();
    match format {
        NumberFormat::De => format!("{sign}{grouped}{decimal}{cents:02}\u{a0}{symbol}"),
        NumberFormat::En if symbol.len() > 2 && symbol.is_ascii() => {
            format!("{sign}{symbol}\u{a0}{grouped}{decimal}{cents:02}")
        }
        NumberFormat::En => format!("{sign}{symbol}{grouped}{decimal}{cents:02}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde::Deserialize;

    use NumberFormat::{De, En};

    fn ok(input: &str, format: NumberFormat) -> Parsed {
        parse_amount(input, format, true).unwrap()
    }

    fn err(input: &str, format: NumberFormat) -> AmountError {
        parse_amount(input, format, true).unwrap_err()
    }

    fn cents(c: i64) -> Cents {
        Cents::new(c).unwrap()
    }

    // One test per bullet of R-NF-8.

    #[test]
    fn last_separator_with_one_or_two_digits_is_decimal() {
        for f in [De, En] {
            assert_eq!(ok("12,5", f).cents, cents(1250));
            assert_eq!(ok("12.5", f).cents, cents(1250));
            assert_eq!(ok("1.234,56", f).cents, cents(123_456));
            assert_eq!(ok("1,234.56", f).cents, cents(123_456));
            assert!(!ok("1,234.56", f).ambiguous);
        }
    }

    #[test]
    fn other_separators_must_form_groups_of_three() {
        assert_eq!(err("1.23.4", De), AmountError::BadGrouping);
        assert_eq!(err("12,34.5", En), AmountError::BadGrouping);
        assert_eq!(err("1234.567,8", De), AmountError::BadGrouping);
        assert_eq!(err("1,234,56", De), AmountError::BadGrouping);
        assert_eq!(ok("1.234.567", De).cents, cents(123_456_700));
    }

    #[test]
    fn more_than_two_decimals_are_rejected() {
        assert_eq!(err("1,2345", De), AmountError::TooManyDecimals);
        assert_eq!(err("1.234,567", De), AmountError::TooManyDecimals);
    }

    #[test]
    fn single_separator_with_three_digits_follows_the_setting() {
        assert_eq!(
            ok("1.234", De),
            Parsed {
                cents: cents(123_400),
                ambiguous: true
            }
        );
        assert_eq!(
            ok("1,234", En),
            Parsed {
                cents: cents(123_400),
                ambiguous: true
            }
        );
        assert_eq!(err("1,234", De), AmountError::TooManyDecimals);
        assert_eq!(err("1.234", En), AmountError::TooManyDecimals);
    }

    #[test]
    fn currency_sign_and_spaces_are_ignored() {
        assert_eq!(ok("€ 1 234,56", De).cents, cents(123_456));
        assert_eq!(ok("12,50\u{a0}€", De).cents, cents(1250));
    }

    #[test]
    fn leading_minus_only_where_allowed() {
        assert_eq!(ok("-5", De).cents, cents(-500));
        assert_eq!(ok("−5", De).cents, cents(-500));
        assert_eq!(
            parse_amount("-5", De, false),
            Err(AmountError::NegativeNotAllowed)
        );
        assert_eq!(err("5-", De), AmountError::MisplacedSign);
    }

    #[test]
    fn formats_both_styles() {
        let c = cents(-123_456_789);
        assert_eq!(
            format_amount(c, De, SignStyle::Plain, Currency::Eur),
            "−1.234.567,89\u{a0}€"
        );
        assert_eq!(
            format_amount(c, En, SignStyle::Plain, Currency::Eur),
            "−€1,234,567.89"
        );
        assert_eq!(
            format_amount(c, De, SignStyle::Magnitude, Currency::Eur),
            "1.234.567,89\u{a0}€"
        );
        assert_eq!(
            format_amount(cents(5), En, SignStyle::Always, Currency::Eur),
            "+€0.05"
        );
        assert_eq!(
            format_amount(Cents::ZERO, De, SignStyle::Always, Currency::Eur),
            "0,00\u{a0}€"
        );
        assert_eq!(
            format_amount(cents(100_000), De, SignStyle::Plain, Currency::Eur),
            "1.000,00\u{a0}€"
        );
        assert_eq!(
            format_amount(cents(99_999), En, SignStyle::Plain, Currency::Eur),
            "€999.99"
        );
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Case {
        input: String,
        format: NumberFormat,
        #[serde(default = "yes")]
        allow_negative: bool,
        cents: Option<i64>,
        #[serde(default)]
        ambiguous: bool,
        error: Option<String>,
        #[serde(default, rename = "note")]
        _note: Option<String>,
    }

    fn yes() -> bool {
        true
    }

    #[test]
    fn fixture_cases() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../../fixtures/amount-input.json")).unwrap();
        assert!(cases.len() >= 40);
        for case in cases {
            let got = parse_amount(&case.input, case.format, case.allow_negative);
            let label = format!("{:?} ({:?})", case.input, case.format);
            match (case.cents, case.error) {
                (Some(c), None) => {
                    let parsed = got.unwrap_or_else(|e| panic!("{label}: unexpected {e:?}"));
                    assert_eq!(parsed.cents.get(), c, "{label}");
                    assert_eq!(parsed.ambiguous, case.ambiguous, "{label}: ambiguity");
                }
                (None, Some(code)) => {
                    let e = got.expect_err(&label);
                    assert_eq!(e.code(), code, "{label}");
                }
                _ => panic!("{label}: a case needs exactly one of `cents` and `error`"),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SumCase {
        input: String,
        format: NumberFormat,
        #[serde(default = "yes")]
        allow_negative: bool,
        cents: Option<i64>,
        #[serde(default)]
        ambiguous: bool,
        terms: Option<u32>,
        error: Option<String>,
    }

    #[test]
    fn sum_fixture_cases() {
        let cases: Vec<SumCase> =
            serde_json::from_str(include_str!("../../../fixtures/amount-sum.json")).unwrap();
        for case in cases {
            let got = parse_sum(&case.input, case.format, case.allow_negative);
            let label = format!("{:?} ({:?})", case.input, case.format);
            match (case.cents, case.terms, case.error) {
                (Some(c), Some(terms), None) => {
                    let sum = got.unwrap_or_else(|e| panic!("{label}: unexpected {e:?}"));
                    assert_eq!(sum.cents.get(), c, "{label}");
                    assert_eq!(sum.ambiguous, case.ambiguous, "{label}: ambiguity");
                    assert_eq!(sum.terms, terms, "{label}: terms");
                }
                (None, None, Some(code)) => {
                    let e = got.expect_err(&label);
                    assert_eq!(e.code(), code, "{label}");
                }
                _ => panic!("{label}: needs `cents` and `terms`, or `error`"),
            }
        }
    }

    #[test]
    fn a_strict_amount_is_not_a_sum() {
        assert_eq!(err("10-12", De), AmountError::MisplacedSign);
        assert_eq!(err("1+1", De), AmountError::MisplacedSign);
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FormatCase {
        cents: i64,
        format: NumberFormat,
        sign: SignStyle,
        #[serde(default)]
        currency: Currency,
        text: String,
    }

    /// The same cases are checked by the UI's `formatAmount` (app/src/lib/format.test.ts).
    #[test]
    fn format_fixture_cases() {
        let cases: Vec<FormatCase> =
            serde_json::from_str(include_str!("../../../fixtures/amount-format.json")).unwrap();
        assert!(cases.len() >= 100);
        for case in cases {
            assert_eq!(
                format_amount(cents(case.cents), case.format, case.sign, case.currency),
                case.text,
                "{}",
                case.cents
            );
        }
    }

    #[test]
    fn symbols_of_every_currency_are_ignored() {
        assert_eq!(ok("$1,234.56", En).cents, cents(123_456));
        assert_eq!(ok("£ 12.50", En).cents, cents(1250));
        assert_eq!(ok("CHF 12,50", De).cents, cents(1250));
        assert_eq!(
            parse_sum("CHF 1,50+CHF 2", De, false).unwrap().cents,
            cents(350)
        );
    }

    fn any_format() -> impl Strategy<Value = NumberFormat> {
        prop_oneof![Just(De), Just(En)]
    }

    proptest! {
        #[test]
        fn formatted_amounts_parse_back_unambiguously(
            c in -MAX_AMOUNT.get()..=MAX_AMOUNT.get(),
            display in any_format(),
            setting in any_format(),
            currency in prop::sample::select(&Currency::ALL[..]),
        ) {
            let text = format_amount(cents(c), display, SignStyle::Plain, currency);
            // Whatever the setting, a formatted amount always has two decimals and is read back.
            let parsed = parse_amount(&text, setting, true).unwrap();
            prop_assert_eq!(parsed.cents.get(), c);
            prop_assert!(!parsed.ambiguous);
        }

        #[test]
        fn parsing_never_panics(input in "[-−+€ 0-9.,a]{0,20}", f in any_format(), neg: bool) {
            let _ = parse_amount(&input, f, neg);
            let _ = parse_sum(&input, f, neg);
        }

        #[test]
        fn a_valid_amount_reads_the_same_as_a_sum(
            input in "[-−+€ 0-9.,a]{0,20}",
            f in any_format(),
            neg: bool,
        ) {
            if let Ok(parsed) = parse_amount(&input, f, neg) {
                let sum = parse_sum(&input, f, neg).unwrap();
                prop_assert_eq!(sum.cents, parsed.cents);
                prop_assert_eq!(sum.ambiguous, parsed.ambiguous);
                prop_assert_eq!(sum.terms, 1);
            }
        }
    }
}
