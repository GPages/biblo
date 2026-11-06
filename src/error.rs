use std::fmt;

use chrono::NaiveDate;

use crate::codes::student_code;
use crate::dates::format_date;
use crate::isbn::IsbnError;

/// Erreurs métier et techniques. Les messages (`Display`) sont en français
/// et destinés à être affichés tels quels à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    EmptyTitle,
    /// Un livre s'identifie par son ISBN : il est obligatoire.
    IsbnRequired,
    InvalidIsbn(IsbnError),
    EmptyLastName,
    EmptyFirstName,
    InvalidYear(i32),
    BookNotFound(u32),
    StudentNotFound(u32),
    LoanNotFound(u32),
    BookArchived(u32),
    /// Le livre est déjà emprunté.
    AlreadyBorrowed(u32),
    /// Retour demandé pour un livre qui n'est pas emprunté.
    NotBorrowed(u32),
    /// Archivage refusé : le livre est actuellement emprunté.
    BookOnLoan(u32),
    /// Archivage refusé : l'élève a encore des livres.
    StudentHasLoans {
        student: u32,
        count: usize,
    },
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
            Error::IsbnRequired => write!(
                f,
                "L'ISBN est obligatoire : scannez le code-barres du livre (13 chiffres) ou tapez-le."
            ),
            Error::InvalidIsbn(e) => write!(f, "{e}"),
            Error::EmptyLastName => write!(f, "Le nom est obligatoire."),
            Error::EmptyFirstName => write!(f, "Le prénom est obligatoire."),
            Error::InvalidYear(y) => write!(f, "Année de parution invalide : {y}."),
            Error::BookNotFound(_) => write!(f, "Livre introuvable."),
            Error::StudentNotFound(id) => write!(f, "Élève introuvable : {}.", student_code(*id)),
            Error::LoanNotFound(id) => write!(f, "Emprunt introuvable (n°{id})."),
            Error::BookArchived(_) => write!(f, "Ce livre a été retiré du catalogue."),
            Error::AlreadyBorrowed(_) => write!(f, "Ce livre est déjà emprunté."),
            Error::NotBorrowed(_) => write!(f, "Ce livre n'est pas emprunté."),
            Error::BookOnLoan(_) => write!(
                f,
                "Ce livre est actuellement emprunté : enregistrez d'abord son retour."
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
