//! The ISBN database in the running app: where it is (a setting), its state
//! (the index is built in the background the first time), and lookups.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use biblo::config::Config;
use biblo::isbn::{normalize, BookInfo, IsbnDb, Opened, Progress};
use biblo::Error;
use eframe::egui;

/// The index lives in the data folder, always writable (the database may be
/// on a read-only USB key or share).
const INDEX_FILE: &str = "isbn-index.bin";

enum State {
    NotConfigured,
    Preparing {
        progress: Arc<Progress>,
        done: Receiver<Result<IsbnDb, String>>,
    },
    Ready(IsbnDb),
    Failed(String),
}

/// What a lookup found.
pub enum Lookup {
    Found {
        isbn: String,
        info: BookInfo,
    },
    NotFound {
        isbn: String,
    },
    /// Not an ISBN (wrong length or check digit), or no usable database.
    Unusable(String),
}

/// What to tell the user about the database.
pub enum Status<'a> {
    NotConfigured,
    /// Fraction done, 0.0 to 1.0.
    Preparing(f32),
    Ready {
        entries: u64,
    },
    Failed(&'a str),
}

pub struct IsbnService {
    dir: PathBuf,
    config: Config,
    state: State,
}

impl IsbnService {
    pub fn new(dir: &Path) -> IsbnService {
        let mut service = IsbnService {
            dir: dir.to_path_buf(),
            config: Config::load(dir),
            state: State::NotConfigured,
        };
        service.start();
        service
    }

    pub fn path(&self) -> Option<&Path> {
        self.config.isbn_database.as_deref()
    }

    pub fn status(&self) -> Status<'_> {
        match &self.state {
            State::NotConfigured => Status::NotConfigured,
            State::Preparing { progress, .. } => Status::Preparing(progress.fraction()),
            State::Ready(db) => Status::Ready {
                entries: db.count(),
            },
            State::Failed(message) => Status::Failed(message),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    /// Changes the database (`None` to stop using one) and saves the choice.
    pub fn set_path(&mut self, path: Option<PathBuf>) -> Result<(), Error> {
        self.config.isbn_database = path;
        let saved = self.config.save(&self.dir);
        self.start();
        saved
    }

    /// Opens the database; builds its index in the background if needed.
    fn start(&mut self) {
        let Some(csv) = self.config.isbn_database.clone() else {
            self.state = State::NotConfigured;
            return;
        };
        let index = self.dir.join(INDEX_FILE);
        if !csv.is_file() {
            self.state = State::Failed(format!(
                "Fichier introuvable : {} (clé USB débranchée ? chemin mal écrit ?)",
                csv.display()
            ));
            return;
        }
        self.state = match IsbnDb::open(&csv, &index) {
            Ok(Opened::Ready(db)) => State::Ready(db),
            Ok(Opened::NeedsIndex) => {
                let progress = Arc::new(Progress::default());
                let (sender, done) = mpsc::channel();
                let worker = Arc::clone(&progress);
                std::thread::spawn(move || {
                    let result =
                        IsbnDb::build_index(&csv, &index, &worker).map_err(|e| e.to_string());
                    let _ = sender.send(result);
                });
                State::Preparing { progress, done }
            }
            Err(e) => State::Failed(e.to_string()),
        };
    }

    /// Call every frame: picks up the end of the background work.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let State::Preparing { done, .. } = &self.state else {
            return;
        };
        match done.try_recv() {
            Ok(Ok(db)) => self.state = State::Ready(db),
            Ok(Err(message)) => self.state = State::Failed(message),
            Err(TryRecvError::Disconnected) => {
                self.state = State::Failed("La préparation de la base a été interrompue.".into());
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(250)),
        }
    }

    /// `input`: an ISBN as typed or scanned.
    pub fn lookup(&self, input: &str) -> Lookup {
        let isbn = match normalize(input) {
            Ok(isbn) => isbn,
            Err(e) => return Lookup::Unusable(e.to_string()),
        };
        match &self.state {
            State::NotConfigured => Lookup::Unusable(
                "Aucune base ISBN n'est configurée (⚙ Réglages) : remplissez les champs vous-même."
                    .into(),
            ),
            State::Preparing { progress, .. } => Lookup::Unusable(format!(
                "La base ISBN se prépare ({} %) : réessayez dans un instant, ou remplissez les champs vous-même.",
                (progress.fraction() * 100.0) as u32
            )),
            State::Failed(message) => Lookup::Unusable(format!("Base ISBN inutilisable : {message}")),
            State::Ready(db) => match db.lookup(&isbn) {
                Ok(Some(info)) => Lookup::Found { isbn, info },
                Ok(None) => Lookup::NotFound { isbn },
                Err(e) => Lookup::Unusable(format!("Lecture de la base ISBN impossible : {e}")),
            },
        }
    }
}
