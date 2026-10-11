//! Window shell: tabs, status bar, and helpers shared by the views.

mod books;
mod loans;
mod print_queue;
mod register;
mod settings;
mod students;
mod theme;

use std::path::{Path, PathBuf};

use biblo::search::fold;
use biblo::{code128, Store};
use eframe::egui::{self, Color32, Margin, RichText, Stroke};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Loans,
    Books,
    Students,
    Register,
    Settings,
}

pub struct App {
    data_dir: PathBuf,
    /// `Err` when the data file could not be read: nothing is ever written then.
    store: Result<Store, String>,
    view: View,
    loans: loans::LoansView,
    books: books::BooksView,
    students: students::StudentsView,
    /// Books out this many days or more are late (set in the pupils tab).
    late_days: i64,
    register: register::RegisterView,
    settings: settings::SettingsView,
}

impl App {
    pub fn new(ctx: &egui::Context, data_dir: PathBuf) -> App {
        theme::apply(ctx);
        let store = Store::open(&data_dir, biblo::today()).map_err(|e| e.to_string());
        App {
            data_dir,
            store,
            view: View::Loans,
            loans: loans::LoansView::default(),
            books: books::BooksView::default(),
            students: students::StudentsView::default(),
            late_days: 21,
            register: register::RegisterView::default(),
            settings: settings::SettingsView::default(),
        }
    }

    /// After a restore: forget what the screens held (selected pupil, forms,
    /// cards to print…), it may not exist in the restored data.
    fn reset_views(&mut self) {
        self.loans = loans::LoansView::default();
        self.books = books::BooksView::default();
        self.students = students::StudentsView::default();
        self.register = register::RegisterView::default();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(theme::BLUE)
                    .inner_margin(Margin::symmetric(18, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("📚 Biblo")
                            .family(theme::bold())
                            .size(30.0)
                            .color(Color32::WHITE),
                    );
                    // In a narrow window the tabs need the room.
                    if ui.available_width() > 950.0 {
                        ui.label(
                            RichText::new(if cfg!(feature = "demo") {
                                "Démonstration (données fictives)"
                            } else {
                                "La bibliothèque de l'école"
                            })
                            .size(16.0)
                            .color(Color32::from_white_alpha(220)),
                        );
                    }
                    ui.add_space(30.0);
                    for (view, text) in [
                        (View::Loans, "🔁  Prêts et retours"),
                        (View::Books, "📖  Livres"),
                        (View::Students, "👫  Élèves"),
                        (View::Register, "✏  Enregistrer"),
                    ] {
                        // The selected tab is a white pill, the others are see-through.
                        let (fill, color) = if self.view == view {
                            (Color32::WHITE, theme::BLUE)
                        } else {
                            (Color32::from_white_alpha(45), Color32::WHITE)
                        };
                        let tab = egui::Button::new(
                            RichText::new(text)
                                .family(theme::bold())
                                .size(19.0)
                                .color(color),
                        )
                        .fill(fill)
                        .stroke(Stroke::NONE)
                        .corner_radius(18)
                        .min_size(egui::vec2(0.0, 40.0));
                        if ui.add(tab).clicked() {
                            self.view = view;
                        }
                    }
                    // Settings: a small icon on the far right, away from the tabs.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (fill, color) = if self.view == View::Settings {
                            (Color32::WHITE, theme::BLUE)
                        } else {
                            (Color32::TRANSPARENT, Color32::from_white_alpha(200))
                        };
                        let button = egui::Button::new(RichText::new("⚙").size(22.0).color(color))
                            .fill(fill)
                            .stroke(Stroke::NONE)
                            .corner_radius(18)
                            .min_size(egui::vec2(40.0, 40.0));
                        if ui
                            .add(button)
                            .on_hover_text("Réglages : sauvegardes")
                            .clicked()
                        {
                            self.view = View::Settings;
                        }
                    });
                });
            });

        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(0xF7, 0xEB, 0xCF))
                    .inner_margin(Margin::symmetric(18, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Données : {}", self.data_dir.display()))
                            .small()
                            .color(Color32::from_rgb(0x7A, 0x6A, 0x4A)),
                    );
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

        let central = egui::Frame::new()
            .fill(theme::PAPER)
            .inner_margin(Margin::same(16));
        egui::CentralPanel::default().frame(central).show(ui, |ui| {
            if self.view == View::Settings {
                if self.settings.ui(ui, &mut self.store, &self.data_dir) {
                    self.reset_views();
                }
                return;
            }
            self.main_view(ui);
        });
    }
}

impl App {
    fn main_view(&mut self, ui: &mut egui::Ui) {
        match &mut self.store {
            Err(message) => theme::card(ui, |ui| {
                ui.heading("⚠ Impossible d'ouvrir les données");
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, message.as_str());
                ui.add_space(8.0);
                ui.label(format!(
                    "Le fichier n'a pas été modifié. Dossier : {}",
                    self.data_dir.display()
                ));
                ui.add_space(8.0);
                if ui
                    .add(theme::main_button(
                        "💾 Revenir à une sauvegarde…",
                        theme::BLUE,
                    ))
                    .clicked()
                {
                    self.view = View::Settings;
                }
            }),
            Ok(store) => match self.view {
                View::Loans => self.loans.ui(ui, store, self.late_days),
                View::Books => self.books.ui(ui, store),
                View::Students => self.students.ui(ui, store, &mut self.late_days),
                View::Register => self.register.ui(ui, store, &self.data_dir),
                // Drawn before, even without data.
                View::Settings => {}
            },
        }
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
                ui.label(RichText::new(format!("✔ {text}")).color(theme::GREEN));
            }
            Message::Err(text) => {
                ui.label(RichText::new(format!("⚠ {text}")).color(theme::RED));
            }
        }
    }
}

/// Text field followed by a "Choisir…" list of the values already in use,
/// so that a class is always spelled the same way.
pub fn text_with_choices(
    ui: &mut egui::Ui,
    id: &str,
    text: &mut String,
    hint: &str,
    choices: Vec<String>,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(hint)
                .desired_width(200.0),
        );
        if !choices.is_empty() {
            egui::ComboBox::from_id_salt(id)
                .selected_text("Choisir…")
                .height(500.0)
                .show_ui(ui, |ui| {
                    for choice in choices {
                        let current = fold(&choice) == fold(text);
                        if ui.selectable_label(current, &choice).clicked() {
                            *text = choice;
                        }
                    }
                });
        }
    });
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
