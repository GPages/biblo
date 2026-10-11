//! Main view: the list of books, filtered live as the user types.

use biblo::codes::book_code;
use biblo::dates::format_date;
use biblo::search::{fold, Query};
use biblo::{Book, Store};
use eframe::egui::{self, Align, Layout, RichText};
use egui_extras::{Column, TableBuilder};

use super::theme;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Status {
    #[default]
    All,
    Available,
    OnLoan,
}

#[derive(Default)]
pub struct BooksView {
    query: String,
    status: Status,
}

struct Row<'a> {
    book: &'a Book,
    code: String,
    year: String,
    /// `None` when the book is on the shelf.
    loan: Option<String>,
}

impl BooksView {
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &Store) {
        let lib = store.lib();
        let open_loans = lib.open_loans_by_book();

        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").size(22.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(
                            "titre, auteur, collection, catégorie, année, ISBN, code, élève…",
                        )
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
        });

        let query = Query::new(&self.query);
        let mut rows: Vec<(String, Row)> = lib
            .books()
            .iter()
            .filter(|b| !b.archived)
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

        theme::card(ui, |ui| {
            if total == 0 {
                ui.label(
                    "📚 Aucun livre pour l'instant : ajoutez-en dans l'onglet « Enregistrer ».",
                );
            } else if rows.is_empty() {
                ui.label("🔍 Aucun livre ne correspond.");
            } else {
                book_table(ui, &rows);
            }
        });
    }
}

fn book_table(ui: &mut egui::Ui, rows: &[(String, Row)]) {
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::auto())
        .column(Column::initial(230.0).at_least(80.0).clip(true))
        .column(Column::initial(150.0).at_least(60.0).clip(true))
        .column(Column::initial(110.0).at_least(60.0).clip(true))
        .column(Column::initial(120.0).at_least(60.0).clip(true))
        .column(Column::auto())
        .column(Column::initial(90.0).at_least(50.0).clip(true))
        .column(Column::remainder().at_least(100.0).clip(true))
        .header(26.0, |mut header| {
            for title in [
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
            body.rows(30.0, rows.len(), |mut table_row| {
                let (_, row) = &rows[table_row.index()];
                table_row.col(|ui| {
                    ui.monospace(&row.code);
                });
                for text in [&row.book.title, &row.book.author, &row.book.collection] {
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
}
