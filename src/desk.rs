//! Loan desk: what an entry means, and what can be done with the pupil and
//! the book chosen on the screen. Pure: nothing is modified here, the screen
//! applies the choice through `Store::apply`.
//!
//! The screen has a pupil field and a book field, side by side. A scanned
//! code goes where it belongs whatever the field (`E…` to the pupil, `L…` to
//! the book); a bare number or a name is read for the field it was typed in.

use crate::codes::{book_code, parse_code_or_number, student_code, Kind, Scanned};
use crate::dates::format_date;
use crate::error::Error;
use crate::library::Library;
use crate::search::{fold, Query};

/// The field an entry was typed or scanned into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Student,
    Book,
}

/// What one entry means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// Empty entry.
    Nothing,
    /// A known pupil (card scanned, code or number typed).
    Student(u32),
    /// A known book that is still in the catalogue.
    Book(u32),
    /// Not a code: a pupil's name or a book's title to look for.
    Search(Field, String),
    /// Unknown pupil, or unknown or archived book.
    Refused(Error),
}

/// What can be done with the chosen pupil and book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposal {
    /// The pupil, the book, or both, are still missing.
    Incomplete,
    /// A free book and a pupil: the loan can be recorded.
    Lend { student: u32, book: u32 },
    /// The book is out: it cannot be lent to anyone, only returned.
    Return { book: u32, loan: u32, borrower: u32 },
}

pub fn read_entry(lib: &Library, field: Field, input: &str) -> Entry {
    let text = input.trim();
    if text.is_empty() {
        return Entry::Nothing;
    }
    let expected = match field {
        Field::Student => Kind::Student,
        Field::Book => Kind::Book,
    };
    match parse_code_or_number(text, expected) {
        None => Entry::Search(field, text.to_string()),
        Some(Scanned::Student(id)) => match lib.student(id) {
            None => Entry::Refused(Error::StudentNotFound(id)),
            Some(_) => Entry::Student(id),
        },
        Some(Scanned::Book(id)) => match lib.book(id) {
            None => Entry::Refused(Error::BookNotFound(id)),
            Some(b) if b.archived => Entry::Refused(Error::BookArchived(id)),
            Some(_) => Entry::Book(id),
        },
    }
}

pub fn propose(lib: &Library, student: Option<u32>, book: Option<u32>) -> Proposal {
    let Some(book) = book.filter(|&id| lib.book(id).is_some()) else {
        return Proposal::Incomplete;
    };
    if let Some(loan) = lib.open_loan_for_book(book) {
        return Proposal::Return {
            book,
            loan: loan.id,
            borrower: loan.student_id,
        };
    }
    match student.filter(|&id| lib.student(id).is_some()) {
        Some(student) => Proposal::Lend { student, book },
        None => Proposal::Incomplete,
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

/// Books still in the catalogue matching `text` (title, author, collection,
/// category or code), sorted by title; each copy is listed on its own.
pub fn find_books(lib: &Library, text: &str) -> Vec<u32> {
    let query = Query::new(text);
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(String, u32)> = lib
        .books()
        .iter()
        .filter(|b| !b.archived)
        .filter(|b| {
            let code = book_code(b.id);
            query.matches(&[&b.title, &b.author, &b.collection, &b.category, &code])
        })
        .map(|b| (fold(&b.title), b.id))
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

    /// Books 1 (Matilda) and 2 (Tintin), pupils 1 (Martin Léa) and
    /// 2 (Durand Tom); book 2 is lent to Tom.
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
    fn scanned_codes_go_to_their_side_whatever_the_field() {
        let lib = library();
        for field in [Field::Student, Field::Book] {
            assert_eq!(read_entry(&lib, field, "E000001"), Entry::Student(1));
            assert_eq!(read_entry(&lib, field, "l2\r\n"), Entry::Book(2));
        }
    }

    #[test]
    fn numbers_and_names_are_read_for_their_field() {
        let lib = library();
        assert_eq!(read_entry(&lib, Field::Student, " 1 "), Entry::Student(1));
        assert_eq!(read_entry(&lib, Field::Book, " 1 "), Entry::Book(1));
        assert_eq!(
            read_entry(&lib, Field::Student, "Léa"),
            Entry::Search(Field::Student, "Léa".into())
        );
        assert_eq!(
            read_entry(&lib, Field::Book, " matilda "),
            Entry::Search(Field::Book, "matilda".into())
        );
        assert_eq!(read_entry(&lib, Field::Book, "  "), Entry::Nothing);
    }

    #[test]
    fn azerty_scan_is_understood() {
        let lib = library();
        // L000001 typed by a US-layout scanner on an AZERTY Windows
        assert_eq!(read_entry(&lib, Field::Student, "Lààààà&"), Entry::Book(1));
    }

    #[test]
    fn unknown_or_archived_items_are_refused() {
        let mut lib = library();
        assert_eq!(
            read_entry(&lib, Field::Student, "E99"),
            Entry::Refused(Error::StudentNotFound(99))
        );
        assert_eq!(
            read_entry(&lib, Field::Book, "99"),
            Entry::Refused(Error::BookNotFound(99))
        );
        lib.archive_book(1).unwrap();
        assert_eq!(
            read_entry(&lib, Field::Book, "L1"),
            Entry::Refused(Error::BookArchived(1))
        );
        lib.delete_student(1).unwrap();
        assert_eq!(
            read_entry(&lib, Field::Student, "E1"),
            Entry::Refused(Error::StudentNotFound(1))
        );
    }

    #[test]
    fn a_free_book_and_a_pupil_make_a_loan() {
        let lib = library();
        assert_eq!(
            propose(&lib, Some(1), Some(1)),
            Proposal::Lend {
                student: 1,
                book: 1
            }
        );
        assert_eq!(propose(&lib, Some(1), None), Proposal::Incomplete);
        assert_eq!(propose(&lib, None, Some(1)), Proposal::Incomplete);
        assert_eq!(propose(&lib, None, None), Proposal::Incomplete);
        // a pupil deleted in the meantime
        assert_eq!(propose(&lib, Some(99), Some(1)), Proposal::Incomplete);
    }

    #[test]
    fn a_book_that_is_out_can_only_be_returned() {
        let lib = library();
        let loan = lib.open_loan_for_book(2).unwrap().id;
        let back = Proposal::Return {
            book: 2,
            loan,
            borrower: 2,
        };
        // whoever is chosen: nobody, another pupil, or the borrower
        assert_eq!(propose(&lib, None, Some(2)), back);
        assert_eq!(propose(&lib, Some(1), Some(2)), back);
        assert_eq!(propose(&lib, Some(2), Some(2)), back);
    }

    #[test]
    fn finds_books_by_title_author_or_code() {
        let mut lib = library();
        let copy = lib
            .add_book(BookInput {
                title: "Matilda".into(),
                author: "Roald Dahl".into(),
                ..Default::default()
            })
            .unwrap();
        lib.add_book(BookInput {
            title: "Le Bon Gros Géant".into(),
            author: "Roald Dahl".into(),
            ..Default::default()
        })
        .unwrap();
        // every copy, sorted by title
        assert_eq!(find_books(&lib, "matil"), vec![1, copy]);
        assert_eq!(find_books(&lib, "dahl"), vec![4, copy]);
        assert_eq!(find_books(&lib, "geant"), vec![4]);
        assert_eq!(find_books(&lib, "L000002"), vec![2]);
        assert!(find_books(&lib, "").is_empty());
        lib.archive_book(1).unwrap();
        assert_eq!(find_books(&lib, "matilda"), vec![copy]);
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
