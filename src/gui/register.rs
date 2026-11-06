//! "Enregistrer" tab: register books and pupils, and print their barcodes.
//! Books and pupils each have their own screen, so the lists get room.

use std::path::Path;

use biblo::codes::student_code;
use biblo::dates::{format_date, parse_date, parse_year};
use biblo::search::{fold, Query};
use biblo::{BookInput, Library, Store};
use eframe::egui::{self, Align, Layout, Margin, RichText};
use egui_extras::{Column, TableBuilder};

use super::isbn::{IsbnService, Lookup};
use super::print_queue::PrintQueue;
use super::{text_with_choices, theme, Message};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Books,
    Students,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BookField {
    /// The ISBN when the database is there (scan the book first), else the title.
    First,
    Isbn,
    Title,
    Author,
}

pub struct RegisterView {
    section: Section,

    // New book form.
    title: String,
    author: String,
    collection: String,
    category: String,
    year: String,
    isbn: String,
    copies: usize,
    /// The author field is the one being filled in: its suggestions show.
    author_active: bool,
    book_message: Option<Message>,
    /// Field that gets the keyboard on the next frame.
    focus: Option<BookField>,
    /// What the ISBN lookup said.
    isbn_message: Option<Message>,
    /// Title and collection as the ISBN lookup filled them in: a new lookup
    /// replaces them, but never something typed by hand.
    filled_title: String,
    filled_collection: String,

    /// Books registered in this session, newest first: (title, ISBN, copies).
    recent_books: Vec<(String, String, usize)>,

    // New pupil form.
    last_name: String,
    first_name: String,
    birth_date: String,
    class: String,
    student_message: Option<Message>,
    student_query: String,

    student_cards: PrintQueue,
}

impl Default for RegisterView {
    fn default() -> Self {
        RegisterView {
            section: Section::Books,
            title: String::new(),
            author: String::new(),
            collection: String::new(),
            category: String::new(),
            year: String::new(),
            isbn: String::new(),
            copies: 1,
            author_active: false,
            book_message: None,
            focus: None,
            isbn_message: None,
            filled_title: String::new(),
            filled_collection: String::new(),
            recent_books: Vec::new(),
            last_name: String::new(),
            first_name: String::new(),
            birth_date: String::new(),
            class: String::new(),
            student_message: None,
            student_query: String::new(),
            student_cards: PrintQueue::new(),
        }
    }
}

/// A column with one scrollbar, as tall as the window. Plain top-down
/// layout: `columns` would stretch buttons and justify text.
fn scroll_column(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical().id_salt(id).show(ui, |ui| {
        ui.with_layout(Layout::top_down(Align::Min), add);
    });
}

impl RegisterView {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Store,
        data_dir: &Path,
        isbn: &IsbnService,
    ) {
        ui.horizontal(|ui| {
            let section = &mut self.section;
            ui.selectable_value(section, Section::Books, "📖 Enregistrer des livres");
            ui.selectable_value(section, Section::Students, "🎒 Enregistrer des élèves");
        });
        ui.add_space(8.0);
        match self.section {
            Section::Books => ui.columns(2, |cols| {
                scroll_column(&mut cols[0], "book_form_column", |ui| {
                    theme::card(ui, |ui| self.book_form(ui, store, isbn));
                });
                scroll_column(&mut cols[1], "recent_books_column", |ui| {
                    theme::card(ui, |ui| self.recent_books_card(ui));
                });
            }),
            Section::Students => ui.columns(2, |cols| {
                scroll_column(&mut cols[0], "student_form_column", |ui| {
                    theme::card(ui, |ui| self.student_form(ui, store));
                    ui.add_space(14.0);
                    theme::card(ui, |ui| self.student_cards.ui(ui, store.lib(), data_dir));
                });
                // No scroll area around the list: its table scrolls by itself,
                // using the whole height of the window.
                cols[1].with_layout(Layout::top_down(Align::Min), |ui| {
                    theme::card(ui, |ui| self.student_list(ui, store));
                });
            }),
        }
    }

    // ------------------------------------------------------------ books

    fn book_form(&mut self, ui: &mut egui::Ui, store: &mut Store, isbn: &IsbnService) {
        ui.heading("📖 Nouveau livre");
        ui.add_space(6.0);
        egui::Grid::new("book_form")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                // Top-aligned: the message under the field makes this row taller.
                ui.with_layout(Layout::top_down(Align::Min), |ui| ui.label("ISBN *"));
                self.isbn_field(ui, isbn);
                ui.end_row();
                ui.label("Titre *");
                let title =
                    ui.add(egui::TextEdit::singleline(&mut self.title).desired_width(320.0));
                if self.focus == Some(BookField::Title)
                    || (self.focus == Some(BookField::First) && !isbn.is_ready())
                {
                    title.request_focus();
                    self.focus = None;
                }
                ui.end_row();
                // Top-aligned: the suggestions make this row taller.
                ui.with_layout(Layout::top_down(Align::Min), |ui| ui.label("Auteur"));
                self.author_field(ui, store.lib());
                ui.end_row();
                ui.label("Collection");
                ui.add(egui::TextEdit::singleline(&mut self.collection).desired_width(320.0));
                ui.end_row();
                ui.label("Catégorie");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.category)
                            .hint_text("Roman, BD, Album…")
                            .desired_width(200.0),
                    );
                    // Picking an existing category keeps the spelling consistent.
                    let categories = store.lib().categories();
                    if !categories.is_empty() {
                        egui::ComboBox::from_id_salt("category_pick")
                            .selected_text("Choisir…")
                            .show_ui(ui, |ui| {
                                for c in categories {
                                    let current = fold(&c) == fold(&self.category);
                                    if ui.selectable_label(current, &c).clicked() {
                                        self.category = c;
                                    }
                                }
                            });
                    }
                });
                ui.end_row();
                ui.label("Année de parution");
                ui.add(
                    egui::TextEdit::singleline(&mut self.year)
                        .hint_text("aaaa")
                        .desired_width(60.0),
                );
                ui.end_row();
                ui.label("Exemplaires");
                ui.add(egui::DragValue::new(&mut self.copies).range(1..=100));
                ui.end_row();
            });
        ui.weak(
            "* obligatoire. L'ISBN est le code-barres imprimé sur le livre : c'est lui qu'on \
             scanne pour les prêts. Les exemplaires d'un même livre ont le même ISBN.",
        );
        ui.add_space(6.0);

        // Warn as soon as the book looks already registered.
        let probe = BookInput {
            title: self.title.clone(),
            author: self.author.clone(),
            isbn: self.isbn.clone(),
            ..Default::default()
        };
        let lib = store.lib();
        let same = lib.same_books(&probe);
        // The copies of this book already registered: (title, author, copies, out).
        let existing = same.first().and_then(|&id| lib.book(id)).map(|b| {
            let out = same
                .iter()
                .filter(|&&id| lib.open_loan_for_book(id).is_some())
                .count();
            (b.title.clone(), b.author.clone(), same.len(), out)
        });

        match existing {
            None => {
                if ui
                    .add(theme::main_button("Enregistrer le livre", theme::GREEN))
                    .clicked()
                {
                    self.book_message = Some(self.save_book(store, None));
                }
            }
            Some((title, author, copies, out)) => {
                // A message about an earlier action would be confusing next to the warning.
                self.book_message = None;
                let mut save_anyway = false;
                egui::Frame::new()
                    .fill(theme::ON_LOAN.0)
                    .corner_radius(12)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("⚠ Ce livre (même ISBN) est déjà enregistré :")
                                .family(theme::bold())
                                .color(theme::ON_LOAN.1),
                        );
                        let mut text = title;
                        if !author.is_empty() {
                            text.push_str(&format!(" — {author}"));
                        }
                        ui.add(egui::Label::new(text).truncate());
                        ui.label(format!(
                            "{copies} exemplaire(s) enregistré(s), dont {out} prêté(s).",
                        ));
                        ui.add_space(4.0);
                        ui.label("Pour ajouter un exemplaire de plus du même livre :");
                        save_anyway = ui
                            .add(theme::main_button(
                                "Enregistrer quand même (nouvel exemplaire)",
                                theme::ON_LOAN.1,
                            ))
                            .clicked();
                    });
                if save_anyway {
                    let first = same.first().copied();
                    self.book_message = Some(self.save_book(store, first));
                }
            }
        }
        if let Some(m) = &self.book_message {
            m.show(ui);
        }
    }

    /// The books registered since the app was started, so that the teacher
    /// can see what was added.
    fn recent_books_card(&self, ui: &mut egui::Ui) {
        ui.heading(format!(
            "🆕 Livres enregistrés ({})",
            self.recent_books.len()
        ));
        ui.add_space(4.0);
        if self.recent_books.is_empty() {
            ui.label("Les livres que vous enregistrez apparaissent ici.");
            return;
        }
        for (title, isbn, copies) in &self.recent_books {
            ui.horizontal(|ui| {
                ui.monospace(isbn);
                if *copies > 1 {
                    theme::badge(ui, &format!("{copies} exemplaires"), theme::INFO);
                }
                ui.add(egui::Label::new(title).truncate());
            });
        }
    }

    /// The author field, with the authors already registered that match what
    /// is typed: click one, or press Tab to complete with the first.
    fn author_field(&mut self, ui: &mut egui::Ui, lib: &Library) {
        let id = egui::Id::new("book_author");
        let (suggestions, completion) = author_suggestions(&lib.authors(), &self.author);
        ui.vertical(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.author)
                    .id(id)
                    .desired_width(320.0)
                    // Tab only completes while there is something to complete;
                    // otherwise it moves to the next field as usual.
                    .lock_focus(completion.is_some()),
            );
            if self.focus == Some(BookField::Author) {
                field.request_focus();
                self.focus = None;
            }
            // Clicking a suggestion takes the focus away from the field: the
            // list stays until another field gets the focus.
            let focused = ui.memory(|m| m.focused());
            if focused == Some(id) {
                self.author_active = true;
            } else if focused.is_some() {
                self.author_active = false;
            }
            if let Some(full) = completion
                .clone()
                .filter(|_| field.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Tab)))
            {
                self.set_author(ui.ctx(), id, full);
                return;
            }
            if !self.author_active {
                return;
            }
            let mut chosen = None;
            for author in &suggestions {
                if ui.small_button(author).clicked() {
                    chosen = Some(author.clone());
                }
            }
            if completion.is_some() {
                ui.weak("Tab : compléter avec le premier");
            }
            if let Some(author) = chosen {
                self.set_author(ui.ctx(), id, author);
                self.author_active = false;
            }
        });
    }

    /// Fills the author field and puts the cursor at its end.
    fn set_author(&mut self, ctx: &egui::Context, id: egui::Id, author: String) {
        let end = author.chars().count();
        self.author = author;
        if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
            let cursor = egui::text::CCursor::new(end);
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
            egui::TextEdit::store_state(ctx, id, state);
        }
    }

    fn clear_book_form(&mut self) {
        for field in [
            &mut self.title,
            &mut self.author,
            &mut self.collection,
            &mut self.category,
            &mut self.year,
            &mut self.isbn,
        ] {
            field.clear();
        }
        self.copies = 1;
        self.isbn_message = None;
        self.filled_title.clear();
        self.filled_collection.clear();
        self.focus = Some(BookField::First);
    }

    /// The ISBN field: Enter (what the scanner sends) or the button fills in
    /// the title and publisher from the ISBN database.
    fn isbn_field(&mut self, ui: &mut egui::Ui, isbn: &IsbnService) {
        ui.vertical(|ui| {
            // As wide as the other fields: a long message wraps, instead of
            // widening the whole form.
            ui.set_max_width(400.0);
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.isbn)
                        .hint_text("scannez le code-barres")
                        .desired_width(220.0),
                );
                if self.focus == Some(BookField::Isbn)
                    || (self.focus == Some(BookField::First) && isbn.is_ready())
                {
                    field.request_focus();
                    self.focus = None;
                }
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let button = ui.add_enabled(
                    !self.isbn.trim().is_empty(),
                    egui::Button::new("🔍 Remplir"),
                );
                if entered || button.clicked() {
                    self.lookup_isbn(isbn);
                }
            });
            if let Some(m) = &self.isbn_message {
                m.show(ui);
            }
        });
    }

    /// Empties the title and collection if they still hold what the last
    /// lookup filled in (not what was typed).
    fn forget_filled(&mut self) {
        if self.title == self.filled_title {
            self.title.clear();
        }
        if self.collection == self.filled_collection {
            self.collection.clear();
        }
        self.filled_title.clear();
        self.filled_collection.clear();
    }

    fn lookup_isbn(&mut self, isbn: &IsbnService) {
        self.isbn_message = Some(match isbn.lookup(&self.isbn) {
            Lookup::Found { isbn: code, info } => {
                self.isbn = code;
                let mut text = if info.title.is_empty() {
                    "Trouvé, mais sans titre : saisissez-le.".to_string()
                } else {
                    format!("Trouvé : « {} ».", info.title)
                };
                let title_filled = fill(&mut self.title, &mut self.filled_title, &info.title);
                if !title_filled && !info.title.is_empty() {
                    text.push_str(" (Le titre saisi a été gardé.)");
                }
                if fill(
                    &mut self.collection,
                    &mut self.filled_collection,
                    &info.publisher,
                ) {
                    text.push_str(&format!(
                        " Éditeur « {} » placé dans Collection.",
                        info.publisher
                    ));
                }
                if info.title_garbled {
                    text.push_str(
                        " ⚠ Des caractères du titre sont illisibles dans la base : corrigez-le.",
                    );
                }
                text.push_str(" Complétez l'auteur et la catégorie.");
                self.focus = Some(BookField::Author);
                Message::Ok(text)
            }
            Lookup::NotFound { isbn: code } => {
                self.isbn = code.clone();
                // What an earlier lookup put in the form is another book's.
                self.forget_filled();
                self.focus = Some(BookField::Title);
                Message::Err(format!(
                    "ISBN {code} introuvable dans la base : remplissez les champs vous-même."
                ))
            }
            Lookup::Unusable(message) => Message::Err(message),
        });
    }

    /// `copy_of`: the existing book this one is a new copy of; its details
    /// fill the fields left empty and its spelling of title and author wins.
    fn save_book(&mut self, store: &mut Store, copy_of: Option<u32>) -> Message {
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
        let mut input = BookInput {
            title: self.title.clone(),
            author: self.author.clone(),
            collection: self.collection.clone(),
            category: self.category.clone(),
            year,
            isbn: self.isbn.clone(),
        };
        if let Some(book) = copy_of.and_then(|id| store.lib().book(id)) {
            input = input.filled_from(book);
        }
        match store.apply(|lib| lib.add_copies(input, self.copies)) {
            Err(e) => Message::Err(e.to_string()),
            Ok(ids) => {
                let saved = store
                    .lib()
                    .book(ids[0])
                    .map(|b| (b.title.clone(), b.isbn.clone()));
                self.clear_book_form();
                let Some((title, isbn)) = saved else {
                    return Message::Ok("Livre enregistré.".into());
                };
                self.recent_books
                    .insert(0, (title.clone(), isbn, ids.len()));
                self.recent_books.truncate(30);
                match ids.len() {
                    1 => Message::Ok(format!("Livre enregistré : « {title} ».")),
                    n => Message::Ok(format!("{n} exemplaires enregistrés : « {title} ».")),
                }
            }
        }
    }

    // --------------------------------------------------------- students

    fn student_form(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        ui.heading("🎒 Nouvel élève");
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
                        .hint_text("jj/mm/aaaa ou jjmmaaaa")
                        .desired_width(240.0),
                );
                ui.end_row();
                ui.label("Classe");
                let classes = store.lib().classes();
                text_with_choices(ui, "class_pick", &mut self.class, "CP, CE1…", classes);
                ui.end_row();
            });
        ui.weak("* obligatoire. La classe peut être ajoutée ou changée plus tard, dans l'onglet Élèves.");
        ui.add_space(6.0);
        if ui
            .add(theme::main_button("Enregistrer l'élève", theme::GREEN))
            .clicked()
        {
            self.student_message = Some(self.save_student(store));
        }
        if let Some(m) = &self.student_message {
            m.show(ui);
        }
    }

    fn save_student(&mut self, store: &mut Store) -> Message {
        let Some(birth) = parse_date(&self.birth_date) else {
            return Message::Err(
                "Date de naissance invalide : par exemple 14/05/2017 ou 14052017.".into(),
            );
        };
        let today = biblo::today();
        if birth > today {
            return Message::Err("La date de naissance est dans le futur.".into());
        }
        let (last, first) = (self.last_name.clone(), self.first_name.clone());
        let class = self.class.clone();
        let added = store.apply(|lib| {
            let id = lib.add_student(&last, &first, birth, today)?;
            lib.set_student_class(id, &class)?;
            Ok(id)
        });
        match added {
            Err(e) => Message::Err(e.to_string()),
            Ok(id) => {
                self.student_cards.push([id]);
                self.last_name.clear();
                self.first_name.clear();
                self.birth_date.clear();
                // The class stays: pupils are usually entered class by class.
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
        let all: Vec<_> = lib.students().iter().collect();
        ui.heading(format!("👫 Élèves inscrits ({})", all.len()));
        ui.add_space(4.0);
        let query = Query::new(&self.student_query);
        let mut rows: Vec<(String, u32, [String; 5])> = all
            .iter()
            .map(|s| {
                let key = fold(&format!("{}\n{}", s.last_name, s.first_name));
                let cells = [
                    student_code(s.id),
                    s.last_name.clone(),
                    s.first_name.clone(),
                    format_date(s.birth_date),
                    s.class.clone(),
                ];
                (key, s.id, cells)
            })
            .filter(|(_, _, cells)| query.matches(&cells.each_ref().map(String::as_str)))
            .collect();
        rows.sort();

        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").size(20.0));
            ui.add(
                egui::TextEdit::singleline(&mut self.student_query)
                    .hint_text("nom, prénom, classe, code…")
                    .desired_width(260.0),
            );
        });
        let shown = format!("Ajouter ces {} élève(s) aux cartes", rows.len());
        if ui
            .add_enabled(!rows.is_empty(), egui::Button::new(shown))
            .clicked()
        {
            let added = self.student_cards.push(rows.iter().map(|(_, id, _)| *id));
            self.student_cards.set_message(Message::Ok(format!(
                "{added} carte(s) ajoutée(s) à la liste."
            )));
        }
        ui.add_space(4.0);

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
            .column(Column::auto())
            .column(Column::remainder().clip(true))
            .header(26.0, |mut header| {
                for title in ["Code", "Nom", "Prénom", "Né(e) le", "Classe"] {
                    header.col(|ui| {
                        ui.label(RichText::new(title).family(theme::bold()));
                    });
                }
            })
            .body(|body| {
                body.rows(28.0, rows.len(), |mut row| {
                    let (_, _, cells) = &rows[row.index()];
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

/// Puts `value` in `field` unless something was typed there by hand: an empty
/// field, or what the previous lookup put there, is replaced. Returns
/// whether it was filled.
fn fill(field: &mut String, previous: &mut String, value: &str) -> bool {
    if value.is_empty() || !(field.trim().is_empty() || field == previous) {
        return false;
    }
    *field = value.to_string();
    *previous = value.to_string();
    true
}

/// At most this many suggestions under the author field.
const MAX_AUTHOR_SUGGESTIONS: usize = 5;

/// The registered `authors` that contain what is typed, those that start with
/// it first, and the one Tab completes to (the first, if it starts with the
/// text). What is already typed in full is not suggested again.
fn author_suggestions(authors: &[String], typed: &str) -> (Vec<String>, Option<String>) {
    let typed = fold(typed.trim());
    if typed.is_empty() {
        return (Vec::new(), None);
    }
    let (mut starting, mut containing) = (Vec::new(), Vec::new());
    for author in authors {
        let key = fold(author);
        if key == typed {
            continue;
        }
        if key.starts_with(&typed) {
            starting.push(author.clone());
        } else if key.contains(&typed) {
            containing.push(author.clone());
        }
    }
    let completion = starting.first().cloned();
    starting.append(&mut containing);
    starting.truncate(MAX_AUTHOR_SUGGESTIONS);
    (starting, completion)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authors() -> Vec<String> {
        [
            "Anne Fine",
            "Éric Carle",
            "Roald Dahl",
            "René Goscinny, Jean-Jacques Sempé",
        ]
        .map(String::from)
        .to_vec()
    }

    #[test]
    fn suggests_authors_starting_with_or_containing_the_text() {
        let (list, completion) = author_suggestions(&authors(), "re");
        // "René…" starts with it; "Éric Carle" and "Roald Dahl" do not contain it
        assert_eq!(list, vec!["René Goscinny, Jean-Jacques Sempé"]);
        assert_eq!(
            completion.as_deref(),
            Some("René Goscinny, Jean-Jacques Sempé")
        );
        // accents and case are ignored, and the start of a later word matches
        let (list, completion) = author_suggestions(&authors(), " DAH");
        assert_eq!(list, vec!["Roald Dahl"]);
        assert_eq!(completion, None);
        let (list, completion) = author_suggestions(&authors(), "eric");
        assert_eq!(list, vec!["Éric Carle"]);
        assert_eq!(completion.as_deref(), Some("Éric Carle"));
    }

    #[test]
    fn nothing_to_suggest_when_empty_or_already_complete() {
        assert_eq!(author_suggestions(&authors(), "  "), (Vec::new(), None));
        assert_eq!(
            author_suggestions(&authors(), "roald dahl"),
            (Vec::new(), None)
        );
        assert_eq!(author_suggestions(&authors(), "zzz"), (Vec::new(), None));
    }
}
