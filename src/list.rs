//! The list view: every task of the project in one sortable table.

use crate::app::{Action, App, SortKey};
use crate::model::{self, rgb, Project};
use crate::theme::Pal;
use crate::widgets;
use eframe::egui::{self, Align, Button, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2};

impl App {
    pub fn list_view(&mut self, ui: &mut egui::Ui, project: &Project, pal: &Pal) {
        let today = model::today();
        let mut rows: Vec<(usize, &model::Column, &model::Task)> = Vec::new();
        for (ci, c) in project.columns.iter().enumerate() {
            for t in &c.tasks {
                if self.filters.matches(t, project, c.is_done, today) {
                    rows.push((ci, c, t));
                }
            }
        }
        let (key, asc) = self.sort;
        rows.sort_by(|a, b| {
            let o = match key {
                SortKey::Board => std::cmp::Ordering::Equal,
                SortKey::Title => a.2.title.to_lowercase().cmp(&b.2.title.to_lowercase()),
                SortKey::Status => a.0.cmp(&b.0),
                SortKey::Priority => b.2.priority.rank().cmp(&a.2.priority.rank()),
                SortKey::Due => match (a.2.due, b.2.due) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
                SortKey::Assignee => {
                    let ka = (a.2.assignee.is_empty(), a.2.assignee.to_lowercase());
                    let kb = (b.2.assignee.is_empty(), b.2.assignee.to_lowercase());
                    ka.cmp(&kb)
                }
                SortKey::Points => b.2.points.cmp(&a.2.points),
                SortKey::Updated => b.2.updated.cmp(&a.2.updated),
            };
            if asc {
                o
            } else {
                o.reverse()
            }
        });

        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} task{}", rows.len(), if rows.len() == 1 { "" } else { "s" })).color(pal.weak));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.sort.0 != SortKey::Board && ui.add(Button::new(RichText::new("Board order").color(pal.accent)).frame(false)).clicked() {
                    self.sort = (SortKey::Board, true);
                }
                ui.label(RichText::new("Click a column title to sort").color(pal.weak).size(12.0));
            });
        });
        ui.add_space(6.0);

        Frame::new()
            .fill(pal.card)
            .corner_radius(12)
            .stroke(Stroke::new(1.0, pal.border))
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                egui::ScrollArea::both().id_salt("list_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("task_table")
                        .num_columns(8)
                        .striped(true)
                        .spacing(Vec2::new(18.0, 10.0))
                        .min_row_height(30.0)
                        .show(ui, |ui| {
                            for (k, name) in [
                                (SortKey::Title, "Task"),
                                (SortKey::Status, "Status"),
                                (SortKey::Priority, "Priority"),
                                (SortKey::Due, "Due"),
                                (SortKey::Assignee, "Assignee"),
                                (SortKey::Board, "Labels"),
                                (SortKey::Points, "Points"),
                                (SortKey::Updated, "Checklist"),
                            ] {
                                let sortable = !matches!(name, "Labels" | "Checklist");
                                let arrow = if self.sort.0 == k && sortable { if self.sort.1 { " ↑" } else { " ↓" } } else { "" };
                                let text = RichText::new(format!("{}{arrow}", name.to_uppercase())).size(11.0).strong().color(pal.weak);
                                if sortable {
                                    if ui.add(egui::Label::new(text).sense(Sense::click())).clicked() {
                                        if self.sort.0 == k {
                                            self.sort.1 = !self.sort.1;
                                        } else {
                                            self.sort = (k, true);
                                        }
                                    }
                                } else {
                                    ui.label(text);
                                }
                            }
                            ui.end_row();

                            for (_, col, t) in &rows {
                                let selected = self.selected_task == Some(t.id);
                                let title = RichText::new(&t.title).color(if col.is_done { pal.weak } else { pal.text });
                                let title = if col.is_done { title.strikethrough() } else { title };
                                ui.scope(|ui| {
                                    ui.set_min_width(240.0);
                                    if ui.add(Button::selectable(selected, title).frame_when_inactive(false)).clicked() {
                                        let next = if selected { None } else { Some(t.id) };
                                        self.actions.push(Action::SelectTask(next));
                                    }
                                });
                                ui.horizontal(|ui| {
                                    widgets::dot(ui, rgb(col.color), 4.5);
                                    ui.label(&col.name);
                                });
                                widgets::priority_chip(ui, t.priority, pal);
                                match t.due {
                                    Some(d) => {
                                        let c = widgets::due_color(d, today, col.is_done, pal);
                                        ui.label(RichText::new(widgets::short_date(d, today)).color(c));
                                    }
                                    None => {
                                        ui.label(RichText::new("—").color(pal.weak));
                                    }
                                }
                                ui.horizontal(|ui| {
                                    if t.assignee.is_empty() {
                                        ui.label(RichText::new("Unassigned").color(pal.weak));
                                    } else {
                                        widgets::avatar(ui, &t.assignee, 10.0);
                                        ui.label(&t.assignee);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    for l in t.labels.iter().filter_map(|l| project.label(*l)) {
                                        widgets::chip(ui, &l.name, rgb(l.color), pal);
                                    }
                                });
                                ui.label(if t.points > 0 { t.points.to_string() } else { "—".into() });
                                let (d, n) = t.checklist_progress();
                                if n > 0 {
                                    ui.horizontal(|ui| {
                                        widgets::progress_bar(ui, d as f32 / n as f32, 50.0, 6.0, if d == n { pal.success } else { pal.accent }, pal);
                                        ui.label(RichText::new(format!("{d}/{n}")).color(pal.weak).size(12.0));
                                    });
                                } else {
                                    ui.label(RichText::new("—").color(pal.weak));
                                }
                                ui.end_row();
                            }
                        });
                    if rows.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(if project.task_count() == 0 { "This project has no tasks yet." } else { "No tasks match the filters." }).color(pal.weak));
                        });
                    }
                });
            });
    }
}
