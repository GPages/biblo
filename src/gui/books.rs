//! Books tab: the list of books, filtered live as the user types, with a
//! note on each copy (damaged, to repair…).

use biblo::codes::book_code;
use biblo::dates::format_date;
use biblo::search::{fold, Query};
use biblo::{Book, Store};
use eframe::egui::{self, Align, Layout, Margin, RichText};
use egui_extras::{Column, TableBuilder};

use super::{theme, Message};

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

#[derive(Default)]
pub struct BooksView {
    query: String,
    status: Status,
    only_notes: bool,
    note_form: Option<NoteForm>,
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

        let mut edit_note = None;
        theme::card(ui, |ui| {
            if total == 0 {
                ui.label(
                    "📚 Aucun livre pour l'instant : ajoutez-en dans l'onglet « Enregistrer ».",
                );
            } else if rows.is_empty() {
                ui.label("🔍 Aucun livre ne correspond.");
            } else {
                edit_note = book_table(ui, &rows);
            }
        });
        if let Some(book) = edit_note.and_then(|id| lib.book(id)) {
            self.note_form = Some(NoteForm {
                book: book.id,
                text: book.note.clone(),
                focus: true,
            });
            self.message = None;
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
}

/// The note on one line, for the list.
fn one_line(note: &str) -> String {
    note.lines().collect::<Vec<_>>().join(" · ")
}

/// Returns the book whose 📝 button was clicked.
fn book_table(ui: &mut egui::Ui, rows: &[(String, Row)]) -> Option<u32> {
    let mut edit_note = None;
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
                    let hint = if row.book.note.is_empty() {
                        "Ajouter une note (abîmé, à réparer…)"
                    } else {
                        "Modifier ou supprimer la note"
                    };
                    if ui.small_button("📝").on_hover_text(hint).clicked() {
                        edit_note = Some(row.book.id);
                    }
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
    edit_note
}
