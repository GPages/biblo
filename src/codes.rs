//! Format des cartes d'élève et interprétation de ce qui arrive du lecteur.
//!
//! Un élève est `E000045` (préfixe + numéro sur 6 chiffres). Les livres n'ont
//! pas de code à eux : on scanne leur ISBN (voir `isbn`).
//!
//! Le lecteur Inateck se comporte comme un clavier : il « tape » le code puis
//! Entrée. Si le lecteur est configuré en disposition US alors que Windows est
//! en AZERTY, les chiffres arrivent sous forme de `& é " ' ( - è _ ç à`
//! (touches de la rangée du haut, sans Maj). `normalize` corrige ça.
//! Le préfixe `E` est à la même place en QWERTY et AZERTY, donc jamais altéré.

pub const STUDENT_PREFIX: char = 'E';

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

/// Le numéro d'élève d'une carte (`E000045`, `e45`) ou d'un numéro seul
/// (`45`, utile pour la saisie manuelle quand le lecteur est HS).
pub fn parse_student(input: &str) -> Option<u32> {
    let s = normalize(input);
    match s.strip_prefix(STUDENT_PREFIX) {
        Some(digits) => parse_id(digits),
        None => parse_id(&s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_student_codes() {
        assert_eq!(student_code(45), "E000045");
    }

    #[test]
    fn parses_cards_and_bare_numbers() {
        assert_eq!(parse_student("E000045"), Some(45));
        assert_eq!(parse_student("  e45\r\n"), Some(45));
        assert_eq!(parse_student("45"), Some(45));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_student(""), None);
        assert_eq!(parse_student("E"), None);
        assert_eq!(parse_student("X123"), None);
        assert_eq!(parse_student("E12x"), None);
        assert_eq!(parse_student("EE12"), None);
        assert_eq!(parse_student("E99999999999"), None);
        // an old book label is not a pupil
        assert_eq!(parse_student("L000123"), None);
        // an ISBN is too long to be a pupil number
        assert_eq!(parse_student("9782070612758"), None);
    }

    #[test]
    fn recovers_from_us_layout_scanner_on_azerty() {
        // E000045 tapé par un lecteur US sur un Windows AZERTY
        assert_eq!(parse_student("Eàààà'("), Some(45));
        // 123456789 : toute la rangée de chiffres sauf le 0
        assert_eq!(parse_student("E&é\"'(-è_ç"), Some(123_456_789));
    }
}
