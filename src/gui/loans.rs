//! Home screen: lend and take back books with the barcode scanner, or by
//! typing codes and pupil names when the scanner does not work.

use biblo::codes::{book_code, student_code};
use biblo::dates::{format_date, parse_date};
use biblo::desk::{decide, find_students, Decision};
use biblo::{Library, Store};
use chrono::NaiveDate;
use eframe::egui::{self, FontId, Key, Margin, RichText};

use super::theme::{self, Badge};

/// Loans out for this many days or more are shown as late.
const LATE_DAYS: i64 = 21;
/// Pupils suggested under the scan field while a name is typed.
const MAX_SUGGESTIONS: usize = 8;

/// Big message shown after every scan.
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

/// The last loan or return (loan id), so a wrong scan can be undone.
#[derive(Clone, Copy)]
enum LastAction {
    Lent(u32),
    Returned(u32),
}

pub struct LoansView {
    input: String,
    focus_scan: bool,
    /// The pupil who is borrowing.
    student: Option<u32>,
    /// Book whose return waits for a confirmation.
    pending_return: Option<u32>,
    feedback: Option<Feedback>,
    last_action: Option<LastAction>,
    /// Date used for loans and returns: today, unless edited.
    date_text: String,
    date_edited: bool,
}

impl Default for LoansView {
    fn default() -> Self {
        LoansView {
            input: String::new(),
            focus_scan: true,
            student: None,
            pending_return: None,
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

impl LoansView {
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        let today = biblo::today();
        if !self.date_edited {
            // Follows the clock, even if the app stays open past midnight.
            self.date_text = format_date(today);
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.next_student();
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            theme::card(ui, |ui| self.scan_card(ui, store, today));
            if let Some(colors) = self.feedback.as_ref().map(|f| f.colors) {
                ui.add_space(12.0);
                self.feedback_banner(ui, store, colors, today);
            }
            ui.add_space(12.0);
            theme::card(ui, |ui| self.student_card(ui, store.lib(), today));
        });
    }

    fn scan_card(&mut self, ui: &mut egui::Ui, store: &mut Store, today: NaiveDate) {
        ui.heading("🔁 Prêts et retours");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let hint = if self.student.is_some() {
                "Scannez le livre…"
            } else {
                "Scannez une carte d'élève ou un livre…"
            };
            let scan = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .font(FontId::proportional(28.0))
                    .hint_text(hint)
                    .desired_width(560.0),
            );
            // Check Enter first: giving the focus back would hide the submission.
            if scan.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.on_enter(store, today);
                self.focus_scan = true;
            }
            // The scanner types into whatever has the focus: keep it here
            // unless the user clicked into another field.
            let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
            if std::mem::take(&mut self.focus_scan) || nothing_focused {
                scan.request_focus();
            }
            ui.add_space(16.0);
            ui.label("Date :");
            let date = ui.add(egui::TextEdit::singleline(&mut self.date_text).desired_width(120.0));
            if date.changed() {
                self.date_edited = true;
            }
            if self.date_edited && ui.button("Aujourd'hui").clicked() {
                self.date_edited = false;
                self.focus_scan = true;
            }
        });
        ui.weak("Sans lecteur : tapez le code (E12, L345 ou juste le numéro) ou le nom de l'élève, puis Entrée.");
        if self.date_edited {
            ui.colored_label(
                theme::ON_LOAN.1,
                format!(
                    "⚠ Les prêts et retours sont enregistrés à la date du {}, pas aujourd'hui.",
                    self.date_text.trim()
                ),
            );
        }

        // Typing a name: suggest the matching pupils.
        if let Decision::SearchName(text) = decide(store.lib(), self.student, &self.input) {
            if text.chars().count() >= 2 {
                self.suggestions(ui, store.lib(), &text);
            }
        }
    }

    fn suggestions(&mut self, ui: &mut egui::Ui, lib: &Library, text: &str) {
        let found = find_students(lib, text);
        ui.add_space(6.0);
        if found.is_empty() {
            ui.label(format!("Aucun élève ne correspond à « {text} »."));
            return;
        }
        for &id in found.iter().take(MAX_SUGGESTIONS) {
            let Some(s) = lib.student(id) else { continue };
            ui.horizontal(|ui| {
                if ui.button("Choisir").clicked() {
                    self.select_student(lib, id);
                }
                ui.label(format!("{} ({})", s.full_name(), format_date(s.birth_date)));
                ui.weak(student_code(id));
            });
        }
        if found.len() > MAX_SUGGESTIONS {
            ui.weak(format!(
                "… et {} autre(s) : précisez le nom.",
                found.len() - MAX_SUGGESTIONS
            ));
        }
    }

    fn feedback_banner(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Store,
        colors: Badge,
        today: NaiveDate,
    ) {
        let (mut confirm, mut cancel, mut undo) = (false, false, false);
        egui::Frame::new()
            .fill(colors.0)
            .corner_radius(16)
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(f) = &self.feedback {
                    ui.label(
                        RichText::new(&f.title)
                            .family(theme::bold())
                            .size(28.0)
                            .color(colors.1),
                    );
                    if !f.detail.is_empty() {
                        ui.label(RichText::new(&f.detail).size(19.0).color(colors.1));
                    }
                }
                if self.pending_return.is_some() {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        confirm = ui
                            .add(theme::main_button("✔ Enregistrer le retour", theme::GREEN))
                            .clicked();
                        cancel = ui.button("Annuler").clicked();
                        ui.weak("(Entrée = enregistrer le retour)");
                    });
                } else if self.last_action.is_some()
                    && self.feedback.as_ref().is_some_and(|f| f.undoable)
                {
                    ui.add_space(4.0);
                    undo = ui.button("↩ Annuler cette opération").clicked();
                }
            });
        if confirm {
            if let Some(book) = self.pending_return {
                self.confirm_return(store, book, today);
            }
        }
        if cancel {
            self.pending_return = None;
            self.feedback = None;
        }
        if undo {
            self.undo(store);
        }
        if confirm || cancel || undo {
            self.focus_scan = true;
        }
    }

    fn student_card(&mut self, ui: &mut egui::Ui, lib: &Library, today: NaiveDate) {
        let Some(s) = self.student.and_then(|id| lib.student(id)) else {
            ui.heading("🎒 Comment faire ?");
            ui.add_space(4.0);
            ui.label("Pour un prêt : 1. scannez la carte de l'élève, 2. scannez le livre.");
            ui.label("Pour un retour : scannez simplement le livre.");
            return;
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("🎒 {}", s.full_name()))
                    .family(theme::bold())
                    .size(28.0),
            );
            ui.weak(format!(
                "né(e) le {} · {}",
                format_date(s.birth_date),
                student_code(s.id)
            ));
            if ui.button("Élève suivant (Échap)").clicked() {
                self.next_student();
            }
        });
        let loans = lib.open_loans_of_student(s.id);
        ui.add_space(4.0);
        if loans.is_empty() {
            ui.label("Aucun livre emprunté pour l'instant.");
            return;
        }
        ui.label(format!("Livres empruntés ({}) :", loans.len()));
        for loan in loans {
            let days = loan.days_out(today);
            ui.horizontal(|ui| {
                ui.monospace(book_code(loan.book_id));
                ui.label(title(lib, loan.book_id));
                ui.weak(format!(
                    "depuis le {} ({days} j)",
                    format_date(loan.borrowed_on)
                ));
                if days >= LATE_DAYS {
                    theme::badge(ui, "⏰ en retard", theme::ERROR);
                }
            });
        }
    }

    // ------------------------------------------------------------ actions

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

    fn on_enter(&mut self, store: &mut Store, today: NaiveDate) {
        let input = std::mem::take(&mut self.input);
        // Enter on an empty field confirms a proposed return.
        if input.trim().is_empty() {
            if let Some(book) = self.pending_return {
                self.confirm_return(store, book, today);
            }
            return;
        }
        self.pending_return = None;
        let decision = decide(store.lib(), self.student, &input);
        let lib = store.lib();
        match decision {
            Decision::Nothing => {}
            Decision::SelectStudent(id) => self.select_student(lib, id),
            Decision::Lend { book, student } => self.lend(store, book, student, today),
            Decision::ProposeReturn { book, loan } => {
                let loan = lib.loans().iter().find(|l| l.id == loan);
                let (who, since, days) = loan
                    .map(|l| {
                        (
                            name(lib, l.student_id),
                            format_date(l.borrowed_on),
                            l.days_out(today),
                        )
                    })
                    .unwrap_or_default();
                self.pending_return = Some(book);
                self.feedback = Some(Feedback::new(
                    theme::ON_LOAN,
                    format!("📕 {} est prêté à {who}", title(lib, book)),
                    format!("depuis le {since} ({days} jours). Enregistrer son retour ?"),
                ));
            }
            Decision::NeedStudent { book } => {
                self.feedback = Some(Feedback::new(
                    theme::INFO,
                    format!("📗 {} est disponible", title(lib, book)),
                    "Pour le prêter, scannez d'abord la carte de l'élève.",
                ));
            }
            Decision::SearchName(text) => match find_students(lib, &text).as_slice() {
                [] => {
                    self.feedback = Some(Feedback::new(
                        theme::ERROR,
                        "⚠ Code ou nom inconnu",
                        format!("Aucun élève ne correspond à « {text} »."),
                    ));
                }
                [id] => self.select_student(lib, *id),
                found => {
                    // Keep the text so the suggestions stay visible.
                    self.feedback = Some(Feedback::new(
                        theme::INFO,
                        format!("{} élèves correspondent", found.len()),
                        "Cliquez sur « Choisir » à côté du bon élève.",
                    ));
                    self.input = input;
                }
            },
            Decision::Refused(e) => {
                self.feedback = Some(Feedback::new(theme::ERROR, "⚠ Impossible", e.to_string()));
            }
        }
    }

    fn select_student(&mut self, lib: &Library, id: u32) {
        self.student = Some(id);
        self.input.clear();
        self.pending_return = None;
        self.focus_scan = true;
        self.feedback = Some(Feedback::new(
            theme::INFO,
            format!("🎒 {}", name(lib, id)),
            "Scannez maintenant le livre.",
        ));
    }

    fn next_student(&mut self) {
        self.student = None;
        self.pending_return = None;
        self.input.clear();
        self.feedback = None;
        self.focus_scan = true;
    }

    fn lend(&mut self, store: &mut Store, book: u32, student: u32, today: NaiveDate) {
        let Some(date) = self.date(today) else { return };
        match store.apply(|lib| lib.lend(book, student, date)) {
            Ok(loan) => {
                let lib = store.lib();
                self.last_action = Some(LastAction::Lent(loan));
                self.feedback = Some(
                    Feedback::new(
                        theme::AVAILABLE,
                        format!("✔ Prêté : {}", title(lib, book)),
                        format!(
                            "à {} le {} ({})",
                            name(lib, student),
                            format_date(date),
                            book_code(book)
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

    fn confirm_return(&mut self, store: &mut Store, book: u32, today: NaiveDate) {
        let Some(date) = self.date(today) else { return };
        self.pending_return = None;
        match store.apply(|lib| lib.return_book(book, date)) {
            Ok(loan) => {
                let lib = store.lib();
                let who = lib
                    .loans()
                    .iter()
                    .find(|l| l.id == loan)
                    .map(|l| name(lib, l.student_id))
                    .unwrap_or_default();
                self.last_action = Some(LastAction::Returned(loan));
                self.feedback = Some(
                    Feedback::new(
                        theme::AVAILABLE,
                        format!("✔ Rendu : {}", title(lib, book)),
                        format!("Merci {who} ! Le livre est de nouveau disponible."),
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
