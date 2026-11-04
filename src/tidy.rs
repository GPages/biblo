//! Cleaning up titles from the ISBN database. They come from online shops:
//! lower case, or every word capitalised, a spare " - [Livre en VO]" at the
//! end, a missing space after a comma, and sometimes characters lost to a bad
//! encoding. `tidy_title` fixes what can be fixed without guessing; what it
//! cannot fix is left alone, and `is_garbled` tells the screen to say so.

/// Words that stay in lower case inside a title written in Title Case.
const SMALL_WORDS: &[&str] = &[
    // French
    "le", "la", "les", "un", "une", "des", "de", "du", "d", "l", "et", "en", "au", "aux", "à",
    "pour", "par", "sur", "sous", "dans", "avec", "sans", "ou", "ni", "ce", "ces", "cet", "cette",
    // English
    "the", "a", "an", "of", "and", "in", "on", "for", "to", "at", "by", "with", "from", "or",
];

/// Trailing parts added by the shops, which are not part of the title.
const SHOP_LABELS: &[&str] = &[
    "[",
    "type ",
    "livre",
    "ebook",
    "e-book",
    "version originale",
    "format ",
    "poche",
    "broché",
    "paperback",
    "hardcover",
    "hardback",
    "relié",
];

/// Words whose « œ » was lost (replaced by `¿`): the one repair that is safe,
/// because only one word fits. (Then spelled "oe", like every « œ ».)
const OE_WORDS: &[(&str, &str)] = &[
    ("v¿ux", "vœux"),
    ("v¿u", "vœu"),
    ("c¿ur", "cœur"),
    ("c¿urs", "cœurs"),
    ("s¿ur", "sœur"),
    ("s¿urs", "sœurs"),
    ("¿uvre", "œuvre"),
    ("¿uvres", "œuvres"),
    ("¿il", "œil"),
    ("b¿uf", "bœuf"),
    ("n¿ud", "nœud"),
    ("n¿uds", "nœuds"),
    ("¿uf", "œuf"),
    ("¿ufs", "œufs"),
    ("m¿urs", "mœurs"),
    ("ch¿ur", "chœur"),
    ("ch¿urs", "chœurs"),
    ("m¿ur", "mœur"),
];

/// Characters that were lost and cannot be guessed.
pub fn is_garbled(title: &str) -> bool {
    title.contains('¿') || title.contains('\u{fffd}')
}

pub fn tidy_title(title: &str) -> String {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = strip_shop_labels(&title);
    let title = repair_oe(&title);
    let title = spell_out_oe(&title);
    let title = space_after_commas(&title);
    fix_case(&title)
}

/// " - [Livre en VO]", " - Type Rose Harness-L", " - ebook (ePub) - Livre"…
/// at the end, and a final `[Version Originale]` tag. A real subtitle after
/// " - " is kept.
fn strip_shop_labels(title: &str) -> String {
    let mut parts: Vec<&str> = title.split(" - ").collect();
    while parts.len() > 1 {
        let last = parts[parts.len() - 1].to_lowercase();
        if SHOP_LABELS.iter().any(|label| last.starts_with(label)) {
            parts.pop();
        } else {
            break;
        }
    }
    let mut out = parts.join(" - ");
    // A final tag without the dash: "Matilda [Livre en VO]".
    if let (true, Some(open)) = (out.ends_with(']'), out.rfind('[')) {
        let tag = out[open..].to_lowercase();
        if open > 0
            && ["livre", "version", "vo]", "ebook", "format"]
                .iter()
                .any(|w| tag.contains(w))
        {
            out.truncate(open);
        }
    }
    out.trim_end_matches([' ', '-', ',', ':', ';', '/'])
        .to_string()
}

fn repair_oe(title: &str) -> String {
    if !title.contains('¿') {
        return title.to_string();
    }
    let mut out = String::with_capacity(title.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let lower = word.to_lowercase();
        match OE_WORDS.iter().find(|(broken, _)| *broken == lower) {
            Some((_, fixed)) => out.push_str(&match_case(word, fixed)),
            None => out.push_str(word),
        }
        word.clear();
    };
    for c in title.chars() {
        if c.is_alphabetic() || c == '¿' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// « œ » is written "oe": that is what people type when they search.
fn spell_out_oe(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    let mut out = String::with_capacity(title.len() + 2);
    for (i, &c) in chars.iter().enumerate() {
        match c {
            'œ' => out.push_str("oe"),
            // "ŒUVRES" → "OEUVRES", "Œil" → "Oeil"
            'Œ' if chars.get(i + 1).is_some_and(|n| n.is_uppercase()) => out.push_str("OE"),
            'Œ' => out.push_str("Oe"),
            _ => out.push(c),
        }
    }
    out
}

/// `fixed` written like `original`: capitalised if it was, upper case if it was.
fn match_case(original: &str, fixed: &str) -> String {
    let letters: Vec<char> = original.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return fixed.to_uppercase();
    }
    match original.chars().next() {
        Some(first) if first.is_uppercase() => capitalize(fixed),
        _ => fixed.to_string(),
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "Kraft,Étiquette" → "Kraft, Étiquette" (but not "6,5").
fn space_after_commas(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    let mut out = String::with_capacity(title.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        if c == ',' && chars.get(i + 1).is_some_and(|n| n.is_alphabetic()) {
            out.push(' ');
        }
    }
    out
}

fn fix_case(title: &str) -> String {
    let has_upper = title.chars().any(char::is_uppercase);
    let has_lower = title.chars().any(char::is_lowercase);
    let letters = title.chars().filter(|c| c.is_alphabetic()).count();
    if !has_upper && has_lower {
        // "la vie de saint serge": only the first letter.
        return capitalize(title);
    }
    if has_upper && !has_lower && letters > 5 {
        // "CARTAS PERSAS": shouting. Words with a dot or a digit ("R.E.M.",
        // "2E") are kept.
        let lowered: Vec<String> = title
            .split(' ')
            .map(|w| {
                if w.chars().any(|c| c.is_ascii_digit() || c == '.') {
                    w.to_string()
                } else {
                    w.to_lowercase()
                }
            })
            .collect();
        return capitalize(&lowered.join(" "));
    }
    if is_title_case(title) {
        return lower_small_words(title);
    }
    title.to_string()
}

/// Every word starts with a capital, small words included ("Cartes De Vœux
/// En Papier"): the shops' style, not a title's.
fn is_title_case(title: &str) -> bool {
    let words: Vec<&str> = title
        .split(' ')
        .filter(|w| w.chars().any(char::is_alphabetic))
        .collect();
    if words.len() < 3 {
        return false;
    }
    // "Papier", not "PAPIER" nor "McDonald": a title that mixes other styles
    // in is left alone.
    let capital = |w: &str| {
        // "D'Anniversaire": each side of the apostrophe on its own.
        w.split(['\'', '’'])
            .filter(|p| p.chars().any(char::is_alphabetic))
            .all(|part| {
                let mut letters = part.chars().filter(|c| c.is_alphabetic());
                letters.next().is_some_and(char::is_uppercase) && !letters.any(char::is_uppercase)
            })
    };
    let small_capitalised = words.iter().skip(1).any(|w| is_small(w) && capital(w));
    words.iter().all(|w| capital(w)) && small_capitalised
}

/// "De", "D'" (of "D'Anniversaire")…
fn is_small(word: &str) -> bool {
    let bare = word
        .split(['\'', '’'])
        .next()
        .unwrap_or(word)
        .trim_matches(|c: char| !c.is_alphabetic())
        .to_lowercase();
    let elision = word.contains(['\'', '’']);
    SMALL_WORDS.contains(&bare.as_str()) && (!elision || ["d", "l"].contains(&bare.as_str()))
}

fn lower_small_words(title: &str) -> String {
    let mut after_break = true;
    let words: Vec<String> = title
        .split(' ')
        .map(|word| {
            let keep = after_break || !is_small(word);
            // A word ending a sentence-like part (":") starts the next one.
            after_break = word.ends_with([':', '.', '!', '?']);
            if keep {
                word.to_string()
            } else {
                // "D'Anniversaire" → "d'Anniversaire": only the small part.
                match word.find(['\'', '’']) {
                    Some(at) => {
                        let (head, tail) = word.split_at(at);
                        format!("{}{tail}", head.to_lowercase())
                    }
                    None => word.to_lowercase(),
                }
            }
        })
        .collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_labels_at_the_end_are_removed_but_subtitles_stay() {
        assert_eq!(
            tidy_title("Weird Facts About the Human Body - [Version Originale]"),
            "Weird Facts About the Human Body"
        );
        assert_eq!(
            tidy_title("Cine Y Ciencias - [Livre en VO]"),
            "Cine Y Ciencias"
        );
        assert_eq!(tidy_title("Matilda [Livre en VO]"), "Matilda");
        assert_eq!(
            tidy_title("Harnais pour chien - Type Rose Harness-L"),
            "Harnais pour chien"
        );
        assert_eq!(
            tidy_title("Heliophysics - Cambridge University Press - Livre en Anglais - Paperback"),
            "Heliophysics - Cambridge University Press"
        );
        assert_eq!(
            tidy_title("Tintin - Le Lotus bleu"),
            "Tintin - Le Lotus bleu"
        );
        // a tag in the middle is part of the title
        assert_eq!(
            tidy_title("Mémoires [!] inconnues"),
            "Mémoires [!] inconnues"
        );
    }

    #[test]
    fn lost_oe_is_restored_only_where_one_word_fits() {
        assert_eq!(
            tidy_title("Cartes De V¿Ux En Papier"),
            "Cartes de Voeux en Papier"
        );
        assert_eq!(
            tidy_title("cartes de v¿ux d'anniversaire"),
            "Cartes de voeux d'anniversaire"
        );
        assert_eq!(tidy_title("Mon c¿ur"), "Mon coeur");
        // ¿¿ could be é, è, ê…: left alone, and reported
        let garbled = tidy_title("Remplacement de l'¿¿cran");
        assert!(is_garbled(&garbled));
        assert!(!is_garbled("Cartes de voeux"));
        // a real œ is spelled "oe" too
        assert_eq!(tidy_title("Le Cœur et l'Œuvre"), "Le Coeur et l'Oeuvre");
        assert_eq!(
            tidy_title("ŒUVRES COMPLÈTES DE MOLIÈRE"),
            "Oeuvres complètes de molière"
        );
        assert_eq!(tidy_title("Œil pour œil"), "Oeil pour oeil");
    }

    #[test]
    fn commas_get_a_space_but_decimals_do_not() {
        assert_eq!(
            tidy_title("Kraft,Étiquette de remerciement"),
            "Kraft, Étiquette de remerciement"
        );
        assert_eq!(
            tidy_title("écran de 6,5 pouces, avec outils"),
            "Écran de 6,5 pouces, avec outils"
        );
    }

    #[test]
    fn lower_case_and_shouting_titles_are_capitalised() {
        assert_eq!(
            tidy_title("la vie de saint serge de radonège racontée aux enfants"),
            "La vie de saint serge de radonège racontée aux enfants"
        );
        assert_eq!(
            tidy_title("CUERPO Y PSICOANÁLISIS: POR UN PSICOANÁLISIS MAS ACTIVO"),
            "Cuerpo y psicoanálisis: por un psicoanálisis mas activo"
        );
        // too short, or with dots: left as they are
        assert_eq!(tidy_title("R.E.M."), "R.E.M.");
        assert_eq!(tidy_title("AI"), "AI");
    }

    #[test]
    fn title_case_gets_its_small_words_back_in_lower_case() {
        assert_eq!(
            tidy_title("Cartes De Voeux En Papier Kraft"),
            "Cartes de Voeux en Papier Kraft"
        );
        assert_eq!(tidy_title("The Lord Of The Rings"), "The Lord of the Rings");
        assert_eq!(
            tidy_title("Carte D'Anniversaire De Mariage"),
            "Carte d'Anniversaire de Mariage"
        );
        assert_eq!(
            tidy_title("Roman: Le Retour De La Reine"),
            "Roman: Le Retour de la Reine"
        );
    }

    #[test]
    fn good_titles_are_not_touched() {
        for title in [
            "Le Petit Prince",
            "Harry Potter à l'école des sorciers",
            "L'épreuve du feu",
            "Les Contes du chat perché",
            "Astérix le Gaulois",
            "Vingt mille lieues sous les mers",
            "Charlie et la chocolaterie",
        ] {
            assert_eq!(tidy_title(title), title);
        }
    }
}
