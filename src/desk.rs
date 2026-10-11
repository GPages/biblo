//! Loan desk: decides what one scan (or typed entry) means. Pure: nothing is
//! modified here, the screen applies the decision through `Store::apply`.
//!
//! Lending and returning are two separate modes, so the same scan never
//! means "lend" in one case and "return" in another: in lend mode a book
//! can only be lent, in return mode it can only be taken back.

use crate::codes::{parse_code_or_number, student_code, Kind, Scanned};
use crate::dates::format_date;
use crate::error::Error;
use crate::library::Library;
use crate::search::{fold, Query};

/// What a scan means in lend mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LendDecision {
    /// Empty entry.
    Nothing,
    /// A pupil card: this pupil now borrows.
    SelectStudent(u32),
    /// A free book while a pupil is selected.
    Lend { book: u32, student: u32 },
    /// A book that is on loan: it must be returned (return mode) first.
    AlreadyLent { book: u32, loan: u32 },
    /// A free book, but no pupil selected yet.
    NeedStudent { book: u32 },
    /// Not a code: look for pupils by name.
    SearchName(String),
    /// Unknown pupil, or unknown or archived book.
    Refused(Error),
}

/// What a scan means in return mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnDecision {
    /// Empty entry.
    Nothing,
    /// A book on loan: record its return.
    Return { book: u32, loan: u32 },
    /// A book that is not on loan: nothing to return.
    NotLent { book: u32 },
    /// A pupil card: returns only take books.
    StudentCard(u32),
    /// Neither a code nor a number.
    NotACode(String),
    /// Unknown or archived book.
    Refused(Error),
}

/// Lend mode. `student`: the pupil currently selected. A bare number means a
/// pupil when none is selected, and a book once a pupil is selected.
pub fn decide_lend(lib: &Library, student: Option<u32>, input: &str) -> LendDecision {
    let text = input.trim();
    if text.is_empty() {
        return LendDecision::Nothing;
    }
    let expected = if student.is_some() {
        Kind::Book
    } else {
        Kind::Student
    };
    match parse_code_or_number(text, expected) {
        None => LendDecision::SearchName(text.to_string()),
        Some(Scanned::Student(id)) => match lib.student(id) {
            None => LendDecision::Refused(Error::StudentNotFound(id)),
            Some(_) => LendDecision::SelectStudent(id),
        },
        Some(Scanned::Book(id)) => match lib.book(id) {
            None => LendDecision::Refused(Error::BookNotFound(id)),
            Some(b) if b.archived => LendDecision::Refused(Error::BookArchived(id)),
            Some(_) => match (lib.open_loan_for_book(id), student) {
                (Some(loan), _) => LendDecision::AlreadyLent {
                    book: id,
                    loan: loan.id,
                },
                (None, Some(student)) => LendDecision::Lend { book: id, student },
                (None, None) => LendDecision::NeedStudent { book: id },
            },
        },
    }
}

/// Return mode: only books are expected, so a bare number is a book.
pub fn decide_return(lib: &Library, input: &str) -> ReturnDecision {
    let text = input.trim();
    if text.is_empty() {
        return ReturnDecision::Nothing;
    }
    match parse_code_or_number(text, Kind::Book) {
        None => ReturnDecision::NotACode(text.to_string()),
        Some(Scanned::Student(id)) => ReturnDecision::StudentCard(id),
        Some(Scanned::Book(id)) => match (lib.book(id), lib.open_loan_for_book(id)) {
            (None, _) => ReturnDecision::Refused(Error::BookNotFound(id)),
            (Some(_), Some(loan)) => ReturnDecision::Return {
                book: id,
                loan: loan.id,
            },
            (Some(b), None) if b.archived => ReturnDecision::Refused(Error::BookArchived(id)),
            (Some(_), None) => ReturnDecision::NotLent { book: id },
        },
    }
}

/// Pupils matching `text` (name, first name, birth date, class or
/// code), sorted by name.
pub fn find_students(lib: &Library, text: &str) -> Vec<u32> {
    let query = Query::new(text);
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(String, u32)> = lib
        .students()
        .iter()
        .filter(|s| {
            let code = student_code(s.id);
            let birth = format_date(s.birth_date);
            query.matches(&[&s.last_name, &s.first_name, &birth, &s.class, &code])
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
        lib.add_student("Martin", "Léa", d(2017, 5, 14), d(2025, 9, 1))
            .unwrap();
        lib.add_student("Durand", "Tom", d(2016, 11, 3), d(2025, 9, 1))
            .unwrap();
        lib.lend(2, 2, d(2026, 10, 1)).unwrap();
        lib
    }

    #[test]
    fn pupil_card_selects_the_pupil() {
        let lib = library();
        assert_eq!(
            decide_lend(&lib, None, "E000001"),
            LendDecision::SelectStudent(1)
        );
        // another card replaces the current pupil
        assert_eq!(
            decide_lend(&lib, Some(1), "e2"),
            LendDecision::SelectStudent(2)
        );
        // a bare number is a pupil when none is selected
        assert_eq!(decide_lend(&lib, None, "1"), LendDecision::SelectStudent(1));
    }

    #[test]
    fn free_book_is_lent_to_the_selected_pupil() {
        let lib = library();
        assert_eq!(
            decide_lend(&lib, Some(1), "L000001"),
            LendDecision::Lend {
                book: 1,
                student: 1
            }
        );
        // a bare number is a book once a pupil is selected
        assert_eq!(
            decide_lend(&lib, Some(1), " 1\r\n"),
            LendDecision::Lend {
                book: 1,
                student: 1
            }
        );
        assert_eq!(
            decide_lend(&lib, None, "L1"),
            LendDecision::NeedStudent { book: 1 }
        );
    }

    #[test]
    fn lend_mode_never_returns_a_book() {
        let lib = library();
        let loan = lib.open_loan_for_book(2).unwrap().id;
        let expected = LendDecision::AlreadyLent { book: 2, loan };
        assert_eq!(decide_lend(&lib, None, "L000002"), expected);
        assert_eq!(decide_lend(&lib, Some(1), "L000002"), expected);
    }

    #[test]
    fn return_mode_takes_back_books_only() {
        let lib = library();
        let loan = lib.open_loan_for_book(2).unwrap().id;
        let expected = ReturnDecision::Return { book: 2, loan };
        assert_eq!(decide_return(&lib, "L000002"), expected);
        // a bare number is always a book here
        assert_eq!(decide_return(&lib, " 2\r\n"), expected);
        // book 1 is on the shelf: nothing to return
        assert_eq!(
            decide_return(&lib, "L1"),
            ReturnDecision::NotLent { book: 1 }
        );
        assert_eq!(
            decide_return(&lib, "E000001"),
            ReturnDecision::StudentCard(1)
        );
        assert_eq!(
            decide_return(&lib, "Léa"),
            ReturnDecision::NotACode("Léa".into())
        );
        assert_eq!(decide_return(&lib, "  "), ReturnDecision::Nothing);
    }

    #[test]
    fn azerty_scan_is_understood() {
        let lib = library();
        // L000001 typed by a US-layout scanner on an AZERTY Windows
        assert_eq!(
            decide_lend(&lib, Some(1), "Lààààà&"),
            LendDecision::Lend {
                book: 1,
                student: 1
            }
        );
        let loan = lib.open_loan_for_book(2).unwrap().id;
        assert_eq!(
            decide_return(&lib, "Làààààé"),
            ReturnDecision::Return { book: 2, loan }
        );
    }

    #[test]
    fn unknown_or_archived_items_are_refused() {
        let mut lib = library();
        assert_eq!(
            decide_lend(&lib, None, "E99"),
            LendDecision::Refused(Error::StudentNotFound(99))
        );
        assert_eq!(
            decide_lend(&lib, Some(1), "L99"),
            LendDecision::Refused(Error::BookNotFound(99))
        );
        assert_eq!(
            decide_return(&lib, "L99"),
            ReturnDecision::Refused(Error::BookNotFound(99))
        );
        lib.archive_book(1).unwrap();
        assert_eq!(
            decide_lend(&lib, Some(1), "L1"),
            LendDecision::Refused(Error::BookArchived(1))
        );
        assert_eq!(
            decide_return(&lib, "L1"),
            ReturnDecision::Refused(Error::BookArchived(1))
        );
        lib.delete_student(1).unwrap();
        assert_eq!(
            decide_lend(&lib, None, "E1"),
            LendDecision::Refused(Error::StudentNotFound(1))
        );
    }

    #[test]
    fn text_that_is_not_a_code_searches_by_name() {
        let lib = library();
        assert_eq!(decide_lend(&lib, None, "  "), LendDecision::Nothing);
        assert_eq!(
            decide_lend(&lib, None, " martin "),
            LendDecision::SearchName("martin".into())
        );
        // starts like a code but is a name
        assert_eq!(
            decide_lend(&lib, None, "Léa"),
            LendDecision::SearchName("Léa".into())
        );
    }

    #[test]
    fn finds_pupils_by_name_without_accents() {
        let mut lib = library();
        lib.add_student("Bénard", "Chloé", d(2018, 2, 27), d(2025, 9, 1))
            .unwrap();
        assert_eq!(find_students(&lib, "lea"), vec![1]);
        assert_eq!(find_students(&lib, "benard chloe"), vec![3]);
        assert_eq!(find_students(&lib, "E000002"), vec![2]);
        // the class tells two pupils with the same first name apart
        lib.add_student("Petit", "Léa", d(2019, 1, 2), d(2025, 9, 1))
            .unwrap();
        lib.set_student_class(1, "CM1").unwrap();
        assert_eq!(find_students(&lib, "lea"), vec![1, 4]);
        assert_eq!(find_students(&lib, "lea cm1"), vec![1]);
        // every pupil whose name contains "r", sorted by last name
        assert_eq!(find_students(&lib, "r"), vec![3, 2, 1]);
        assert!(find_students(&lib, "").is_empty());
        lib.delete_student(1).unwrap();
        assert_eq!(find_students(&lib, "lea"), vec![4]);
    }
}
