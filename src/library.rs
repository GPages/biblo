//! Modèle de données et règles métier. Aucune entrée/sortie ici : tout est
//! testable en mémoire. Chaque opération valide AVANT de modifier, donc une
//! erreur ne laisse jamais la bibliothèque à moitié modifiée.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::Error;

pub const CURRENT_VERSION: u32 = 1;
pub const MIN_YEAR: i32 = 1000;
pub const MAX_YEAR: i32 = 2200;

fn clean_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn name_key(s: &str) -> String {
    clean_text(s).to_lowercase()
}

// ---------------------------------------------------------------- Livres

/// Données saisies pour créer ou modifier un livre. Seul le titre est obligatoire.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookInput {
    pub title: String,
    pub author: String,
    pub collection: String,
    /// Année de parution (stricte : 4 chiffres, `MIN_YEAR..=MAX_YEAR`).
    pub year: Option<i32>,
    pub isbn: String,
}

impl BookInput {
    /// Nettoie (espaces) et valide.
    pub fn cleaned(&self) -> Result<BookInput, Error> {
        let title = clean_text(&self.title);
        if title.is_empty() {
            return Err(Error::EmptyTitle);
        }
        if let Some(y) = self.year {
            if !(MIN_YEAR..=MAX_YEAR).contains(&y) {
                return Err(Error::InvalidYear(y));
            }
        }
        Ok(BookInput {
            title,
            author: clean_text(&self.author),
            collection: clean_text(&self.collection),
            year: self.year,
            isbn: clean_text(&self.isbn),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Book {
    pub id: u32,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub collection: String,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub isbn: String,
    /// Retiré du catalogue (l'historique des prêts est conservé).
    #[serde(default)]
    pub archived: bool,
}

// ---------------------------------------------------------------- Élèves

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Student {
    pub id: u32,
    pub last_name: String,
    pub first_name: String,
    #[serde(default)]
    pub archived: bool,
}

impl Student {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.last_name, self.first_name)
    }
}

// ----------------------------------------------------------------- Prêts

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loan {
    pub id: u32,
    pub book_id: u32,
    pub student_id: u32,
    pub borrowed_on: NaiveDate,
    /// `None` tant que le livre n'est pas rendu.
    pub returned_on: Option<NaiveDate>,
}

impl Loan {
    pub fn is_open(&self) -> bool {
        self.returned_on.is_none()
    }

    /// Nombre de jours depuis l'emprunt (jusqu'au retour s'il a eu lieu,
    /// sinon jusqu'à `today`).
    pub fn days_out(&self, today: NaiveDate) -> i64 {
        let end = self.returned_on.unwrap_or(today);
        (end - self.borrowed_on).num_days()
    }
}

// ----------------------------------------------------------- Bibliothèque

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub version: u32,
    next_book_id: u32,
    next_student_id: u32,
    next_loan_id: u32,
    books: Vec<Book>,
    students: Vec<Student>,
    loans: Vec<Loan>,
}

impl Default for Library {
    fn default() -> Self {
        Library {
            version: CURRENT_VERSION,
            next_book_id: 1,
            next_student_id: 1,
            next_loan_id: 1,
            books: Vec::new(),
            students: Vec::new(),
            loans: Vec::new(),
        }
    }
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    /// À appeler après lecture du fichier : refuse un format trop récent et
    /// garantit qu'un identifiant déjà utilisé ne sera jamais réattribué
    /// (même si le fichier a été modifié à la main).
    pub fn after_load(&mut self) -> Result<(), Error> {
        if self.version > CURRENT_VERSION {
            return Err(Error::UnsupportedVersion(self.version));
        }
        let max_book = self.books.iter().map(|b| b.id).max().unwrap_or(0);
        let max_student = self.students.iter().map(|s| s.id).max().unwrap_or(0);
        let max_loan = self.loans.iter().map(|l| l.id).max().unwrap_or(0);
        self.next_book_id = self.next_book_id.max(max_book + 1);
        self.next_student_id = self.next_student_id.max(max_student + 1);
        self.next_loan_id = self.next_loan_id.max(max_loan + 1);
        Ok(())
    }

    // ------------------------------------------------------------ lecture

    pub fn books(&self) -> &[Book] {
        &self.books
    }

    pub fn students(&self) -> &[Student] {
        &self.students
    }

    pub fn loans(&self) -> &[Loan] {
        &self.loans
    }

    pub fn book(&self, id: u32) -> Option<&Book> {
        self.books.iter().find(|b| b.id == id)
    }

    pub fn student(&self, id: u32) -> Option<&Student> {
        self.students.iter().find(|s| s.id == id)
    }

    // -------------------------------------------------------------- livres

    pub fn add_book(&mut self, input: BookInput) -> Result<u32, Error> {
        let ids = self.add_books(vec![input])?;
        Ok(ids[0])
    }

    /// Ajout en lot, tout ou rien : si une ligne est invalide, rien n'est ajouté.
    pub fn add_books(&mut self, inputs: Vec<BookInput>) -> Result<Vec<u32>, Error> {
        let cleaned = inputs
            .iter()
            .map(|i| i.cleaned())
            .collect::<Result<Vec<BookInput>, Error>>()?;
        let mut ids = Vec::with_capacity(cleaned.len());
        for input in cleaned {
            let id = self.next_book_id;
            self.next_book_id += 1;
            self.books.push(Book {
                id,
                title: input.title,
                author: input.author,
                collection: input.collection,
                year: input.year,
                isbn: input.isbn,
                archived: false,
            });
            ids.push(id);
        }
        Ok(ids)
    }

    /// `copies` exemplaires du même titre, chacun avec son propre code-barres.
    pub fn add_copies(&mut self, input: BookInput, copies: usize) -> Result<Vec<u32>, Error> {
        self.add_books(vec![input; copies])
    }

    pub fn update_book(&mut self, id: u32, input: &BookInput) -> Result<(), Error> {
        let clean = input.cleaned()?;
        let book = self
            .books
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(Error::BookNotFound(id))?;
        book.title = clean.title;
        book.author = clean.author;
        book.collection = clean.collection;
        book.year = clean.year;
        book.isbn = clean.isbn;
        Ok(())
    }

    /// Retire le livre du catalogue (refusé s'il est emprunté).
    pub fn archive_book(&mut self, id: u32) -> Result<(), Error> {
        if self.book(id).is_none() {
            return Err(Error::BookNotFound(id));
        }
        if self.open_loan_for_book(id).is_some() {
            return Err(Error::BookOnLoan(id));
        }
        if let Some(b) = self.books.iter_mut().find(|b| b.id == id) {
            b.archived = true;
        }
        Ok(())
    }

    pub fn unarchive_book(&mut self, id: u32) -> Result<(), Error> {
        let book = self
            .books
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(Error::BookNotFound(id))?;
        book.archived = false;
        Ok(())
    }

    // -------------------------------------------------------------- élèves

    fn clean_names(last_name: &str, first_name: &str) -> Result<(String, String), Error> {
        let last = clean_text(last_name);
        let first = clean_text(first_name);
        if last.is_empty() {
            return Err(Error::EmptyLastName);
        }
        if first.is_empty() {
            return Err(Error::EmptyFirstName);
        }
        Ok((last, first))
    }

    /// Un élève est identifié par nom + prénom : on refuse un doublon parmi
    /// les élèves non archivés (sans tenir compte de la casse).
    fn name_taken(&self, last: &str, first: &str, except: Option<u32>) -> bool {
        let (l, f) = (name_key(last), name_key(first));
        self.students.iter().any(|s| {
            !s.archived
                && Some(s.id) != except
                && name_key(&s.last_name) == l
                && name_key(&s.first_name) == f
        })
    }

    pub fn add_student(&mut self, last_name: &str, first_name: &str) -> Result<u32, Error> {
        let (last, first) = Self::clean_names(last_name, first_name)?;
        if self.name_taken(&last, &first, None) {
            return Err(Error::DuplicateStudent {
                last_name: last,
                first_name: first,
            });
        }
        let id = self.next_student_id;
        self.next_student_id += 1;
        self.students.push(Student {
            id,
            last_name: last,
            first_name: first,
            archived: false,
        });
        Ok(id)
    }

    pub fn update_student(
        &mut self,
        id: u32,
        last_name: &str,
        first_name: &str,
    ) -> Result<(), Error> {
        let (last, first) = Self::clean_names(last_name, first_name)?;
        if self.student(id).is_none() {
            return Err(Error::StudentNotFound(id));
        }
        if self.name_taken(&last, &first, Some(id)) {
            return Err(Error::DuplicateStudent {
                last_name: last,
                first_name: first,
            });
        }
        if let Some(s) = self.students.iter_mut().find(|s| s.id == id) {
            s.last_name = last;
            s.first_name = first;
        }
        Ok(())
    }

    /// Archive l'élève (refusé s'il a des livres en cours). L'historique reste.
    pub fn archive_student(&mut self, id: u32) -> Result<(), Error> {
        if self.student(id).is_none() {
            return Err(Error::StudentNotFound(id));
        }
        let count = self.open_loans_of_student(id).len();
        if count > 0 {
            return Err(Error::StudentHasLoans { student: id, count });
        }
        if let Some(s) = self.students.iter_mut().find(|s| s.id == id) {
            s.archived = true;
        }
        Ok(())
    }

    pub fn unarchive_student(&mut self, id: u32) -> Result<(), Error> {
        let (last, first) = match self.student(id) {
            Some(s) => (s.last_name.clone(), s.first_name.clone()),
            None => return Err(Error::StudentNotFound(id)),
        };
        if self.name_taken(&last, &first, Some(id)) {
            return Err(Error::DuplicateStudent {
                last_name: last,
                first_name: first,
            });
        }
        if let Some(s) = self.students.iter_mut().find(|s| s.id == id) {
            s.archived = false;
        }
        Ok(())
    }

    // --------------------------------------------------------------- prêts

    /// Enregistre un emprunt. Renvoie l'identifiant du prêt.
    pub fn lend(&mut self, book_id: u32, student_id: u32, date: NaiveDate) -> Result<u32, Error> {
        match self.book(book_id) {
            None => return Err(Error::BookNotFound(book_id)),
            Some(b) if b.archived => return Err(Error::BookArchived(book_id)),
            Some(_) => {}
        }
        match self.student(student_id) {
            None => return Err(Error::StudentNotFound(student_id)),
            Some(s) if s.archived => return Err(Error::StudentArchived(student_id)),
            Some(_) => {}
        }
        if self.open_loan_for_book(book_id).is_some() {
            return Err(Error::AlreadyBorrowed(book_id));
        }
        let id = self.next_loan_id;
        self.next_loan_id += 1;
        self.loans.push(Loan {
            id,
            book_id,
            student_id,
            borrowed_on: date,
            returned_on: None,
        });
        Ok(id)
    }

    /// Enregistre le retour d'un livre. Renvoie l'identifiant du prêt clos.
    pub fn return_book(&mut self, book_id: u32, date: NaiveDate) -> Result<u32, Error> {
        let loan = self
            .loans
            .iter_mut()
            .find(|l| l.book_id == book_id && l.returned_on.is_none())
            .ok_or(Error::NotBorrowed(book_id))?;
        if date < loan.borrowed_on {
            return Err(Error::InvalidLoanDates);
        }
        loan.returned_on = Some(date);
        Ok(loan.id)
    }

    /// Correction manuelle des dates d'un prêt (`returned_on = None` rouvre le prêt).
    pub fn edit_loan(
        &mut self,
        loan_id: u32,
        borrowed_on: NaiveDate,
        returned_on: Option<NaiveDate>,
    ) -> Result<(), Error> {
        if let Some(r) = returned_on {
            if r < borrowed_on {
                return Err(Error::InvalidLoanDates);
            }
        }
        let idx = self
            .loans
            .iter()
            .position(|l| l.id == loan_id)
            .ok_or(Error::LoanNotFound(loan_id))?;
        let book_id = self.loans[idx].book_id;
        if returned_on.is_none()
            && self
                .loans
                .iter()
                .any(|l| l.id != loan_id && l.book_id == book_id && l.returned_on.is_none())
        {
            return Err(Error::AlreadyBorrowed(book_id));
        }
        let loan = &mut self.loans[idx];
        loan.borrowed_on = borrowed_on;
        loan.returned_on = returned_on;
        Ok(())
    }

    /// Supprime un prêt enregistré par erreur.
    pub fn cancel_loan(&mut self, loan_id: u32) -> Result<(), Error> {
        let idx = self
            .loans
            .iter()
            .position(|l| l.id == loan_id)
            .ok_or(Error::LoanNotFound(loan_id))?;
        self.loans.remove(idx);
        Ok(())
    }

    // ------------------------------------------------------------ requêtes

    pub fn open_loan_for_book(&self, book_id: u32) -> Option<&Loan> {
        self.loans
            .iter()
            .find(|l| l.book_id == book_id && l.returned_on.is_none())
    }

    /// Tous les prêts en cours, indexés par livre (un seul passage : à
    /// utiliser pour afficher la colonne « statut » d'une longue liste).
    pub fn open_loans_by_book(&self) -> HashMap<u32, &Loan> {
        self.loans
            .iter()
            .filter(|l| l.is_open())
            .map(|l| (l.book_id, l))
            .collect()
    }

    pub fn open_loans_of_student(&self, student_id: u32) -> Vec<&Loan> {
        self.loans
            .iter()
            .filter(|l| l.student_id == student_id && l.is_open())
            .collect()
    }

    /// Historique d'un livre, du plus récent au plus ancien.
    pub fn loans_of_book(&self, book_id: u32) -> Vec<&Loan> {
        let mut v: Vec<&Loan> = self.loans.iter().filter(|l| l.book_id == book_id).collect();
        v.sort_by_key(|l| std::cmp::Reverse((l.borrowed_on, l.id)));
        v
    }

    /// Historique d'un élève, du plus récent au plus ancien.
    pub fn loans_of_student(&self, student_id: u32) -> Vec<&Loan> {
        let mut v: Vec<&Loan> = self
            .loans
            .iter()
            .filter(|l| l.student_id == student_id)
            .collect();
        v.sort_by_key(|l| std::cmp::Reverse((l.borrowed_on, l.id)));
        v
    }

    /// Prêts en cours depuis `min_days` jours ou plus, du plus ancien au plus
    /// récent. (`min_days = 0` donne tous les prêts en cours.)
    pub fn overdue(&self, today: NaiveDate, min_days: i64) -> Vec<&Loan> {
        let mut v: Vec<&Loan> = self
            .loans
            .iter()
            .filter(|l| l.is_open() && l.days_out(today) >= min_days)
            .collect();
        v.sort_by_key(|l| (l.borrowed_on, l.id));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn book(title: &str) -> BookInput {
        BookInput {
            title: title.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn title_is_required_and_text_is_cleaned() {
        let mut lib = Library::new();
        assert_eq!(lib.add_book(book("   ")), Err(Error::EmptyTitle));
        let id = lib
            .add_book(BookInput {
                title: "  Le  Petit   Prince ".into(),
                author: " Saint-Exupéry ".into(),
                ..Default::default()
            })
            .unwrap();
        let b = lib.book(id).unwrap();
        assert_eq!(b.title, "Le Petit Prince");
        assert_eq!(b.author, "Saint-Exupéry");
    }

    #[test]
    fn year_must_be_valid() {
        let mut lib = Library::new();
        let mut input = book("A");
        input.year = Some(19);
        assert_eq!(lib.add_book(input.clone()), Err(Error::InvalidYear(19)));
        input.year = Some(2019);
        assert!(lib.add_book(input).is_ok());
    }

    #[test]
    fn ids_are_never_reused() {
        let mut lib = Library::new();
        let a = lib.add_book(book("A")).unwrap();
        lib.archive_book(a).unwrap();
        let b = lib.add_book(book("B")).unwrap();
        assert_ne!(a, b);
        let s1 = lib.add_student("Martin", "Léa").unwrap();
        lib.archive_student(s1).unwrap();
        let s2 = lib.add_student("Martin", "Léa").unwrap();
        assert_ne!(s1, s2);
    }

    #[test]
    fn batch_is_all_or_nothing() {
        let mut lib = Library::new();
        let res = lib.add_books(vec![book("A"), book(""), book("C")]);
        assert_eq!(res, Err(Error::EmptyTitle));
        assert!(lib.books().is_empty());
        // le compteur n'a pas bougé : le premier id valide est 1
        assert_eq!(lib.add_book(book("D")).unwrap(), 1);
    }

    #[test]
    fn copies_get_distinct_consecutive_ids() {
        let mut lib = Library::new();
        let ids = lib.add_copies(book("Tintin"), 3).unwrap();
        assert_eq!(ids, vec![1, 2, 3]);
        assert!(lib.books().iter().all(|b| b.title == "Tintin"));
    }

    #[test]
    fn lend_then_return() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        let loan = lib.lend(b, s, d(2026, 10, 1)).unwrap();
        assert!(lib.open_loan_for_book(b).is_some());
        assert_eq!(lib.return_book(b, d(2026, 10, 8)), Ok(loan));
        assert!(lib.open_loan_for_book(b).is_none());
        assert_eq!(
            lib.return_book(b, d(2026, 10, 9)),
            Err(Error::NotBorrowed(b))
        );
        assert_eq!(lib.loans_of_book(b).len(), 1);
    }

    #[test]
    fn return_cannot_precede_loan() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        lib.lend(b, s, d(2026, 10, 10)).unwrap();
        assert_eq!(
            lib.return_book(b, d(2026, 10, 9)),
            Err(Error::InvalidLoanDates)
        );
        assert!(lib.open_loan_for_book(b).is_some());
    }

    #[test]
    fn cannot_lend_twice() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s1 = lib.add_student("Martin", "Léa").unwrap();
        let s2 = lib.add_student("Durand", "Tom").unwrap();
        lib.lend(b, s1, d(2026, 10, 1)).unwrap();
        assert_eq!(
            lib.lend(b, s2, d(2026, 10, 2)),
            Err(Error::AlreadyBorrowed(b))
        );
    }

    #[test]
    fn unknown_or_archived_items_are_refused() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        assert_eq!(lib.lend(99, s, d(2026, 10, 1)), Err(Error::BookNotFound(99)));
        assert_eq!(lib.lend(b, 99, d(2026, 10, 1)), Err(Error::StudentNotFound(99)));
        lib.archive_book(b).unwrap();
        assert_eq!(lib.lend(b, s, d(2026, 10, 1)), Err(Error::BookArchived(b)));
        lib.unarchive_book(b).unwrap();
        lib.archive_student(s).unwrap();
        assert_eq!(
            lib.lend(b, s, d(2026, 10, 1)),
            Err(Error::StudentArchived(s))
        );
    }

    #[test]
    fn archiving_is_blocked_while_on_loan() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        lib.lend(b, s, d(2026, 10, 1)).unwrap();
        assert_eq!(lib.archive_book(b), Err(Error::BookOnLoan(b)));
        assert_eq!(
            lib.archive_student(s),
            Err(Error::StudentHasLoans {
                student: s,
                count: 1
            })
        );
        lib.return_book(b, d(2026, 10, 2)).unwrap();
        assert_eq!(lib.archive_book(b), Ok(()));
        assert_eq!(lib.archive_student(s), Ok(()));
        // l'historique est conservé
        assert_eq!(lib.loans_of_student(s).len(), 1);
    }

    #[test]
    fn duplicate_student_names_are_refused() {
        let mut lib = Library::new();
        let s = lib.add_student("Martin", "Léa").unwrap();
        assert!(matches!(
            lib.add_student("  martin ", "LÉA"),
            Err(Error::DuplicateStudent { .. })
        ));
        // se renommer soi-même avec le même nom reste permis
        assert_eq!(lib.update_student(s, "Martin", "Léa"), Ok(()));
        // un élève archivé libère son nom
        lib.archive_student(s).unwrap();
        let s2 = lib.add_student("Martin", "Léa").unwrap();
        // ... et on ne peut pas le désarchiver tant que le nom est pris
        assert!(matches!(
            lib.unarchive_student(s),
            Err(Error::DuplicateStudent { .. })
        ));
        assert_ne!(s, s2);
        assert_eq!(lib.add_student("", "Léa"), Err(Error::EmptyLastName));
        assert_eq!(lib.add_student("Durand", " "), Err(Error::EmptyFirstName));
    }

    #[test]
    fn overdue_threshold_and_order() {
        let today = d(2026, 10, 10);
        let mut lib = Library::new();
        let s = lib.add_student("Martin", "Léa").unwrap();
        let b1 = lib.add_book(book("B1")).unwrap();
        let b2 = lib.add_book(book("B2")).unwrap();
        let b3 = lib.add_book(book("B3")).unwrap();
        let b4 = lib.add_book(book("B4")).unwrap();
        // ajoutés dans le désordre
        lib.lend(b2, s, d(2026, 9, 19)).unwrap(); // 21 jours
        lib.lend(b3, s, d(2026, 9, 20)).unwrap(); // 20 jours
        lib.lend(b1, s, d(2026, 9, 15)).unwrap(); // 25 jours
        lib.lend(b4, s, d(2026, 8, 1)).unwrap();
        lib.return_book(b4, d(2026, 8, 10)).unwrap(); // rendu : jamais listé

        let late: Vec<u32> = lib.overdue(today, 21).iter().map(|l| l.book_id).collect();
        assert_eq!(late, vec![b1, b2]);

        let all: Vec<u32> = lib.overdue(today, 0).iter().map(|l| l.book_id).collect();
        assert_eq!(all, vec![b1, b2, b3]);
    }

    #[test]
    fn edit_loan_rules() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        let loan1 = lib.lend(b, s, d(2026, 10, 1)).unwrap();
        lib.return_book(b, d(2026, 10, 5)).unwrap();
        let _loan2 = lib.lend(b, s, d(2026, 10, 6)).unwrap();

        // rouvrir loan1 alors que loan2 est ouvert : refusé
        assert_eq!(
            lib.edit_loan(loan1, d(2026, 10, 1), None),
            Err(Error::AlreadyBorrowed(b))
        );
        // retour avant emprunt : refusé
        assert_eq!(
            lib.edit_loan(loan1, d(2026, 10, 5), Some(d(2026, 10, 1))),
            Err(Error::InvalidLoanDates)
        );
        assert_eq!(
            lib.edit_loan(999, d(2026, 10, 1), None),
            Err(Error::LoanNotFound(999))
        );
        // correction valide
        assert_eq!(
            lib.edit_loan(loan1, d(2026, 9, 30), Some(d(2026, 10, 5))),
            Ok(())
        );
        let l = lib.loans().iter().find(|l| l.id == loan1).unwrap();
        assert_eq!(l.borrowed_on, d(2026, 9, 30));
        assert_eq!(l.returned_on, Some(d(2026, 10, 5)));
    }

    #[test]
    fn cancel_loan_frees_the_book() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        let loan = lib.lend(b, s, d(2026, 10, 1)).unwrap();
        assert_eq!(lib.cancel_loan(loan), Ok(()));
        assert!(lib.open_loan_for_book(b).is_none());
        assert_eq!(lib.cancel_loan(loan), Err(Error::LoanNotFound(loan)));
    }

    #[test]
    fn open_loans_by_book_matches_open_loan_for_book() {
        let mut lib = Library::new();
        let b1 = lib.add_book(book("A")).unwrap();
        let b2 = lib.add_book(book("B")).unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        lib.lend(b1, s, d(2026, 10, 1)).unwrap();
        let map = lib.open_loans_by_book();
        assert!(map.contains_key(&b1));
        assert!(!map.contains_key(&b2));
    }

    #[test]
    fn json_round_trip() {
        let mut lib = Library::new();
        let b = lib
            .add_book(BookInput {
                title: "A".into(),
                year: Some(2019),
                ..Default::default()
            })
            .unwrap();
        let s = lib.add_student("Martin", "Léa").unwrap();
        lib.lend(b, s, d(2026, 10, 1)).unwrap();
        let json = serde_json::to_string_pretty(&lib).unwrap();
        // les dates sont lisibles dans le fichier
        assert!(json.contains("2026-10-01"));
        let back: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(back, lib);
    }

    #[test]
    fn hand_edited_file_cannot_cause_id_reuse() {
        let json = r#"{"version":1,"books":[{"id":7,"title":"X"}],"students":[],"loans":[]}"#;
        let mut lib: Library = serde_json::from_str(json).unwrap();
        lib.after_load().unwrap();
        assert_eq!(lib.add_book(book("Y")).unwrap(), 8);
    }

    #[test]
    fn newer_format_is_refused() {
        let mut lib: Library = serde_json::from_str(r#"{"version":99}"#).unwrap();
        assert_eq!(lib.after_load(), Err(Error::UnsupportedVersion(99)));
    }
}
