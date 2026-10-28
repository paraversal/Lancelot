//! IBAN and BIC validation for person records (R-PER-2).
//!
//! IBANs are checked against the length for their country (SEPA area) and the ISO 13616
//! mod-97 checksum. They are stored normalised: no spaces, upper case.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// A validated, normalised IBAN, e.g. `DE89370400440532013000`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(try_from = "String", into = "String")]
pub struct Iban(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum IbanError {
    #[error("only letters and digits are allowed")]
    InvalidCharacter,
    #[error("unknown or non-SEPA country code")]
    UnknownCountry,
    #[error("an IBAN for this country has {expected} characters")]
    WrongLength { expected: usize },
    #[error("checksum does not match; check for a typo")]
    BadChecksum,
}

impl IbanError {
    /// Stable identifier for the UI.
    pub fn code(self) -> &'static str {
        match self {
            IbanError::InvalidCharacter => "invalid_character",
            IbanError::UnknownCountry => "unknown_country",
            IbanError::WrongLength { .. } => "wrong_length",
            IbanError::BadChecksum => "bad_checksum",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum BicError {
    #[error("a BIC has 8 or 11 characters: bank, country, location and optional branch")]
    InvalidFormat,
}

impl BicError {
    pub fn code(self) -> &'static str {
        "invalid_bic"
    }
}

/// IBAN lengths of the SEPA countries. French overseas departments use `FR`.
const LENGTHS: &[(&str, usize)] = &[
    ("AD", 24),
    ("AT", 20),
    ("BE", 16),
    ("BG", 22),
    ("CH", 21),
    ("CY", 28),
    ("CZ", 24),
    ("DE", 22),
    ("DK", 18),
    ("EE", 20),
    ("ES", 24),
    ("FI", 18),
    ("FR", 27),
    ("GB", 22),
    ("GI", 23),
    ("GR", 27),
    ("HR", 21),
    ("HU", 28),
    ("IE", 22),
    ("IS", 26),
    ("IT", 27),
    ("LI", 21),
    ("LT", 20),
    ("LU", 20),
    ("LV", 21),
    ("MC", 27),
    ("MT", 31),
    ("NL", 18),
    ("NO", 15),
    ("PL", 28),
    ("PT", 25),
    ("RO", 24),
    ("SE", 24),
    ("SI", 19),
    ("SK", 24),
    ("SM", 27),
    ("VA", 22),
];

/// Removes all whitespace and makes letters upper case.
pub fn normalise(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Validates an IBAN given in any spacing and case.
pub fn validate_iban(input: &str) -> Result<Iban, IbanError> {
    let iban = normalise(input);
    if !iban.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(IbanError::InvalidCharacter);
    }
    let country = iban.get(..2).ok_or(IbanError::UnknownCountry)?;
    let expected = LENGTHS
        .iter()
        .find(|(code, _)| *code == country)
        .map(|(_, len)| *len)
        .ok_or(IbanError::UnknownCountry)?;
    if iban.len() != expected {
        return Err(IbanError::WrongLength { expected });
    }
    if !iban[2..4].chars().all(|c| c.is_ascii_digit()) || mod97(&iban) != 1 {
        return Err(IbanError::BadChecksum);
    }
    Ok(Iban(iban))
}

/// ISO 13616 checksum: move the first four characters to the end, replace letters with
/// 10…35, and take the number modulo 97. A valid IBAN gives 1.
fn mod97(iban: &str) -> u32 {
    iban[4..]
        .chars()
        .chain(iban[..4].chars())
        .fold(0u32, |acc, c| {
            let value = c.to_digit(36).unwrap_or(0);
            if value < 10 {
                (acc * 10 + value) % 97
            } else {
                (acc * 100 + value) % 97
            }
        })
}

impl Iban {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Groups of four for display: `DE89 3704 0044 0532 0130 00`.
    pub fn formatted(&self) -> String {
        self.0
            .as_bytes()
            .chunks(4)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl TryFrom<String> for Iban {
    type Error = IbanError;

    fn try_from(s: String) -> Result<Iban, IbanError> {
        validate_iban(&s)
    }
}

impl From<Iban> for String {
    fn from(iban: Iban) -> String {
        iban.0
    }
}

/// Validates a BIC (ISO 9362) and returns it normalised.
pub fn validate_bic(input: &str) -> Result<String, BicError> {
    let bic = normalise(input);
    let bytes = bic.as_bytes();
    let shape_ok = matches!(bytes.len(), 8 | 11)
        && bytes[..6].iter().all(u8::is_ascii_uppercase)
        && bytes[6..].iter().all(u8::is_ascii_alphanumeric);
    if shape_ok {
        Ok(bic)
    } else {
        Err(BicError::InvalidFormat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn accepts_valid_ibans_in_any_spacing_and_case() {
        for input in [
            "DE89370400440532013000",
            "DE89 3704 0044 0532 0130 00",
            "de89 3704 0044 0532 0130 00",
            "AT611904300234573201",
            "NL91ABNA0417164300",
            "GB29NWBK60161331926819",
            "FR1420041010050500013M02606",
            "CH9300762011623852957",
            "NO9386011117947",
            "MT84MALT011000012345MTLCAST001S",
        ] {
            assert!(validate_iban(input).is_ok(), "{input}");
        }
    }

    #[test]
    fn normalises_and_formats() {
        let iban = validate_iban(" de89 3704\u{a0}0044 0532 0130 00 ").unwrap();
        assert_eq!(iban.as_str(), "DE89370400440532013000");
        assert_eq!(iban.formatted(), "DE89 3704 0044 0532 0130 00");
    }

    #[test]
    fn rejects_invalid_ibans() {
        assert_eq!(validate_iban("DE89-3704"), Err(IbanError::InvalidCharacter));
        assert_eq!(validate_iban("US12345678"), Err(IbanError::UnknownCountry));
        assert_eq!(validate_iban("D"), Err(IbanError::UnknownCountry));
        assert_eq!(validate_iban(""), Err(IbanError::UnknownCountry));
        assert_eq!(
            validate_iban("DE8937040044053201300"),
            Err(IbanError::WrongLength { expected: 22 })
        );
        assert_eq!(
            validate_iban("DE88370400440532013000"),
            Err(IbanError::BadChecksum)
        );
        assert_eq!(
            validate_iban("DEXX370400440532013000"),
            Err(IbanError::BadChecksum)
        );
    }

    #[test]
    fn serde_validates() {
        let iban: Iban = serde_json::from_str("\"DE89370400440532013000\"").unwrap();
        assert_eq!(
            serde_json::to_string(&iban).unwrap(),
            "\"DE89370400440532013000\""
        );
        assert!(serde_json::from_str::<Iban>("\"DE00370400440532013000\"").is_err());
    }

    #[test]
    fn validates_bics() {
        assert_eq!(validate_bic("COBADEFFXXX"), Ok("COBADEFFXXX".into()));
        assert_eq!(validate_bic("cobadeff"), Ok("COBADEFF".into()));
        assert_eq!(validate_bic("COBADEF"), Err(BicError::InvalidFormat));
        assert_eq!(validate_bic("COBA1EFF"), Err(BicError::InvalidFormat));
        assert_eq!(validate_bic("COBADEFFXX"), Err(BicError::InvalidFormat));
    }

    /// Builds a valid German IBAN for an 18-digit BBAN.
    fn german_iban(bban: &str) -> String {
        let check = 98 - mod97(&format!("DE00{bban}"));
        format!("DE{check:02}{bban}")
    }

    proptest! {
        #[test]
        fn generated_ibans_are_valid(bban in "[0-9]{18}") {
            prop_assert!(validate_iban(&german_iban(&bban)).is_ok());
        }

        #[test]
        fn any_single_digit_change_is_caught(bban in "[0-9]{18}", pos in 2usize..22, delta in 1u32..10) {
            let iban = german_iban(&bban);
            let mut chars: Vec<char> = iban.chars().collect();
            let digit = chars[pos].to_digit(10).unwrap();
            chars[pos] = char::from_digit((digit + delta) % 10, 10).unwrap();
            let typo: String = chars.into_iter().collect();
            prop_assert_eq!(validate_iban(&typo), Err(IbanError::BadChecksum));
        }

        #[test]
        fn swapping_two_adjacent_digits_is_caught(bban in "[0-9]{18}", pos in 4usize..21) {
            let iban = german_iban(&bban);
            let mut chars: Vec<char> = iban.chars().collect();
            prop_assume!(chars[pos] != chars[pos + 1]);
            chars.swap(pos, pos + 1);
            let typo: String = chars.into_iter().collect();
            prop_assert_eq!(validate_iban(&typo), Err(IbanError::BadChecksum));
        }
    }
}
