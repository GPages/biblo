//! Books tab: the list of books, filtered live as the user types, with a
//! note on each copy (damaged, to repair…).

use biblo::codes::book_code;
use biblo::dates::{format_date, parse_year};
use biblo::search::{fold, Query};
use biblo::{Book, BookInput, Store};
use eframe::egui::{self, Align, Layout, Margin, RichText};
use egui_extras::{Column, TableBuilder};

use super::{text_with_choices, theme, Message};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Status {
    #[default]
    All,
    Available,
    OnLoan,
}

/// The note being written for one copy.
struct NoteForm {
    book: u32,
    text: String,
    focus: bool,
}

/// A book's details being edited.
struct EditForm {
    id: u32,
    title: String,
    author: String,
    collection: String,
    category: String,
    year: String,
    isbn: String,
    error: Option<String>,
}

impl EditForm {
    fn new(book: &Book) -> EditForm {
        EditForm {
            id: book.id,
            title: book.title.clone(),
            author: book.author.clone(),
            collection: book.collection.clone(),
            category: book.category.clone(),
            year: book.year.map(|y| y.to_string()).unwrap_or_default(),
            isbn: book.isbn.clone(),
            error: None,
        }
    }
}

/// What the user clicked on in a row of the list.
enum RowAction {
    Note(u32),
    Edit(u32),
}

#[derive(Default)]
pub struct BooksView {
    query: String,
    status: Status,
    only_notes: bool,
    note_form: Option<NoteForm>,
    edit_form: Option<EditForm>,
    message: Option<Message>,
}

struct Row<'a> {
    book: &'a Book,
    code: String,
    year: String,
    /// `None` when the book is on the shelf.
    loan: Option<String>,
}

impl BooksView {
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").size(22.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("titre, auteur, catégorie, code, élève, note…")
                        .desired_width(440.0),
                );
                if ui.button("Effacer").clicked() {
                    self.query.clear();
                }
                ui.add_space(12.0);
                ui.selectable_value(&mut self.status, Status::All, "Tous");
                ui.selectable_value(&mut self.status, Status::Available, "Disponibles");
                ui.selectable_value(&mut self.status, Status::OnLoan, "Prêtés");
            });
            ui.checkbox(
                &mut self.only_notes,
                "📝 Seulement les livres qui ont une note (abîmés, à réparer…)",
            );
            if let Some(m) = &self.message {
                m.show(ui);
            }
            self.note_box(ui, store);
            self.edit_box(ui, store);
        });

        let lib = store.lib();
        let open_loans = lib.open_loans_by_book();
        let query = Query::new(&self.query);
        let mut rows: Vec<(String, Row)> = lib
            .books()
            .iter()
            .filter(|b| !b.archived)
            .filter(|b| !self.only_notes || !b.note.is_empty())
            .filter_map(|book| {
                let loan = open_loans.get(&book.id);
                match (self.status, loan) {
                    (Status::Available, Some(_)) | (Status::OnLoan, None) => return None,
                    _ => {}
                }
                let row = Row {
                    book,
                    code: book_code(book.id),
                    year: book.year.map(|y| y.to_string()).unwrap_or_default(),
                    loan: loan.map(|l| {
                        let who = lib
                            .student(l.student_id)
                            .map(|s| s.full_name())
                            .unwrap_or_default();
                        format!("Prêté à {who} ({})", format_date(l.borrowed_on))
                    }),
                };
                let status = row.loan.as_deref().unwrap_or("Disponible");
                let fields = [
                    row.code.as_str(),
                    &book.title,
                    &book.author,
                    &book.collection,
                    &book.category,
                    &row.year,
                    &book.isbn,
                    status,
                    &book.note,
                ];
                query.matches(&fields).then(|| (fold(&book.title), row))
            })
            .collect();
        rows.sort_by(|(a, ra), (b, rb)| a.cmp(b).then(ra.book.id.cmp(&rb.book.id)));

        let total = lib.books().iter().filter(|b| !b.archived).count();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.weak(format!("{} livre(s) affiché(s) sur {}", rows.len(), total));
        });
        ui.add_space(4.0);

        let mut action = None;
        theme::card(ui, |ui| {
            if total == 0 {
                ui.label(
                    "📚 Aucun livre pour l'instant : ajoutez-en dans l'onglet « Enregistrer ».",
                );
            } else if rows.is_empty() {
                ui.label("🔍 Aucun livre ne correspond.");
            } else {
                action = book_table(ui, &rows);
            }
        });
        match action {
            Some(RowAction::Note(id)) => {
                if let Some(book) = lib.book(id) {
                    self.note_form = Some(NoteForm {
                        book: book.id,
                        text: book.note.clone(),
                        focus: true,
                    });
                    self.edit_form = None;
                    self.message = None;
                }
            }
            Some(RowAction::Edit(id)) => {
                if let Some(book) = lib.book(id) {
                    self.edit_form = Some(EditForm::new(book));
                    self.note_form = None;
                    self.message = None;
                }
            }
            None => {}
        }
    }

    /// Editor for the note of the copy chosen in the list.
    fn note_box(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        let Some(form) = &mut self.note_form else {
            return;
        };
        let Some(book) = store.lib().book(form.book) else {
            self.note_form = None;
            return;
        };
        let what = format!("« {} » ({})", book.title, book_code(book.id));
        let had_note = !book.note.is_empty();
        let (mut save, mut remove, mut cancel) = (false, false, false);
        egui::Frame::new()
            .fill(theme::NOTE.0)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("📝 Note sur {what}"))
                        .family(theme::bold())
                        .color(theme::NOTE.1),
                );
                let field = ui.add(
                    egui::TextEdit::multiline(&mut form.text)
                        .hint_text("Par exemple : couverture abîmée, page 12 déchirée, à réparer…")
                        .desired_rows(3)
                        .desired_width(600.0),
                );
                if std::mem::take(&mut form.focus) {
                    field.request_focus();
                }
                ui.horizontal(|ui| {
                    save = ui
                        .add(theme::main_button("Enregistrer la note", theme::GREEN))
                        .clicked();
                    if had_note {
                        remove = ui.button("Supprimer la note").clicked();
                    }
                    cancel = ui.button("Annuler").clicked();
                });
            });
        if save || remove {
            let (id, text) = (form.book, if remove { "" } else { form.text.as_str() });
            let result = store.apply(|lib| lib.set_book_note(id, text));
            let removed = store.lib().book(id).is_some_and(|b| b.note.is_empty());
            self.message = Some(match result {
                Err(e) => Message::Err(e.to_string()),
                Ok(()) if removed => Message::Ok(format!("Note supprimée : {what}.")),
                Ok(()) => Message::Ok(format!("Note enregistrée : {what}.")),
            });
            self.note_form = None;
        } else if cancel {
            self.note_form = None;
        }
    }

    /// Form to change the details of the book chosen in the list.
    fn edit_box(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        let Some(form) = &mut self.edit_form else {
            return;
        };
        let Some(book) = store.lib().book(form.id) else {
            self.edit_form = None;
            return;
        };
        let what = format!("« {} » ({})", book.title, book_code(book.id));
        let old_title = book.title.clone();
        let categories = store.lib().categories();
        let (mut save, mut cancel) = (false, false);
        egui::Frame::new()
            .fill(theme::INFO.0)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("✏ Fiche de {what}"))
                        .family(theme::bold())
                        .color(theme::INFO.1),
                );
                egui::Grid::new("edit_book")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Titre *");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.title).desired_width(320.0),
                        );
                        ui.end_row();
                        ui.label("Auteur");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.author).desired_width(320.0),
                        );
                        ui.end_row();
                        ui.label("Collection");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.collection)
                                .desired_width(320.0),
                        );
                        ui.end_row();
                        ui.label("Catégorie");
                        text_with_choices(
                            ui,
                            "edit_book_category_pick",
                            &mut form.category,
                            "Roman, BD, Album…",
                            categories,
                        );
                        ui.end_row();
                        ui.label("Année de parution");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.year)
                                .hint_text("aaaa")
                                .desired_width(60.0),
                        );
                        ui.end_row();
                        ui.label("ISBN");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.isbn).desired_width(220.0),
                        );
                        ui.end_row();
                    });
                ui.weak("* obligatoire. Le code-barres et la note de l'exemplaire ne changent pas.");
                ui.horizontal(|ui| {
                    save = ui
                        .add(theme::main_button("Enregistrer", theme::GREEN))
                        .clicked();
                    cancel = ui.button("Annuler").clicked();
                });
                if let Some(error) = &form.error {
                    ui.colored_label(theme::RED, format!("⚠ {error}"));
                }
            });
        if save {
            match save_edit(store, form, &old_title) {
                Ok(text) => {
                    self.message = Some(Message::Ok(text));
                    self.edit_form = None;
                }
                Err(error) => form.error = Some(error),
            }
        } else if cancel {
            self.edit_form = None;
        }
    }
}

/// The note on one line, for the list.
fn one_line(note: &str) -> String {
    note.lines().collect::<Vec<_>>().join(" · ")
}

/// What was clicked on in a row of the list.
fn book_table(ui: &mut egui::Ui, rows: &[(String, Row)]) -> Option<RowAction> {
    let mut action = None;
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::auto())
        .column(Column::auto())
        .column(Column::initial(210.0).at_least(80.0).clip(true))
        .column(Column::initial(135.0).at_least(60.0).clip(true))
        .column(Column::initial(110.0).at_least(60.0).clip(true))
        .column(Column::initial(120.0).at_least(60.0).clip(true))
        .column(Column::auto())
        .column(Column::initial(75.0).at_least(50.0).clip(true))
        .column(Column::remainder().at_least(100.0).clip(true))
        .header(26.0, |mut header| {
            for title in [
                "",
                "Code",
                "Titre",
                "Auteur",
                "Collection",
                "Catégorie",
                "Année",
                "ISBN",
                "Statut",
            ] {
                header.col(|ui| {
                    ui.label(RichText::new(title).family(theme::bold()));
                });
            }
        })
        .body(|body| {
            // A book with a note gets a taller row: the note goes under the title.
            let heights = rows
                .iter()
                .map(|(_, r)| if r.book.note.is_empty() { 30.0 } else { 52.0 });
            body.heterogeneous_rows(heights, |mut table_row| {
                let (_, row) = &rows[table_row.index()];
                table_row.col(|ui| {
                    ui.horizontal(|ui| {
                        let hint = if row.book.note.is_empty() {
                            "Ajouter une note (abîmé, à réparer…)"
                        } else {
                            "Modifier ou supprimer la note"
                        };
                        if ui.small_button("📝").on_hover_text(hint).clicked() {
                            action = Some(RowAction::Note(row.book.id));
                        }
                        if ui
                            .small_button("✏")
                            .on_hover_text("Modifier la fiche (titre, auteur, collection…)")
                            .clicked()
                        {
                            action = Some(RowAction::Edit(row.book.id));
                        }
                    });
                });
                table_row.col(|ui| {
                    ui.monospace(&row.code);
                });
                table_row.col(|ui| {
                    if row.book.note.is_empty() {
                        ui.add(egui::Label::new(&row.book.title).truncate());
                        return;
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add(egui::Label::new(&row.book.title).truncate());
                        let note = RichText::new(format!("📝 {}", one_line(&row.book.note)))
                            .italics()
                            .size(15.0)
                            .color(theme::NOTE.1);
                        ui.add(egui::Label::new(note).truncate())
                            .on_hover_text(&row.book.note);
                    });
                });
                for text in [&row.book.author, &row.book.collection] {
                    table_row.col(|ui| {
                        ui.label(text);
                    });
                }
                table_row.col(|ui| {
                    let category = &row.book.category;
                    if !category.is_empty() {
                        theme::badge(ui, category, theme::category_badge(category));
                    }
                });
                table_row.col(|ui| {
                    ui.label(&row.year);
                });
                table_row.col(|ui| {
                    ui.label(&row.book.isbn);
                });
                table_row.col(|ui| match &row.loan {
                    None => theme::badge(ui, "Disponible", theme::AVAILABLE),
                    Some(text) => theme::badge(ui, text, theme::ON_LOAN),
                });
            });
        });
    action
}

/// Saves the edited details; returns the message to show, or the error.
fn save_edit(store: &mut Store, form: &EditForm, old_title: &str) -> Result<String, String> {
    let year = if form.year.trim().is_empty() {
        None
    } else {
        match parse_year(&form.year) {
            Some(y) => Some(y),
            None => {
                return Err("Année de parution invalide : 4 chiffres, par exemple 2019.".into())
            }
        }
    };
    let input = BookInput {
        title: form.title.clone(),
        author: form.author.clone(),
        collection: form.collection.clone(),
        category: form.category.clone(),
        year,
        isbn: form.isbn.clone(),
    };
    store
        .apply(|lib| lib.update_book(form.id, &input))
        .map_err(|e| e.to_string())?;
    let book = store.lib().book(form.id);
    let mut text = match book {
        Some(b) => format!("Fiche enregistrée : « {} » ({}).", b.title, book_code(b.id)),
        None => "Fiche enregistrée.".to_string(),
    };
    // The label shows the title next to the barcode.
    if book.is_some_and(|b| b.title != old_title) {
        text.push_str(
            " Le code-barres ne change pas ; pour que l'étiquette affiche le nouveau titre, \
             réimprimez-la depuis l'onglet Enregistrer.",
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Temporary folder, removed at the end of the test.
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
                "biblo-gui-test-{}-{}-{}",
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

    /// A store holding one book, "Matilda" by Roald Dahl, with a note.
    fn store_with_book() -> (TempDir, Store, u32) {
        let t = TempDir::new();
        let mut store = Store::open(t.0.join("data"), d(2026, 10, 10)).unwrap();
        let id = store
            .apply(|lib| {
                lib.add_book(BookInput {
                    title: "Matilda".into(),
                    author: "Roald Dahl".into(),
                    year: Some(1988),
                    ..Default::default()
                })
            })
            .unwrap();
        store.apply(|lib| lib.set_book_note(id, "Couverture abîmée")).unwrap();
        (t, store, id)
    }

    #[test]
    fn save_edit_updates_author_and_keeps_note_and_code() {
        let (_t, mut store, id) = store_with_book();
        let mut form = EditForm::new(store.lib().book(id).unwrap());
        form.author = "  R.   Dahl ".into();
        let msg = save_edit(&mut store, &form, "Matilda").unwrap();
        let book = store.lib().book(id).unwrap();
        assert_eq!(book.author, "R. Dahl");
        // the per-copy note and the id (hence the barcode) are untouched
        assert_eq!(book.note, "Couverture abîmée");
        assert_eq!(book.id, id);
        assert_eq!(book.year, Some(1988));
        assert!(msg.contains("Fiche enregistrée"));
        assert!(!msg.contains("réimprimez"));
    }

    #[test]
    fn save_edit_mentions_reprint_only_when_the_title_changes() {
        let (_t, mut store, id) = store_with_book();
        let mut form = EditForm::new(store.lib().book(id).unwrap());
        form.title = "Matilda (poche)".into();
        let msg = save_edit(&mut store, &form, "Matilda").unwrap();
        assert!(msg.contains("réimprimez"));
        assert_eq!(store.lib().book(id).unwrap().title, "Matilda (poche)");
    }

    #[test]
    fn save_edit_rejects_bad_year_and_empty_title_without_changing_anything() {
        let (_t, mut store, id) = store_with_book();
        let mut form = EditForm::new(store.lib().book(id).unwrap());
        form.year = "19".into();
        let err = save_edit(&mut store, &form, "Matilda").unwrap_err();
        assert!(err.contains("Année de parution invalide"));
        assert_eq!(store.lib().book(id).unwrap().year, Some(1988));

        form.year = "1990".into();
        form.title = "   ".into();
        let err = save_edit(&mut store, &form, "Matilda").unwrap_err();
        assert!(err.contains("titre est obligatoire"));
        let book = store.lib().book(id).unwrap();
        assert_eq!(book.title, "Matilda");
        assert_eq!(book.year, Some(1988));

        // a valid year goes through, an empty one clears it
        form.title = "Matilda".into();
        save_edit(&mut store, &form, "Matilda").unwrap();
        assert_eq!(store.lib().book(id).unwrap().year, Some(1990));
        form.year = " ".into();
        save_edit(&mut store, &form, "Matilda").unwrap();
        assert_eq!(store.lib().book(id).unwrap().year, None);
    }
}
