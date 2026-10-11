//! Dates strictes, affichées et saisies au format jj/mm/aaaa.

use chrono::{Datelike, NaiveDate};

use crate::library::{MAX_YEAR, MIN_YEAR};

/// Chiffres uniquement, longueur 1..=max_len.
fn digits(s: &str, max_len: usize) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() || s.len() > max_len || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Lit `jj/mm/aaaa` : séparateurs `/`, `-`, `.` ou espace, ou bien 8 chiffres
/// collés `jjmmaaaa` (`18041992`). Année sur 4 chiffres obligatoire ; date
/// réelle obligatoire : le 31/02 est refusé.
pub fn parse_date(input: &str) -> Option<NaiveDate> {
    let s = input.trim();
    let parts: Vec<&str> = if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        vec![&s[0..2], &s[2..4], &s[4..8]]
    } else {
        s.split(|c: char| matches!(c, '/' | '-' | '.') || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .collect()
    };
    if parts.len() != 3 || parts[2].len() != 4 {
        return None;
    }
    let day = digits(parts[0], 2)?;
    let month = digits(parts[1], 2)?;
    let year = digits(parts[2], 4)? as i32;
    NaiveDate::from_ymd_opt(year, month, day)
}

/// `jj/mm/aaaa`
pub fn format_date(d: NaiveDate) -> String {
    format!("{:02}/{:02}/{:04}", d.day(), d.month(), d.year())
}

/// Année de parution : 4 chiffres, entre `MIN_YEAR` et `MAX_YEAR`.
pub fn parse_year(input: &str) -> Option<i32> {
    let s = input.trim();
    if s.len() != 4 {
        return None;
    }
    let y = digits(s, 4)? as i32;
    if (MIN_YEAR..=MAX_YEAR).contains(&y) {
        Some(y)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn parses_valid_dates() {
        assert_eq!(parse_date("05/03/2026"), Some(d(2026, 3, 5)));
        assert_eq!(parse_date("5/3/2026"), Some(d(2026, 3, 5)));
        assert_eq!(parse_date(" 05-03-2026 "), Some(d(2026, 3, 5)));
        assert_eq!(parse_date("05.03.2026"), Some(d(2026, 3, 5)));
        assert_eq!(parse_date("29/02/2028"), Some(d(2028, 2, 29)));
    }

    #[test]
    fn parses_dates_without_slashes() {
        assert_eq!(parse_date("18041992"), Some(d(1992, 4, 18)));
        assert_eq!(parse_date(" 05032026 "), Some(d(2026, 3, 5)));
        assert_eq!(parse_date("18 04 1992"), Some(d(1992, 4, 18)));
        assert_eq!(parse_date("18 / 04 / 1992"), Some(d(1992, 4, 18)));
        assert_eq!(parse_date("1804199"), None);
        assert_eq!(parse_date("180419920"), None);
        assert_eq!(parse_date("31022026"), None);
        assert_eq!(parse_date("00012026"), None);
        assert_eq!(parse_date("1804 1992"), None);
    }

    #[test]
    fn rejects_invalid_dates() {
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("31/02/2026"), None);
        assert_eq!(parse_date("29/02/2027"), None);
        assert_eq!(parse_date("32/01/2026"), None);
        assert_eq!(parse_date("01/13/2026"), None);
        assert_eq!(parse_date("01/01/26"), None);
        assert_eq!(parse_date("01/01/+202"), None);
        assert_eq!(parse_date("01/01"), None);
        assert_eq!(parse_date("a/b/cccc"), None);
        assert_eq!(parse_date("2026-03-05"), None);
    }

    #[test]
    fn formats_dates() {
        assert_eq!(format_date(d(2026, 3, 5)), "05/03/2026");
    }

    #[test]
    fn parses_years() {
        assert_eq!(parse_year("2019"), Some(2019));
        assert_eq!(parse_year(" 1999 "), Some(1999));
        assert_eq!(parse_year("19"), None);
        assert_eq!(parse_year("20190"), None);
        assert_eq!(parse_year("0999"), None);
        assert_eq!(parse_year("abcd"), None);
    }
}
