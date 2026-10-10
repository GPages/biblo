//! Window shell: tabs, status bar, and helpers shared by the views.

mod books;
mod register;

use std::path::{Path, PathBuf};

use biblo::{code128, Store};
use eframe::egui::{self, Color32, RichText};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Books,
    Register,
}

pub struct App {
    data_dir: PathBuf,
    /// `Err` when the data file could not be read: nothing is ever written then.
    store: Result<Store, String>,
    view: View,
    books: books::BooksView,
    register: register::RegisterView,
}

impl App {
    pub fn new(data_dir: PathBuf) -> App {
        let store = Store::open(&data_dir, biblo::today()).map_err(|e| e.to_string());
        App {
            data_dir,
            store,
            view: View::Books,
            books: books::BooksView::default(),
            register: register::RegisterView::default(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (view, text) in [(View::Books, "Livres"), (View::Register, "Enregistrer")] {
                    ui.selectable_value(&mut self.view, view, RichText::new(text).size(18.0));
                }
            });
            ui.add_space(2.0);
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.weak(format!("Données : {}", self.data_dir.display()));
                if let Ok(store) = &self.store {
                    if let Some(e) = store.backup_error() {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            format!("Sauvegarde du jour impossible : {e}"),
                        );
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| match &mut self.store {
            Err(message) => {
                ui.heading("Impossible d'ouvrir les données");
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, message.as_str());
                ui.add_space(8.0);
                ui.label(format!(
                    "Le fichier n'a pas été modifié. Dossier : {}",
                    self.data_dir.display()
                ));
                ui.label(
                    "Vous pouvez restaurer une sauvegarde depuis le sous-dossier « backups ».",
                );
            }
            Ok(store) => match self.view {
                View::Books => self.books.ui(ui, store),
                View::Register => self.register.ui(ui, store, &self.data_dir),
            },
        });
    }
}

/// Feedback shown under a form after an action.
pub enum Message {
    Ok(String),
    Err(String),
}

impl Message {
    pub fn show(&self, ui: &mut egui::Ui) {
        match self {
            Message::Ok(text) => {
                ui.label(
                    RichText::new(text)
                        .color(Color32::from_rgb(40, 140, 60))
                        .strong(),
                );
            }
            Message::Err(text) => {
                ui.label(
                    RichText::new(text)
                        .color(ui.visuals().error_fg_color)
                        .strong(),
                );
            }
        }
    }
}

/// Draws a barcode on a white background (also in dark mode), quiet zones included.
pub fn barcode(ui: &mut egui::Ui, code: &str, module: f32, height: f32) {
    let Some(modules) = code128::encode(code) else {
        return;
    };
    let width = (modules.len() + 2 * code128::QUIET_ZONE) as f32 * module;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    // Whole pixels keep the bar edges sharp.
    let x0 = (rect.left() + code128::QUIET_ZONE as f32 * module).round();
    for (i, &bar) in modules.iter().enumerate() {
        if bar {
            let x = x0 + i as f32 * module;
            let bar_rect = egui::Rect::from_min_max(
                egui::pos2(x, rect.top() + 2.0),
                egui::pos2(x + module, rect.bottom() - 2.0),
            );
            painter.rect_filled(bar_rect, 0.0, Color32::BLACK);
        }
    }
}

/// Opens a file with the system's default application (the PDF viewer).
pub fn open_file(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let mut command = std::process::Command::new("explorer");
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("xdg-open");
    let mut child = command.arg(path).spawn()?;
    // Reap the launcher process so it does not linger.
    std::thread::spawn(move || child.wait());
    Ok(())
}
