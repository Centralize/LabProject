//! The overview: totals, progress per column and priority, upcoming
//! deadlines and how much each team member has on their plate.

use crate::app::{Action, App};
use crate::model::{self, rgb, Priority, Project};
use crate::theme::Pal;
use crate::widgets;
use chrono::Duration;
use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2};

fn card_frame(pal: &Pal) -> Frame {
    Frame::new().fill(pal.card).corner_radius(12).stroke(Stroke::new(1.0, pal.border)).inner_margin(Margin::same(16))
}

impl App {
    pub fn overview_view(&mut self, ui: &mut egui::Ui, project: &Project, pal: &Pal) {
        let today = model::today();
        let total = project.task_count();
        let done = project.done_count();
        let open: Vec<(&model::Column, &model::Task)> = project.all_tasks().filter(|(c, _)| !c.is_done).collect();
        let overdue = open.iter().filter(|(_, t)| t.due.is_some_and(|d| d < today)).count();
        let week = open.iter().filter(|(_, t)| t.due.is_some_and(|d| d >= today && d <= today + Duration::days(7))).count();
        let points_total: u32 = project.all_tasks().map(|(_, t)| t.points).sum();
        let points_done: u32 = project.all_tasks().filter(|(c, _)| c.is_done).map(|(_, t)| t.points).sum();

        egui::ScrollArea::vertical().id_salt("overview_scroll").auto_shrink([false, false]).show(ui, |ui| {
            // ---- stat cards
            let gap = 14.0;
            let w = ((ui.available_width() - gap * 3.0) / 4.0).max(150.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                let pct = if total == 0 { 0 } else { (done as f32 / total as f32 * 100.0).round() as i32 };
                stat(ui, w, pal, "Total tasks", &total.to_string(), &format!("{} open", total - done), pal.accent);
                stat(ui, w, pal, "Completed", &done.to_string(), &format!("{pct}% of all tasks"), pal.success);
                stat(ui, w, pal, "Overdue", &overdue.to_string(), if overdue == 0 { "Nothing late" } else { "Need attention" }, pal.danger);
                stat(ui, w, pal, "Due this week", &week.to_string(), "In the next 7 days", pal.warn);
            });
            ui.add_space(gap);

            // ---- overall progress
            card_frame(pal).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Overall progress").strong().size(15.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if points_total > 0 {
                            ui.label(RichText::new(format!("{points_done} of {points_total} points done")).color(pal.weak));
                        }
                    });
                });
                ui.add_space(8.0);
                let w = ui.available_width();
                widgets::progress_bar(ui, project.progress(), w, 12.0, pal.success, pal);
                ui.add_space(10.0);
                // stacked bar per column
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
                if total > 0 {
                    let mut x = rect.min.x;
                    for c in &project.columns {
                        if c.tasks.is_empty() {
                            continue;
                        }
                        let cw = rect.width() * c.tasks.len() as f32 / total as f32;
                        let r = egui::Rect::from_min_size(egui::pos2(x, rect.min.y), Vec2::new(cw - 2.0, rect.height()));
                        ui.painter().rect_filled(r, 5.0, rgb(c.color));
                        if cw > 60.0 {
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{} {}", c.tasks.len(), c.name), egui::FontId::proportional(11.5), Color32::WHITE);
                        }
                        x += cw;
                    }
                } else {
                    ui.painter().rect_filled(rect, 5.0, pal.hover);
                }
            });
            ui.add_space(gap);

            ui.columns(2, |cols| {
                // ---- left: by column and by priority
                card_frame(pal).show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Tasks by status").strong().size(15.0));
                    ui.add_space(8.0);
                    let max = project.columns.iter().map(|c| c.tasks.len()).max().unwrap_or(1).max(1);
                    for c in &project.columns {
                        bar_row(ui, pal, &c.name, c.tasks.len(), max, rgb(c.color));
                    }
                    if project.columns.is_empty() {
                        ui.label(RichText::new("No columns yet").color(pal.weak));
                    }
                });
                cols[0].add_space(gap);
                card_frame(pal).show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Open tasks by priority").strong().size(15.0));
                    ui.add_space(8.0);
                    let counts: Vec<(Priority, usize)> = Priority::ALL.iter().rev().map(|p| (*p, open.iter().filter(|(_, t)| t.priority == *p).count())).collect();
                    let max = counts.iter().map(|c| c.1).max().unwrap_or(1).max(1);
                    for (p, n) in counts {
                        bar_row(ui, pal, p.name(), n, max, p.color());
                    }
                });

                // ---- right: deadlines and workload
                card_frame(pal).show(&mut cols[1], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Upcoming deadlines").strong().size(15.0));
                    ui.add_space(6.0);
                    let mut due: Vec<&(&model::Column, &model::Task)> = open.iter().filter(|(_, t)| t.due.is_some()).collect();
                    due.sort_by_key(|(_, t)| t.due);
                    if due.is_empty() {
                        ui.label(RichText::new("No open tasks have a due date.").color(pal.weak));
                    }
                    for (c, t) in due.iter().take(8) {
                        let d = t.due.unwrap_or(today);
                        let mut clicked = false;
                        ui.horizontal(|ui| {
                            widgets::dot(ui, t.priority.color(), 4.5);
                            let title_w = (ui.available_width() - 96.0).max(60.0);
                            let r = ui
                                .allocate_ui(Vec2::new(title_w, 22.0), |ui| {
                                    ui.add(egui::Label::new(RichText::new(&t.title).color(pal.text)).truncate().sense(Sense::click()))
                                })
                                .inner;
                            if r.clicked() {
                                clicked = true;
                            }
                            r.on_hover_text(format!("{} · {} priority", c.name, t.priority.name()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(widgets::short_date(d, today)).color(widgets::due_color(d, today, false, pal)).strong());
                            });
                        });
                        if clicked {
                            self.actions.push(Action::SelectTask(Some(t.id)));
                        }
                    }
                });
                cols[1].add_space(gap);
                card_frame(pal).show(&mut cols[1], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Team workload (open tasks)").strong().size(15.0));
                    ui.add_space(8.0);
                    let mut people: Vec<(String, usize)> = project.members.iter().map(|m| (m.clone(), open.iter().filter(|(_, t)| &t.assignee == m).count())).collect();
                    for (_, t) in &open {
                        if !t.assignee.is_empty() && !people.iter().any(|(n, _)| n == &t.assignee) {
                            people.push((t.assignee.clone(), open.iter().filter(|(_, x)| x.assignee == t.assignee).count()));
                        }
                    }
                    let unassigned = open.iter().filter(|(_, t)| t.assignee.is_empty()).count();
                    let max = people.iter().map(|p| p.1).max().unwrap_or(0).max(unassigned).max(1);
                    for (name, n) in &people {
                        ui.horizontal(|ui| {
                            widgets::avatar(ui, name, 10.0);
                            bar_row(ui, pal, name, *n, max, crate::theme::person_color(name));
                        });
                    }
                    if unassigned > 0 {
                        ui.horizontal(|ui| {
                            widgets::dot(ui, pal.weak, 9.0);
                            bar_row(ui, pal, "Unassigned", unassigned, max, pal.weak);
                        });
                    }
                    if people.is_empty() && unassigned == 0 {
                        ui.label(RichText::new("Add team members in the project settings.").color(pal.weak));
                    }
                });
            });
        });
    }
}

fn stat(ui: &mut egui::Ui, w: f32, pal: &Pal, title: &str, value: &str, sub: &str, color: Color32) {
    card_frame(pal).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(w - 34.0);
            ui.horizontal(|ui| {
                widgets::dot(ui, color, 4.5);
                ui.label(RichText::new(title).color(pal.weak).size(13.0));
            });
            ui.label(RichText::new(value).size(30.0).strong().color(pal.text));
            ui.label(RichText::new(sub).color(pal.weak).size(12.0));
        });
    });
}

fn bar_row(ui: &mut egui::Ui, pal: &Pal, name: &str, n: usize, max: usize, color: Color32) {
    ui.horizontal(|ui| {
        let name_w = 120.0;
        let g = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(13.0), pal.text);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(name_w, 22.0), Sense::hover());
        let clipped = ui.painter().with_clip_rect(r);
        let truncated = g.size().x > name_w;
        clipped.galley(egui::pos2(r.min.x, r.center().y - g.size().y / 2.0), g, pal.text);
        if truncated {
            resp.on_hover_text(name);
        }
        let w = (ui.available_width() - 40.0).max(40.0);
        let (br, _) = ui.allocate_exact_size(Vec2::new(w, 10.0), Sense::hover());
        widgets::paint_progress(ui.painter(), br, n as f32 / max as f32, color, pal);
        ui.label(RichText::new(n.to_string()).strong());
    });
}
