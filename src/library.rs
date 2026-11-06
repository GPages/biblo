//! Modèle de données et règles métier. Aucune entrée/sortie ici : tout est
//! testable en mémoire. Chaque opération valide AVANT de modifier, donc une
//! erreur ne laisse jamais la bibliothèque à moitié modifiée.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::isbn::normalize as normalize_isbn;
use crate::search::fold;

pub const CURRENT_VERSION: u32 = 1;
pub const MIN_YEAR: i32 = 1000;
pub const MAX_YEAR: i32 = 2200;

fn clean_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Note sur plusieurs lignes : espaces superflus retirés ligne par ligne,
/// lignes vides supprimées.
fn clean_note(s: &str) -> String {
    s.lines()
        .map(clean_text)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clé de comparaison des noms : sans casse ni accents (« Léa » = « LEA »).
fn name_key(s: &str) -> String {
    fold(&clean_text(s))
}

/// Rang d'une classe dans l'ordre de l'école, d'après le début de son nom
/// (`key` déjà passé par `fold`) : « ce1 b » et « ce1-ce2 » se rangent avec
/// le CE1. Les noms inconnus viennent après.
fn class_rank(key: &str) -> usize {
    const LEVELS: [&str; 8] = ["ps", "ms", "gs", "cp", "ce1", "ce2", "cm1", "cm2"];
    LEVELS
        .iter()
        .position(|level| key.starts_with(level))
        .unwrap_or(LEVELS.len())
}

// ---------------------------------------------------------------- Livres

/// Données saisies pour créer ou modifier un livre. Le titre et l'ISBN sont
/// obligatoires : l'ISBN (le code-barres imprimé sur le livre) est ce qu'on
/// scanne pour les prêts et les retours.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookInput {
    pub title: String,
    pub author: String,
    pub collection: String,
    /// Texte libre (« Roman », « BD », « Documentaire »…).
    pub category: String,
    /// Année de parution (stricte : 4 chiffres, `MIN_YEAR..=MAX_YEAR`).
    pub year: Option<i32>,
    /// ISBN-10 ou ISBN-13, avec ou sans tirets ; conservé en ISBN-13.
    pub isbn: String,
}

impl BookInput {
    /// Nettoie (espaces) et valide.
    pub fn cleaned(&self) -> Result<BookInput, Error> {
        let title = clean_text(&self.title);
        if title.is_empty() {
            return Err(Error::EmptyTitle);
        }
        if self.isbn.trim().is_empty() {
            return Err(Error::IsbnRequired);
        }
        let isbn = normalize_isbn(&self.isbn).map_err(Error::InvalidIsbn)?;
        if let Some(y) = self.year {
            if !(MIN_YEAR..=MAX_YEAR).contains(&y) {
                return Err(Error::InvalidYear(y));
            }
        }
        Ok(BookInput {
            title,
            author: clean_text(&self.author),
            collection: clean_text(&self.collection),
            category: clean_text(&self.category),
            year: self.year,
            isbn,
        })
    }

    /// Nouvel exemplaire de `book` : reprend l'orthographe du titre (et de
    /// l'auteur s'il est connu) et complète les champs laissés vides.
    pub fn filled_from(&self, book: &Book) -> BookInput {
        let mine_or = |mine: &str, theirs: &str| {
            if mine.trim().is_empty() {
                theirs.to_string()
            } else {
                mine.to_string()
            }
        };
        BookInput {
            title: book.title.clone(),
            author: mine_or(&book.author, &self.author),
            collection: mine_or(&self.collection, &book.collection),
            category: mine_or(&self.category, &book.category),
            year: self.year.or(book.year),
            isbn: mine_or(&self.isbn, &book.isbn),
        }
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
    pub category: String,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub isbn: String,
    /// Retiré du catalogue (l'historique des prêts est conservé).
    #[serde(default)]
    pub archived: bool,
    /// Remarque libre sur cet exemplaire (« couverture abîmée »…) ; vide si
    /// aucune. Propre à l'exemplaire : un nouvel exemplaire n'en hérite pas.
    #[serde(default)]
    pub note: String,
}

// ---------------------------------------------------------------- Élèves

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Student {
    pub id: u32,
    pub last_name: String,
    pub first_name: String,
    pub birth_date: NaiveDate,
    /// Date d'inscription (`None` pour les élèves enregistrés avant que
    /// Biblo ne la conserve).
    #[serde(default)]
    pub registered_on: Option<NaiveDate>,
    /// Classe (facultative), par exemple « CE1 » ; vide si inconnue.
    #[serde(default)]
    pub class: String,
}

/// Résultat d'une suppression en lot.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeleteReport {
    pub deleted: Vec<u32>,
    /// Laissés de côté : ils ont encore des livres.
    pub kept_with_books: Vec<u32>,
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
                category: input.category,
                year: input.year,
                isbn: input.isbn,
                archived: false,
                note: String::new(),
            });
            ids.push(id);
        }
        Ok(ids)
    }

    /// `copies` exemplaires du même livre : ils ont le même ISBN, et chacun
    /// son propre prêt.
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
        book.category = clean.category;
        book.year = clean.year;
        book.isbn = clean.isbn;
        Ok(())
    }

    /// Remplace la note d'un exemplaire (une note vide la supprime).
    pub fn set_book_note(&mut self, id: u32, note: &str) -> Result<(), Error> {
        let book = self
            .books
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(Error::BookNotFound(id))?;
        book.note = clean_note(note);
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

    /// Un élève est identifié par nom + prénom + date de naissance : on refuse
    /// un doublon (sans tenir compte de la casse ni des accents).
    fn name_taken(&self, last: &str, first: &str, birth: NaiveDate, except: Option<u32>) -> bool {
        let (l, f) = (name_key(last), name_key(first));
        self.students.iter().any(|s| {
            Some(s.id) != except
                && name_key(&s.last_name) == l
                && name_key(&s.first_name) == f
                && s.birth_date == birth
        })
    }

    pub fn add_student(
        &mut self,
        last_name: &str,
        first_name: &str,
        birth_date: NaiveDate,
        registered_on: NaiveDate,
    ) -> Result<u32, Error> {
        let (last, first) = Self::clean_names(last_name, first_name)?;
        if self.name_taken(&last, &first, birth_date, None) {
            return Err(Error::DuplicateStudent {
                last_name: last,
                first_name: first,
                birth_date,
            });
        }
        let id = self.next_student_id;
        self.next_student_id += 1;
        self.students.push(Student {
            id,
            last_name: last,
            first_name: first,
            birth_date,
            registered_on: Some(registered_on),
            class: String::new(),
        });
        Ok(id)
    }

    /// Change la classe d'un élève (une classe vide = pas de classe).
    pub fn set_student_class(&mut self, id: u32, class: &str) -> Result<(), Error> {
        let s = self
            .students
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(Error::StudentNotFound(id))?;
        s.class = clean_text(class);
        Ok(())
    }

    pub fn update_student(
        &mut self,
        id: u32,
        last_name: &str,
        first_name: &str,
        birth_date: NaiveDate,
    ) -> Result<(), Error> {
        let (last, first) = Self::clean_names(last_name, first_name)?;
        if self.student(id).is_none() {
            return Err(Error::StudentNotFound(id));
        }
        if self.name_taken(&last, &first, birth_date, Some(id)) {
            return Err(Error::DuplicateStudent {
                last_name: last,
                first_name: first,
                birth_date,
            });
        }
        if let Some(s) = self.students.iter_mut().find(|s| s.id == id) {
            s.last_name = last;
            s.first_name = first;
            s.birth_date = birth_date;
        }
        Ok(())
    }

    /// Supprime plusieurs élèves d'un coup. Ceux qui ont encore des livres
    /// sont laissés de côté (et listés dans le rapport) ; les codes inconnus
    /// sont ignorés.
    pub fn delete_students(&mut self, ids: &[u32]) -> DeleteReport {
        let mut report = DeleteReport::default();
        for &id in ids {
            match self.delete_student(id) {
                Ok(()) => report.deleted.push(id),
                Err(Error::StudentHasLoans { .. }) => report.kept_with_books.push(id),
                Err(_) => {}
            }
        }
        report
    }

    /// Supprime définitivement l'élève et l'historique de ses prêts (refusé
    /// s'il a des livres en cours). Son code n'est jamais réattribué : une
    /// ancienne carte ne peut pas désigner un autre élève.
    pub fn delete_student(&mut self, id: u32) -> Result<(), Error> {
        if self.student(id).is_none() {
            return Err(Error::StudentNotFound(id));
        }
        let count = self.open_loans_of_student(id).len();
        if count > 0 {
            return Err(Error::StudentHasLoans { student: id, count });
        }
        self.students.retain(|s| s.id != id);
        self.loans.retain(|l| l.student_id != id);
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
        if self.student(student_id).is_none() {
            return Err(Error::StudentNotFound(student_id));
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

    /// Élèves inscrits au plus tard le `cutoff`. Ceux dont la
    /// date d'inscription est inconnue ne sont jamais sélectionnés.
    pub fn registered_on_or_before(&self, cutoff: NaiveDate) -> Vec<u32> {
        self.students
            .iter()
            .filter(|s| s.registered_on.is_some_and(|d| d <= cutoff))
            .map(|s| s.id)
            .collect()
    }

    /// Exemplaires (non archivés) du livre qui a cet ISBN (ISBN-10 ou 13,
    /// tel qu'écrit ou scanné), dans l'ordre d'enregistrement ; rien si ce
    /// n'est pas un ISBN valide.
    pub fn copies_of(&self, isbn: &str) -> Vec<u32> {
        let Ok(isbn) = normalize_isbn(isbn) else {
            return Vec::new();
        };
        self.books
            .iter()
            .filter(|b| !b.archived && b.isbn == isbn)
            .map(|b| b.id)
            .collect()
    }

    /// Livres (non archivés) qui ont le même ISBN que `input` : sert à
    /// prévenir avant un doublon ; un nouvel exemplaire reste possible avec
    /// `add_book`.
    pub fn same_books(&self, input: &BookInput) -> Vec<u32> {
        self.copies_of(&input.isbn)
    }

    /// Un exemplaire de cet ISBN qui n'est pas emprunté (le premier).
    pub fn free_copy_of(&self, isbn: &str) -> Option<u32> {
        self.copies_of(isbn)
            .into_iter()
            .find(|&id| self.open_loan_for_book(id).is_none())
    }

    /// Les emprunts en cours des exemplaires de cet ISBN.
    pub fn open_loans_for_isbn(&self, isbn: &str) -> Vec<&Loan> {
        self.copies_of(isbn)
            .into_iter()
            .filter_map(|id| self.open_loan_for_book(id))
            .collect()
    }

    /// Change l'ISBN d'un exemplaire (pour les livres enregistrés avant que
    /// l'ISBN ne soit obligatoire).
    pub fn set_book_isbn(&mut self, id: u32, isbn: &str) -> Result<(), Error> {
        if isbn.trim().is_empty() {
            return Err(Error::IsbnRequired);
        }
        let isbn = normalize_isbn(isbn).map_err(Error::InvalidIsbn)?;
        let book = self
            .books
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(Error::BookNotFound(id))?;
        book.isbn = isbn;
        Ok(())
    }

    /// Catégories déjà utilisées (livres non archivés), sans doublon de casse
    /// ni d'accents (la première orthographe rencontrée l'emporte), triées.
    pub fn categories(&self) -> Vec<String> {
        let mut seen: Vec<(String, String)> = Vec::new();
        for b in self.books.iter().filter(|b| !b.archived && !b.category.is_empty()) {
            let key = fold(&b.category);
            if !seen.iter().any(|(k, _)| *k == key) {
                seen.push((key, b.category.clone()));
            }
        }
        seen.sort();
        seen.into_iter().map(|(_, c)| c).collect()
    }

    /// Auteurs déjà enregistrés (livres non archivés), tels qu'ils ont été
    /// saisis (« Roald Dahl », « René Goscinny, Jean-Jacques Sempé »), sans
    /// doublon de casse ni d'accents (la première orthographe rencontrée
    /// l'emporte), triés.
    pub fn authors(&self) -> Vec<String> {
        let mut seen: Vec<(String, String)> = Vec::new();
        for b in self.books.iter().filter(|b| !b.archived && !b.author.is_empty()) {
            let key = fold(&b.author);
            if !seen.iter().any(|(k, _)| *k == key) {
                seen.push((key, b.author.clone()));
            }
        }
        seen.sort();
        seen.into_iter().map(|(_, a)| a).collect()
    }

    /// Classes déjà utilisées, sans doublon de casse ni d'accents (la
    /// première orthographe rencontrée l'emporte), dans l'ordre de l'école :
    /// PS, MS, GS, CP, CE1, CE2, CM1, CM2, puis les autres par ordre
    /// alphabétique.
    pub fn classes(&self) -> Vec<String> {
        let mut seen: Vec<(usize, String, String)> = Vec::new();
        for s in self.students.iter().filter(|s| !s.class.is_empty()) {
            let key = fold(&s.class);
            if !seen.iter().any(|(_, k, _)| *k == key) {
                seen.push((class_rank(&key), key, s.class.clone()));
            }
        }
        seen.sort();
        seen.into_iter().map(|(_, _, c)| c).collect()
    }

    /// Élèves d'une classe (sans tenir compte de la casse ni des accents),
    /// triés par nom puis prénom.
    pub fn students_in_class(&self, class: &str) -> Vec<u32> {
        let key = name_key(class);
        if key.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(String, u32)> = self
            .students
            .iter()
            .filter(|s| name_key(&s.class) == key)
            .map(|s| (fold(&s.full_name()), s.id))
            .collect();
        found.sort();
        found.into_iter().map(|(_, id)| id).collect()
    }

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

    const BIRTH: NaiveDate = NaiveDate::from_ymd_opt(2017, 5, 14).unwrap();
    const REG: NaiveDate = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap();

    fn book(title: &str) -> BookInput {
        BookInput {
            title: title.to_string(),
            isbn: crate::isbn::test_isbn(title),
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
                isbn: crate::isbn::test_isbn("Le Petit Prince"),
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
        let s1 = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        lib.delete_student(s1).unwrap();
        let s2 = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert_ne!(s1, s2);
        // même après relecture du fichier : une vieille carte ne désigne
        // jamais un autre élève
        lib.delete_student(s2).unwrap();
        let json = serde_json::to_string(&lib).unwrap();
        let mut reloaded: Library = serde_json::from_str(&json).unwrap();
        reloaded.after_load().unwrap();
        assert!(reloaded.add_student("Durand", "Tom", BIRTH, REG).unwrap() > s2);
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
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
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
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
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
        let s1 = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        let s2 = lib.add_student("Durand", "Tom", BIRTH, REG).unwrap();
        lib.lend(b, s1, d(2026, 10, 1)).unwrap();
        assert_eq!(
            lib.lend(b, s2, d(2026, 10, 2)),
            Err(Error::AlreadyBorrowed(b))
        );
    }

    #[test]
    fn unknown_archived_or_deleted_items_are_refused() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert_eq!(lib.lend(99, s, d(2026, 10, 1)), Err(Error::BookNotFound(99)));
        assert_eq!(lib.lend(b, 99, d(2026, 10, 1)), Err(Error::StudentNotFound(99)));
        lib.archive_book(b).unwrap();
        assert_eq!(lib.lend(b, s, d(2026, 10, 1)), Err(Error::BookArchived(b)));
        lib.unarchive_book(b).unwrap();
        lib.delete_student(s).unwrap();
        assert_eq!(
            lib.lend(b, s, d(2026, 10, 1)),
            Err(Error::StudentNotFound(s))
        );
    }

    #[test]
    fn archive_and_delete_are_blocked_while_on_loan() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        let other = lib.add_student("Durand", "Tom", BIRTH, REG).unwrap();
        lib.lend(b, other, d(2026, 9, 1)).unwrap();
        lib.return_book(b, d(2026, 9, 8)).unwrap();
        lib.lend(b, s, d(2026, 10, 1)).unwrap();
        assert_eq!(lib.archive_book(b), Err(Error::BookOnLoan(b)));
        assert_eq!(
            lib.delete_student(s),
            Err(Error::StudentHasLoans {
                student: s,
                count: 1
            })
        );
        lib.return_book(b, d(2026, 10, 2)).unwrap();
        assert_eq!(lib.archive_book(b), Ok(()));
        assert_eq!(lib.delete_student(s), Ok(()));
        // l'élève et ses prêts sont effacés, pas ceux des autres
        assert!(lib.student(s).is_none());
        assert!(lib.loans_of_student(s).is_empty());
        assert_eq!(lib.loans_of_student(other).len(), 1);
        assert_eq!(lib.delete_student(s), Err(Error::StudentNotFound(s)));
    }

    #[test]
    fn duplicate_student_names_are_refused() {
        let mut lib = Library::new();
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert!(matches!(
            lib.add_student("  martin ", "LÉA", BIRTH, REG),
            Err(Error::DuplicateStudent { .. })
        ));
        // se renommer soi-même avec le même nom reste permis
        assert_eq!(lib.update_student(s, "Martin", "Léa", BIRTH), Ok(()));
        // un élève supprimé libère son nom
        lib.delete_student(s).unwrap();
        let s2 = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert_ne!(s, s2);
        assert_eq!(
            lib.add_student("", "Léa", BIRTH, REG),
            Err(Error::EmptyLastName)
        );
        assert_eq!(
            lib.add_student("Durand", " ", BIRTH, REG),
            Err(Error::EmptyFirstName)
        );
    }

    #[test]
    fn duplicate_check_ignores_accents() {
        let mut lib = Library::new();
        lib.add_student("Lefèvre", "Léa", BIRTH, REG).unwrap();
        assert!(matches!(
            lib.add_student("LEFEVRE", "Lea", BIRTH, REG),
            Err(Error::DuplicateStudent { .. })
        ));
        assert!(matches!(
            lib.add_student("lefevre", "léa", BIRTH, REG),
            Err(Error::DuplicateStudent { .. })
        ));
        assert_eq!(lib.students().len(), 1);
    }

    #[test]
    fn registration_date_is_kept_and_old_files_load() {
        let mut lib = Library::new();
        let id = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert_eq!(lib.student(id).unwrap().registered_on, Some(REG));
        let json = r#"{"students":[{"id":1,"last_name":"A","first_name":"B","birth_date":"2017-01-02"}]}"#;
        let old: Library = serde_json::from_str(json).unwrap();
        assert_eq!(old.student(1).unwrap().registered_on, None);
    }

    #[test]
    fn class_is_optional_cleaned_and_editable() {
        let mut lib = Library::new();
        let id = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert_eq!(lib.student(id).unwrap().class, "");
        lib.set_student_class(id, "  CE1   b ").unwrap();
        assert_eq!(lib.student(id).unwrap().class, "CE1 b");
        lib.set_student_class(id, " ").unwrap();
        assert_eq!(lib.student(id).unwrap().class, "");
        assert_eq!(
            lib.set_student_class(99, "CP"),
            Err(Error::StudentNotFound(99))
        );
        // les fichiers d'avant les classes se chargent toujours
        let json = r#"{"students":[{"id":1,"last_name":"A","first_name":"B","birth_date":"2017-01-02"}]}"#;
        let old: Library = serde_json::from_str(json).unwrap();
        assert_eq!(old.student(1).unwrap().class, "");
    }

    #[test]
    fn classes_are_distinct_and_in_school_order() {
        let mut lib = Library::new();
        for (i, class) in ["CM2", "Ulis", "cp", "CE1-CE2", "", "CP", "ce1", "GS", "Cm1"]
            .into_iter()
            .enumerate()
        {
            let id = lib
                .add_student("Élève", &format!("N{i}"), BIRTH, REG)
                .unwrap();
            lib.set_student_class(id, class).unwrap();
        }
        // CE1-CE2 se range avec le CE1 ; la première orthographe l'emporte
        assert_eq!(
            lib.classes(),
            vec!["GS", "cp", "ce1", "CE1-CE2", "Cm1", "CM2", "Ulis"]
        );
    }

    #[test]
    fn students_in_class_are_sorted_by_name() {
        let mut lib = Library::new();
        for (last, first, class) in [
            ("Martin", "Léa", "CE1"),
            ("Durand", "Tom", "ce1"),
            ("Bénard", "Chloé", "CE2"),
            ("Abel", "Zoé", ""),
            ("Durand", "Jade", "Cé1"),
        ] {
            let id = lib.add_student(last, first, BIRTH, REG).unwrap();
            lib.set_student_class(id, class).unwrap();
        }
        // Durand Jade, Durand Tom, Martin Léa
        assert_eq!(lib.students_in_class(" CE1 "), vec![5, 2, 1]);
        assert_eq!(lib.students_in_class("CE2"), vec![3]);
        assert!(lib.students_in_class("CM2").is_empty());
        // « pas de classe » n'est pas une classe
        assert!(lib.students_in_class("").is_empty());
    }

    #[test]
    fn registered_on_or_before_skips_unknown_dates() {
        // élève 1 : enregistré avant que Biblo ne garde la date d'inscription
        let json = r#"{"next_student_id":2,"students":[
            {"id":1,"last_name":"Z","first_name":"Z","birth_date":"2015-01-02"}]}"#;
        let mut lib: Library = serde_json::from_str(json).unwrap();
        lib.after_load().unwrap();
        let old = lib.add_student("A", "A", BIRTH, d(2020, 9, 1)).unwrap();
        let limit = lib.add_student("B", "B", BIRTH, d(2022, 10, 11)).unwrap();
        lib.add_student("C", "C", BIRTH, d(2022, 10, 12)).unwrap();
        assert_eq!(
            lib.registered_on_or_before(d(2022, 10, 11)),
            vec![old, limit]
        );
    }

    #[test]
    fn batch_delete_keeps_pupils_who_still_have_a_book() {
        let mut lib = Library::new();
        let b = lib.add_book(book("A")).unwrap();
        let s1 = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        let s2 = lib.add_student("Durand", "Tom", BIRTH, REG).unwrap();
        let s3 = lib.add_student("Petit", "Lucas", BIRTH, REG).unwrap();
        lib.lend(b, s2, d(2026, 10, 1)).unwrap();
        let report = lib.delete_students(&[s1, s2, s3, 99]);
        assert_eq!(report.deleted, vec![s1, s3]);
        assert_eq!(report.kept_with_books, vec![s2]);
        assert!(lib.student(s1).is_none());
        assert!(lib.student(s2).is_some());
        // déjà supprimé : rien à faire
        assert_eq!(lib.delete_students(&[s1]), DeleteReport::default());
    }

    #[test]
    fn same_name_with_other_birth_date_is_another_student() {
        let mut lib = Library::new();
        let a = lib
            .add_student("Martin", "Léa", d(2017, 5, 14), REG)
            .unwrap();
        let b = lib
            .add_student("MARTIN", "léa", d(2018, 1, 2), REG)
            .unwrap();
        assert_ne!(a, b);
        // changer la date de b pour celle de a : doublon
        assert!(matches!(
            lib.update_student(b, "Martin", "Léa", d(2017, 5, 14)),
            Err(Error::DuplicateStudent { .. })
        ));
        assert_eq!(lib.student(b).unwrap().birth_date, d(2018, 1, 2));
    }

    #[test]
    fn overdue_threshold_and_order() {
        let today = d(2026, 10, 10);
        let mut lib = Library::new();
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
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
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
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
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
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
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
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
                isbn: crate::isbn::test_isbn("A"),
                ..Default::default()
            })
            .unwrap();
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        lib.lend(b, s, d(2026, 10, 1)).unwrap();
        let json = serde_json::to_string_pretty(&lib).unwrap();
        // les dates sont lisibles dans le fichier
        assert!(json.contains("2026-10-01"));
        let back: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(back, lib);
    }

    #[test]
    fn category_is_cleaned_stored_and_updated() {
        let mut lib = Library::new();
        let id = lib
            .add_book(BookInput {
                title: "Tintin au Tibet".into(),
                category: "  Bande   dessinée ".into(),
                isbn: crate::isbn::test_isbn("Tintin au Tibet"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(lib.book(id).unwrap().category, "Bande dessinée");
        let mut input = book("Tintin au Tibet");
        input.category = "BD".into();
        lib.update_book(id, &input).unwrap();
        assert_eq!(lib.book(id).unwrap().category, "BD");
    }

    #[test]
    fn authors_are_distinct_and_sorted() {
        let mut lib = Library::new();
        for (title, author) in [
            ("A", "Roald Dahl"),
            ("B", "roald dahl"),
            ("C", "Éric Carle"),
            ("D", ""),
            ("E", "Anne Fine"),
        ] {
            let mut input = book(title);
            input.author = author.into();
            lib.add_book(input).unwrap();
        }
        // the first spelling wins; accents and case are ignored
        assert_eq!(lib.authors(), vec!["Anne Fine", "Éric Carle", "Roald Dahl"]);
    }

    #[test]
    fn file_without_category_still_loads() {
        let json = r#"{"version":1,"books":[{"id":1,"title":"X","collection":"Folio"}]}"#;
        let lib: Library = serde_json::from_str(json).unwrap();
        assert_eq!(lib.book(1).unwrap().category, "");
    }

    #[test]
    fn note_is_cleaned_kept_on_update_and_removable() {
        let mut lib = Library::new();
        let id = lib.add_book(book("Matilda")).unwrap();
        assert_eq!(lib.book(id).unwrap().note, "");
        lib.set_book_note(id, "  Couverture   abîmée \n\n  page 12 déchirée  ")
            .unwrap();
        assert_eq!(
            lib.book(id).unwrap().note,
            "Couverture abîmée\npage 12 déchirée"
        );
        // modifier le titre ne touche pas à la note
        lib.update_book(id, &book("Matilda (poche)")).unwrap();
        assert_eq!(
            lib.book(id).unwrap().note,
            "Couverture abîmée\npage 12 déchirée"
        );
        // un nouvel exemplaire n'hérite pas de la note
        let copy = lib.add_book(book("Matilda")).unwrap();
        assert_eq!(lib.book(copy).unwrap().note, "");
        lib.set_book_note(id, " \n ").unwrap();
        assert_eq!(lib.book(id).unwrap().note, "");
        assert_eq!(lib.set_book_note(99, "x"), Err(Error::BookNotFound(99)));
        // les fichiers d'avant les notes se chargent toujours
        let json = r#"{"version":1,"books":[{"id":1,"title":"X"}]}"#;
        let old: Library = serde_json::from_str(json).unwrap();
        assert_eq!(old.book(1).unwrap().note, "");
    }

    #[test]
    fn new_copy_takes_details_from_existing_book() {
        let mut lib = Library::new();
        let id = lib
            .add_book(BookInput {
                title: "Matilda".into(),
                author: "Roald Dahl".into(),
                category: "Roman".into(),
                year: Some(1988),
                isbn: "978-2-07-061275-8".into(),
                ..Default::default()
            })
            .unwrap();
        let typed = BookInput {
            title: "matilda".into(),
            collection: "Folio Junior".into(),
            ..Default::default()
        };
        assert_eq!(
            typed.filled_from(lib.book(id).unwrap()),
            BookInput {
                title: "Matilda".into(),
                author: "Roald Dahl".into(),
                collection: "Folio Junior".into(),
                category: "Roman".into(),
                year: Some(1988),
                // stored as an ISBN-13, without hyphens
                isbn: "9782070612758".into(),
            }
        );
    }

    #[test]
    fn isbn_is_required_valid_and_kept_as_isbn_13() {
        let mut lib = Library::new();
        let mut input = book("Matilda");
        input.isbn = "  ".into();
        assert_eq!(lib.add_book(input.clone()), Err(Error::IsbnRequired));
        input.isbn = "9782070612759".into();
        assert!(matches!(
            lib.add_book(input.clone()),
            Err(Error::InvalidIsbn(_))
        ));
        input.isbn = "abc".into();
        assert!(matches!(lib.add_book(input.clone()), Err(Error::InvalidIsbn(_))));
        // an ISBN-10 with hyphens is kept as the ISBN-13 of the same book
        input.isbn = "2-07-061275-9".into();
        let id = lib.add_book(input).unwrap();
        assert_eq!(lib.book(id).unwrap().isbn, "9782070612758");
        assert!(lib.books().len() == 1);
    }

    #[test]
    fn copies_share_their_isbn_and_are_found_by_it_in_any_form() {
        let mut lib = Library::new();
        let ids = lib.add_copies(book("L’Œil du loup"), 3).unwrap();
        let other = lib.add_book(book("L’Œil du lynx")).unwrap();
        let isbn = crate::isbn::test_isbn("L’Œil du loup");
        assert_eq!(lib.copies_of(&isbn), ids);
        assert_eq!(lib.same_books(&book("L’Œil du loup")), ids);
        // another ISBN is another book, even with the same title
        let mut edition = book("L’Œil du loup");
        edition.isbn = crate::isbn::test_isbn("autre édition");
        assert!(lib.same_books(&edition).is_empty());
        assert_eq!(lib.copies_of(&crate::isbn::test_isbn("L’Œil du lynx")), vec![other]);
        // not an ISBN, or unknown: no copies
        assert!(lib.copies_of("L000001").is_empty());
        assert!(lib.copies_of(&crate::isbn::test_isbn("inconnu")).is_empty());
    }

    #[test]
    fn a_free_copy_is_found_among_the_copies() {
        let mut lib = Library::new();
        let ids = lib.add_copies(book("Matilda"), 2).unwrap();
        let isbn = crate::isbn::test_isbn("Matilda");
        let s = lib.add_student("Martin", "Léa", BIRTH, REG).unwrap();
        assert_eq!(lib.free_copy_of(&isbn), Some(ids[0]));
        lib.lend(ids[0], s, d(2026, 10, 1)).unwrap();
        assert_eq!(lib.free_copy_of(&isbn), Some(ids[1]));
        assert_eq!(lib.open_loans_for_isbn(&isbn).len(), 1);
        lib.lend(ids[1], s, d(2026, 10, 1)).unwrap();
        assert_eq!(lib.free_copy_of(&isbn), None);
        assert_eq!(lib.open_loans_for_isbn(&isbn).len(), 2);
    }

    #[test]
    fn a_book_registered_before_the_isbn_was_required_can_be_given_one() {
        let json = r#"{"version":1,"books":[{"id":1,"title":"X"}]}"#;
        let mut lib: Library = serde_json::from_str(json).unwrap();
        lib.after_load().unwrap();
        // the old file still loads, the book just has no ISBN yet
        assert_eq!(lib.book(1).unwrap().isbn, "");
        assert!(lib.copies_of("").is_empty());
        assert_eq!(lib.set_book_isbn(1, " "), Err(Error::IsbnRequired));
        assert!(matches!(
            lib.set_book_isbn(1, "123"),
            Err(Error::InvalidIsbn(_))
        ));
        lib.set_book_isbn(1, "978-2-07-061275-8").unwrap();
        assert_eq!(lib.book(1).unwrap().isbn, "9782070612758");
        assert_eq!(lib.copies_of("9782070612758"), vec![1]);
        assert_eq!(lib.set_book_isbn(9, "9782070612758"), Err(Error::BookNotFound(9)));
    }

    #[test]
    fn same_books_matches_the_isbn_in_any_written_form() {
        let mut lib = Library::new();
        let mut input = book("Matilda");
        input.isbn = "978-2-07-061275-8".into();
        let id = lib.add_book(input).unwrap();
        // another title, same ISBN: the same book
        let mut probe = book("Matilda (poche)");
        probe.isbn = "9782070612758".into();
        assert_eq!(lib.same_books(&probe), vec![id]);
        probe.isbn = "2070612759".into();
        assert_eq!(lib.same_books(&probe), vec![id]);
    }

    #[test]
    fn same_books_skips_archived_and_empty_input() {
        let mut lib = Library::new();
        let id = lib.add_book(book("Tintin")).unwrap();
        assert!(lib.same_books(&BookInput::default()).is_empty());
        assert_eq!(lib.same_books(&book("Tintin")), vec![id]);
        lib.archive_book(id).unwrap();
        assert!(lib.same_books(&book("Tintin")).is_empty());
    }

    #[test]
    fn categories_are_distinct_and_sorted() {
        let mut lib = Library::new();
        for (title, category) in [
            ("A", "Roman"),
            ("B", "bd"),
            ("C", "Documentaire"),
            ("D", "ROMAN"),
            ("E", ""),
            ("F", "Bd"),
            ("G", "Écologie"),
        ] {
            let mut input = book(title);
            input.category = category.into();
            lib.add_book(input).unwrap();
        }
        // the first spelling wins; accents and case are ignored for sorting and duplicates
        assert_eq!(lib.categories(), vec!["bd", "Documentaire", "Écologie", "Roman"]);
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
