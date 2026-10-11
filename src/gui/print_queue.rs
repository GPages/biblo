//! Books or pupils waiting to be printed on the next A4 label sheet.

use std::path::Path;

use biblo::codes::{book_code, parse_code_or_number, student_code, Kind, Scanned};
use biblo::dates::format_date;
use biblo::labels::{self, Label};
use biblo::{Error, Library};
use eframe::egui;

use super::{barcode, open_file, theme, Message};

/// The French texts that differ between book labels and pupil cards.
struct Texts {
    heading: &'static str,
    auto_added: &'static str,
    field: &'static str,
    hint: &'static str,
    wrong_kind: &'static str,
    file_prefix: &'static str,
    unit: &'static str,
}

fn texts(kind: Kind) -> Texts {
    match kind {
        Kind::Book => Texts {
            heading: "🏷 Étiquettes de livres à imprimer",
            auto_added: "Les livres enregistrés s'ajoutent ici automatiquement.",
            field: "Réimprimer une étiquette, code du livre :",
            hint: "L000123",
            wrong_kind: "C'est un code d'élève, pas un code de livre.",
            file_prefix: "etiquettes-livres",
            unit: "étiquette(s)",
        },
        Kind::Student => Texts {
            heading: "🎫 Cartes d'élèves à imprimer",
            auto_added: "Les élèves enregistrés s'ajoutent ici automatiquement.",
            field: "Ajouter une carte, code de l'élève :",
            hint: "E000045",
            wrong_kind: "C'est un code de livre, pas un code d'élève.",
            file_prefix: "cartes-eleves",
            unit: "carte(s)",
        },
    }
}

pub struct PrintQueue {
    kind: Kind,
    ids: Vec<u32>,
    code_input: String,
    /// Pupil cards: the class chosen to be added as a whole.
    class: String,
    message: Option<Message>,
}

impl PrintQueue {
    pub fn new(kind: Kind) -> PrintQueue {
        PrintQueue {
            kind,
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

    fn code(&self, id: u32) -> String {
        match self.kind {
            Kind::Book => book_code(id),
            Kind::Student => student_code(id),
        }
    }

    /// What gets printed for `id`; `None` if it no longer exists.
    fn label(&self, lib: &Library, id: u32) -> Option<Label> {
        let caption = match self.kind {
            Kind::Book => lib.book(id)?.title.clone(),
            Kind::Student => {
                let s = lib.student(id)?;
                format!("{} ({})", s.full_name(), format_date(s.birth_date))
            }
        };
        Some(Label {
            code: self.code(id),
            caption,
        })
    }

    /// Only existing pupils and non-archived books can be printed.
    fn check(&self, lib: &Library, id: u32) -> Result<(), Error> {
        match self.kind {
            Kind::Book => match lib.book(id) {
                None => Err(Error::BookNotFound(id)),
                Some(b) if b.archived => Err(Error::BookArchived(id)),
                Some(_) => Ok(()),
            },
            Kind::Student => match lib.student(id) {
                None => Err(Error::StudentNotFound(id)),
                Some(_) => Ok(()),
            },
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, lib: &Library, data_dir: &Path) {
        let t = texts(self.kind);
        ui.heading(format!("{} ({})", t.heading, self.ids.len()));
        ui.add_space(4.0);
        ui.label(t.auto_added);
        ui.horizontal(|ui| {
            ui.label(t.field);
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.code_input)
                    .hint_text(t.hint)
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
        if self.kind == Kind::Student {
            self.class_row(ui, lib);
        }
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
        let id = match (self.kind, parse_code_or_number(&input, self.kind)) {
            (_, None) => Err(format!("Code illisible : « {} ».", input.trim())),
            (Kind::Book, Some(Scanned::Book(id))) | (Kind::Student, Some(Scanned::Student(id))) => {
                self.check(lib, id).map(|_| id).map_err(|e| e.to_string())
            }
            _ => Err(texts(self.kind).wrong_kind.to_string()),
        };
        self.message = Some(match id {
            Err(text) => Message::Err(text),
            Ok(id) if self.push([id]) == 0 => {
                Message::Err(format!("{} est déjà dans la liste.", self.code(id)))
            }
            Ok(id) => Message::Ok(format!("{} ajouté à la liste.", self.code(id))),
        });
    }

    fn make_sheet(&self, lib: &Library, data_dir: &Path) -> Message {
        let t = texts(self.kind);
        let sheet: Vec<Label> = self
            .ids
            .iter()
            .filter_map(|&id| self.label(lib, id))
            .collect();
        let pdf = labels::sheet_pdf(&sheet);
        let dir = data_dir.join("etiquettes");
        let name = format!(
            "{}-{}.pdf",
            t.file_prefix,
            chrono::Local::now().format("%Y-%m-%d-%H%M%S")
        );
        let path = dir.join(name);
        let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, pdf));
        if let Err(e) = written {
            return Message::Err(format!("Impossible d'écrire le PDF : {e}"));
        }
        let pages = sheet.len().div_ceil(labels::LABELS_PER_PAGE);
        let mut text = format!(
            "Feuille créée : {} {} sur {} page(s).\n{}\n\
             Imprimez-la en taille réelle (100 %), sans « ajuster à la page ».",
            sheet.len(),
            t.unit,
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
