//! Persistance : un seul fichier JSON (`biblo.json`), écrit de façon atomique,
//! plus une sauvegarde par jour dans `backups/` (les 30 dernières conservées).
//!
//! Usage typique :
//! ```ignore
//! let mut store = Store::open("biblo-data", today())?;
//! let id = store.apply(|lib| lib.add_book(input))?; // modifie ET sauvegarde
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};

use crate::error::Error;
use crate::library::Library;

pub const DATA_FILE: &str = "biblo.json";
pub const BACKUP_DIR: &str = "backups";
pub const BACKUPS_KEPT: usize = 30;

fn io_err(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}

pub struct Store {
    dir: PathBuf,
    lib: Library,
    backup_error: Option<String>,
}

impl Store {
    /// Ouvre (ou crée) le dossier de données.
    ///
    /// Si `biblo.json` est illisible, une erreur est renvoyée et le fichier
    /// n'est JAMAIS écrasé : l'utilisateur peut le réparer ou restaurer une
    /// sauvegarde.
    pub fn open(dir: impl AsRef<Path>, today: NaiveDate) -> Result<Store, Error> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).map_err(io_err)?;
        let path = dir.join(DATA_FILE);

        if !path.exists() {
            let store = Store {
                dir,
                lib: Library::new(),
                backup_error: None,
            };
            store.save()?;
            return Ok(store);
        }

        let text = fs::read_to_string(&path).map_err(io_err)?;
        // Le Bloc-notes peut ajouter un BOM UTF-8 en début de fichier.
        let text = text.trim_start_matches('\u{feff}');
        let mut lib: Library =
            serde_json::from_str(text).map_err(|e| Error::Json(e.to_string()))?;
        lib.after_load()?;

        // Sauvegarde du jour : n'empêche jamais d'ouvrir l'application.
        let backup_error = backup_if_needed(&dir, &path, today)
            .err()
            .map(|e| e.to_string());

        Ok(Store {
            dir,
            lib,
            backup_error,
        })
    }

    pub fn lib(&self) -> &Library {
        &self.lib
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Message d'erreur si la sauvegarde quotidienne a échoué à l'ouverture.
    pub fn backup_error(&self) -> Option<&str> {
        self.backup_error.as_deref()
    }

    /// Applique une modification puis sauvegarde. Si `f` renvoie une erreur,
    /// rien n'est écrit (les méthodes de `Library` valident avant de modifier).
    /// Si la sauvegarde échoue, l'erreur est renvoyée ; la modification reste
    /// en mémoire et sera écrite à la prochaine sauvegarde réussie.
    pub fn apply<T>(
        &mut self,
        f: impl FnOnce(&mut Library) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let out = f(&mut self.lib)?;
        self.save()?;
        Ok(out)
    }

    /// Écriture atomique : fichier temporaire, fsync, puis renommage.
    /// Une coupure de courant laisse l'ancien fichier intact.
    pub fn save(&self) -> Result<(), Error> {
        let json = serde_json::to_vec_pretty(&self.lib).map_err(|e| Error::Json(e.to_string()))?;
        let target = self.dir.join(DATA_FILE);
        let tmp = self.dir.join(format!("{DATA_FILE}.tmp"));
        {
            let mut f = fs::File::create(&tmp).map_err(io_err)?;
            f.write_all(&json).map_err(io_err)?;
            f.sync_all().map_err(io_err)?;
        }
        fs::rename(&tmp, &target).map_err(io_err)?;
        Ok(())
    }
}

/// Copie `biblo.json` vers `backups/biblo-AAAA-MM-JJ.json` s'il n'y a pas
/// déjà de sauvegarde pour `today`, puis ne garde que les `BACKUPS_KEPT` plus récentes.
fn backup_if_needed(dir: &Path, data: &Path, today: NaiveDate) -> Result<(), Error> {
    let bdir = dir.join(BACKUP_DIR);
    fs::create_dir_all(&bdir).map_err(io_err)?;
    let name = format!(
        "biblo-{:04}-{:02}-{:02}.json",
        today.year(),
        today.month(),
        today.day()
    );
    let dest = bdir.join(name);
    if !dest.exists() {
        fs::copy(data, &dest).map_err(io_err)?;
    }
    prune_backups(&bdir)
}

/// `biblo-AAAA-MM-JJ.json` (21 caractères) ?
fn is_backup_name(n: &str) -> bool {
    n.len() == 21
        && n.is_ascii()
        && n.starts_with("biblo-")
        && n.ends_with(".json")
        && n[6..16]
            .chars()
            .enumerate()
            .all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })
}

fn prune_backups(bdir: &Path) -> Result<(), Error> {
    let mut names: Vec<String> = fs::read_dir(bdir)
        .map_err(io_err)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_backup_name(n))
        .collect();
    names.sort(); // l'ordre alphabétique = l'ordre chronologique
    if names.len() > BACKUPS_KEPT {
        let excess = names.len() - BACKUPS_KEPT;
        for n in &names[..excess] {
            let _ = fs::remove_file(bdir.join(n));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::BookInput;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Dossier temporaire supprimé à la fin du test.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let p = std::env::temp_dir().join(format!(
                "biblo-test-{}-{}-{}",
                std::process::id(),
                n,
                nanos
            ));
            fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

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
    fn creates_file_on_first_open() {
        let t = TempDir::new();
        let dir = t.0.join("data");
        let store = Store::open(&dir, d(2026, 10, 10)).unwrap();
        assert!(dir.join(DATA_FILE).exists());
        assert!(store.lib().books().is_empty());
    }

    #[test]
    fn changes_survive_reopen() {
        let t = TempDir::new();
        let id;
        {
            let mut store = Store::open(&t.0, d(2026, 10, 10)).unwrap();
            id = store.apply(|lib| lib.add_book(book("Le Petit Prince"))).unwrap();
            let s = store.apply(|lib| lib.add_student("Martin", "Léa")).unwrap();
            store
                .apply(|lib| lib.lend(id, s, d(2026, 10, 10)))
                .unwrap();
        }
        let store = Store::open(&t.0, d(2026, 10, 11)).unwrap();
        assert_eq!(store.lib().book(id).unwrap().title, "Le Petit Prince");
        assert!(store.lib().open_loan_for_book(id).is_some());
    }

    #[test]
    fn failed_apply_changes_nothing_on_disk() {
        let t = TempDir::new();
        let mut store = Store::open(&t.0, d(2026, 10, 10)).unwrap();
        let before = fs::read(t.0.join(DATA_FILE)).unwrap();
        let res = store.apply(|lib| lib.add_book(book("  ")));
        assert_eq!(res, Err(Error::EmptyTitle));
        assert_eq!(fs::read(t.0.join(DATA_FILE)).unwrap(), before);
    }

    #[test]
    fn no_temp_file_left_after_save() {
        let t = TempDir::new();
        let mut store = Store::open(&t.0, d(2026, 10, 10)).unwrap();
        store.apply(|lib| lib.add_book(book("A"))).unwrap();
        assert!(!t.0.join(format!("{DATA_FILE}.tmp")).exists());
    }

    #[test]
    fn corrupt_file_is_reported_and_left_untouched() {
        let t = TempDir::new();
        fs::write(t.0.join(DATA_FILE), "{ pas du json").unwrap();
        let res = Store::open(&t.0, d(2026, 10, 10));
        assert!(matches!(res, Err(Error::Json(_))));
        assert_eq!(
            fs::read_to_string(t.0.join(DATA_FILE)).unwrap(),
            "{ pas du json"
        );
        // aucune sauvegarde d'un fichier cassé
        assert!(!t.0.join(BACKUP_DIR).exists());
    }

    #[test]
    fn newer_format_is_refused_and_left_untouched() {
        let t = TempDir::new();
        fs::write(t.0.join(DATA_FILE), r#"{"version": 99}"#).unwrap();
        let res = Store::open(&t.0, d(2026, 10, 10));
        assert!(matches!(res, Err(Error::UnsupportedVersion(99))));
    }

    #[test]
    fn utf8_bom_from_notepad_is_accepted() {
        let t = TempDir::new();
        let json = serde_json::to_string(&Library::new()).unwrap();
        fs::write(t.0.join(DATA_FILE), format!("\u{feff}{json}")).unwrap();
        assert!(Store::open(&t.0, d(2026, 10, 10)).is_ok());
    }

    #[test]
    fn one_backup_per_day() {
        let t = TempDir::new();
        Store::open(&t.0, d(2026, 10, 10)).unwrap(); // création du fichier
        Store::open(&t.0, d(2026, 10, 10)).unwrap();
        Store::open(&t.0, d(2026, 10, 10)).unwrap();
        Store::open(&t.0, d(2026, 10, 11)).unwrap();
        let bdir = t.0.join(BACKUP_DIR);
        assert!(bdir.join("biblo-2026-10-10.json").exists());
        assert!(bdir.join("biblo-2026-10-11.json").exists());
        assert_eq!(fs::read_dir(&bdir).unwrap().count(), 2);
    }

    #[test]
    fn only_the_30_latest_backups_are_kept() {
        let t = TempDir::new();
        Store::open(&t.0, d(2026, 1, 1)).unwrap(); // crée biblo.json
        let bdir = t.0.join(BACKUP_DIR);
        fs::create_dir_all(&bdir).unwrap();
        for day in 1..=31 {
            fs::write(bdir.join(format!("biblo-2026-01-{day:02}.json")), "{}").unwrap();
        }
        for day in 1..=4 {
            fs::write(bdir.join(format!("biblo-2026-02-{day:02}.json")), "{}").unwrap();
        }
        fs::write(bdir.join("notes.txt"), "à garder").unwrap();

        Store::open(&t.0, d(2026, 2, 10)).unwrap();

        let backups = fs::read_dir(&bdir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| is_backup_name(&e.file_name().to_string_lossy()))
            .count();
        assert_eq!(backups, BACKUPS_KEPT);
        assert!(bdir.join("biblo-2026-02-10.json").exists());
        assert!(!bdir.join("biblo-2026-01-01.json").exists());
        assert!(bdir.join("notes.txt").exists());
    }

    #[test]
    fn backup_name_check() {
        assert!(is_backup_name("biblo-2026-10-10.json"));
        assert!(!is_backup_name("biblo-2026-10-10.json.tmp"));
        assert!(!is_backup_name("biblo-20261010.json"));
        assert!(!is_backup_name("autre-2026-10-10.json"));
        assert!(!is_backup_name("biblo-2026-1é-10.json"));
    }
}
