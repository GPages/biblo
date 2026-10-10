use std::fmt;

use chrono::NaiveDate;

use crate::codes::{book_code, student_code};
use crate::dates::format_date;

/// Erreurs métier et techniques. Les messages (`Display`) sont en français
/// et destinés à être affichés tels quels à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    EmptyTitle,
    EmptyLastName,
    EmptyFirstName,
    InvalidYear(i32),
    BookNotFound(u32),
    StudentNotFound(u32),
    LoanNotFound(u32),
    BookArchived(u32),
    StudentArchived(u32),
    /// Le livre est déjà emprunté.
    AlreadyBorrowed(u32),
    /// Retour demandé pour un livre qui n'est pas emprunté.
    NotBorrowed(u32),
    /// Archivage refusé : le livre est actuellement emprunté.
    BookOnLoan(u32),
    /// Archivage refusé : l'élève a encore des livres.
    StudentHasLoans { student: u32, count: usize },
    DuplicateStudent {
        last_name: String,
        first_name: String,
        birth_date: NaiveDate,
    },
    InvalidLoanDates,
    Io(String),
    Json(String),
    UnsupportedVersion(u32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EmptyTitle => write!(f, "Le titre est obligatoire."),
            Error::EmptyLastName => write!(f, "Le nom est obligatoire."),
            Error::EmptyFirstName => write!(f, "Le prénom est obligatoire."),
            Error::InvalidYear(y) => write!(f, "Année de parution invalide : {y}."),
            Error::BookNotFound(id) => write!(f, "Livre introuvable : {}.", book_code(*id)),
            Error::StudentNotFound(id) => write!(f, "Élève introuvable : {}.", student_code(*id)),
            Error::LoanNotFound(id) => write!(f, "Emprunt introuvable (n°{id})."),
            Error::BookArchived(id) => {
                write!(f, "Le livre {} a été retiré du catalogue.", book_code(*id))
            }
            Error::StudentArchived(id) => {
                write!(f, "L'élève {} a été archivé.", student_code(*id))
            }
            Error::AlreadyBorrowed(id) => {
                write!(f, "Le livre {} est déjà emprunté.", book_code(*id))
            }
            Error::NotBorrowed(id) => {
                write!(f, "Le livre {} n'est pas emprunté.", book_code(*id))
            }
            Error::BookOnLoan(id) => write!(
                f,
                "Le livre {} est actuellement emprunté : retour d'abord.",
                book_code(*id)
            ),
            Error::StudentHasLoans { student, count } => write!(
                f,
                "L'élève {} a encore {} livre(s) emprunté(s) : retours d'abord.",
                student_code(*student),
                count
            ),
            Error::DuplicateStudent {
                last_name,
                first_name,
                birth_date,
            } => write!(
                f,
                "Un élève « {last_name} {first_name} » né(e) le {} existe déjà.",
                format_date(*birth_date)
            ),
            Error::InvalidLoanDates => write!(
                f,
                "Dates incohérentes : le retour ne peut pas précéder l'emprunt."
            ),
            Error::Io(m) => write!(f, "Erreur de fichier : {m}"),
            Error::Json(m) => write!(f, "Fichier de données illisible : {m}"),
            Error::UnsupportedVersion(v) => write!(
                f,
                "Fichier de données créé par une version plus récente de Biblo (format {v})."
            ),
        }
    }
}

impl std::error::Error for Error {}
