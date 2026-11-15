//! Pop-up dialogs: creating/editing a project and confirming deletions.

use crate::app::{Action, App, Confirm, ProjectDialog};
use crate::model::{self, rgb, Label, Template, PALETTE};
use crate::theme::Pal;
use crate::widgets;
use eframe::egui::{self, Align, Button, Color32, Frame, Id, Key, Layout, Margin, RichText, TextEdit, Vec2};

enum DialogResult {
    Keep,
    Cancel,
    Save,
    Delete,
}

impl App {
    pub fn dialogs(&mut self, ctx: &egui::Context, pal: &Pal) {
        self.project_dialog_ui(ctx, pal);
        self.confirm_ui(ctx, pal);
    }

    fn project_dialog_ui(&mut self, ctx: &egui::Context, pal: &Pal) {
        let Some(d) = &mut self.project_dialog else { return };
        let mut result = DialogResult::Keep;
        let modal = egui::Modal::new(Id::unique("project_dialog"))
            .frame(Frame::new().fill(pal.card).corner_radius(14).inner_margin(Margin::same(22)).stroke(egui::Stroke::new(1.0, pal.border)))
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.label(RichText::new(if d.editing.is_some() { "Project settings" } else { "New project" }).size(20.0).strong());
                ui.add_space(10.0);

                ui.label(RichText::new("Name").color(pal.weak));
                let r = ui.add(TextEdit::singleline(&mut d.name).hint_text("e.g. Office move").desired_width(f32::INFINITY).id_salt("pd_name"));
                if d.focus {
                    r.request_focus();
                    d.focus = false;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    result = DialogResult::Save;
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Description").color(pal.weak));
                ui.add(TextEdit::multiline(&mut d.description).hint_text("What is this project about? (optional)").desired_rows(2).desired_width(f32::INFINITY).id_salt("pd_desc"));
                ui.add_space(6.0);
                ui.label(RichText::new("Colour").color(pal.weak));
                widgets::color_picker(ui, &mut d.color);

                if d.editing.is_none() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Board layout").color(pal.weak));
                    for t in Template::ALL {
                        ui.radio_value(&mut d.template, t, t.name());
                    }
                    let cols: Vec<&str> = d.template.columns().iter().map(|c| c.0).collect();
                    if !cols.is_empty() {
                        ui.label(RichText::new(format!("Columns: {}", cols.join("  ›  "))).color(pal.weak).size(12.0));
                    }
                } else {
                    ui.add_space(10.0);
                    ui.columns(2, |cols| {
                        // labels
                        let ui = &mut cols[0];
                        ui.label(RichText::new("Labels").color(pal.weak));
                        let mut remove = None;
                        egui::ScrollArea::vertical().id_salt("pd_labels").max_height(170.0).show(ui, |ui| {
                            for (i, l) in d.labels.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                                    ui.painter().circle_filled(rect.center(), 7.0, rgb(l.color));
                                    if resp.on_hover_text("Click to change the colour").clicked() {
                                        let idx = PALETTE.iter().position(|c| *c == l.color).unwrap_or(0);
                                        l.color = PALETTE[(idx + 1) % PALETTE.len()];
                                    }
                                    ui.add(TextEdit::singleline(&mut l.name).desired_width(120.0).id_salt(("pd_label", i)));
                                    if widgets::x_button(ui, pal, 22.0, "Remove label").clicked() {
                                        remove = Some(i);
                                    }
                                });
                            }
                        });
                        if let Some(i) = remove {
                            d.labels.remove(i);
                        }
                        ui.horizontal(|ui| {
                            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                            ui.painter().circle_filled(rect.center(), 7.0, rgb(d.new_label_color));
                            if resp.on_hover_text("Click to change the colour").clicked() {
                                let idx = PALETTE.iter().position(|c| *c == d.new_label_color).unwrap_or(0);
                                d.new_label_color = PALETTE[(idx + 1) % PALETTE.len()];
                            }
                            let r = ui.add(TextEdit::singleline(&mut d.new_label).hint_text("New label").desired_width(120.0).id_salt("pd_new_label"));
                            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if (ui.small_button("Add").clicked() || enter) && !d.new_label.trim().is_empty() {
                                d.labels.push(Label { id: 0, name: d.new_label.trim().to_string(), color: d.new_label_color });
                                d.new_label.clear();
                                let idx = PALETTE.iter().position(|c| *c == d.new_label_color).unwrap_or(0);
                                d.new_label_color = PALETTE[(idx + 3) % PALETTE.len()];
                                r.request_focus();
                            }
                        });

                        // members
                        let ui = &mut cols[1];
                        ui.label(RichText::new("Team members").color(pal.weak));
                        let mut remove = None;
                        egui::ScrollArea::vertical().id_salt("pd_members").max_height(170.0).show(ui, |ui| {
                            for (i, m) in d.members.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    widgets::avatar(ui, m, 10.0);
                                    ui.label(m);
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::x_button(ui, pal, 22.0, "Remove member").clicked() {
                                            remove = Some(i);
                                        }
                                    });
                                });
                            }
                            if d.members.is_empty() {
                                ui.label(RichText::new("Nobody yet").color(pal.weak).size(12.0));
                            }
                        });
                        if let Some(i) = remove {
                            d.members.remove(i);
                        }
                        ui.horizontal(|ui| {
                            let r = ui.add(TextEdit::singleline(&mut d.new_member).hint_text("Name").desired_width(130.0).id_salt("pd_new_member"));
                            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if (ui.small_button("Add").clicked() || enter) && !d.new_member.trim().is_empty() {
                                let name = d.new_member.trim().to_string();
                                if !d.members.contains(&name) {
                                    d.members.push(name);
                                }
                                d.new_member.clear();
                                r.request_focus();
                            }
                        });
                    });
                }

                if let Some(e) = &d.error {
                    ui.add_space(6.0);
                    ui.label(RichText::new(e).color(pal.danger));
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if d.editing.is_some() && ui.add(Button::new(RichText::new("Delete project…").color(pal.danger))).clicked() {
                        result = DialogResult::Delete;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if d.editing.is_some() { "Save" } else { "Create project" };
                        let b = Button::new(RichText::new(label).color(Color32::WHITE).strong()).fill(pal.accent).corner_radius(8).min_size(Vec2::new(110.0, 32.0));
                        if ui.add(b).clicked() {
                            result = DialogResult::Save;
                        }
                        if ui.add(Button::new("Cancel").min_size(Vec2::new(80.0, 32.0))).clicked() {
                            result = DialogResult::Cancel;
                        }
                    });
                });
            });
        if modal.should_close() && matches!(result, DialogResult::Keep) {
            result = DialogResult::Cancel;
        }

        match result {
            DialogResult::Keep => {}
            DialogResult::Cancel => self.project_dialog = None,
            DialogResult::Delete => {
                if let Some(d) = self.project_dialog.take() {
                    if let Some(id) = d.editing {
                        self.confirm = Some(Confirm::DeleteProject(id, d.name));
                    }
                }
            }
            DialogResult::Save => {
                let name_empty = self.project_dialog.as_ref().is_some_and(|d| d.name.trim().is_empty());
                if name_empty {
                    if let Some(d) = &mut self.project_dialog {
                        d.error = Some("Please give the project a name.".into());
                        d.focus = true;
                    }
                    return;
                }
                if let Some(d) = self.project_dialog.take() {
                    self.save_project_dialog(d);
                }
            }
        }
    }

    fn save_project_dialog(&mut self, d: ProjectDialog) {
        let name = d.name.trim().to_string();
        match d.editing {
            None => {
                self.snapshot();
                let id = self.ws.create_project(&name, d.description.trim(), d.color, d.template);
                self.actions.push(Action::SelectProject(id));
                self.touch();
                self.show_toast(format!("Created \"{name}\""));
            }
            Some(pid) => {
                // give new labels real ids
                let mut labels = d.labels;
                for l in &mut labels {
                    if l.id == 0 {
                        l.id = self.ws.new_id();
                    }
                    if l.name.trim().is_empty() {
                        l.name = "Label".into();
                    }
                }
                self.snapshot();
                if let Some(p) = self.ws.project_mut(pid) {
                    p.name = name;
                    p.description = d.description.trim().to_string();
                    p.color = d.color;
                    let keep: Vec<u64> = labels.iter().map(|l| l.id).collect();
                    let removed_members: Vec<String> = p.members.iter().filter(|m| !d.members.contains(m)).cloned().collect();
                    for c in &mut p.columns {
                        for t in &mut c.tasks {
                            t.labels.retain(|l| keep.contains(l));
                            if removed_members.contains(&t.assignee) {
                                t.assignee.clear();
                            }
                        }
                    }
                    p.labels = labels;
                    p.members = d.members;
                }
                // a removed label can't stay in the filter
                if let Some(l) = self.filters.label {
                    if self.ws.project(pid).and_then(|p| p.label(l)).is_none() {
                        self.filters.label = None;
                    }
                }
                self.touch();
            }
        }
    }

    fn confirm_ui(&mut self, ctx: &egui::Context, pal: &Pal) {
        let Some(c) = &self.confirm else { return };
        let (title, body, button) = match c {
            Confirm::DeleteTask(_, t) => ("Delete task?", format!("\"{}\" will be deleted.", t), "Delete task"),
            Confirm::DeleteColumn(_, n, k) => ("Delete column?", format!("\"{n}\" and its {k} task{} will be deleted.", if *k == 1 { "" } else { "s" }), "Delete column"),
            Confirm::ClearColumn(_, n, k) => ("Delete all tasks?", format!("All {k} task{} in \"{n}\" will be deleted.", if *k == 1 { "" } else { "s" }), "Delete tasks"),
            Confirm::DeleteProject(_, n) => ("Delete project?", format!("\"{n}\" and all of its tasks will be deleted."), "Delete project"),
        };
        let mut decision: Option<bool> = None;
        let modal = egui::Modal::new(Id::unique("confirm_dialog"))
            .frame(Frame::new().fill(pal.card).corner_radius(14).inner_margin(Margin::same(22)).stroke(egui::Stroke::new(1.0, pal.border)))
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(RichText::new(title).size(18.0).strong());
                ui.add_space(6.0);
                ui.label(body);
                ui.label(RichText::new("You can undo this with Ctrl+Z.").color(pal.weak).size(12.5));
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let b = Button::new(RichText::new(button).color(Color32::WHITE).strong()).fill(pal.danger).corner_radius(8).min_size(Vec2::new(110.0, 32.0));
                    if ui.add(b).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        decision = Some(true);
                    }
                    if ui.add(Button::new("Cancel").min_size(Vec2::new(80.0, 32.0))).clicked() {
                        decision = Some(false);
                    }
                });
            });
        if modal.should_close() && decision.is_none() {
            decision = Some(false);
        }
        if let Some(yes) = decision {
            let c = self.confirm.take();
            if yes {
                match c {
                    Some(Confirm::DeleteTask(id, _)) => self.actions.push(Action::DeleteTask(id)),
                    Some(Confirm::DeleteColumn(id, _, _)) => self.actions.push(Action::DeleteColumn(id)),
                    Some(Confirm::ClearColumn(id, _, _)) => self.actions.push(Action::ClearColumn(id)),
                    Some(Confirm::DeleteProject(id, _)) => self.actions.push(Action::DeleteProject(id)),
                    None => {}
                }
            }
        }
        let _ = model::today;
    }
}
