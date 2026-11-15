//! The task panel on the right: everything about one task, editable in place.

use crate::app::{Action, App, Confirm};
use crate::model::{self, rgb, CheckItem, Comment, Label, Priority, Project};
use crate::theme::{on_soft, soft, Pal};
use crate::widgets;
use chrono::{Datelike, Duration, NaiveDate};
use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Key, Layout, Margin, PopupCloseBehavior,
    Response, RichText, Sense, Stroke, StrokeKind, TextEdit, Vec2,
};

#[derive(Default)]
struct Outcome {
    /// Something is about to change: remember the state from before this frame.
    snapshot: bool,
    changed: bool,
    actions: Vec<Action>,
}

impl Outcome {
    fn edit(&mut self) {
        self.snapshot = true;
        self.changed = true;
    }

    /// Text fields: remember the state once when editing starts.
    fn text(&mut self, r: &Response) {
        if r.gained_focus() {
            self.snapshot = true;
        }
        if r.changed() {
            self.changed = true;
        }
    }
}

struct PanelState {
    new_check: String,
    new_comment: String,
    cal_month: (i32, u32),
    focus_title: bool,
}

impl App {
    pub fn details_panel(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let Some(tid) = self.selected_task else { return };
        let Some(pid) = self.ws.selected else { return };
        if self.ws.project(pid).and_then(|p| p.task(tid)).is_none() {
            self.selected_task = None;
            return;
        }
        let before = self.ws.projects.clone();
        let mut out = Outcome::default();
        let mut st = PanelState {
            new_check: std::mem::take(&mut self.new_check),
            new_comment: std::mem::take(&mut self.new_comment),
            cal_month: self.cal_month,
            focus_title: self.focus_title,
        };
        let today = model::today();
        if let Some(project) = self.ws.project_mut(pid) {
            egui::Panel::right("details")
                .default_size(400.0)
                .size_range(340.0..=620.0)
                .resizable(true)
                .frame(Frame::new().fill(pal.header).inner_margin(Margin { left: 18, right: 12, top: 14, bottom: 10 }).stroke(Stroke::new(1.0, pal.border)))
                .show(ui, |ui| details_ui(ui, project, tid, &mut st, &mut out, pal, today));
        }
        self.new_check = st.new_check;
        self.new_comment = st.new_comment;
        self.cal_month = st.cal_month;
        self.focus_title = st.focus_title;
        if out.snapshot {
            self.push_undo(before);
        }
        if out.changed {
            if let Some(t) = self.ws.project_mut(pid).and_then(|p| p.task_mut(tid)) {
                t.updated = model::now();
            }
            self.touch();
        }
        self.actions.extend(out.actions);
    }
}

#[allow(clippy::too_many_arguments)]
fn details_ui(ui: &mut egui::Ui, project: &mut Project, tid: u64, st: &mut PanelState, out: &mut Outcome, pal: &Pal, today: NaiveDate) {
    let columns: Vec<(u64, String, [u8; 3], bool)> = project.columns.iter().map(|c| (c.id, c.name.clone(), c.color, c.is_done)).collect();
    let labels: Vec<Label> = project.labels.clone();
    let members: Vec<String> = project.members.clone();
    let pid = project.id;
    let Some((ci, _)) = project.locate(tid) else { return };
    let (col_id, col_name, col_color, col_done) = columns[ci].clone();
    let Some(task) = project.task_mut(tid) else { return };

    // ---- top bar
    ui.horizontal(|ui| {
        widgets::chip(ui, &col_name, rgb(col_color), pal);
        if task.is_overdue(today, col_done) {
            widgets::chip(ui, "Overdue", pal.danger, pal);
        } else if col_done {
            widgets::chip(ui, "✔ Done", pal.success, pal);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::x_button(ui, pal, 26.0, "Close (Esc)").clicked() {
                out.actions.push(Action::SelectTask(None));
            }
            let more = widgets::dots_button(ui, pal);
            egui::Popup::menu(&more).width(180.0).show(|ui| {
                if ui.button("Duplicate task").clicked() {
                    out.actions.push(Action::DuplicateTask(tid));
                    ui.close();
                }
                ui.menu_button("Move to", |ui| {
                    for (id, name, _, _) in &columns {
                        if *id != col_id && ui.button(name).clicked() {
                            out.actions.push(Action::MoveTask { task: tid, col: *id, before: None });
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button(RichText::new("Delete task…").color(pal.danger)).clicked() {
                    out.actions.push(Action::Ask(Confirm::DeleteTask(tid, task.title.clone())));
                    ui.close();
                }
            });
        });
    });
    ui.add_space(8.0);

    // ---- title
    let r = ui.add(
        TextEdit::singleline(&mut task.title)
            .font(FontId::proportional(19.0))
            .hint_text("What needs to be done?")
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(6, 6))
            .id_salt(("title", tid)),
    );
    if st.focus_title {
        r.request_focus();
        st.focus_title = false;
    }
    out.text(&r);
    if r.lost_focus() && task.title.trim().is_empty() {
        task.title = "Untitled task".into();
        out.changed = true;
    }
    ui.add_space(6.0);

    egui::ScrollArea::vertical().id_salt(("details_scroll", tid)).auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width() - 6.0);

        // ---- properties
        egui::Grid::new(("props", tid)).num_columns(2).spacing(Vec2::new(14.0, 10.0)).min_col_width(80.0).show(ui, |ui| {
            prop_label(ui, "Status", pal);
            egui::ComboBox::from_id_salt(("status", tid))
                .selected_text(col_name.as_str())
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (id, name, color, _) in &columns {
                        ui.horizontal(|ui| {
                            widgets::dot(ui, rgb(*color), 4.0);
                            if ui.selectable_label(*id == col_id, name.as_str()).clicked() && *id != col_id {
                                out.actions.push(Action::MoveTask { task: tid, col: *id, before: None });
                                ui.close();
                            }
                        });
                    }
                });
            ui.end_row();

            prop_label(ui, "Priority", pal);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for p in Priority::ALL {
                    if priority_toggle(ui, p, task.priority == p, pal).clicked() && task.priority != p {
                        task.priority = p;
                        out.edit();
                    }
                }
            });
            ui.end_row();

            prop_label(ui, "Assignee", pal);
            ui.horizontal(|ui| {
                if !task.assignee.is_empty() {
                    widgets::avatar(ui, &task.assignee, 11.0);
                }
                let shown = if task.assignee.is_empty() { "Unassigned".to_string() } else { task.assignee.clone() };
                egui::ComboBox::from_id_salt(("assignee", tid)).selected_text(shown).width(170.0).show_ui(ui, |ui| {
                    let mut a = task.assignee.clone();
                    ui.selectable_value(&mut a, String::new(), "Unassigned");
                    let mut list = members.clone();
                    if !a.is_empty() && !list.contains(&a) {
                        list.push(a.clone());
                    }
                    for m in &list {
                        ui.selectable_value(&mut a, m.clone(), m.as_str());
                    }
                    ui.separator();
                    if ui.button("Manage team members…").clicked() {
                        out.actions.push(Action::OpenProjectDialog(Some(pid)));
                        ui.close();
                    }
                    if a != task.assignee {
                        task.assignee = a;
                        out.edit();
                    }
                });
            });
            ui.end_row();

            prop_label(ui, "Due date", pal);
            ui.horizontal(|ui| {
                let text = match task.due {
                    Some(d) => format!("📅  {}", d.format("%a, %b %-d, %Y")),
                    None => "📅  Set a due date".to_string(),
                };
                let color = match task.due {
                    Some(d) => widgets::due_color(d, today, col_done, pal),
                    None => pal.weak,
                };
                let color = if color == pal.weak { pal.text } else { color };
                let btn = ui.add(Button::new(RichText::new(text).color(color)));
                if btn.clicked() {
                    let d = task.due.unwrap_or(today);
                    st.cal_month = (d.year(), d.month());
                }
                egui::Popup::from_toggle_button_response(&btn)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        if calendar(ui, &mut task.due, &mut st.cal_month, today, pal) {
                            out.edit();
                            ui.close();
                        }
                    });
                if task.due.is_some() && widgets::x_button(ui, pal, 22.0, "Remove due date").clicked() {
                    task.due = None;
                    out.edit();
                }
            });
            ui.end_row();

            prop_label(ui, "Points", pal);
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut task.points).range(0..=100).speed(0.1));
                if r.drag_started() || (r.changed() && !r.dragged()) {
                    out.snapshot = true;
                }
                if r.changed() {
                    out.changed = true;
                }
                ui.label(RichText::new("effort estimate").color(pal.weak).size(12.0));
            });
            ui.end_row();

            prop_label(ui, "Labels", pal);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                for l in &labels {
                    let on = task.labels.contains(&l.id);
                    if label_toggle(ui, l, on, pal).clicked() {
                        if on {
                            task.labels.retain(|x| *x != l.id);
                        } else {
                            task.labels.push(l.id);
                        }
                        out.edit();
                    }
                }
                if ui.add(Button::new(RichText::new("Edit labels").color(pal.accent).size(12.0)).frame(false)).clicked() {
                    out.actions.push(Action::OpenProjectDialog(Some(pid)));
                }
            });
            ui.end_row();
        });

        ui.add_space(10.0);
        ui.separator();

        // ---- description
        widgets::section(ui, "Description", pal);
        let r = ui.add(
            TextEdit::multiline(&mut task.description)
                .hint_text("Add more detail…")
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .id_salt(("desc", tid)),
        );
        out.text(&r);
        ui.add_space(8.0);

        // ---- checklist
        let (done, total) = task.checklist_progress();
        ui.horizontal(|ui| {
            widgets::section(ui, "Checklist", pal);
            if total > 0 {
                ui.label(RichText::new(format!("{done}/{total}")).color(pal.weak).size(12.0));
                let w = (ui.available_width() - 8.0).max(30.0);
                widgets::progress_bar(ui, done as f32 / total as f32, w, 6.0, if done == total { pal.success } else { pal.accent }, pal);
            }
        });
        let mut remove = None;
        let mut move_up = None;
        for (i, item) in task.checklist.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if ui.checkbox(&mut item.done, "").changed() {
                    out.edit();
                }
                let w = (ui.available_width() - 34.0).max(60.0);
                let mut te = TextEdit::singleline(&mut item.text).desired_width(w).id_salt(("check", tid, i));
                if item.done {
                    te = te.text_color(pal.weak);
                }
                let r = ui.add(te);
                out.text(&r);
                let x = widgets::x_button(ui, pal, 22.0, "Remove item");
                if x.clicked() {
                    remove = Some(i);
                }
                if r.has_focus() && ui.input(|i| i.modifiers.alt && i.key_pressed(Key::ArrowUp)) && i > 0 {
                    move_up = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            task.checklist.remove(i);
            out.edit();
        }
        if let Some(i) = move_up {
            task.checklist.swap(i, i - 1);
            out.edit();
        }
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 60.0).max(80.0);
            let r = ui.add(TextEdit::singleline(&mut st.new_check).hint_text("Add an item, then Enter").desired_width(w).id_salt(("new_check", tid)));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let add = ui.add_enabled(!st.new_check.trim().is_empty(), Button::new("Add")).clicked();
            if (enter || add) && !st.new_check.trim().is_empty() {
                task.checklist.push(CheckItem { text: st.new_check.trim().to_string(), done: false });
                st.new_check.clear();
                out.edit();
                r.request_focus();
            }
        });
        ui.add_space(10.0);

        // ---- comments
        widgets::section(ui, &format!("Comments ({})", task.comments.len()), pal);
        let mut delete_comment = None;
        for (i, c) in task.comments.iter().enumerate() {
            let resp = Frame::new()
                .fill(pal.hover)
                .corner_radius(8)
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(c.at.format("%b %-d, %Y at %H:%M").to_string()).color(pal.weak).size(11.5));
                    ui.add(egui::Label::new(RichText::new(&c.text).color(pal.text)).wrap());
                })
                .response;
            resp.context_menu(|ui| {
                if ui.button(RichText::new("Delete comment").color(pal.danger)).clicked() {
                    delete_comment = Some(i);
                    ui.close();
                }
            });
            ui.add_space(2.0);
        }
        if let Some(i) = delete_comment {
            task.comments.remove(i);
            out.edit();
        }
        let r = ui.add(
            TextEdit::multiline(&mut st.new_comment)
                .hint_text("Write a comment… (Ctrl+Enter to post)")
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .id_salt(("new_comment", tid)),
        );
        let ctrl_enter = r.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter));
        let post = ui
            .with_layout(Layout::right_to_left(Align::Min), |ui| {
                ui.add_enabled(!st.new_comment.trim().is_empty(), Button::new("Post comment")).clicked()
            })
            .inner;
        if (post || ctrl_enter) && !st.new_comment.trim().is_empty() {
            task.comments.push(Comment { text: st.new_comment.trim().to_string(), at: model::now() });
            st.new_comment.clear();
            out.edit();
        }

        ui.add_space(14.0);
        ui.separator();
        let mut info = format!(
            "Created {}  ·  Updated {}",
            task.created.format("%b %-d, %Y"),
            task.updated.format("%b %-d, %H:%M")
        );
        if let Some(c) = task.completed {
            info.push_str(&format!("  ·  Finished {}", c.format("%b %-d")));
        }
        ui.label(RichText::new(info).color(pal.weak).size(11.5));
        ui.add_space(6.0);
        if ui.add(Button::new(RichText::new("Delete task").color(pal.danger))).clicked() {
            out.actions.push(Action::Ask(Confirm::DeleteTask(tid, task.title.clone())));
        }
        ui.add_space(10.0);
    });
}

fn prop_label(ui: &mut egui::Ui, text: &str, pal: &Pal) {
    ui.label(RichText::new(text).color(pal.weak));
}

fn priority_toggle(ui: &mut egui::Ui, p: Priority, on: bool, pal: &Pal) -> Response {
    let g = ui.painter().layout_no_wrap(p.name().to_string(), FontId::proportional(12.5), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 18.0, 24.0), Sense::click());
    let c = p.color();
    let (fill, text) = if on { (c, Color32::WHITE) } else if resp.hovered() { (soft(c, pal), on_soft(c, pal)) } else { (Color32::TRANSPARENT, pal.weak) };
    ui.painter().rect_filled(rect, 6.0, fill);
    if !on {
        ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, pal.border), StrokeKind::Inside);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, p.name(), FontId::proportional(12.5), text);
    resp
}

fn label_toggle(ui: &mut egui::Ui, l: &Label, on: bool, pal: &Pal) -> Response {
    let c = rgb(l.color);
    let text = if on { format!("✔ {}", l.name) } else { l.name.clone() };
    let g = ui.painter().layout_no_wrap(text.clone(), FontId::proportional(12.0), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 16.0, 22.0), Sense::click());
    if on {
        ui.painter().rect_filled(rect, 6.0, soft(c, pal));
        ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, c), StrokeKind::Inside);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.0), on_soft(c, pal));
    } else {
        if resp.hovered() {
            ui.painter().rect_filled(rect, 6.0, pal.hover);
        }
        ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, pal.border), StrokeKind::Inside);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.0), pal.weak);
    }
    resp.on_hover_text(if on { "Click to remove" } else { "Click to add" })
}

/// A month calendar. Returns true when a date was picked.
fn calendar(ui: &mut egui::Ui, value: &mut Option<NaiveDate>, month: &mut (i32, u32), today: NaiveDate, pal: &Pal) -> bool {
    let mut picked = false;
    let (y, m) = *month;
    let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(today);
    let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) }.unwrap_or(first + Duration::days(31));
    let days = (next - first).num_days() as u32;
    let offset = first.weekday().num_days_from_monday();
    ui.set_min_width(250.0);
    ui.horizontal(|ui| {
        if ui.add(Button::new(" ‹ ").frame(false)).on_hover_text("Previous month").clicked() {
            *month = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(Button::new(" › ").frame(false)).on_hover_text("Next month").clicked() {
                *month = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            }
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(first.format("%B %Y").to_string()).strong());
            });
        });
    });
    ui.add_space(4.0);
    let cell = Vec2::new(32.0, 28.0);
    egui::Grid::new("calendar_grid").spacing(Vec2::new(2.0, 2.0)).show(ui, |ui| {
        for d in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
            ui.allocate_ui(cell, |ui| {
                ui.centered_and_justified(|ui| ui.label(RichText::new(d).size(11.0).color(pal.weak)));
            });
        }
        ui.end_row();
        let mut col = 0;
        for _ in 0..offset {
            ui.allocate_exact_size(cell, Sense::hover());
            col += 1;
        }
        for day in 1..=days {
            let date = NaiveDate::from_ymd_opt(y, m, day).unwrap_or(first);
            let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
            let selected = *value == Some(date);
            let is_today = date == today;
            let painter = ui.painter();
            if selected {
                painter.rect_filled(rect, CornerRadius::same(6), pal.accent);
            } else if resp.hovered() {
                painter.rect_filled(rect, CornerRadius::same(6), pal.hover);
            }
            if is_today && !selected {
                painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, pal.accent), StrokeKind::Inside);
            }
            let tc = if selected { Color32::WHITE } else if col >= 5 { pal.weak } else { pal.text };
            painter.text(rect.center(), Align2::CENTER_CENTER, day.to_string(), FontId::proportional(13.0), tc);
            if resp.clicked() {
                *value = Some(date);
                picked = true;
            }
            col += 1;
            if col == 7 {
                col = 0;
                ui.end_row();
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (name, d) in [("Today", 0), ("Tomorrow", 1), ("In a week", 7)] {
            if ui.small_button(name).clicked() {
                *value = Some(today + Duration::days(d));
                picked = true;
            }
        }
    });
    picked
}
