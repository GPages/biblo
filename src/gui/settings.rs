//! Settings (the ⚙ button at the far right of the header): go back to a
//! backup of the data. Works even when the data file cannot be opened.

use std::path::{Path, PathBuf};

use biblo::dates::format_date;
use biblo::store::{list_backups, read_library, restore_backup, Backup, BackupKind, BACKUP_DIR};
use biblo::Store;
use chrono::{Datelike, Local, NaiveDate, Weekday};
use eframe::egui::{self, Margin, RichText};

use super::isbn::{IsbnService, Status};
use super::{open_file, theme, Message};

/// One line of the list: a backup and what it holds.
struct Row {
    backup: Backup,
    /// "54 livres, 44 élèves, 23 prêts en cours", or why it cannot be read.
    contents: Result<String, String>,
}

#[derive(Default)]
pub struct SettingsView {
    /// Read when first shown and after a restore.
    rows: Option<Vec<Row>>,
    /// Backup whose restoration waits for a confirmation (index in `rows`).
    confirm: Option<usize>,
    message: Option<Message>,
    folder_message: Option<Message>,
    /// The path being typed for the ISBN database (read from the settings
    /// the first time).
    isbn_path: Option<String>,
    isbn_message: Option<Message>,
}

/// 8703397 → "8 703 397".
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    out
}

fn weekday(day: NaiveDate) -> &'static str {
    match day.weekday() {
        Weekday::Mon => "lundi",
        Weekday::Tue => "mardi",
        Weekday::Wed => "mercredi",
        Weekday::Thu => "jeudi",
        Weekday::Fri => "vendredi",
        Weekday::Sat => "samedi",
        Weekday::Sun => "dimanche",
    }
}

fn describe(backup: &Backup) -> String {
    match backup.kind {
        BackupKind::Daily(day) => {
            format!("Sauvegarde du {} {}", weekday(day), format_date(day))
        }
        BackupKind::BeforeRestore(at) => format!(
            "Copie d'avant restauration, {} {} à {}",
            weekday(at.date()),
            format_date(at.date()),
            at.format("%H:%M")
        ),
    }
}

fn load_rows(data_dir: &Path) -> Result<Vec<Row>, String> {
    let backups = list_backups(data_dir).map_err(|e| e.to_string())?;
    Ok(backups
        .into_iter()
        .map(|backup| {
            let contents = read_library(&backup.path)
                .map(|lib| {
                    let books = lib.books().iter().filter(|b| !b.archived).count();
                    let out = lib
                        .loans()
                        .iter()
                        .filter(|l| l.returned_on.is_none())
                        .count();
                    format!(
                        "{books} livres, {} élèves, {out} prêts en cours",
                        lib.students().len()
                    )
                })
                .map_err(|e| e.to_string());
            Row { backup, contents }
        })
        .collect())
}

impl SettingsView {
    /// Returns true after a restore: the data in memory has been replaced.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Result<Store, String>,
        data_dir: &Path,
        isbn: &mut IsbnService,
    ) -> bool {
        if self.rows.is_none() {
            match load_rows(data_dir) {
                Ok(rows) => self.rows = Some(rows),
                Err(e) => {
                    self.rows = Some(Vec::new());
                    self.message = Some(Message::Err(format!(
                        "Impossible de lire le dossier des sauvegardes : {e}"
                    )));
                }
            }
        }
        let mut restored = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            theme::card(ui, |ui| self.isbn_card(ui, isbn));
            ui.add_space(14.0);
            theme::card(ui, |ui| {
                ui.heading("💾 Sauvegardes des données");
                ui.add_space(4.0);
                ui.label(
                    "Biblo copie toutes les données (livres, élèves, prêts) une fois par jour \
                     d'utilisation, au premier lancement de la journée : une sauvegarde contient \
                     donc les données telles qu'elles étaient avant les prêts et retours de ce \
                     jour-là. Les 30 dernières sont gardées.",
                );
                ui.weak(format!("Dossier : {}", data_dir.join(BACKUP_DIR).display()));
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Revenir à une sauvegarde")
                        .family(theme::bold())
                        .size(19.0),
                );
                ui.label(
                    "Tout ce qui a été enregistré après la sauvegarde choisie sera perdu. Par \
                     précaution, les données actuelles sont d'abord copiées : cette copie \
                     apparaît en haut de la liste et permet d'annuler la restauration.",
                );
                ui.add_space(6.0);
                restored = self.confirm_box(ui, store, data_dir);
                if let Some(m) = &self.message {
                    m.show(ui);
                    ui.add_space(6.0);
                }
                self.list(ui);
            });
            ui.add_space(14.0);
            theme::card(ui, |ui| self.folder(ui, data_dir));
        });
        restored
    }

    /// Where the ISBN database is, and whether it is ready.
    fn isbn_card(&mut self, ui: &mut egui::Ui, isbn: &mut IsbnService) {
        let typed = self.isbn_path.get_or_insert_with(|| {
            isbn.path()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        });
        ui.heading("📚 Base de données ISBN");
        ui.add_space(4.0);
        ui.label(
            "Pour remplir automatiquement le titre d'un livre à partir de son ISBN (onglet \
             Enregistrer), indiquez le fichier CSV de la base, par exemple \
             open4goods-isbn-dataset.csv. Le fichier n'est pas copié : il est lu là où il est \
             (disque, clé USB…).",
        );
        let (mut save, mut remove) = (false, false);
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(typed)
                    .hint_text(
                        r"Chemin du fichier, par exemple C:\Biblo\open4goods-isbn-dataset.csv",
                    )
                    .desired_width(520.0),
            );
            save = ui
                .add(theme::main_button("Enregistrer", theme::GREEN))
                .clicked()
                || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            remove = isbn.path().is_some() && ui.button("Retirer").clicked();
        });
        if save || remove {
            // « Copier en tant que chemin » in Windows puts the path in quotes.
            let path = typed.trim().trim_matches('"').trim().to_string();
            let new = (!remove && !path.is_empty()).then(|| PathBuf::from(&path));
            if remove {
                typed.clear();
            }
            self.isbn_message = Some(match isbn.set_path(new) {
                Ok(()) => Message::Ok("Réglage enregistré.".into()),
                Err(e) => Message::Err(format!("Le réglage n'a pas pu être enregistré : {e}")),
            });
        }
        if let Some(m) = &self.isbn_message {
            m.show(ui);
        }
        match isbn.status() {
            Status::NotConfigured => {
                ui.weak("Aucune base configurée : le champ ISBN du formulaire ne remplit rien.");
            }
            Status::Preparing(fraction) => {
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .show_percentage()
                        .desired_width(400.0),
                );
                ui.label(
                    "Préparation de la base (une seule fois, de quelques secondes à quelques \
                     minutes selon l'ordinateur). Biblo reste utilisable.",
                );
            }
            Status::Ready { entries } => {
                ui.colored_label(
                    theme::GREEN,
                    format!("✔ Base prête : {} livres connus.", thousands(entries)),
                );
            }
            Status::Failed(message) => {
                ui.colored_label(theme::RED, format!("⚠ {message}"));
            }
        }
    }

    /// Where everything is, to copy it to a USB key or to send the log.
    fn folder(&mut self, ui: &mut egui::Ui, data_dir: &Path) {
        ui.heading("📁 Dossier des données");
        ui.add_space(4.0);
        ui.label(format!(
            "Toutes les données de Biblo sont ici : {}",
            data_dir.display()
        ));
        ui.label(format!(
            "En cas de problème au démarrage, le fichier {} de ce dossier explique ce qui \
             s'est passé : c'est lui qu'il faut transmettre.",
            crate::journal::LOG_FILE
        ));
        ui.add_space(4.0);
        if ui.button("📂 Ouvrir le dossier").clicked() {
            self.folder_message = open_file(data_dir)
                .err()
                .map(|e| Message::Err(format!("Impossible d'ouvrir le dossier : {e}")));
        }
        if let Some(m) = &self.folder_message {
            m.show(ui);
        }
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        let Some(rows) = &self.rows else { return };
        if rows.is_empty() {
            ui.label("Aucune sauvegarde pour l'instant : la première sera faite au prochain lancement de Biblo.");
            return;
        }
        egui::Grid::new("backups")
            .num_columns(3)
            .striped(true)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                for (i, row) in rows.iter().enumerate() {
                    ui.label(describe(&row.backup));
                    match &row.contents {
                        Ok(text) => ui.weak(text),
                        // The technical details only on hover.
                        Err(e) => ui
                            .colored_label(theme::RED, "fichier illisible")
                            .on_hover_text(e),
                    };
                    let button = egui::Button::new("Restaurer…");
                    if ui.add_enabled(row.contents.is_ok(), button).clicked() {
                        self.confirm = Some(i);
                        self.message = None;
                    }
                    ui.end_row();
                }
            });
    }

    /// Asks before restoring; returns true once restored.
    fn confirm_box(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Result<Store, String>,
        data_dir: &Path,
    ) -> bool {
        let Some(row) = self.confirm.and_then(|i| self.rows.as_ref()?.get(i)) else {
            return false;
        };
        let what = describe(&row.backup);
        let (mut yes, mut no) = (false, false);
        egui::Frame::new()
            .fill(theme::ERROR.0)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("⚠ Revenir aux données de : {what} ?"))
                        .family(theme::bold())
                        .color(theme::ERROR.1),
                );
                ui.label(
                    RichText::new(
                        "Tout ce qui a été enregistré depuis (prêts, retours, livres, élèves, \
                         notes…) sera perdu. Les données actuelles seront d'abord copiées.",
                    )
                    .color(theme::ERROR.1),
                );
                ui.horizontal(|ui| {
                    yes = ui
                        .add(theme::main_button("Oui, restaurer", theme::RED))
                        .clicked();
                    no = ui.button("Annuler").clicked();
                });
            });
        if no {
            self.confirm = None;
        }
        if !yes {
            return false;
        }
        let path = row.backup.path.clone();
        self.confirm = None;
        let now = Local::now().naive_local();
        self.message = Some(match restore_backup(data_dir, &path, biblo::today(), now) {
            Ok((restored, copy)) => {
                *store = Ok(restored);
                let mut text = format!("Les données sont revenues à : {what}.");
                if copy.is_some() {
                    text.push_str(
                        " Les données d'avant ont été copiées (« Copie d'avant \
                             restauration » en haut de la liste).",
                    );
                }
                Message::Ok(text)
            }
            Err(e) => {
                // Show what is really on disk now.
                *store = Store::open(data_dir, biblo::today()).map_err(|e| e.to_string());
                Message::Err(format!("Restauration impossible : {e}"))
            }
        });
        self.rows = None;
        true
    }
}
