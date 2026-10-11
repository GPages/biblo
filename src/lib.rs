//! Biblo : cœur de l'application (sans interface graphique).
//!
//! - `library` : modèle de données et règles métier (livres, élèves, prêts)
//! - `store`   : persistance JSON atomique + sauvegardes quotidiennes
//! - `codes`   : format des codes-barres et lecture des scans
//! - `dates`   : lecture/affichage strict des dates (jj/mm/aaaa)
//! - `search`  : filtrage instantané, sans tenir compte des majuscules ni des accents
//! - `code128` : encodage des codes-barres Code 128
//! - `labels`  : planches d'étiquettes A4 au format PDF
//! - `desk`    : interprétation d'un scan au poste de prêt
//! - `demo`    : données de démonstration

pub mod code128;
pub mod codes;
pub mod dates;
pub mod demo;
pub mod desk;
pub mod error;
pub mod labels;
pub mod library;
pub mod search;
pub mod store;

pub use error::Error;
pub use library::{Book, BookInput, DeleteReport, Library, Loan, Student};
pub use store::Store;

/// Date du jour (heure locale de la machine).
pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}
