//! Format des codes-barres et interprétation de ce qui arrive du lecteur.
//!
//! Un livre est `L000123`, un élève `E000045` (préfixe + numéro sur 6 chiffres).
//! Le lecteur Inateck se comporte comme un clavier : il « tape » le code puis
//! Entrée. Si le lecteur est configuré en disposition US alors que Windows est
//! en AZERTY, les chiffres arrivent sous forme de `& é " ' ( - è _ ç à`
//! (touches de la rangée du haut, sans Maj). `normalize` corrige ça.
//! Les préfixes `L` et `E` sont à la même place en QWERTY et AZERTY, donc
//! jamais altérés.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Book,
    Student,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scanned {
    Book(u32),
    Student(u32),
}

pub const BOOK_PREFIX: char = 'L';
pub const STUDENT_PREFIX: char = 'E';

pub fn book_code(id: u32) -> String {
    format!("{}{:06}", BOOK_PREFIX, id)
}

pub fn student_code(id: u32) -> String {
    format!("{}{:06}", STUDENT_PREFIX, id)
}

/// Retire espaces/retours chariot, remet les chiffres AZERTY-sans-Maj en
/// chiffres, passe les lettres en majuscules.
fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .map(|c| match c {
            '&' => '1',
            'é' => '2',
            '"' => '3',
            '\'' => '4',
            '(' => '5',
            '-' => '6',
            'è' => '7',
            '_' => '8',
            'ç' => '9',
            'à' => '0',
            other => other.to_ascii_uppercase(),
        })
        .collect()
}

fn parse_id(digits: &str) -> Option<u32> {
    if digits.is_empty() || digits.len() > 9 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Lit un code complet avec préfixe (`L123`, `l000123`, `E45`…).
pub fn parse_code(input: &str) -> Option<Scanned> {
    let s = normalize(input);
    let mut chars = s.chars();
    let prefix = chars.next()?;
    let id = parse_id(chars.as_str())?;
    match prefix {
        BOOK_PREFIX => Some(Scanned::Book(id)),
        STUDENT_PREFIX => Some(Scanned::Student(id)),
        _ => None,
    }
}

/// Comme `parse_code`, mais accepte aussi un numéro seul (`123`), interprété
/// selon `default` (utile pour la saisie manuelle quand le lecteur est HS).
pub fn parse_code_or_number(input: &str, default: Kind) -> Option<Scanned> {
    if let Some(s) = parse_code(input) {
        return Some(s);
    }
    let id = parse_id(&normalize(input))?;
    Some(match default {
        Kind::Book => Scanned::Book(id),
        Kind::Student => Scanned::Student(id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_codes() {
        assert_eq!(book_code(123), "L000123");
        assert_eq!(student_code(45), "E000045");
    }

    #[test]
    fn parses_full_codes() {
        assert_eq!(parse_code("L000123"), Some(Scanned::Book(123)));
        assert_eq!(parse_code("l123"), Some(Scanned::Book(123)));
        assert_eq!(parse_code("E000045"), Some(Scanned::Student(45)));
        assert_eq!(parse_code("  E45\r\n"), Some(Scanned::Student(45)));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_code(""), None);
        assert_eq!(parse_code("L"), None);
        assert_eq!(parse_code("123"), None);
        assert_eq!(parse_code("X123"), None);
        assert_eq!(parse_code("L12x"), None);
        assert_eq!(parse_code("LL12"), None);
        assert_eq!(parse_code("L99999999999"), None);
    }

    #[test]
    fn bare_numbers_use_default_kind() {
        assert_eq!(parse_code_or_number("123", Kind::Book), Some(Scanned::Book(123)));
        assert_eq!(
            parse_code_or_number("123", Kind::Student),
            Some(Scanned::Student(123))
        );
        // Un préfixe explicite l'emporte sur le défaut.
        assert_eq!(
            parse_code_or_number("E7", Kind::Book),
            Some(Scanned::Student(7))
        );
        assert_eq!(parse_code_or_number("abc", Kind::Book), None);
    }

    #[test]
    fn recovers_from_us_layout_scanner_on_azerty() {
        // L000123 tapé par un lecteur US sur un Windows AZERTY
        assert_eq!(parse_code("Lààà&é\""), Some(Scanned::Book(123)));
        // E000045
        assert_eq!(parse_code("Eàààà'("), Some(Scanned::Student(45)));
        // 123456789 : toute la rangée de chiffres sauf le 0
        assert_eq!(
            parse_code("L&é\"'(-è_ç"),
            Some(Scanned::Book(123_456_789))
        );
    }
}
