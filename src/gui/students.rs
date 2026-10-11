//! Pupils tab: every pupil, filtered live, with a filter on books kept too
//! long, and a cleanup that deletes former pupils in one go.

use std::collections::HashSet;

use biblo::codes::student_code;
use biblo::dates::format_date;
use biblo::search::{fold, Query};
use biblo::{Library, Store, Student};
use chrono::{Months, NaiveDate};
use eframe::egui::{self, Align, Layout, Margin, RichText};
use egui_extras::{Column, TableBuilder};

use super::{theme, Message};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    List,
    Cleanup,
}

pub struct StudentsView {
    mode: Mode,
    query: String,
    only_late: bool,
    /// Pupil whose deletion waits for a confirmation.
    confirm_delete: Option<u32>,
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
            only_late: false,
            confirm_delete: None,
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
                        .hint_text("nom, prénom, date, code, titre d'un livre emprunté…")
                        .desired_width(440.0),
                );
                if ui.button("Effacer").clicked() {
                    self.query.clear();
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.only_late,
                    "Seulement les élèves qui ont un livre depuis au moins",
                );
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
        });

        let lib = store.lib();
        let query = Query::new(&self.query);
        let all = rows(lib, today);
        let total = all.len();
        let mut shown: Vec<Row> = all
            .into_iter()
            .filter(|r| !self.only_late || r.oldest_days() >= *late_days)
            .filter(|r| {
                let mut fields = vec![
                    r.code.as_str(),
                    &r.student.last_name,
                    &r.student.first_name,
                    &r.birth,
                    &r.registered,
                ];
                fields.extend(r.loans.iter().map(|(title, _)| title.as_str()));
                query.matches(&fields)
            })
            .collect();
        if self.only_late {
            // The longest-kept books first.
            shown.sort_by_key(|r| std::cmp::Reverse(r.oldest_days()));
        } else {
            shown.sort_by_cached_key(|r| fold(&r.student.full_name()));
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.weak(format!("{} élève(s) affiché(s) sur {}", shown.len(), total));
        });
        ui.add_space(4.0);

        let mut delete = None;
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
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder().at_least(120.0).clip(true))
                .header(26.0, |mut header| {
                    for title in [
                        "",
                        "Code",
                        "Nom",
                        "Prénom",
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
            self.list_message = None;
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
