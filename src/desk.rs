//! Loan desk: decides what one scan (or typed entry) means. Pure: nothing is
//! modified here, the screen applies the decision through `Store::apply`.

use crate::codes::{parse_code_or_number, student_code, Kind, Scanned};
use crate::dates::format_date;
use crate::error::Error;
use crate::library::Library;
use crate::search::{fold, Query};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Empty entry.
    Nothing,
    /// A pupil card: this pupil now borrows.
    SelectStudent(u32),
    /// A free book while a pupil is selected.
    Lend { book: u32, student: u32 },
    /// A book that is on loan: offer to record its return.
    ProposeReturn { book: u32, loan: u32 },
    /// A free book, but no pupil selected yet.
    NeedStudent { book: u32 },
    /// Not a code: look for pupils by name.
    SearchName(String),
    /// Unknown or archived book or pupil.
    Refused(Error),
}

/// `student`: the pupil currently selected. A bare number means a pupil when
/// none is selected, and a book once a pupil is selected.
pub fn decide(lib: &Library, student: Option<u32>, input: &str) -> Decision {
    let text = input.trim();
    if text.is_empty() {
        return Decision::Nothing;
    }
    let expected = if student.is_some() {
        Kind::Book
    } else {
        Kind::Student
    };
    match parse_code_or_number(text, expected) {
        None => Decision::SearchName(text.to_string()),
        Some(Scanned::Student(id)) => match lib.student(id) {
            None => Decision::Refused(Error::StudentNotFound(id)),
            Some(s) if s.archived => Decision::Refused(Error::StudentArchived(id)),
            Some(_) => Decision::SelectStudent(id),
        },
        Some(Scanned::Book(id)) => match lib.book(id) {
            None => Decision::Refused(Error::BookNotFound(id)),
            Some(b) if b.archived => Decision::Refused(Error::BookArchived(id)),
            Some(_) => match (lib.open_loan_for_book(id), student) {
                (Some(loan), _) => Decision::ProposeReturn {
                    book: id,
                    loan: loan.id,
                },
                (None, Some(student)) => Decision::Lend { book: id, student },
                (None, None) => Decision::NeedStudent { book: id },
            },
        },
    }
}

/// Non-archived pupils matching `text` (name, first name, birth date or
/// code), sorted by name.
pub fn find_students(lib: &Library, text: &str) -> Vec<u32> {
    let query = Query::new(text);
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(String, u32)> = lib
        .students()
        .iter()
        .filter(|s| !s.archived)
        .filter(|s| {
            let code = student_code(s.id);
            let birth = format_date(s.birth_date);
            query.matches(&[&s.last_name, &s.first_name, &birth, &code])
        })
        .map(|s| (fold(&s.full_name()), s.id))
        .collect();
    found.sort();
    found.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::BookInput;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// Books 1 and 2, pupils 1 (Martin Léa) and 2 (Durand Tom); book 2 is lent to Tom.
    fn library() -> Library {
        let mut lib = Library::new();
        for title in ["Matilda", "Tintin"] {
            lib.add_book(BookInput {
                title: title.into(),
                ..Default::default()
            })
            .unwrap();
        }
        lib.add_student("Martin", "Léa", d(2017, 5, 14)).unwrap();
        lib.add_student("Durand", "Tom", d(2016, 11, 3)).unwrap();
        lib.lend(2, 2, d(2026, 10, 1)).unwrap();
        lib
    }

    #[test]
    fn pupil_card_selects_the_pupil() {
        let lib = library();
        assert_eq!(decide(&lib, None, "E000001"), Decision::SelectStudent(1));
        // another card replaces the current pupil
        assert_eq!(decide(&lib, Some(1), "e2"), Decision::SelectStudent(2));
        // a bare number is a pupil when none is selected
        assert_eq!(decide(&lib, None, "1"), Decision::SelectStudent(1));
    }

    #[test]
    fn free_book_is_lent_to_the_selected_pupil() {
        let lib = library();
        assert_eq!(
            decide(&lib, Some(1), "L000001"),
            Decision::Lend {
                book: 1,
                student: 1
            }
        );
        // a bare number is a book once a pupil is selected
        assert_eq!(
            decide(&lib, Some(1), " 1\r\n"),
            Decision::Lend {
                book: 1,
                student: 1
            }
        );
        assert_eq!(decide(&lib, None, "L1"), Decision::NeedStudent { book: 1 });
    }

    #[test]
    fn book_on_loan_proposes_its_return() {
        let lib = library();
        let loan = lib.open_loan_for_book(2).unwrap().id;
        let expected = Decision::ProposeReturn { book: 2, loan };
        assert_eq!(decide(&lib, None, "L000002"), expected);
        assert_eq!(decide(&lib, Some(1), "L000002"), expected);
    }

    #[test]
    fn azerty_scan_is_understood() {
        let lib = library();
        // L000001 typed by a US-layout scanner on an AZERTY Windows
        assert_eq!(
            decide(&lib, Some(1), "Lààààà&"),
            Decision::Lend {
                book: 1,
                student: 1
            }
        );
    }

    #[test]
    fn unknown_or_archived_items_are_refused() {
        let mut lib = library();
        assert_eq!(
            decide(&lib, None, "E99"),
            Decision::Refused(Error::StudentNotFound(99))
        );
        assert_eq!(
            decide(&lib, Some(1), "L99"),
            Decision::Refused(Error::BookNotFound(99))
        );
        lib.archive_book(1).unwrap();
        assert_eq!(
            decide(&lib, Some(1), "L1"),
            Decision::Refused(Error::BookArchived(1))
        );
        lib.archive_student(1).unwrap();
        assert_eq!(
            decide(&lib, None, "E1"),
            Decision::Refused(Error::StudentArchived(1))
        );
    }

    #[test]
    fn text_that_is_not_a_code_searches_by_name() {
        let lib = library();
        assert_eq!(decide(&lib, None, "  "), Decision::Nothing);
        assert_eq!(
            decide(&lib, None, " martin "),
            Decision::SearchName("martin".into())
        );
        // starts like a code but is a name
        assert_eq!(
            decide(&lib, None, "Léa"),
            Decision::SearchName("Léa".into())
        );
    }

    #[test]
    fn finds_pupils_by_name_without_accents() {
        let mut lib = library();
        lib.add_student("Bénard", "Chloé", d(2018, 2, 27)).unwrap();
        assert_eq!(find_students(&lib, "lea"), vec![1]);
        assert_eq!(find_students(&lib, "benard chloe"), vec![3]);
        assert_eq!(find_students(&lib, "E000002"), vec![2]);
        // every pupil whose name contains "r", sorted by last name
        assert_eq!(find_students(&lib, "r"), vec![3, 2, 1]);
        assert!(find_students(&lib, "").is_empty());
        lib.archive_student(1).unwrap();
        assert!(find_students(&lib, "lea").is_empty());
    }
}
