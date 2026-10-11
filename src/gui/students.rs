//! Pupils tab: every pupil, filtered live (by class, by books borrowed or
//! kept too long), with each pupil's details editable, and a cleanup that
//! deletes former pupils in one go.

use std::collections::HashSet;

use biblo::codes::student_code;
use biblo::dates::{format_date, parse_date};
use biblo::search::{fold, Query};
use biblo::{Library, Store, Student};
use chrono::{Months, NaiveDate};
use eframe::egui::{self, Align, Layout, Margin, RichText};
use egui_extras::{Column, TableBuilder};

use super::{text_with_choices, theme, Message};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    List,
    Cleanup,
}

/// Which pupils the list shows, by their loans.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoanFilter {
    All,
    WithBook,
    /// A book out for `late_days` days or more.
    Late,
}

/// Which pupils the list shows, by class.
#[derive(Clone, PartialEq, Eq)]
enum ClassFilter {
    All,
    Named(String),
    Without,
}

impl ClassFilter {
    fn matches(&self, class: &str) -> bool {
        match self {
            ClassFilter::All => true,
            ClassFilter::Named(name) => fold(name) == fold(class),
            ClassFilter::Without => class.is_empty(),
        }
    }

    fn label(&self) -> &str {
        match self {
            ClassFilter::All => "Toutes les classes",
            ClassFilter::Named(name) => name,
            ClassFilter::Without => "Sans classe",
        }
    }
}

/// A pupil's details being edited.
struct EditForm {
    id: u32,
    last_name: String,
    first_name: String,
    birth_date: String,
    class: String,
    error: Option<String>,
}

impl EditForm {
    fn new(s: &Student) -> EditForm {
        EditForm {
            id: s.id,
            last_name: s.last_name.clone(),
            first_name: s.first_name.clone(),
            birth_date: format_date(s.birth_date),
            class: s.class.clone(),
            error: None,
        }
    }
}

pub struct StudentsView {
    mode: Mode,
    query: String,
    loan_filter: LoanFilter,
    class_filter: ClassFilter,
    /// Pupil whose deletion waits for a confirmation.
    confirm_delete: Option<u32>,
    edit: Option<EditForm>,
    list_message: Option<Message>,
    /// Cleanup: pupils registered this many years ago or more.
    years: u32,
    /// Cleanup: candidates the user unticked.
    unticked: HashSet<u32>,
    confirming: bool,
    cleanup_message: Option<Message>,
}

impl Default for StudentsView {
    fn default() -> Self {
        StudentsView {
            mode: Mode::List,
            query: String::new(),
            loan_filter: LoanFilter::All,
            class_filter: ClassFilter::All,
            confirm_delete: None,
            edit: None,
            list_message: None,
            years: 4,
            unticked: HashSet::new(),
            confirming: false,
            cleanup_message: None,
        }
    }
}

/// One line of the list.
struct Row<'a> {
    student: &'a Student,
    code: String,
    birth: String,
    registered: String,
    /// (title, days out), oldest first.
    loans: Vec<(String, i64)>,
}

impl Row<'_> {
    fn oldest_days(&self) -> i64 {
        self.loans.first().map_or(-1, |(_, days)| *days)
    }
}

fn rows<'a>(lib: &'a Library, today: NaiveDate) -> Vec<Row<'a>> {
    lib.students()
        .iter()
        .map(|s| {
            let mut loans = lib.open_loans_of_student(s.id);
            loans.sort_by_key(|l| (l.borrowed_on, l.id));
            Row {
                student: s,
                code: student_code(s.id),
                birth: format_date(s.birth_date),
                registered: s.registered_on.map(format_date).unwrap_or_default(),
                loans: loans
                    .iter()
                    .map(|l| {
                        let title = lib.book(l.book_id).map(|b| b.title.clone());
                        (title.unwrap_or_default(), l.days_out(today))
                    })
                    .collect(),
            }
        })
        .collect()
}

impl StudentsView {
    /// `late_days`: books out this many days or more count as late (shared
    /// with the loans screen).
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store, late_days: &mut i64) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.mode, Mode::List, "👫 Liste des élèves");
            ui.selectable_value(&mut self.mode, Mode::Cleanup, "🗑 Nettoyage");
        });
        ui.add_space(8.0);
        match self.mode {
            Mode::List => self.list(ui, store, late_days),
            Mode::Cleanup => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    theme::card(ui, |ui| self.cleanup(ui, store));
                });
            }
        }
    }

    // ------------------------------------------------------------- list

    fn list(&mut self, ui: &mut egui::Ui, store: &mut Store, late_days: &mut i64) {
        let today = biblo::today();
        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").size(22.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("nom, prénom, classe, code, titre d'un livre emprunté…")
                        .desired_width(440.0),
                );
                if ui.button("Effacer").clicked() {
                    self.query.clear();
                }
                ui.add_space(16.0);
                ui.label("Classe :");
                egui::ComboBox::from_id_salt("class_filter")
                    .selected_text(self.class_filter.label())
                    .height(500.0)
                    .show_ui(ui, |ui| {
                        let f = &mut self.class_filter;
                        ui.selectable_value(f, ClassFilter::All, "Toutes les classes");
                        for class in store.lib().classes() {
                            ui.selectable_value(f, ClassFilter::Named(class.clone()), class);
                        }
                        ui.selectable_value(f, ClassFilter::Without, "Sans classe");
                    });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Afficher :");
                let f = &mut self.loan_filter;
                ui.radio_value(f, LoanFilter::All, "tous les élèves");
                ui.radio_value(f, LoanFilter::WithBook, "ceux qui ont un livre");
                ui.radio_value(f, LoanFilter::Late, "ceux qui ont un livre depuis au moins");
                ui.add(
                    egui::DragValue::new(late_days)
                        .range(1..=365)
                        .suffix(" jours"),
                );
            });
            if let Some(m) = &self.list_message {
                m.show(ui);
            }
            self.confirm_delete_box(ui, store);
            self.edit_box(ui, store);
        });

        let lib = store.lib();
        let query = Query::new(&self.query);
        let all = rows(lib, today);
        let total = all.len();
        let mut shown: Vec<Row> = all
            .into_iter()
            .filter(|r| match self.loan_filter {
                LoanFilter::All => true,
                LoanFilter::WithBook => !r.loans.is_empty(),
                LoanFilter::Late => r.oldest_days() >= *late_days,
            })
            .filter(|r| self.class_filter.matches(&r.student.class))
            .filter(|r| {
                let mut fields = vec![
                    r.code.as_str(),
                    &r.student.last_name,
                    &r.student.first_name,
                    &r.student.class,
                    &r.birth,
                    &r.registered,
                ];
                fields.extend(r.loans.iter().map(|(title, _)| title.as_str()));
                query.matches(&fields)
            })
            .collect();
        if self.loan_filter == LoanFilter::Late {
            // The longest-kept books first.
            shown.sort_by_key(|r| std::cmp::Reverse(r.oldest_days()));
        } else {
            shown.sort_by_cached_key(|r| fold(&r.student.full_name()));
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let books: usize = shown.iter().map(|r| r.loans.len()).sum();
            ui.weak(format!(
                "{} élève(s) affiché(s) sur {total} · {books} livre(s) emprunté(s) par ces élèves",
                shown.len()
            ));
        });
        ui.add_space(4.0);

        let (mut delete, mut edit) = (None, None);
        theme::card(ui, |ui| {
            if shown.is_empty() {
                ui.label("Aucun élève ne correspond.");
                return;
            }
            let late = *late_days;
            TableBuilder::new(ui)
                .id_salt("students_tab")
                .striped(true)
                .resizable(true)
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::initial(150.0).at_least(60.0).clip(true))
                .column(Column::initial(130.0).at_least(60.0).clip(true))
                .column(Column::initial(80.0).at_least(50.0).clip(true))
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder().at_least(120.0).clip(true))
                .header(26.0, |mut header| {
                    for title in [
                        "",
                        "Code",
                        "Nom",
                        "Prénom",
                        "Classe",
                        "Né(e) le",
                        "Inscrit(e) le",
                        "Livres empruntés",
                    ] {
                        header.col(|ui| {
                            ui.label(RichText::new(title).family(theme::bold()));
                        });
                    }
                })
                .body(|body| {
                    body.rows(30.0, shown.len(), |mut table_row| {
                        let row = &shown[table_row.index()];
                        table_row.col(|ui| {
                            if ui
                                .small_button("✏")
                                .on_hover_text("Modifier la fiche (nom, classe…)")
                                .clicked()
                            {
                                edit = Some(row.student.id);
                            }
                            if ui
                                .small_button("🗑")
                                .on_hover_text("Supprimer cet élève")
                                .clicked()
                            {
                                delete = Some(row.student.id);
                            }
                        });
                        table_row.col(|ui| {
                            ui.monospace(&row.code);
                        });
                        for text in [
                            &row.student.last_name,
                            &row.student.first_name,
                            &row.student.class,
                            &row.birth,
                            &row.registered,
                        ] {
                            table_row.col(|ui| {
                                ui.label(text);
                            });
                        }
                        table_row.col(|ui| {
                            if let Some((_, days)) = row.loans.first() {
                                let (text, colors) = if *days >= late {
                                    (format!("⏰ {days} j"), theme::ERROR)
                                } else {
                                    (format!("{days} j"), theme::ON_LOAN)
                                };
                                theme::badge(ui, &text, colors);
                                let titles: Vec<&str> =
                                    row.loans.iter().map(|(t, _)| t.as_str()).collect();
                                ui.label(titles.join(", "));
                            }
                        });
                    });
                });
        });

        if delete.is_some() {
            self.confirm_delete = delete;
            self.edit = None;
            self.list_message = None;
        }
        if let Some(s) = edit.and_then(|id| store.lib().student(id)) {
            self.edit = Some(EditForm::new(s));
            self.confirm_delete = None;
            self.list_message = None;
        }
    }

    /// Form to change the details of the pupil chosen in the list.
    fn edit_box(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        let Some(form) = &mut self.edit else {
            return;
        };
        let Some(who) = store
            .lib()
            .student(form.id)
            .map(|s| format!("{} ({})", s.full_name(), student_code(s.id)))
        else {
            self.edit = None;
            return;
        };
        let classes = store.lib().classes();
        let (mut save, mut cancel) = (false, false);
        egui::Frame::new()
            .fill(theme::INFO.0)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("✏ Fiche de {who}"))
                        .family(theme::bold())
                        .color(theme::INFO.1),
                );
                egui::Grid::new("edit_student")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Nom");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.last_name).desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label("Prénom");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.first_name).desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label("Date de naissance");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.birth_date)
                                .hint_text("jj/mm/aaaa ou jjmmaaaa")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label("Classe");
                        text_with_choices(
                            ui,
                            "edit_class_pick",
                            &mut form.class,
                            "CP, CE1…",
                            classes,
                        );
                        ui.end_row();
                    });
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
            match save_edit(store, form) {
                Ok(text) => {
                    self.list_message = Some(Message::Ok(text));
                    self.edit = None;
                }
                Err(error) => form.error = Some(error),
            }
        } else if cancel {
            self.edit = None;
        }
    }

    /// Asks before deleting the pupil chosen in the list.
    fn confirm_delete_box(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        let Some(id) = self.confirm_delete else {
            return;
        };
        let lib = store.lib();
        let Some(s) = lib.student(id) else {
            self.confirm_delete = None;
            return;
        };
        let who = format!("{} (né(e) le {})", s.full_name(), format_date(s.birth_date));
        let books = lib.open_loans_of_student(id);
        let (mut yes, mut no) = (false, false);
        egui::Frame::new()
            .fill(theme::ERROR.0)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if books.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "Supprimer définitivement {who} et l'historique de ses prêts ?"
                        ))
                        .family(theme::bold())
                        .color(theme::ERROR.1),
                    );
                    ui.horizontal(|ui| {
                        yes = ui
                            .add(theme::main_button("Oui, supprimer", theme::RED))
                            .clicked();
                        no = ui.button("Annuler").clicked();
                    });
                } else {
                    let titles: Vec<String> = books
                        .iter()
                        .filter_map(|l| lib.book(l.book_id))
                        .map(|b| format!("« {} »", b.title))
                        .collect();
                    ui.label(
                        RichText::new(format!(
                            "⚠ {who} a encore {} : enregistrez d'abord le retour.",
                            titles.join(", ")
                        ))
                        .family(theme::bold())
                        .color(theme::ERROR.1),
                    );
                    no = ui.button("OK").clicked();
                }
            });
        if yes {
            let name = s.full_name();
            self.list_message = Some(match store.apply(|lib| lib.delete_student(id)) {
                Ok(()) => Message::Ok(format!("{name} a été supprimé(e).")),
                Err(e) => Message::Err(e.to_string()),
            });
        }
        if yes || no {
            self.confirm_delete = None;
        }
    }

    // ---------------------------------------------------------- cleanup

    fn cleanup(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        let today = biblo::today();
        let cutoff = today
            .checked_sub_months(Months::new(12 * self.years))
            .unwrap_or(today);
        ui.heading("🗑 Supprimer les anciens élèves");
        ui.add_space(4.0);
        ui.label(
            "La suppression est définitive : l'élève et l'historique de ses prêts sont \
             effacés, et son code ne sera jamais redonné. En cas d'erreur, les sauvegardes \
             du dossier backups permettent de revenir en arrière (une par jour \
             d'utilisation, les 30 dernières).",
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Élèves inscrits il y a au moins");
            if ui
                .add(
                    egui::DragValue::new(&mut self.years)
                        .range(1..=20)
                        .suffix(" ans"),
                )
                .changed()
            {
                self.confirming = false;
            }
            ui.label(format!("(le {} ou avant)", format_date(cutoff)));
        });
        ui.add_space(6.0);

        let lib = store.lib();
        let candidates = lib.registered_on_or_before(cutoff);
        let (with_books, free): (Vec<u32>, Vec<u32>) = candidates
            .iter()
            .partition(|&&id| !lib.open_loans_of_student(id).is_empty());
        let describe = |id: u32| {
            lib.student(id)
                .map(|s| {
                    let registered = s.registered_on.map(format_date).unwrap_or_default();
                    format!(
                        "{} (né(e) le {}, inscrit(e) le {registered})",
                        s.full_name(),
                        format_date(s.birth_date)
                    )
                })
                .unwrap_or_default()
        };

        if candidates.is_empty() {
            ui.label(format!(
                "Aucun élève inscrit le {} ou avant.",
                format_date(cutoff)
            ));
        }

        // Pupils who still have a book cannot be deleted: say so loudly.
        if !with_books.is_empty() {
            egui::Frame::new()
                .fill(theme::ERROR.0)
                .corner_radius(12)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(format!(
                            "⚠ {} élève(s) ont encore un livre : ils ne seront pas supprimés \
                             tant que le livre n'est pas rendu.",
                            with_books.len()
                        ))
                        .family(theme::bold())
                        .color(theme::ERROR.1),
                    );
                    for &id in &with_books {
                        let books: Vec<String> = lib
                            .open_loans_of_student(id)
                            .iter()
                            .map(|l| {
                                let title = lib
                                    .book(l.book_id)
                                    .map(|b| b.title.clone())
                                    .unwrap_or_default();
                                format!("« {title} » depuis le {}", format_date(l.borrowed_on))
                            })
                            .collect();
                        ui.label(
                            RichText::new(format!("• {} : {}", describe(id), books.join(", ")))
                                .color(theme::ERROR.1),
                        );
                    }
                });
            ui.add_space(8.0);
        }

        if !free.is_empty() {
            ui.label(format!(
                "Élèves à supprimer ({}) : décochez ceux qui sont encore à l'école.",
                free.len()
            ));
            ui.horizontal(|ui| {
                if ui.small_button("Tout cocher").clicked() {
                    self.unticked.clear();
                }
                if ui.small_button("Tout décocher").clicked() {
                    self.unticked.extend(free.iter().copied());
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("cleanup_list")
                .max_height(320.0)
                .show(ui, |ui| {
                    for &id in &free {
                        let mut ticked = !self.unticked.contains(&id);
                        if ui.checkbox(&mut ticked, describe(id)).changed() {
                            self.confirming = false;
                            if ticked {
                                self.unticked.remove(&id);
                            } else {
                                self.unticked.insert(id);
                            }
                        }
                    }
                });
        }

        let chosen: Vec<u32> = free
            .iter()
            .copied()
            .filter(|id| !self.unticked.contains(id))
            .collect();
        ui.add_space(8.0);
        if !self.confirming {
            let label = format!("Supprimer les {} élève(s) cochés", chosen.len());
            if ui
                .add_enabled(!chosen.is_empty(), theme::main_button(&label, theme::RED))
                .clicked()
            {
                self.confirming = true;
            }
        } else {
            ui.label(
                RichText::new(format!(
                    "Confirmer : supprimer définitivement {} élève(s) ?",
                    chosen.len()
                ))
                .family(theme::bold()),
            );
            ui.horizontal(|ui| {
                if ui
                    .add(theme::main_button("Oui, supprimer", theme::RED))
                    .clicked()
                {
                    self.confirming = false;
                    self.cleanup_message = Some(self.delete(store, &chosen));
                }
                if ui.button("Annuler").clicked() {
                    self.confirming = false;
                }
            });
        }
        if let Some(m) = &self.cleanup_message {
            m.show(ui);
        }
    }

    fn delete(&mut self, store: &mut Store, ids: &[u32]) -> Message {
        match store.apply(|lib| Ok(lib.delete_students(ids))) {
            Err(e) => Message::Err(e.to_string()),
            Ok(report) => {
                let mut text = format!("{} élève(s) supprimé(s).", report.deleted.len());
                if !report.kept_with_books.is_empty() {
                    let lib = store.lib();
                    let names: Vec<String> = report
                        .kept_with_books
                        .iter()
                        .map(|&id| lib.student(id).map(|s| s.full_name()).unwrap_or_default())
                        .collect();
                    text.push_str(&format!(
                        " Gardés car ils ont encore un livre : {}.",
                        names.join(", ")
                    ));
                }
                Message::Ok(text)
            }
        }
    }
}

/// Saves the edited details; returns the message to show, or the error.
fn save_edit(store: &mut Store, form: &EditForm) -> Result<String, String> {
    let birth = parse_date(&form.birth_date)
        .ok_or("Date de naissance invalide : par exemple 14/05/2017 ou 14052017.")?;
    if birth > biblo::today() {
        return Err("La date de naissance est dans le futur.".into());
    }
    let before = store.lib().student(form.id).cloned();
    store
        .apply(|lib| {
            lib.update_student(form.id, &form.last_name, &form.first_name, birth)?;
            lib.set_student_class(form.id, &form.class)
        })
        .map_err(|e| e.to_string())?;
    let after = store.lib().student(form.id);
    let name = after.map(|s| s.full_name()).unwrap_or_default();
    let mut text = format!("Fiche de {name} enregistrée.");
    // The card shows the name and birth date next to the barcode.
    let card_changed = match (before, after) {
        (Some(b), Some(a)) => b.full_name() != a.full_name() || b.birth_date != a.birth_date,
        _ => false,
    };
    if card_changed {
        text.push_str(
            " Le code de sa carte ne change pas ; pour que la carte affiche les \
             nouvelles informations, réimprimez-la depuis l'onglet Enregistrer.",
        );
    }
    Ok(text)
}
