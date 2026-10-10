//! Second view: register books (and print their barcode labels) and pupils.

use std::path::Path;

use biblo::codes::{book_code, parse_code_or_number, student_code, Kind, Scanned};
use biblo::dates::{format_date, parse_date, parse_year};
use biblo::labels::{self, Label};
use biblo::search::{fold, Query};
use biblo::{BookInput, Store};
use eframe::egui::{self, Align, Layout};
use egui_extras::{Column, TableBuilder};

use super::{barcode, open_file, Message};

pub struct RegisterView {
    // New book form.
    title: String,
    author: String,
    collection: String,
    year: String,
    isbn: String,
    copies: usize,
    book_message: Option<Message>,
    focus_title: bool,

    /// Book ids waiting to be printed on the next label sheet.
    to_print: Vec<u32>,
    reprint_code: String,
    labels_message: Option<Message>,

    // New pupil form.
    last_name: String,
    first_name: String,
    birth_date: String,
    student_message: Option<Message>,
    student_query: String,
}

impl Default for RegisterView {
    fn default() -> Self {
        RegisterView {
            title: String::new(),
            author: String::new(),
            collection: String::new(),
            year: String::new(),
            isbn: String::new(),
            copies: 1,
            book_message: None,
            focus_title: false,
            to_print: Vec::new(),
            reprint_code: String::new(),
            labels_message: None,
            last_name: String::new(),
            first_name: String::new(),
            birth_date: String::new(),
            student_message: None,
            student_query: String::new(),
        }
    }
}

impl RegisterView {
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store, data_dir: &Path) {
        // Plain top-down layout: `columns` would stretch buttons and justify text.
        let plain = Layout::top_down(Align::Min);
        ui.columns(2, |cols| {
            egui::ScrollArea::vertical()
                .id_salt("books_column")
                .show(&mut cols[0], |ui| {
                    ui.with_layout(plain, |ui| {
                        self.book_form(ui, store);
                        ui.add_space(12.0);
                        ui.separator();
                        self.label_sheet(ui, store, data_dir);
                    });
                });
            egui::ScrollArea::vertical()
                .id_salt("students_column")
                .show(&mut cols[1], |ui| {
                    ui.with_layout(plain, |ui| {
                        self.student_form(ui, store);
                        ui.add_space(12.0);
                        ui.separator();
                        self.student_list(ui, store);
                    });
                });
        });
    }

    // ------------------------------------------------------------ books

    fn book_form(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        ui.heading("Nouveau livre");
        ui.add_space(6.0);
        egui::Grid::new("book_form")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Titre *");
                let title =
                    ui.add(egui::TextEdit::singleline(&mut self.title).desired_width(320.0));
                if std::mem::take(&mut self.focus_title) {
                    title.request_focus();
                }
                ui.end_row();
                for (label, value) in [
                    ("Auteur", &mut self.author),
                    ("Collection", &mut self.collection),
                ] {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(value).desired_width(320.0));
                    ui.end_row();
                }
                ui.label("Année de parution");
                ui.add(
                    egui::TextEdit::singleline(&mut self.year)
                        .hint_text("aaaa")
                        .desired_width(60.0),
                );
                ui.end_row();
                ui.label("ISBN");
                ui.add(egui::TextEdit::singleline(&mut self.isbn).desired_width(200.0));
                ui.end_row();
                ui.label("Exemplaires");
                ui.add(egui::DragValue::new(&mut self.copies).range(1..=100));
                ui.end_row();
            });
        ui.weak("* obligatoire. Chaque exemplaire reçoit son propre code-barres.");
        ui.add_space(6.0);
        if ui.button("Enregistrer le livre").clicked() {
            self.book_message = Some(self.save_book(store));
        }
        if let Some(m) = &self.book_message {
            m.show(ui);
        }
    }

    fn save_book(&mut self, store: &mut Store) -> Message {
        let year = if self.year.trim().is_empty() {
            None
        } else {
            match parse_year(&self.year) {
                Some(y) => Some(y),
                None => {
                    return Message::Err(
                        "Année de parution invalide : 4 chiffres, par exemple 2019.".into(),
                    )
                }
            }
        };
        let input = BookInput {
            title: self.title.clone(),
            author: self.author.clone(),
            collection: self.collection.clone(),
            year,
            isbn: self.isbn.clone(),
        };
        match store.apply(|lib| lib.add_copies(input, self.copies)) {
            Err(e) => Message::Err(e.to_string()),
            Ok(ids) => {
                self.to_print.extend(&ids);
                for field in [
                    &mut self.title,
                    &mut self.author,
                    &mut self.collection,
                    &mut self.year,
                    &mut self.isbn,
                ] {
                    field.clear();
                }
                self.copies = 1;
                self.focus_title = true;
                match ids.as_slice() {
                    [id] => Message::Ok(format!("Livre enregistré : {}.", book_code(*id))),
                    _ => Message::Ok(format!(
                        "{} exemplaires enregistrés : {} à {}.",
                        ids.len(),
                        book_code(ids[0]),
                        book_code(ids[ids.len() - 1])
                    )),
                }
            }
        }
    }

    fn label_sheet(&mut self, ui: &mut egui::Ui, store: &Store, data_dir: &Path) {
        let lib = store.lib();
        ui.heading(format!("Étiquettes à imprimer ({})", self.to_print.len()));
        ui.add_space(4.0);
        ui.label("Les livres enregistrés s'ajoutent ici automatiquement.");
        ui.horizontal(|ui| {
            ui.label("Réimprimer une étiquette, code du livre :");
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.reprint_code)
                    .hint_text("L000123")
                    .desired_width(90.0),
            );
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Ajouter").clicked() || entered {
                self.labels_message = self.add_reprint(store);
                if entered {
                    field.request_focus();
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let has_labels = !self.to_print.is_empty();
            if ui
                .add_enabled(has_labels, egui::Button::new("Créer la feuille PDF"))
                .clicked()
            {
                self.labels_message = Some(self.make_sheet(store, data_dir));
            }
            if ui
                .add_enabled(has_labels, egui::Button::new("Vider la liste"))
                .clicked()
            {
                self.to_print.clear();
                self.labels_message = None;
            }
        });
        if let Some(m) = &self.labels_message {
            m.show(ui);
        }
        ui.add_space(6.0);

        let mut remove = None;
        for (i, id) in self.to_print.iter().enumerate() {
            let Some(book) = lib.book(*id) else { continue };
            ui.horizontal(|ui| {
                let code = book_code(book.id);
                barcode(ui, &code, 2.0, 40.0);
                ui.vertical(|ui| {
                    ui.monospace(&code);
                    ui.label(&book.title);
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("Retirer").clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            self.to_print.remove(i);
        }
    }

    fn add_reprint(&mut self, store: &Store) -> Option<Message> {
        if self.reprint_code.trim().is_empty() {
            return None;
        }
        let message = match parse_code_or_number(&self.reprint_code, Kind::Book) {
            None => Message::Err(format!(
                "Code illisible : « {} ».",
                self.reprint_code.trim()
            )),
            Some(Scanned::Student(_)) => {
                Message::Err("C'est un code d'élève, pas un code de livre.".into())
            }
            Some(Scanned::Book(id)) => match store.lib().book(id) {
                None => Message::Err(format!("Livre introuvable : {}.", book_code(id))),
                Some(_) if self.to_print.contains(&id) => {
                    Message::Err(format!("{} est déjà dans la liste.", book_code(id)))
                }
                Some(_) => {
                    self.to_print.push(id);
                    Message::Ok(format!("{} ajouté à la liste.", book_code(id)))
                }
            },
        };
        self.reprint_code.clear();
        Some(message)
    }

    fn make_sheet(&self, store: &Store, data_dir: &Path) -> Message {
        let lib = store.lib();
        let sheet: Vec<Label> = self
            .to_print
            .iter()
            .filter_map(|id| lib.book(*id))
            .map(|b| Label {
                code: book_code(b.id),
                caption: b.title.clone(),
            })
            .collect();
        let pdf = labels::sheet_pdf(&sheet);
        let dir = data_dir.join("etiquettes");
        let name = format!(
            "etiquettes-{}.pdf",
            chrono::Local::now().format("%Y-%m-%d-%H%M%S")
        );
        let path = dir.join(name);
        let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, pdf));
        if let Err(e) = written {
            return Message::Err(format!("Impossible d'écrire le PDF : {e}"));
        }
        let pages = sheet.len().div_ceil(labels::LABELS_PER_PAGE);
        let mut text = format!(
            "Feuille créée : {} étiquette(s) sur {} page(s).\n{}\n\
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

    // --------------------------------------------------------- students

    fn student_form(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        ui.heading("Nouvel élève");
        ui.add_space(6.0);
        egui::Grid::new("student_form")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Nom *");
                ui.add(egui::TextEdit::singleline(&mut self.last_name).desired_width(240.0));
                ui.end_row();
                ui.label("Prénom *");
                ui.add(egui::TextEdit::singleline(&mut self.first_name).desired_width(240.0));
                ui.end_row();
                ui.label("Date de naissance *");
                ui.add(
                    egui::TextEdit::singleline(&mut self.birth_date)
                        .hint_text("jj/mm/aaaa")
                        .desired_width(100.0),
                );
                ui.end_row();
            });
        ui.weak("* obligatoire.");
        ui.add_space(6.0);
        if ui.button("Enregistrer l'élève").clicked() {
            self.student_message = Some(self.save_student(store));
        }
        if let Some(m) = &self.student_message {
            m.show(ui);
        }
    }

    fn save_student(&mut self, store: &mut Store) -> Message {
        let Some(birth) = parse_date(&self.birth_date) else {
            return Message::Err(
                "Date de naissance invalide : jj/mm/aaaa, par exemple 14/05/2017.".into(),
            );
        };
        if birth > biblo::today() {
            return Message::Err("La date de naissance est dans le futur.".into());
        }
        let (last, first) = (self.last_name.clone(), self.first_name.clone());
        match store.apply(|lib| lib.add_student(&last, &first, birth)) {
            Err(e) => Message::Err(e.to_string()),
            Ok(id) => {
                self.last_name.clear();
                self.first_name.clear();
                self.birth_date.clear();
                let name = store
                    .lib()
                    .student(id)
                    .map(|s| s.full_name())
                    .unwrap_or_default();
                Message::Ok(format!("Élève enregistré : {name} ({}).", student_code(id)))
            }
        }
    }

    fn student_list(&mut self, ui: &mut egui::Ui, store: &Store) {
        let lib = store.lib();
        let all: Vec<_> = lib.students().iter().filter(|s| !s.archived).collect();
        ui.heading(format!("Élèves inscrits ({})", all.len()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Rechercher :");
            ui.add(
                egui::TextEdit::singleline(&mut self.student_query)
                    .hint_text("nom, prénom, date, code…")
                    .desired_width(240.0),
            );
        });
        ui.add_space(4.0);

        let query = Query::new(&self.student_query);
        let mut rows: Vec<(String, [String; 4])> = all
            .iter()
            .map(|s| {
                let key = fold(&format!("{}\n{}", s.last_name, s.first_name));
                let cells = [
                    student_code(s.id),
                    s.last_name.clone(),
                    s.first_name.clone(),
                    format_date(s.birth_date),
                ];
                (key, cells)
            })
            .filter(|(_, cells)| query.matches(&cells.each_ref().map(String::as_str)))
            .collect();
        rows.sort();

        if rows.is_empty() {
            ui.label(if all.is_empty() {
                "Aucun élève pour l'instant."
            } else {
                "Aucun élève ne correspond."
            });
            return;
        }
        TableBuilder::new(ui)
            .id_salt("students")
            .striped(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::auto())
            .column(Column::initial(140.0).at_least(60.0).clip(true))
            .column(Column::initial(120.0).at_least(60.0).clip(true))
            .column(Column::remainder())
            .header(22.0, |mut header| {
                for title in ["Code", "Nom", "Prénom", "Né(e) le"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, rows.len(), |mut row| {
                    let (_, cells) = &rows[row.index()];
                    row.col(|ui| {
                        ui.monospace(&cells[0]);
                    });
                    for text in &cells[1..] {
                        row.col(|ui| {
                            ui.label(text);
                        });
                    }
                });
            });
    }
}
