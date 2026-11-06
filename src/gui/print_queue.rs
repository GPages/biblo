//! Pupil cards waiting to be printed on the next A4 sheet. (Books have no
//! label: their ISBN, printed on them, is what gets scanned.)

use std::path::Path;

use biblo::codes::{parse_student, student_code};
use biblo::dates::format_date;
use biblo::labels::{self, Label};
use biblo::{Error, Library};
use eframe::egui;

use super::{barcode, open_file, theme, Message};

const HEADING: &str = "🎫 Cartes d'élèves à imprimer";
const FILE_PREFIX: &str = "cartes-eleves";

pub struct PrintQueue {
    ids: Vec<u32>,
    code_input: String,
    /// The class chosen to be added as a whole.
    class: String,
    message: Option<Message>,
}

impl PrintQueue {
    pub fn new() -> PrintQueue {
        PrintQueue {
            ids: Vec::new(),
            code_input: String::new(),
            class: String::new(),
            message: None,
        }
    }

    /// Adds the ids that are not queued yet; returns how many were added.
    pub fn push(&mut self, ids: impl IntoIterator<Item = u32>) -> usize {
        let before = self.ids.len();
        for id in ids {
            if !self.ids.contains(&id) {
                self.ids.push(id);
            }
        }
        self.ids.len() - before
    }

    pub fn set_message(&mut self, message: Message) {
        self.message = Some(message);
    }

    /// What gets printed for `id`; `None` if it no longer exists.
    fn label(&self, lib: &Library, id: u32) -> Option<Label> {
        let s = lib.student(id)?;
        Some(Label {
            code: student_code(id),
            caption: format!("{} ({})", s.full_name(), format_date(s.birth_date)),
        })
    }

    /// Only existing pupils can be printed.
    fn check(&self, lib: &Library, id: u32) -> Result<(), Error> {
        match lib.student(id) {
            None => Err(Error::StudentNotFound(id)),
            Some(_) => Ok(()),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, lib: &Library, data_dir: &Path) {
        ui.heading(format!("{HEADING} ({})", self.ids.len()));
        ui.add_space(4.0);
        ui.label("Les élèves enregistrés s'ajoutent ici automatiquement.");
        ui.horizontal(|ui| {
            ui.label("Ajouter une carte, code de l'élève :");
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.code_input)
                    .hint_text("E000045")
                    .desired_width(90.0),
            );
            // The scanner types the code then Enter: keep the focus for the next scan.
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Ajouter").clicked() || entered {
                self.add_typed_code(lib);
                if entered {
                    field.request_focus();
                }
            }
        });
        self.class_row(ui, lib);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let has_items = !self.ids.is_empty();
            if ui
                .add_enabled(
                    has_items,
                    theme::main_button("Créer la feuille PDF", theme::BLUE),
                )
                .clicked()
            {
                self.message = Some(self.make_sheet(lib, data_dir));
            }
            if ui
                .add_enabled(has_items, egui::Button::new("Vider la liste"))
                .clicked()
            {
                self.ids.clear();
                self.message = None;
            }
        });
        if let Some(m) = &self.message {
            m.show(ui);
        }
        ui.add_space(6.0);

        // No scroll area of its own: the column around it scrolls, so there
        // is a single scrollbar.
        let mut remove = None;
        for (i, &id) in self.ids.iter().enumerate() {
            let Some(label) = self.label(lib, id) else {
                continue;
            };
            ui.horizontal(|ui| {
                barcode(ui, &label.code, 2.0, 40.0);
                ui.vertical(|ui| {
                    // The button sits next to the code so it never covers the caption.
                    ui.horizontal(|ui| {
                        ui.monospace(&label.code);
                        if ui.small_button("Retirer").clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.label(&label.caption);
                });
            });
        }
        if let Some(i) = remove {
            self.ids.remove(i);
        }
    }

    /// Pupil cards: adds every pupil of a class, in alphabetical order.
    fn class_row(&mut self, ui: &mut egui::Ui, lib: &Library) {
        let classes = lib.classes();
        if classes.is_empty() {
            ui.weak("Pour ajouter toute une classe, indiquez d'abord la classe des élèves.");
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Ajouter toute une classe :");
            let selected = if self.class.is_empty() {
                "Choisir…".to_string()
            } else {
                self.class.clone()
            };
            egui::ComboBox::from_id_salt("cards_class")
                .selected_text(selected)
                .height(500.0)
                .show_ui(ui, |ui| {
                    for class in classes {
                        ui.selectable_value(&mut self.class, class.clone(), class);
                    }
                });
            let pupils = lib.students_in_class(&self.class);
            if pupils.is_empty() {
                return;
            }
            let text = format!("Ajouter les {} élève(s)", pupils.len());
            if ui.button(text).clicked() {
                let added = self.push(pupils.iter().copied());
                let class = &self.class;
                self.message = Some(match added {
                    0 => Message::Err(format!(
                        "Les élèves de {class} sont déjà tous dans la liste."
                    )),
                    n if n < pupils.len() => Message::Ok(format!(
                        "{n} carte(s) ajoutée(s) pour {class} (les autres y étaient déjà)."
                    )),
                    n => Message::Ok(format!("{n} carte(s) ajoutée(s) pour {class}.")),
                });
            }
        });
    }

    fn add_typed_code(&mut self, lib: &Library) {
        let input = std::mem::take(&mut self.code_input);
        if input.trim().is_empty() {
            return;
        }
        let id = match parse_student(&input) {
            None => Err(format!(
                "Code illisible : « {} ». Une carte d'élève s'écrit E000045.",
                input.trim()
            )),
            Some(id) => self.check(lib, id).map(|_| id).map_err(|e| e.to_string()),
        };
        self.message = Some(match id {
            Err(text) => Message::Err(text),
            Ok(id) if self.push([id]) == 0 => {
                Message::Err(format!("{} est déjà dans la liste.", student_code(id)))
            }
            Ok(id) => Message::Ok(format!("{} ajouté à la liste.", student_code(id))),
        });
    }

    fn make_sheet(&self, lib: &Library, data_dir: &Path) -> Message {
        let sheet: Vec<Label> = self
            .ids
            .iter()
            .filter_map(|&id| self.label(lib, id))
            .collect();
        let pdf = labels::sheet_pdf(&sheet);
        let dir = data_dir.join("etiquettes");
        let name = format!(
            "{}-{}.pdf",
            FILE_PREFIX,
            chrono::Local::now().format("%Y-%m-%d-%H%M%S")
        );
        let path = dir.join(name);
        let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, pdf));
        if let Err(e) = written {
            return Message::Err(format!("Impossible d'écrire le PDF : {e}"));
        }
        let pages = sheet.len().div_ceil(labels::LABELS_PER_PAGE);
        let mut text = format!(
            "Feuille créée : {} carte(s) sur {} page(s).\n{}\n\
             Imprimez-la en taille réelle (100 %), sans « ajuster à la page ».",
            sheet.len(),
            pages,
            path.display()
        );
        if let Err(e) = open_file(&path) {
            text.push_str(&format!(
                "\nOuverture automatique impossible ({e}) : ouvrez le fichier à la main."
            ));
        }
        Message::Ok(text)
    }
}
