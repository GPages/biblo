//! Live filtering: case- and accent-insensitive, every word of the query must
//! appear in at least one field ("petit exupery" finds "Le Petit Prince" by
//! "Saint-Exupéry").

/// Lowercase and strip French accents (done by hand, no extra crate).
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => out.push('a'),
            'ç' => out.push('c'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'î' | 'ï' | 'í' | 'ì' => out.push('i'),
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => out.push('o'),
            'ù' | 'û' | 'ü' | 'ú' => out.push('u'),
            'ÿ' | 'ý' => out.push('y'),
            'ñ' => out.push('n'),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            '’' | '‘' => out.push('\''),
            other => out.push(other),
        }
    }
    out
}

/// A parsed search query. Build it once per frame, then test every row.
pub struct Query {
    words: Vec<String>,
}

impl Query {
    pub fn new(text: &str) -> Query {
        Query {
            words: fold(text).split_whitespace().map(str::to_string).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn matches(&self, fields: &[&str]) -> bool {
        if self.words.is_empty() {
            return true;
        }
        // Words contain no whitespace, so joining with '\n' never lets a word
        // match across two fields.
        let haystack = fold(&fields.join("\n"));
        self.words.iter().all(|w| haystack.contains(w.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_french_accents() {
        assert_eq!(fold("Élève À Côté"), "eleve a cote");
        assert_eq!(fold("Saint-Exupéry"), "saint-exupery");
        assert_eq!(fold("ŒUVRE Cœur"), "oeuvre coeur");
        assert_eq!(fold("Noël, Ça, L’été"), "noel, ca, l'ete");
    }

    #[test]
    fn empty_query_matches_everything() {
        let q = Query::new("   ");
        assert!(q.is_empty());
        assert!(q.matches(&[]));
        assert!(q.matches(&["x"]));
    }

    #[test]
    fn every_word_must_match_some_field() {
        let fields = [
            "Le Petit Prince",
            "Saint-Exupéry",
            "Folio",
            "1943",
            "L000012",
        ];
        assert!(Query::new("petit").matches(&fields));
        assert!(Query::new("PETIT exupery").matches(&fields));
        assert!(Query::new("1943 folio").matches(&fields));
        assert!(Query::new("l000012").matches(&fields));
        assert!(Query::new("12").matches(&fields));
        assert!(!Query::new("petit tintin").matches(&fields));
    }

    #[test]
    fn a_word_cannot_span_two_fields() {
        // "prince" ends field 1 and "saint" starts field 2
        let fields = ["Le Petit Prince", "Saint-Exupéry"];
        assert!(!Query::new("princesaint").matches(&fields));
    }
}
