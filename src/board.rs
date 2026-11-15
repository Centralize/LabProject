//! The Kanban board: columns side by side, cards you can drag between them,
//! quick-add at the bottom of each column and a column menu.

use crate::app::{Action, App, Confirm, DragState};
use crate::model::{self, rgb, Column, Project, PALETTE};
use crate::theme::{lerp, Pal};
use crate::widgets;
use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, CursorIcon, FontId, Id, Key, Layout, Order, Pos2, Rect,
    RichText, Sense, Stroke, StrokeKind, TextEdit, UiBuilder, Vec2,
};

const COL_W: f32 = 292.0;

impl App {
    pub fn board_view(&mut self, ui: &mut egui::Ui, project: &Project, pal: &Pal) {
        self.col_rects.clear();
        self.card_rects.clear();
        let today = model::today();

        if self.filters.active() {
            let shown: usize = project
                .columns
                .iter()
                .map(|c| c.tasks.iter().filter(|t| self.filters.matches(t, project, c.is_done, today)).count())
                .sum();
            ui.label(RichText::new(format!("Showing {shown} of {} tasks that match the filters", project.task_count())).color(pal.weak).size(12.5));
            ui.add_space(4.0);
        }

        let height = ui.available_height();
        egui::ScrollArea::horizontal()
            .id_salt("board_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let h = (height - 14.0).max(200.0);
                for col in &project.columns {
                    self.column_ui(ui, project, col, h, pal, today);
                }
                self.add_column_ui(ui, pal);
                ui.add_space(8.0);
            });
        });

        self.handle_drag(ui, project, pal);
    }

    fn column_ui(&mut self, ui: &mut egui::Ui, project: &Project, col: &Column, h: f32, pal: &Pal, today: chrono::NaiveDate) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(COL_W, h), Sense::hover());
        self.col_rects.push((col.id, rect));
        let dragging = self.drag.is_some();
        ui.painter().rect_filled(rect, CornerRadius::same(12), pal.column);

        let inner = rect.shrink2(Vec2::new(10.0, 10.0));
        let mut cui = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)).id_salt(("column", col.id)));

        // ---- header
        let count = col.tasks.len();
        let over = col.wip_limit > 0 && count as u32 > col.wip_limit;
        cui.horizontal(|ui| {
            ui.set_height(28.0);
            widgets::dot(ui, rgb(col.color), 5.0);
            let mut finish_rename: Option<Option<String>> = None;
            if let Some((rid, text)) = &mut self.renaming_col {
                if *rid == col.id {
                    let r = ui.add(TextEdit::singleline(text).desired_width(150.0).font(FontId::proportional(14.5)));
                    if self.rename_focus {
                        r.request_focus();
                        self.rename_focus = false;
                    }
                    if r.lost_focus() {
                        let cancelled = ui.input(|i| i.key_pressed(Key::Escape));
                        finish_rename = Some(if cancelled { None } else { Some(text.clone()) });
                    }
                }
            }
            let renaming_this = self.renaming_col.as_ref().is_some_and(|(id, _)| *id == col.id);
            if let Some(result) = finish_rename {
                if let Some(name) = result {
                    self.actions.push(Action::RenameColumn(col.id, name));
                }
                self.renaming_col = None;
            } else if !renaming_this {
                let name = ui.add(egui::Label::new(RichText::new(&col.name).strong().size(14.5).color(pal.text)).sense(Sense::click()).truncate());
                if name.double_clicked() {
                    self.renaming_col = Some((col.id, col.name.clone()));
                    self.rename_focus = true;
                }
                name.on_hover_text("Double-click to rename");
                // count badge
                let badge = if col.wip_limit > 0 { format!("{count}/{}", col.wip_limit) } else { count.to_string() };
                let badge_color = if over { pal.danger } else { pal.weak };
                let g = ui.painter().layout_no_wrap(badge, FontId::proportional(12.0), if over { Color32::WHITE } else { badge_color });
                let (br, bresp) = ui.allocate_exact_size(Vec2::new(g.size().x + 14.0, 20.0), Sense::hover());
                let fill = if over { pal.danger } else if pal.dark { lerp(pal.column, Color32::WHITE, 0.08) } else { lerp(pal.column, Color32::BLACK, 0.06) };
                ui.painter().rect_filled(br, 10.0, fill);
                ui.painter().galley(br.center() - g.size() / 2.0, g, badge_color);
                if col.wip_limit > 0 {
                    bresp.on_hover_text(if over {
                        format!("Over the limit of {} tasks in progress", col.wip_limit)
                    } else {
                        format!("Limit: {} tasks at a time", col.wip_limit)
                    });
                }
                if col.is_done {
                    ui.label(RichText::new("✔").color(pal.success)).on_hover_text("Tasks here count as done");
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let more = widgets::dots_button(ui, pal);
                egui::Popup::menu(&more).width(220.0).show(|ui| {
                    self.column_menu(ui, project, col, pal);
                });
                if ui.add(Button::new(RichText::new("+").size(16.0).color(pal.weak)).frame(false)).on_hover_text("Add a task here").clicked() {
                    self.quick_add = Some((col.id, String::new()));
                    self.quick_add_focus = true;
                }
            });
        });
        // colour line under the header
        let y = cui.cursor().min.y + 2.0;
        cui.painter().line_segment([Pos2::new(inner.min.x, y), Pos2::new(inner.max.x, y)], Stroke::new(2.0, rgb(col.color).gamma_multiply(0.55)));
        cui.add_space(10.0);

        // ---- cards
        let footer_h = 40.0;
        let list_h = (cui.available_height() - footer_h).max(40.0);
        let visible: Vec<&model::Task> = col.tasks.iter().filter(|t| self.filters.matches(t, project, col.is_done, today)).collect();
        egui::ScrollArea::vertical()
            .id_salt(("col_scroll", col.id))
            .max_height(list_h)
            .auto_shrink([false, false])
            .show(&mut cui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                if visible.is_empty() {
                    let msg = if dragging {
                        "Drop here"
                    } else if col.tasks.is_empty() {
                        "No tasks yet"
                    } else {
                        "No tasks match the filters"
                    };
                    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 64.0), Sense::hover());
                    ui.painter().rect_stroke(r, 10.0, Stroke::new(1.0, pal.border), StrokeKind::Inside);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, msg, FontId::proportional(12.5), pal.weak);
                }
                for task in visible {
                    self.card_ui(ui, project, col, task, pal, today);
                }
                ui.add_space(4.0);
            });

        // ---- footer: quick add
        cui.add_space(6.0);
        let mut submit: Option<String> = None;
        let mut close = false;
        if let Some((qid, text)) = &mut self.quick_add {
            if *qid == col.id {
                let r = cui.add(TextEdit::singleline(text).hint_text("Task title, then Enter").desired_width(f32::INFINITY));
                if self.quick_add_focus {
                    r.request_focus();
                    self.quick_add_focus = false;
                }
                if r.lost_focus() {
                    if cui.input(|i| i.key_pressed(Key::Enter)) && !text.trim().is_empty() {
                        submit = Some(std::mem::take(text));
                    } else {
                        close = true;
                    }
                }
            }
        }
        let adding_here = self.quick_add.as_ref().is_some_and(|(id, _)| *id == col.id);
        if let Some(title) = submit {
            self.actions.push(Action::NewTask { col: col.id, title, open: false });
            self.quick_add_focus = true; // keep typing the next one
        } else if close {
            self.quick_add = None;
        } else if !adding_here {
            let b = Button::new(RichText::new("+  Add a task").color(pal.weak)).frame(false).min_size(Vec2::new(cui.available_width(), 28.0));
            if cui.add(b).clicked() {
                self.quick_add = Some((col.id, String::new()));
                self.quick_add_focus = true;
            }
        }

        // highlight the column the dragged card would land in
        if dragging {
            if let Some(p) = ui.ctx().pointer_hover_pos() {
                if rect.x_range().contains(p.x) {
                    ui.painter().rect_stroke(rect, 12.0, Stroke::new(2.0, pal.accent.gamma_multiply(0.6)), StrokeKind::Inside);
                }
            }
        }
    }

    fn column_menu(&mut self, ui: &mut egui::Ui, project: &Project, col: &Column, pal: &Pal) {
        ui.set_min_width(210.0);
        if ui.button("Rename").clicked() {
            self.renaming_col = Some((col.id, col.name.clone()));
            self.rename_focus = true;
            ui.close();
        }
        if ui.button("Add a task").clicked() {
            self.quick_add = Some((col.id, String::new()));
            self.quick_add_focus = true;
            ui.close();
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Task limit");
            let mut limit = col.wip_limit;
            let r = ui.add(egui::DragValue::new(&mut limit).range(0..=99));
            if r.changed() {
                self.actions.push(Action::SetWip(col.id, limit));
            }
            ui.label(RichText::new(if limit == 0 { "(none)" } else { "" }).color(pal.weak).size(12.0));
        });
        ui.label(RichText::new("Colour").color(pal.weak).size(12.0));
        let mut c = col.color;
        if widgets::color_picker(ui, &mut c) {
            self.actions.push(Action::SetColumnColor(col.id, c));
        }
        let mut done = col.is_done;
        if ui.checkbox(&mut done, "Tasks here count as done").changed() {
            self.actions.push(Action::ToggleDoneColumn(col.id));
        }
        ui.separator();
        let idx = project.column_index(col.id).unwrap_or(0);
        ui.horizontal(|ui| {
            if ui.add_enabled(idx > 0, Button::new("‹ Move left")).clicked() {
                self.actions.push(Action::MoveColumn(col.id, -1));
                ui.close();
            }
            if ui.add_enabled(idx + 1 < project.columns.len(), Button::new("Move right ›")).clicked() {
                self.actions.push(Action::MoveColumn(col.id, 1));
                ui.close();
            }
        });
        ui.separator();
        if ui.add_enabled(!col.tasks.is_empty(), Button::new("Delete all tasks in it…")).clicked() {
            self.actions.push(Action::Ask(Confirm::ClearColumn(col.id, col.name.clone(), col.tasks.len())));
            ui.close();
        }
        if ui.button(RichText::new("Delete column…").color(pal.danger)).clicked() {
            if col.tasks.is_empty() {
                self.actions.push(Action::DeleteColumn(col.id));
            } else {
                self.actions.push(Action::Ask(Confirm::DeleteColumn(col.id, col.name.clone(), col.tasks.len())));
            }
            ui.close();
        }
    }

    fn card_ui(&mut self, ui: &mut egui::Ui, project: &Project, col: &Column, task: &model::Task, pal: &Pal, today: chrono::NaiveDate) {
        let w = ui.available_width();
        let h = widgets::card_height(ui.painter(), task, w);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let resp = ui.interact(rect, Id::unique(("card", task.id)), Sense::click_and_drag());
        self.card_rects.push((col.id, task.id, rect));

        let being_dragged = self.drag.as_ref().is_some_and(|d| d.task == task.id);
        if resp.drag_started() {
            let grab = resp.interact_pointer_pos().unwrap_or(rect.center());
            self.drag = Some(DragState { task: task.id, offset: grab - rect.min, size: rect.size() });
            self.quick_add = None;
        }
        if resp.clicked() {
            let next = if self.selected_task == Some(task.id) { None } else { Some(task.id) };
            self.actions.push(Action::SelectTask(next));
        }
        if resp.hovered() && self.drag.is_none() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.context_menu(|ui| {
            ui.set_min_width(190.0);
            if ui.button("Open").clicked() {
                self.actions.push(Action::SelectTask(Some(task.id)));
                ui.close();
            }
            ui.menu_button("Move to", |ui| {
                for c in &project.columns {
                    if c.id != col.id && ui.button(&c.name).clicked() {
                        self.actions.push(Action::MoveTask { task: task.id, col: c.id, before: None });
                        ui.close();
                    }
                }
            });
            if ui.button("Duplicate").clicked() {
                self.actions.push(Action::DuplicateTask(task.id));
                ui.close();
            }
            ui.separator();
            if ui.button(RichText::new("Delete").color(pal.danger)).clicked() {
                self.actions.push(Action::Ask(Confirm::DeleteTask(task.id, task.title.clone())));
                ui.close();
            }
        });

        if being_dragged {
            // leave a placeholder where the card was
            ui.painter().rect_filled(rect, 10.0, lerp(pal.column, pal.accent, 0.08));
            ui.painter().rect_stroke(rect, 10.0, Stroke::new(1.5, pal.accent.gamma_multiply(0.5)), StrokeKind::Inside);
        } else {
            let selected = self.selected_task == Some(task.id);
            widgets::paint_card(ui.painter(), rect, task, project, col.is_done, today, pal, selected, resp.hovered() && self.drag.is_none());
        }
    }

    fn add_column_ui(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(COL_W, 96.0), Sense::hover());
        let mut cui = ui.new_child(UiBuilder::new().max_rect(rect.shrink(10.0)).layout(Layout::top_down(Align::Min)));
        let mut submit = None;
        let mut close = false;
        if let Some(text) = &mut self.adding_column {
            ui.painter().rect_filled(rect, 12.0, pal.column);
            cui.label(RichText::new("New column").strong());
            let r = cui.add(TextEdit::singleline(text).hint_text("Column name, then Enter").desired_width(f32::INFINITY).id_salt("new_column"));
            if !r.has_focus() && !r.lost_focus() && text.is_empty() {
                r.request_focus();
            }
            if r.lost_focus() {
                if cui.input(|i| i.key_pressed(Key::Enter)) && !text.trim().is_empty() {
                    submit = Some(std::mem::take(text));
                }
                close = true;
            }
        } else {
            let hovered = ui.rect_contains_pointer(rect);
            ui.painter().rect_stroke(rect.with_max_y(rect.min.y + 52.0), 12.0, Stroke::new(1.5, if hovered { pal.accent } else { pal.border }), StrokeKind::Inside);
            let resp = ui.interact(rect.with_max_y(rect.min.y + 52.0), Id::unique("add_column"), Sense::click());
            ui.painter().text(
                Pos2::new(rect.center().x, rect.min.y + 26.0),
                Align2::CENTER_CENTER,
                "+  Add column",
                FontId::proportional(14.0),
                if hovered { pal.accent } else { pal.weak },
            );
            if resp.clicked() {
                self.adding_column = Some(String::new());
            }
            if hovered {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        }
        if let Some(name) = submit {
            self.actions.push(Action::AddColumn(name));
        }
        if close {
            self.adding_column = None;
        }
        let _ = PALETTE;
    }

    /// While a card is dragged: show where it would land, and move it on release.
    fn handle_drag(&mut self, ui: &mut egui::Ui, project: &Project, pal: &Pal) {
        let Some(drag) = self.drag.clone() else { return };
        let ctx = ui.ctx().clone();
        let Some(task) = project.task(drag.task) else {
            self.drag = None;
            return;
        };
        let pointer = ctx.pointer_latest_pos();
        let released = ctx.input(|i| !i.pointer.primary_down());
        ctx.set_cursor_icon(CursorIcon::Grabbing);

        // Where would it land?
        let mut target: Option<(u64, Option<u64>, f32, Rect)> = None;
        if let Some(p) = pointer {
            if let Some((cid, crect)) = self.col_rects.iter().find(|(_, r)| r.x_range().contains(p.x) && p.y >= r.min.y - 40.0 && p.y <= r.max.y + 40.0) {
                let cards: Vec<&(u64, u64, Rect)> = self.card_rects.iter().filter(|(c, t, _)| c == cid && *t != drag.task).collect();
                let before = cards.iter().find(|(_, _, r)| r.center().y > p.y);
                let line_y = match before {
                    Some((_, _, r)) => r.min.y - 4.0,
                    None => cards.last().map(|(_, _, r)| r.max.y + 4.0).unwrap_or(crect.min.y + 52.0),
                };
                target = Some((*cid, before.map(|(_, t, _)| *t), line_y, *crect));
            }
        }

        let layer = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::unique("drag_layer")));
        if let Some((_, _, y, crect)) = target {
            let x0 = crect.min.x + 12.0;
            let x1 = crect.max.x - 12.0;
            layer.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(3.0, pal.accent));
            layer.circle_filled(Pos2::new(x0, y), 4.0, pal.accent);
        }
        if let Some(p) = pointer {
            let r = Rect::from_min_size(p - drag.offset, drag.size);
            let col_done = project.locate(task.id).map(|(c, _)| project.columns[c].is_done).unwrap_or(false);
            layer.rect_filled(r.translate(Vec2::new(4.0, 8.0)), 10.0, Color32::from_black_alpha(if pal.dark { 90 } else { 30 }));
            widgets::paint_card(&layer, r, task, project, col_done, model::today(), pal, true, false);
        }

        if released {
            if let Some((col, before, _, _)) = target {
                self.actions.push(Action::MoveTask { task: drag.task, col, before });
            }
            self.drag = None;
        } else {
            ctx.request_repaint();
        }
    }
}
