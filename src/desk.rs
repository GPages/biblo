//! Loan desk: decides what one scan (or typed entry) means. Pure: nothing is
//! modified here, the screen applies the decision through `Store::apply`.
//!
//! A book is scanned by its ISBN, the barcode printed on it. Several copies
//! of a book share one ISBN: lending takes the first free copy, and a return
//! asks who brought it back when more than one copy is out.
//!
//! Lending and returning are two separate modes, so the same scan never
//! means "lend" in one case and "return" in another: in lend mode a book
//! can only be lent, in return mode it can only be taken back.

use crate::codes::{parse_student, student_code};
use crate::dates::format_date;
use crate::error::Error;
use crate::isbn::{normalize, IsbnError};
use crate::library::Library;
use crate::search::{fold, Query};

/// What a scan means in lend mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LendDecision {
    /// Empty entry.
    Nothing,
    /// A pupil card: this pupil now borrows.
    SelectStudent(u32),
    /// A book with a free copy while a pupil is selected: `book` is that copy.
    Lend { book: u32, student: u32 },
    /// Every copy is out: they must be returned (return mode) first. `book` is
    /// one of the copies, `loans` the loans in progress.
    AllLent { book: u32, loans: Vec<u32> },
    /// A book with a free copy (`book`), but no pupil selected yet.
    NeedStudent { book: u32 },
    /// A valid ISBN that no book of the library has.
    UnknownBook(String),
    /// Not a code: look for pupils by name.
    SearchName(String),
    /// Unknown pupil, or an ISBN that is not valid.
    Refused(Error),
}

/// What a scan means in return mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnDecision {
    /// Empty entry.
    Nothing,
    /// A book on loan: record its return.
    Return { book: u32, loan: u32 },
    /// Several copies are out: the borrower tells which one came back.
    ChooseLoan { book: u32, loans: Vec<u32> },
    /// A book that is not on loan: nothing to return.
    NotLent { book: u32 },
    /// A valid ISBN that no book of the library has.
    UnknownBook(String),
    /// A pupil card: returns only take books.
    StudentCard(u32),
    /// Neither an ISBN nor a card: maybe the start of a title.
    NotACode(String),
    /// An ISBN that is not valid.
    Refused(Error),
}

/// What was typed or scanned.
enum Input {
    Student(u32),
    Isbn(String),
    BadIsbn(IsbnError),
    Other,
}

fn read(text: &str) -> Input {
    match normalize(text) {
        Ok(isbn) => return Input::Isbn(isbn),
        // 10 or 13 digits that do not add up: a bad scan or a typo
        Err(e @ IsbnError::BadChecksum) => return Input::BadIsbn(e),
        Err(_) => {}
    }
    match parse_student(text) {
        Some(id) => Input::Student(id),
        None => Input::Other,
    }
}

/// Lend mode. `student`: the pupil currently selected.
pub fn decide_lend(lib: &Library, student: Option<u32>, input: &str) -> LendDecision {
    let text = input.trim();
    if text.is_empty() {
        return LendDecision::Nothing;
    }
    match read(text) {
        Input::Other => LendDecision::SearchName(text.to_string()),
        Input::BadIsbn(e) => LendDecision::Refused(Error::InvalidIsbn(e)),
        Input::Student(id) => match lib.student(id) {
            None => LendDecision::Refused(Error::StudentNotFound(id)),
            Some(_) => LendDecision::SelectStudent(id),
        },
        Input::Isbn(isbn) => {
            let copies = lib.copies_of(&isbn);
            let Some(&first) = copies.first() else {
                return LendDecision::UnknownBook(isbn);
            };
            match (lib.free_copy_of(&isbn), student) {
                (None, _) => LendDecision::AllLent {
                    book: first,
                    loans: lib
                        .open_loans_for_isbn(&isbn)
                        .iter()
                        .map(|l| l.id)
                        .collect(),
                },
                (Some(book), Some(student)) => LendDecision::Lend { book, student },
                (Some(book), None) => LendDecision::NeedStudent { book },
            }
        }
    }
}

/// Return mode: only books are expected.
pub fn decide_return(lib: &Library, input: &str) -> ReturnDecision {
    let text = input.trim();
    if text.is_empty() {
        return ReturnDecision::Nothing;
    }
    match read(text) {
        Input::Other => ReturnDecision::NotACode(text.to_string()),
        Input::BadIsbn(e) => ReturnDecision::Refused(Error::InvalidIsbn(e)),
        Input::Student(id) => ReturnDecision::StudentCard(id),
        Input::Isbn(isbn) => {
            let copies = lib.copies_of(&isbn);
            let Some(&first) = copies.first() else {
                return ReturnDecision::UnknownBook(isbn);
            };
            let loans = lib.open_loans_for_isbn(&isbn);
            match loans.as_slice() {
                [] => ReturnDecision::NotLent { book: first },
                [loan] => ReturnDecision::Return {
                    book: loan.book_id,
                    loan: loan.id,
                },
                _ => ReturnDecision::ChooseLoan {
                    book: first,
                    loans: loans.iter().map(|l| l.id).collect(),
                },
            }
        }
    }
}

/// Books still in the catalogue matching `text` (title, author, collection,
/// category or ISBN), sorted by title; each copy is listed on its own.
pub fn find_books(lib: &Library, text: &str) -> Vec<u32> {
    let query = Query::new(text);
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(String, u32)> = lib
        .books()
        .iter()
        .filter(|b| !b.archived)
        .filter(|b| query.matches(&[&b.title, &b.author, &b.collection, &b.category, &b.isbn]))
        .map(|b| (fold(&b.title), b.id))
        .collect();
    found.sort();
    found.into_iter().map(|(_, id)| id).collect()
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
    use crate::isbn::test_isbn;
    use crate::library::BookInput;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn book(title: &str) -> BookInput {
        BookInput {
            title: title.into(),
            isbn: test_isbn(title),
            ..Default::default()
        }
    }

    /// Book 1 (Matilda), books 2 and 3 (two copies of Tintin), pupils 1
    /// (Martin Léa) and 2 (Durand Tom); copy 2 of Tintin is lent to Tom.
    fn library() -> Library {
        let mut lib = Library::new();
        lib.add_book(book("Matilda")).unwrap();
        lib.add_copies(book("Tintin"), 2).unwrap();
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
        // a bare number is always a pupil, selected or not
        assert_eq!(decide_lend(&lib, None, "1"), LendDecision::SelectStudent(1));
        assert_eq!(
            decide_lend(&lib, Some(2), " 1\r\n"),
            LendDecision::SelectStudent(1)
        );
    }

    #[test]
    fn free_book_is_lent_to_the_selected_pupil() {
        let lib = library();
        let lend = LendDecision::Lend {
            book: 1,
            student: 1,
        };
        let isbn = test_isbn("Matilda");
        assert_eq!(decide_lend(&lib, Some(1), &isbn), lend);
        assert_eq!(decide_lend(&lib, Some(1), &format!(" {isbn}\r\n")), lend);
        assert_eq!(
            decide_lend(&lib, None, &isbn),
            LendDecision::NeedStudent { book: 1 }
        );
    }

    #[test]
    fn lending_takes_a_free_copy_and_says_when_all_are_out() {
        let mut lib = library();
        let tintin = test_isbn("Tintin");
        // copy 2 is out, copy 3 is free
        assert_eq!(
            decide_lend(&lib, Some(1), &tintin),
            LendDecision::Lend {
                book: 3,
                student: 1
            }
        );
        lib.lend(3, 1, d(2026, 10, 2)).unwrap();
        let loans: Vec<u32> = lib
            .open_loans_for_isbn(&tintin)
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(loans.len(), 2);
        let all_out = LendDecision::AllLent { book: 2, loans };
        assert_eq!(decide_lend(&lib, Some(1), &tintin), all_out);
        assert_eq!(decide_lend(&lib, None, &tintin), all_out);
    }

    #[test]
    fn a_return_takes_the_only_copy_out_or_asks_which() {
        let mut lib = library();
        let tintin = test_isbn("Tintin");
        let loan = lib.open_loan_for_book(2).unwrap().id;
        assert_eq!(
            decide_return(&lib, &tintin),
            ReturnDecision::Return { book: 2, loan }
        );
        // both copies out: the scan cannot tell which one came back
        lib.lend(3, 1, d(2026, 10, 2)).unwrap();
        let loans: Vec<u32> = lib
            .open_loans_for_isbn(&tintin)
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(
            decide_return(&lib, &tintin),
            ReturnDecision::ChooseLoan { book: 2, loans }
        );
        // on the shelf: nothing to return
        assert_eq!(
            decide_return(&lib, &test_isbn("Matilda")),
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
    fn isbn_in_any_written_form_or_scan_layout_is_understood() {
        let lib = library();
        let lend = |input: &str| decide_lend(&lib, Some(1), input);
        let matilda = test_isbn("Matilda");
        let expected = LendDecision::Lend {
            book: 1,
            student: 1,
        };
        // with hyphens and a prefix
        let hyphens = format!(
            "ISBN {}-{}-{}",
            &matilda[..3],
            &matilda[3..4],
            &matilda[4..]
        );
        assert_eq!(lend(&hyphens), expected);
        // typed by a US-layout scanner on an AZERTY Windows
        let azerty: String = matilda
            .chars()
            .map(|c| {
                "à&é\"'(-è_ç"
                    .chars()
                    .nth(c.to_digit(10).unwrap() as usize)
                    .unwrap()
            })
            .collect();
        assert_eq!(lend(&azerty), expected);
        let tintin = test_isbn("Tintin");
        let azerty: String = tintin
            .chars()
            .map(|c| {
                "à&é\"'(-è_ç"
                    .chars()
                    .nth(c.to_digit(10).unwrap() as usize)
                    .unwrap()
            })
            .collect();
        let loan = lib.open_loan_for_book(2).unwrap().id;
        assert_eq!(
            decide_return(&lib, &azerty),
            ReturnDecision::Return { book: 2, loan }
        );
    }

    #[test]
    fn unknown_and_invalid_codes_are_refused() {
        let mut lib = library();
        // a valid ISBN that is not in the library
        let other = test_isbn("Autre livre");
        assert_eq!(
            decide_lend(&lib, Some(1), &other),
            LendDecision::UnknownBook(other.clone())
        );
        assert_eq!(
            decide_return(&lib, &other),
            ReturnDecision::UnknownBook(other.clone())
        );
        // one digit off: the check digit catches it
        let mut typo = test_isbn("Matilda");
        typo.replace_range(12.., if typo.ends_with('0') { "1" } else { "0" });
        assert!(matches!(
            decide_lend(&lib, Some(1), &typo),
            LendDecision::Refused(Error::InvalidIsbn(_))
        ));
        assert!(matches!(
            decide_return(&lib, &typo),
            ReturnDecision::Refused(Error::InvalidIsbn(_))
        ));
        assert_eq!(
            decide_lend(&lib, None, "E99"),
            LendDecision::Refused(Error::StudentNotFound(99))
        );
        // archived books are not in the catalogue any more
        lib.archive_book(1).unwrap();
        assert_eq!(
            decide_lend(&lib, Some(1), &test_isbn("Matilda")),
            LendDecision::UnknownBook(test_isbn("Matilda"))
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
        // starts like a card but is a name
        assert_eq!(
            decide_lend(&lib, None, "Léa"),
            LendDecision::SearchName("Léa".into())
        );
        // an old book label is not a code any more
        assert_eq!(
            decide_lend(&lib, Some(1), "L000001"),
            LendDecision::SearchName("L000001".into())
        );
    }

    #[test]
    fn finds_books_by_title_author_or_isbn() {
        let mut lib = library();
        let mut copy = book("Matilda");
        copy.author = "Roald Dahl".into();
        // a second copy of Matilda, with the author this time
        let copy = lib.add_book(copy).unwrap();
        let mut giant = book("Le Bon Gros Géant");
        giant.author = "Roald Dahl".into();
        let giant = lib.add_book(giant).unwrap();
        // every copy, sorted by title
        assert_eq!(find_books(&lib, "matil"), vec![1, copy]);
        assert_eq!(find_books(&lib, "dahl"), vec![giant, copy]);
        assert_eq!(find_books(&lib, "geant"), vec![giant]);
        assert_eq!(find_books(&lib, &test_isbn("Tintin")), vec![2, 3]);
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
