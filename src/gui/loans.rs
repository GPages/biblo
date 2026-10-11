//! Home screen: lend and take back books with the barcode scanner, or by
//! typing codes and pupil names when the scanner does not work.
//!
//! Lending and returning are two modes, chosen with two big buttons and
//! colour-coded all over the screen (blue: lend, orange: return). A scan
//! only ever does what the current mode says.

use biblo::codes::{book_code, student_code};
use biblo::dates::{format_date, parse_date};
use biblo::desk::{decide_lend, decide_return, find_students, LendDecision, ReturnDecision};
use biblo::{Library, Store};
use chrono::NaiveDate;
use eframe::egui::{self, Color32, FontId, Key, Margin, RichText, Stroke};

use super::theme::{self, Badge};

/// Pupils suggested under the scan field while a name is typed.
const MAX_SUGGESTIONS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Lend,
    Return,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Mode::Lend => "📤 Prêter un livre",
            Mode::Return => "📥 Rendre un livre",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Mode::Lend => theme::LEND,
            Mode::Return => theme::RETURN,
        }
    }

    fn soft(self) -> Badge {
        match self {
            Mode::Lend => theme::LEND_SOFT,
            Mode::Return => theme::RETURN_SOFT,
        }
    }

    /// Banner colours once something has been recorded.
    fn done(self) -> Badge {
        (self.color(), Color32::WHITE)
    }
}

/// Big message shown after every scan.
struct Feedback {
    colors: Badge,
    title: String,
    detail: String,
    /// Shows the undo button (only right after a loan or a return).
    undoable: bool,
    /// Offers to switch to this mode, when the scan was meant for it.
    switch_to: Option<Mode>,
}

impl Feedback {
    fn new(colors: Badge, title: impl Into<String>, detail: impl Into<String>) -> Feedback {
        Feedback {
            colors,
            title: title.into(),
            detail: detail.into(),
            undoable: false,
            switch_to: None,
        }
    }

    fn undoable(mut self) -> Feedback {
        self.undoable = true;
        self
    }

    fn switch_to(mut self, mode: Mode) -> Feedback {
        self.switch_to = Some(mode);
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
    mode: Mode,
    input: String,
    focus_scan: bool,
    /// Lend mode: the pupil who is borrowing; cleared after each loan.
    student: Option<u32>,
    feedback: Option<Feedback>,
    last_action: Option<LastAction>,
    /// Date used for loans and returns: today, unless edited.
    date_text: String,
    date_edited: bool,
}

impl Default for LoansView {
    fn default() -> Self {
        LoansView {
            mode: Mode::Lend,
            input: String::new(),
            focus_scan: true,
            student: None,
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

impl LoansView {
    /// `late_days`: books out this many days or more are shown as late.
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store, late_days: i64) {
        let today = biblo::today();
        if !self.date_edited {
            // Follows the clock, even if the app stays open past midnight.
            self.date_text = format_date(today);
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.reset();
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            self.mode_buttons(ui);
            ui.add_space(12.0);
            theme::outlined_card(ui, self.mode.color(), |ui| self.scan_card(ui, store, today));
            if let Some(colors) = self.feedback.as_ref().map(|f| f.colors) {
                ui.add_space(12.0);
                self.feedback_banner(ui, store, colors);
            }
            ui.add_space(12.0);
            theme::card(ui, |ui| match self.mode {
                Mode::Lend => self.student_card(ui, store.lib(), today, late_days),
                Mode::Return => return_help(ui),
            });
        });
    }

    /// The two big buttons that choose between lending and returning.
    fn mode_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for mode in [Mode::Lend, Mode::Return] {
                let selected = self.mode == mode;
                let (fill, text_color) = if selected {
                    (mode.color(), Color32::WHITE)
                } else {
                    (theme::CARD, mode.color())
                };
                let button = egui::Button::new(
                    RichText::new(mode.title())
                        .family(theme::bold())
                        .size(24.0)
                        .color(text_color),
                )
                .fill(fill)
                .stroke(Stroke::new(2.0, mode.color()))
                .corner_radius(14)
                .min_size(egui::vec2(320.0, 58.0));
                if ui.add(button).clicked() && !selected {
                    self.set_mode(mode);
                }
            }
        });
    }

    fn scan_card(&mut self, ui: &mut egui::Ui, store: &mut Store, today: NaiveDate) {
        ui.label(
            RichText::new(self.mode.title())
                .family(theme::bold())
                .size(28.0)
                .color(self.mode.color()),
        );
        let student = self.student.and_then(|id| store.lib().student(id));
        let (step, hint) = match (self.mode, student) {
            (Mode::Lend, None) => (
                "Étape 1 sur 2 : scannez la carte de l'élève qui emprunte.".to_string(),
                "Carte de l'élève…",
            ),
            (Mode::Lend, Some(s)) => (
                format!(
                    "Étape 2 sur 2 : scannez le livre que {} emprunte.",
                    s.full_name()
                ),
                "Livre à prêter…",
            ),
            (Mode::Return, _) => (
                "Scannez le livre rendu : son retour est enregistré tout de suite.".to_string(),
                "Livre rendu…",
            ),
        };
        ui.label(RichText::new(step).size(19.0));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
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
        ui.weak(match self.mode {
            Mode::Lend => "Sans lecteur : tapez le code (E12, L345 ou juste le numéro) ou le nom de l'élève, puis Entrée.",
            Mode::Return => "Sans lecteur : tapez le code du livre (L345 ou juste le numéro), puis Entrée.",
        });
        if self.date_edited {
            let what = match self.mode {
                Mode::Lend => "Les prêts",
                Mode::Return => "Les retours",
            };
            ui.colored_label(
                theme::ON_LOAN.1,
                format!(
                    "⚠ {what} sont enregistrés à la date du {}, pas aujourd'hui.",
                    self.date_text.trim()
                ),
            );
        }

        // Typing a name: suggest the matching pupils.
        if self.mode == Mode::Lend {
            if let LendDecision::SearchName(text) =
                decide_lend(store.lib(), self.student, &self.input)
            {
                if text.chars().count() >= 2 {
                    self.suggestions(ui, store.lib(), &text);
                }
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
                ui.weak(format!("{} {}", s.class, student_code(id)).trim_start());
            });
        }
        if found.len() > MAX_SUGGESTIONS {
            ui.weak(format!(
                "… et {} autre(s) : précisez le nom.",
                found.len() - MAX_SUGGESTIONS
            ));
        }
    }

    fn feedback_banner(&mut self, ui: &mut egui::Ui, store: &mut Store, colors: Badge) {
        let (mut undo, mut switch) = (false, None);
        egui::Frame::new()
            .fill(colors.0)
            .corner_radius(16)
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let Some(f) = &self.feedback else { return };
                ui.label(
                    RichText::new(&f.title)
                        .family(theme::bold())
                        .size(28.0)
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
                if let Some(mode) = f.switch_to {
                    ui.add_space(4.0);
                    let text = format!("Passer à « {} »", mode.title());
                    if ui.add(theme::main_button(&text, mode.color())).clicked() {
                        switch = Some(mode);
                    }
                }
            });
        if undo {
            self.undo(store);
            self.focus_scan = true;
        }
        if let Some(mode) = switch {
            self.set_mode(mode);
        }
    }

    fn student_card(&mut self, ui: &mut egui::Ui, lib: &Library, today: NaiveDate, late_days: i64) {
        let Some(s) = self.student.and_then(|id| lib.student(id)) else {
            ui.heading("🎒 Comment prêter un livre ?");
            ui.add_space(4.0);
            ui.label("1. Scannez la carte de l'élève (ou tapez son nom).");
            ui.label("2. Scannez le livre : le prêt est enregistré.");
            ui.label("Un livre qui revient ? Cliquez d'abord sur « 📥 Rendre un livre » en haut.");
            return;
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("🎒 {}", s.full_name()))
                    .family(theme::bold())
                    .size(28.0),
            );
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
            if ui.button("Changer d'élève (Échap)").clicked() {
                self.reset();
            }
        });
        let loans = lib.open_loans_of_student(s.id);
        ui.add_space(4.0);
        if loans.is_empty() {
            ui.label("Aucun livre emprunté pour l'instant.");
            return;
        }
        ui.label(format!("Livres déjà empruntés ({}) :", loans.len()));
        for loan in loans {
            let days_out = loan.days_out(today);
            ui.horizontal(|ui| {
                ui.monospace(book_code(loan.book_id));
                ui.label(title(lib, loan.book_id));
                ui.weak(format!(
                    "depuis le {} ({})",
                    format_date(loan.borrowed_on),
                    days(days_out)
                ));
                if days_out >= late_days {
                    theme::badge(ui, "⏰ en retard", theme::ERROR);
                }
            });
        }
    }

    // ------------------------------------------------------------ actions

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.last_action = None;
        self.reset();
    }

    /// Back to the start of the current mode: no pupil, no message.
    fn reset(&mut self) {
        self.student = None;
        self.input.clear();
        self.feedback = None;
        self.focus_scan = true;
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

    fn on_enter(&mut self, store: &mut Store, today: NaiveDate) {
        let input = std::mem::take(&mut self.input);
        match self.mode {
            Mode::Lend => self.on_lend_scan(store, input, today),
            Mode::Return => self.on_return_scan(store, &input, today),
        }
    }

    fn on_lend_scan(&mut self, store: &mut Store, input: String, today: NaiveDate) {
        let decision = decide_lend(store.lib(), self.student, &input);
        let lib = store.lib();
        match decision {
            LendDecision::Nothing => {}
            LendDecision::SelectStudent(id) => self.select_student(lib, id),
            LendDecision::Lend { book, student } => self.lend(store, book, student, today),
            LendDecision::AlreadyLent { book, loan } => {
                let (who, since, out) = loan_info(lib, loan, today);
                self.feedback = Some(
                    Feedback::new(
                        theme::ERROR,
                        format!("⚠ {} est déjà prêté", title(lib, book)),
                        format!(
                            "à {who}, depuis le {since} ({out}). S'il vient d'être rendu, \
                             enregistrez d'abord son retour avec « {} ».",
                            Mode::Return.title()
                        ),
                    )
                    .switch_to(Mode::Return),
                );
            }
            LendDecision::NeedStudent { book } => {
                self.feedback = Some(Feedback::new(
                    theme::ERROR,
                    "⚠ Scannez d'abord la carte de l'élève",
                    format!(
                        "Pour prêter {} : 1. la carte de l'élève, 2. le livre.",
                        title(lib, book)
                    ),
                ));
            }
            LendDecision::SearchName(text) => match find_students(lib, &text).as_slice() {
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
                        theme::LEND_SOFT,
                        format!("{} élèves correspondent", found.len()),
                        "Cliquez sur « Choisir » à côté du bon élève.",
                    ));
                    self.input = input;
                }
            },
            LendDecision::Refused(e) => {
                self.feedback = Some(Feedback::new(theme::ERROR, "⚠ Impossible", e.to_string()));
            }
        }
    }

    fn on_return_scan(&mut self, store: &mut Store, input: &str, today: NaiveDate) {
        let decision = decide_return(store.lib(), input);
        let lib = store.lib();
        match decision {
            ReturnDecision::Nothing => {}
            ReturnDecision::Return { book, .. } => self.return_book(store, book, today),
            ReturnDecision::NotLent { book } => {
                self.feedback = Some(
                    Feedback::new(
                        theme::ERROR,
                        format!("⚠ {} n'est pas prêté", title(lib, book)),
                        format!(
                            "Il est déjà disponible : il n'y a rien à rendre. \
                             Pour le prêter, passez à « {} ».",
                            Mode::Lend.title()
                        ),
                    )
                    .switch_to(Mode::Lend),
                );
            }
            ReturnDecision::StudentCard(_) => {
                self.feedback = Some(
                    Feedback::new(
                        theme::ERROR,
                        "⚠ C'est une carte d'élève",
                        format!(
                            "Pour un retour, scannez seulement le livre. \
                             Pour un prêt, passez à « {} ».",
                            Mode::Lend.title()
                        ),
                    )
                    .switch_to(Mode::Lend),
                );
            }
            ReturnDecision::NotACode(text) => {
                self.feedback = Some(Feedback::new(
                    theme::ERROR,
                    "⚠ Ce n'est pas un code de livre",
                    format!(
                        "« {text} » : scannez le livre, ou tapez le code écrit sous \
                         son code-barres (L345 ou juste le numéro)."
                    ),
                ));
            }
            ReturnDecision::Refused(e) => {
                self.feedback = Some(Feedback::new(theme::ERROR, "⚠ Impossible", e.to_string()));
            }
        }
    }

    fn select_student(&mut self, lib: &Library, id: u32) {
        let title = if self.student.is_some_and(|s| s != id) {
            format!("🎒 Élève changé : {}", name(lib, id))
        } else {
            format!("🎒 {}", name(lib, id))
        };
        self.student = Some(id);
        self.input.clear();
        self.focus_scan = true;
        self.feedback = Some(Feedback::new(
            theme::LEND_SOFT,
            title,
            "Scannez maintenant le livre qu'il ou elle emprunte.",
        ));
    }

    fn lend(&mut self, store: &mut Store, book: u32, student: u32, today: NaiveDate) {
        let Some(date) = self.date(today) else { return };
        match store.apply(|lib| lib.lend(book, student, date)) {
            Ok(loan) => {
                let lib = store.lib();
                // One loan per card scan: the next book needs a card scan again,
                // so a forgotten card can never put a book on the previous pupil.
                self.student = None;
                self.last_action = Some(LastAction::Lent(loan));
                self.feedback = Some(
                    Feedback::new(
                        Mode::Lend.done(),
                        "📤 Prêt enregistré",
                        format!(
                            "{} ({}) est prêté à {} le {}.\n\
                             Livre suivant : scannez à nouveau une carte d'élève.",
                            title(lib, book),
                            book_code(book),
                            name(lib, student),
                            format_date(date)
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

    fn return_book(&mut self, store: &mut Store, book: u32, today: NaiveDate) {
        let Some(date) = self.date(today) else { return };
        match store.apply(|lib| lib.return_book(book, date)) {
            Ok(loan) => {
                let lib = store.lib();
                let (who, since, _) = loan_info(lib, loan, today);
                self.last_action = Some(LastAction::Returned(loan));
                self.feedback = Some(
                    Feedback::new(
                        Mode::Return.done(),
                        "📥 Retour enregistré",
                        format!(
                            "{} ({}) est rendu par {who} (emprunté le {since}).\n\
                             Il est de nouveau disponible.",
                            title(lib, book),
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
                self.mode.soft(),
                "↩ Prêt annulé",
                format!("{book} n'est plus prêté à {who}."),
            ),
            (Ok(()), LastAction::Returned(_)) => Feedback::new(
                self.mode.soft(),
                "↩ Retour annulé",
                format!("{book} est de nouveau prêté à {who}."),
            ),
        });
    }
}

/// Lower card in return mode.
fn return_help(ui: &mut egui::Ui) {
    ui.heading("📥 Comment rendre un livre ?");
    ui.add_space(4.0);
    ui.label("Scannez chaque livre rendu : c'est tout, le retour est enregistré tout de suite.");
    ui.label("Une erreur ? Le bouton « Annuler ce retour » la répare.");
    ui.label("Un livre à prêter ? Cliquez d'abord sur « 📤 Prêter un livre » en haut.");
}
