//! Home screen: lend and take back books. The pupil (left) and the book
//! (right) face each other, so the screen reads « this pupil borrows this
//! book ». A scanned code goes to its side by itself, whichever field has the
//! cursor; without the scanner, the pupil field takes a name and the book
//! field a title.
//!
//! Nothing is recorded until it is validated (button or Enter): a free book
//! is lent to the chosen pupil; a book that is out cannot be lent to anyone,
//! only returned.

use biblo::codes::{book_code, student_code};
use biblo::dates::{format_date, parse_date};
use biblo::desk::{find_books, find_students, propose, read_entry, Entry, Field, Proposal};
use biblo::{Library, Store};
use chrono::NaiveDate;
use eframe::egui::{self, Align, Color32, FontId, Key, Layout, Margin, RichText};

use super::theme::{self, Badge};

/// Pupils or books suggested under a field while a name is typed.
const MAX_SUGGESTIONS: usize = 8;

/// Message about the last loan or return, or about what went wrong.
struct Feedback {
    colors: Badge,
    title: String,
    detail: String,
    /// Shows the undo button (only right after a loan or a return).
    undoable: bool,
}

impl Feedback {
    fn new(colors: Badge, title: impl Into<String>, detail: impl Into<String>) -> Feedback {
        Feedback {
            colors,
            title: title.into(),
            detail: detail.into(),
            undoable: false,
        }
    }

    fn undoable(mut self) -> Feedback {
        self.undoable = true;
        self
    }
}

/// The last loan or return (loan id), so a mistake can be undone.
#[derive(Clone, Copy)]
enum LastAction {
    Lent(u32),
    Returned(u32),
}

pub struct LoansView {
    student_input: String,
    book_input: String,
    /// The pupil who borrows; cleared after each loan, so that the next book
    /// always needs a card again and never goes to the previous pupil.
    student: Option<u32>,
    book: Option<u32>,
    /// Field that gets the keyboard (and so the scanner) on the next frame.
    focus: Option<Field>,
    /// What was wrong with the last entry in each field.
    student_error: Option<String>,
    book_error: Option<String>,
    feedback: Option<Feedback>,
    last_action: Option<LastAction>,
    /// Date used for loans and returns: today, unless edited.
    date_text: String,
    date_edited: bool,
}

impl Default for LoansView {
    fn default() -> Self {
        LoansView {
            student_input: String::new(),
            book_input: String::new(),
            student: None,
            book: None,
            focus: Some(Field::Student),
            student_error: None,
            book_error: None,
            feedback: None,
            last_action: None,
            date_text: String::new(),
            date_edited: false,
        }
    }
}

fn title(lib: &Library, book: u32) -> String {
    lib.book(book)
        .map(|b| format!("« {} »", b.title))
        .unwrap_or_default()
}

fn name(lib: &Library, student: u32) -> String {
    lib.student(student)
        .map(|s| s.full_name())
        .unwrap_or_default()
}

fn days(n: i64) -> String {
    if n > 1 {
        format!("{n} jours")
    } else {
        format!("{n} jour")
    }
}

/// The book's note on a line of its own, so its condition can be checked
/// when it goes out and when it comes back.
fn note_line(lib: &Library, book: u32) -> String {
    lib.book(book)
        .filter(|b| !b.note.is_empty())
        .map(|b| format!("\n📝 Note : {}", b.note.replace('\n', " · ")))
        .unwrap_or_default()
}

/// Borrower, loan date and days out of a loan.
fn loan_info(lib: &Library, loan: u32, today: NaiveDate) -> (String, String, String) {
    lib.loans()
        .iter()
        .find(|l| l.id == loan)
        .map(|l| {
            (
                name(lib, l.student_id),
                format_date(l.borrowed_on),
                days(l.days_out(today)),
            )
        })
        .unwrap_or_default()
}

/// A card whose thick border shows that something is chosen on that side.
fn side_card(ui: &mut egui::Ui, border: Option<Color32>, add: impl FnOnce(&mut egui::Ui)) {
    match border {
        Some(color) => theme::outlined_card(ui, color, add),
        None => theme::card(ui, add),
    }
}

/// Big bold text for the chosen pupil or book.
fn big(text: impl Into<String>) -> RichText {
    RichText::new(text).family(theme::bold()).size(26.0)
}

impl LoansView {
    /// `late_days`: books out this many days or more are shown as late.
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store, late_days: i64) {
        let today = biblo::today();
        if !self.date_edited {
            // Follows the clock, even if the app stays open past midnight.
            self.date_text = format_date(today);
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.clear_all();
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            self.top_bar(ui);
            ui.add_space(10.0);
            let proposal = propose(store.lib(), self.student, self.book);
            let spacing = ui.spacing().item_spacing;
            let sides = ui
                .scope(|ui| {
                    // Room between the two cards for the arrow…
                    ui.spacing_mut().item_spacing.x = 64.0;
                    ui.columns(2, |cols| {
                        // … but the usual spacing inside them.
                        for col in cols.iter_mut() {
                            col.spacing_mut().item_spacing = spacing;
                        }
                        let plain = Layout::top_down(Align::Min);
                        cols[0].with_layout(plain, |ui| {
                            self.student_side(ui, store, today, late_days)
                        });
                        cols[1].with_layout(plain, |ui| self.book_side(ui, store, today));
                    });
                })
                .response
                .rect;
            arrow(ui, sides, &proposal);
            ui.add_space(14.0);
            self.action_area(ui, store, today);
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("🔁 Prêts et retours");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.date_edited && ui.button("Aujourd'hui").clicked() {
                    self.date_edited = false;
                }
                let date =
                    ui.add(egui::TextEdit::singleline(&mut self.date_text).desired_width(120.0));
                if date.changed() {
                    self.date_edited = true;
                }
                ui.label("Date :");
            });
        });
        if self.date_edited {
            ui.colored_label(
                theme::ON_LOAN.1,
                format!(
                    "⚠ Les prêts et retours sont enregistrés à la date du {}, pas aujourd'hui.",
                    self.date_text.trim()
                ),
            );
        }
    }

    /// The field where the scanner should type next: the pupil until one is
    /// chosen, then the book (where Enter also validates).
    fn expected_field(&self) -> Field {
        if self.student.is_none() {
            Field::Student
        } else {
            Field::Book
        }
    }

    /// The text field of one side, with Enter and focus handling.
    fn field(&mut self, ui: &mut egui::Ui, store: &mut Store, field: Field, today: NaiveDate) {
        let (text, hint) = match field {
            Field::Student => (&mut self.student_input, "Carte de l'élève, ou nom…"),
            Field::Book => (&mut self.book_input, "Code du livre, ou titre…"),
        };
        let input = ui.add(
            egui::TextEdit::singleline(text)
                .font(FontId::proportional(24.0))
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        );
        // Check Enter first: giving the focus back would hide the submission.
        if input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            self.on_enter(store, field, today);
        }
        // The scanner types into whatever has the focus: keep it on the
        // expected field, unless the user clicked into another one.
        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
        if self.focus == Some(field) || (nothing_focused && self.expected_field() == field) {
            input.request_focus();
            self.focus = None;
        }
        let error = match field {
            Field::Student => &self.student_error,
            Field::Book => &self.book_error,
        };
        if let Some(error) = error {
            ui.colored_label(theme::RED, format!("⚠ {error}"));
        }
    }

    fn student_side(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Store,
        today: NaiveDate,
        late_days: i64,
    ) {
        let border = self.student.map(|_| theme::LEND);
        side_card(ui, border, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🎒 L'élève");
                if self.student.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✖ Changer").clicked() {
                            self.student = None;
                            self.focus = Some(Field::Student);
                        }
                    });
                }
            });
            self.field(ui, store, Field::Student, today);
            ui.weak("Sans lecteur : tapez le nom (ou le numéro de la carte), puis Entrée.");
            self.suggestions(ui, store.lib(), Field::Student);
            ui.add_space(6.0);

            let lib = store.lib();
            let Some(s) = self.student.and_then(|id| lib.student(id)) else {
                ui.weak("Aucun élève choisi.");
                return;
            };
            ui.label(big(s.full_name()));
            let class = if s.class.is_empty() {
                String::new()
            } else {
                format!("{} · ", s.class)
            };
            ui.weak(format!(
                "{class}né(e) le {} · {}",
                format_date(s.birth_date),
                student_code(s.id)
            ));
            let loans = lib.open_loans_of_student(s.id);
            if loans.is_empty() {
                ui.label("Aucun livre emprunté pour l'instant.");
                return;
            }
            ui.label(format!(
                "Livres déjà empruntés ({}), depuis le :",
                loans.len()
            ));
            for loan in loans {
                let days_out = loan.days_out(today);
                ui.horizontal(|ui| {
                    ui.weak(format_date(loan.borrowed_on));
                    if days_out >= late_days {
                        theme::badge(ui, &format!("⏰ {}", days(days_out)), theme::ERROR);
                    }
                    // Last: a long title is cut to the card's width.
                    ui.add(egui::Label::new(title(lib, loan.book_id)).truncate());
                });
            }
        });
    }

    fn book_side(&mut self, ui: &mut egui::Ui, store: &mut Store, today: NaiveDate) {
        let out = self.book.and_then(|id| store.lib().open_loan_for_book(id));
        let border = match (self.book, out) {
            (None, _) => None,
            (Some(_), None) => Some(theme::GREEN),
            (Some(_), Some(_)) => Some(theme::RETURN),
        };
        side_card(ui, border, |ui| {
            ui.horizontal(|ui| {
                ui.heading("📖 Le livre");
                if self.book.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✖ Changer").clicked() {
                            self.book = None;
                            self.focus = Some(Field::Book);
                        }
                    });
                }
            });
            self.field(ui, store, Field::Book, today);
            ui.weak(
                "Sans lecteur : tapez le titre (ou le numéro sous le code-barres), puis Entrée.",
            );
            self.suggestions(ui, store.lib(), Field::Book);
            ui.add_space(6.0);

            let lib = store.lib();
            let Some(b) = self.book.and_then(|id| lib.book(id)) else {
                ui.weak("Aucun livre choisi.");
                return;
            };
            ui.add(egui::Label::new(big(format!("« {} »", b.title))).truncate());
            ui.horizontal(|ui| {
                let mut about = book_code(b.id);
                if !b.author.is_empty() {
                    about = format!("{} · {about}", b.author);
                }
                ui.weak(about);
                if !b.category.is_empty() {
                    theme::badge(ui, &b.category, theme::category_badge(&b.category));
                }
            });
            match lib.open_loan_for_book(b.id) {
                None => theme::badge(ui, "Disponible", theme::AVAILABLE),
                Some(loan) => theme::badge(
                    ui,
                    &format!(
                        "Emprunté par {} depuis le {} ({})",
                        name(lib, loan.student_id),
                        format_date(loan.borrowed_on),
                        days(loan.days_out(today))
                    ),
                    theme::ON_LOAN,
                ),
            }
            if !b.note.is_empty() {
                ui.label(
                    RichText::new(format!("📝 {}", b.note.replace('\n', " · ")))
                        .italics()
                        .color(theme::NOTE.1),
                );
            }
        });
    }

    /// While a name or a title is typed: the matching pupils or books.
    fn suggestions(&mut self, ui: &mut egui::Ui, lib: &Library, field: Field) {
        let text = match field {
            Field::Student => &self.student_input,
            Field::Book => &self.book_input,
        };
        let Entry::Search(_, text) = read_entry(lib, field, text) else {
            return;
        };
        if text.chars().count() < 2 {
            return;
        }
        let found = match field {
            Field::Student => find_students(lib, &text),
            Field::Book => find_books(lib, &text),
        };
        ui.add_space(4.0);
        if found.is_empty() {
            ui.label(match field {
                Field::Student => format!("Aucun élève ne correspond à « {text} »."),
                Field::Book => format!("Aucun livre ne correspond à « {text} »."),
            });
            return;
        }
        for &id in found.iter().take(MAX_SUGGESTIONS) {
            ui.horizontal(|ui| {
                if ui.button("Choisir").clicked() {
                    match field {
                        Field::Student => self.choose_student(id),
                        Field::Book => self.choose_book(id),
                    }
                }
                match field {
                    Field::Student => {
                        let Some(s) = lib.student(id) else { return };
                        ui.label(s.full_name());
                        let about = format!(
                            "{} {} {}",
                            s.class,
                            format_date(s.birth_date),
                            student_code(id)
                        );
                        ui.weak(about.trim_start());
                    }
                    Field::Book => {
                        ui.monospace(book_code(id));
                        match lib.open_loan_for_book(id) {
                            None => theme::badge(ui, "Disponible", theme::AVAILABLE),
                            Some(loan) => theme::badge(
                                ui,
                                &format!("Emprunté par {}", name(lib, loan.student_id)),
                                theme::ON_LOAN,
                            ),
                        }
                        ui.add(egui::Label::new(title(lib, id)).truncate());
                    }
                }
            });
        }
        if found.len() > MAX_SUGGESTIONS {
            ui.weak(format!(
                "… et {} autre(s) : précisez.",
                found.len() - MAX_SUGGESTIONS
            ));
        }
    }

    /// Under the two cards: the last result, then what can be done now.
    fn action_area(&mut self, ui: &mut egui::Ui, store: &mut Store, today: NaiveDate) {
        if let Some(colors) = self.feedback.as_ref().map(|f| f.colors) {
            self.feedback_banner(ui, store, colors);
            ui.add_space(10.0);
        }
        let lib = store.lib();
        let proposal = propose(lib, self.student, self.book);
        let (mut validate, mut cancel) = (false, false);
        match &proposal {
            Proposal::Lend { student, book } => {
                banner(ui, theme::LEND_SOFT, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let color = theme::LEND_SOFT.1;
                        ui.label(big(format!("🎒 {}", name(lib, *student))).color(color));
                        ui.label(big("➡ emprunte ➡").color(theme::LEND));
                        ui.label(big(format!("📖 {}", title(lib, *book))).color(color));
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        validate = ui
                            .add(theme::main_button(
                                "✔ Valider le prêt (Entrée)",
                                theme::LEND,
                            ))
                            .clicked();
                        cancel = ui.button("Annuler").clicked();
                    });
                });
            }
            Proposal::Return {
                book,
                loan,
                borrower,
            } => {
                let (who, since, out) = loan_info(lib, *loan, today);
                banner(ui, theme::RETURN_SOFT, |ui| {
                    let color = theme::RETURN_SOFT.1;
                    ui.label(
                        big(format!("⚠ {} est déjà emprunté", title(lib, *book))).color(color),
                    );
                    let mut text = format!("par {who}, depuis le {since} ({out}).");
                    match self.student {
                        Some(s) if s == *borrower => {
                            text.push_str(&format!(" {who} est l'élève choisi."));
                        }
                        Some(s) => text
                            .push_str(&format!(" Il ne peut pas être prêté à {}.", name(lib, s))),
                        None => text.push_str(" Il ne peut pas être prêté."),
                    }
                    ui.label(RichText::new(text).size(19.0).color(color));
                    ui.label(
                        RichText::new("S'il s'agit d'un retour, merci de valider :")
                            .size(19.0)
                            .color(color),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        validate = ui
                            .add(theme::main_button(
                                "📥 Valider le retour (Entrée)",
                                theme::RETURN,
                            ))
                            .clicked();
                        cancel = ui.button("Annuler").clicked();
                    });
                });
            }
            Proposal::Incomplete => {
                let text = match (self.student, self.book) {
                    (Some(s), _) => format!(
                        "Scannez maintenant le livre que {} emprunte.\n\
                         Pour un retour, scannez simplement le livre.",
                        name(lib, s)
                    ),
                    (None, Some(b)) => format!(
                        "{} est disponible : scannez la carte de l'élève qui l'emprunte.",
                        title(lib, b)
                    ),
                    (None, None) => "Pour un prêt : la carte de l'élève et le livre, dans \
                                     l'ordre que vous voulez.\n\
                                     Pour un retour : le livre suffit."
                        .to_string(),
                };
                banner(ui, (theme::CARD, theme::INK), |ui| {
                    ui.label(RichText::new(text).size(19.0));
                });
            }
        }
        if validate {
            self.validate(store, today);
        }
        if cancel {
            self.book = None;
            self.focus = Some(self.expected_field());
        }
    }

    fn feedback_banner(&mut self, ui: &mut egui::Ui, store: &mut Store, colors: Badge) {
        let mut undo = false;
        banner(ui, colors, |ui| {
            let Some(f) = &self.feedback else { return };
            ui.label(
                RichText::new(&f.title)
                    .family(theme::bold())
                    .size(26.0)
                    .color(colors.1),
            );
            if !f.detail.is_empty() {
                ui.label(RichText::new(&f.detail).size(19.0).color(colors.1));
            }
            if colors == theme::ERROR {
                ui.label(
                    RichText::new("Rien n'a été enregistré.")
                        .family(theme::bold())
                        .size(19.0)
                        .color(colors.1),
                );
            }
            let undo_text = match self.last_action {
                Some(LastAction::Lent(_)) => "↩ Annuler ce prêt",
                Some(LastAction::Returned(_)) => "↩ Annuler ce retour",
                None => "",
            };
            if f.undoable && !undo_text.is_empty() {
                ui.add_space(4.0);
                undo = ui.button(undo_text).clicked();
            }
        });
        if undo {
            self.undo(store);
            self.focus = Some(self.expected_field());
        }
    }

    // ------------------------------------------------------------ actions

    fn clear_all(&mut self) {
        self.student = None;
        self.book = None;
        self.student_input.clear();
        self.book_input.clear();
        self.student_error = None;
        self.book_error = None;
        self.feedback = None;
        self.focus = Some(Field::Student);
    }

    fn choose_student(&mut self, id: u32) {
        self.student = Some(id);
        self.student_input.clear();
        self.student_error = None;
        // A new loan starts: the previous result would only confuse.
        self.feedback = None;
        self.focus = Some(self.expected_field());
    }

    fn choose_book(&mut self, id: u32) {
        self.book = Some(id);
        self.book_input.clear();
        self.book_error = None;
        self.feedback = None;
        self.focus = Some(self.expected_field());
    }

    fn on_enter(&mut self, store: &mut Store, field: Field, today: NaiveDate) {
        let input = match field {
            Field::Student => std::mem::take(&mut self.student_input),
            Field::Book => std::mem::take(&mut self.book_input),
        };
        self.focus = Some(field);
        // Enter on an empty field validates what is proposed.
        if input.trim().is_empty() {
            self.validate(store, today);
            return;
        }
        let lib = store.lib();
        let error = match read_entry(lib, field, &input) {
            Entry::Nothing => None,
            Entry::Student(id) => {
                self.choose_student(id);
                None
            }
            Entry::Book(id) => {
                self.choose_book(id);
                None
            }
            Entry::Search(Field::Student, text) => match find_students(lib, &text).as_slice() {
                [] => Some(format!("Aucun élève ne correspond à « {text} ».")),
                [id] => {
                    self.choose_student(*id);
                    None
                }
                _ => {
                    // Keep the text: the list to choose from stays visible.
                    self.student_input = input;
                    None
                }
            },
            Entry::Search(Field::Book, text) => match find_books(lib, &text).as_slice() {
                [] => Some(format!("Aucun livre ne correspond à « {text} ».")),
                [id] => {
                    self.choose_book(*id);
                    None
                }
                _ => {
                    self.book_input = input;
                    None
                }
            },
            Entry::Refused(e) => Some(e.to_string()),
        };
        // A new entry replaces the previous error, if any.
        match field {
            Field::Student => self.student_error = error,
            Field::Book => self.book_error = error,
        }
    }

    /// The date typed in the date field, if valid.
    fn date(&mut self, today: NaiveDate) -> Option<NaiveDate> {
        match parse_date(&self.date_text) {
            Some(d) if d <= today => Some(d),
            Some(_) => {
                self.feedback = Some(Feedback::new(
                    theme::ERROR,
                    "⚠ La date est dans le futur",
                    "Corrigez la date ou cliquez sur « Aujourd'hui ».",
                ));
                None
            }
            None => {
                self.feedback = Some(Feedback::new(
                    theme::ERROR,
                    "⚠ Date invalide",
                    "Écrivez-la comme 18/04/2026 ou 18042026, ou cliquez sur « Aujourd'hui ».",
                ));
                None
            }
        }
    }

    /// Records what is proposed: the loan, or the return.
    fn validate(&mut self, store: &mut Store, today: NaiveDate) {
        match propose(store.lib(), self.student, self.book) {
            Proposal::Lend { student, book } => self.lend(store, book, student, today),
            Proposal::Return { book, .. } => self.return_book(store, book, today),
            Proposal::Incomplete => {}
        }
    }

    fn lend(&mut self, store: &mut Store, book: u32, student: u32, today: NaiveDate) {
        let Some(date) = self.date(today) else { return };
        match store.apply(|lib| lib.lend(book, student, date)) {
            Ok(loan) => {
                let lib = store.lib();
                self.student = None;
                self.book = None;
                self.focus = Some(Field::Student);
                self.last_action = Some(LastAction::Lent(loan));
                self.feedback = Some(
                    Feedback::new(
                        (theme::LEND, Color32::WHITE),
                        "📤 Prêt enregistré",
                        format!(
                            "{} emprunte {} ({}), le {}.{}",
                            name(lib, student),
                            title(lib, book),
                            book_code(book),
                            format_date(date),
                            note_line(lib, book)
                        ),
                    )
                    .undoable(),
                );
            }
            Err(e) => {
                self.feedback = Some(Feedback::new(theme::ERROR, "⚠ Impossible", e.to_string()));
            }
        }
    }

    /// The pupil stays chosen: the same pupil often returns a book, then
    /// borrows another one.
    fn return_book(&mut self, store: &mut Store, book: u32, today: NaiveDate) {
        let Some(date) = self.date(today) else { return };
        match store.apply(|lib| lib.return_book(book, date)) {
            Ok(loan) => {
                let lib = store.lib();
                let (who, since, _) = loan_info(lib, loan, today);
                self.book = None;
                self.focus = Some(self.expected_field());
                self.last_action = Some(LastAction::Returned(loan));
                self.feedback = Some(
                    Feedback::new(
                        (theme::RETURN, Color32::WHITE),
                        "📥 Retour enregistré",
                        format!(
                            "{} ({}) est rendu par {who} (emprunté le {since}). \
                             Il est de nouveau disponible.{}",
                            title(lib, book),
                            book_code(book),
                            note_line(lib, book)
                        ),
                    )
                    .undoable(),
                );
            }
            Err(e) => {
                self.feedback = Some(Feedback::new(theme::ERROR, "⚠ Impossible", e.to_string()));
            }
        }
    }

    /// Undoes the last loan (deleted) or return (the loan is open again).
    fn undo(&mut self, store: &mut Store) {
        let Some(action) = self.last_action.take() else {
            return;
        };
        let loan_id = match action {
            LastAction::Lent(id) | LastAction::Returned(id) => id,
        };
        let Some(loan) = store
            .lib()
            .loans()
            .iter()
            .find(|l| l.id == loan_id)
            .cloned()
        else {
            return;
        };
        let result = match action {
            LastAction::Lent(_) => store.apply(|lib| lib.cancel_loan(loan.id)),
            LastAction::Returned(_) => {
                store.apply(|lib| lib.edit_loan(loan.id, loan.borrowed_on, None))
            }
        };
        let lib = store.lib();
        let (book, who) = (title(lib, loan.book_id), name(lib, loan.student_id));
        self.feedback = Some(match (result, action) {
            (Err(e), _) => Feedback::new(theme::ERROR, "⚠ Impossible d'annuler", e.to_string()),
            (Ok(()), LastAction::Lent(_)) => Feedback::new(
                theme::INFO,
                "↩ Prêt annulé",
                format!("{book} n'est plus prêté à {who}."),
            ),
            (Ok(()), LastAction::Returned(_)) => Feedback::new(
                theme::INFO,
                "↩ Retour annulé",
                format!("{book} est de nouveau prêté à {who}."),
            ),
        });
    }
}

/// Full-width coloured box under the two cards.
fn banner(ui: &mut egui::Ui, (fill, _): Badge, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, theme::LINE))
        .corner_radius(16)
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// The arrow between the two cards: « the pupil ➡ borrows the book ». Blue
/// only when that loan can be validated; a book that is out is not borrowed
/// by the pupil on the left.
fn arrow(ui: &egui::Ui, sides: egui::Rect, proposal: &Proposal) {
    let fill = match proposal {
        Proposal::Lend { .. } => theme::LEND,
        Proposal::Return { .. } | Proposal::Incomplete => theme::LINE,
    };
    let center = egui::pos2(sides.center().x, sides.top() + 82.0);
    let painter = ui.painter();
    painter.circle_filled(center, 26.0, fill);
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        "➡",
        FontId::proportional(28.0),
        Color32::WHITE,
    );
}
