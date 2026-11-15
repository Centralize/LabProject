//! Small drawing helpers shared by the views: chips, avatars, progress bars,
//! colour pickers and the Kanban card itself.

use crate::model::{rgb, Priority, Project, Task, PALETTE};
use crate::theme::{initials, on_soft, person_color, soft, Pal};
use chrono::NaiveDate;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

pub fn short_date(d: NaiveDate, today: NaiveDate) -> String {
    let diff = (d - today).num_days();
    match diff {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        _ => {
            if d.format("%Y").to_string() == today.format("%Y").to_string() {
                d.format("%b %-d").to_string()
            } else {
                d.format("%b %-d, %Y").to_string()
            }
        }
    }
}

/// Colour for a due date: overdue, due soon or later.
pub fn due_color(d: NaiveDate, today: NaiveDate, done: bool, p: &Pal) -> Color32 {
    if done {
        p.success
    } else if d < today {
        p.danger
    } else if (d - today).num_days() <= 2 {
        p.warn
    } else {
        p.weak
    }
}

/// Draws a rounded "chip" with text and returns its rectangle.
pub fn paint_chip(painter: &Painter, pos: Pos2, text: &str, color: Color32, p: &Pal, font: f32) -> Rect {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(font), on_soft(color, p));
    let size = Vec2::new(galley.size().x + 12.0, galley.size().y + 4.0);
    let rect = Rect::from_min_size(pos, size);
    painter.rect_filled(rect, CornerRadius::same(5), soft(color, p));
    painter.galley(pos + Vec2::new(6.0, 2.0), galley, Color32::WHITE);
    rect
}

pub fn chip_width(painter: &Painter, text: &str, font: f32) -> f32 {
    painter.layout_no_wrap(text.to_string(), FontId::proportional(font), Color32::WHITE).size().x + 12.0
}

pub fn chip(ui: &mut Ui, text: &str, color: Color32, p: &Pal) -> Response {
    let w = chip_width(ui.painter(), text, 12.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 19.0), Sense::hover());
    paint_chip(ui.painter(), rect.min, text, color, p, 12.0);
    resp
}

pub fn paint_avatar(painter: &Painter, center: Pos2, radius: f32, name: &str) {
    painter.circle_filled(center, radius, person_color(name));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        initials(name),
        FontId::proportional(radius * 0.85),
        Color32::WHITE,
    );
}

pub fn avatar(ui: &mut Ui, name: &str, radius: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(radius * 2.0), Sense::hover());
    paint_avatar(ui.painter(), rect.center(), radius, name);
    resp.on_hover_text(name)
}

pub fn dot(ui: &mut Ui, color: Color32, radius: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(radius * 2.0 + 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius, color);
}

pub fn progress_bar(ui: &mut Ui, frac: f32, width: f32, height: f32, color: Color32, p: &Pal) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_progress(ui.painter(), rect, frac, color, p);
    resp
}

pub fn paint_progress(painter: &Painter, rect: Rect, frac: f32, color: Color32, p: &Pal) {
    let r = CornerRadius::same((rect.height() / 2.0).min(127.0) as u8);
    let track = if p.dark { Color32::from_rgb(52, 56, 70) } else { Color32::from_rgb(226, 229, 237) };
    painter.rect_filled(rect, r, track);
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        let mut fill = rect;
        fill.set_width((rect.width() * f).max(rect.height()));
        painter.rect_filled(fill, r, color);
    }
}

/// A small square button with three dots, for "more actions" menus.
pub fn dots_button(ui: &mut Ui, p: &Pal) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, if p.dark { Color32::from_rgb(56, 60, 76) } else { Color32::from_rgb(218, 222, 232) });
    }
    for i in -1..=1 {
        ui.painter().circle_filled(rect.center() + Vec2::new(i as f32 * 5.0, 0.0), 1.8, p.weak);
    }
    resp.on_hover_text("More actions")
}

/// A small "×" button drawn with two lines (crisper than a font glyph).
pub fn x_button(ui: &mut Ui, p: &Pal, size: f32, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 5.0, if p.dark { Color32::from_rgb(56, 60, 76) } else { Color32::from_rgb(222, 225, 234) });
    }
    let c = if resp.hovered() { p.text } else { p.weak };
    let r = size * 0.2;
    let m = rect.center();
    let stroke = Stroke::new(1.6, c);
    ui.painter().line_segment([m + Vec2::new(-r, -r), m + Vec2::new(r, r)], stroke);
    ui.painter().line_segment([m + Vec2::new(-r, r), m + Vec2::new(r, -r)], stroke);
    resp.on_hover_text(tip)
}

/// A row of colour swatches; returns true when a new colour was picked.
pub fn color_picker(ui: &mut Ui, value: &mut [u8; 3]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for c in PALETTE {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            let selected = *value == c;
            ui.painter().circle_filled(rect.center(), 9.0, rgb(c));
            if selected {
                ui.painter().circle_stroke(rect.center(), 10.5, Stroke::new(2.0, ui.visuals().text_color()));
            } else if resp.hovered() {
                ui.painter().circle_stroke(rect.center(), 10.5, Stroke::new(1.0, ui.visuals().weak_text_color()));
            }
            if resp.clicked() && !selected {
                *value = c;
                changed = true;
            }
        }
    });
    changed
}

pub fn priority_chip(ui: &mut Ui, pr: Priority, p: &Pal) -> Response {
    chip(ui, pr.name(), pr.color(), p)
}

/// A section heading in weak caps.
pub fn section(ui: &mut Ui, text: &str, p: &Pal) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).color(p.weak).strong());
}

// ---------------------------------------------------------------------------
// The Kanban card

const PAD: f32 = 12.0;

fn card_title(task: &Task) -> String {
    if task.title.trim().is_empty() {
        "Untitled task".into()
    } else {
        task.title.clone()
    }
}

pub fn card_height(painter: &Painter, task: &Task, width: f32) -> f32 {
    let title = painter.layout(card_title(task), FontId::proportional(14.5), Color32::WHITE, width - PAD * 2.0 - 4.0);
    let mut h = PAD + title.size().y + 10.0 + 22.0 + PAD;
    if !task.labels.is_empty() {
        h += 24.0;
    }
    h
}

/// Paints a whole card into `rect`.
#[allow(clippy::too_many_arguments)]
pub fn paint_card(
    painter: &Painter,
    rect: Rect,
    task: &Task,
    project: &Project,
    in_done: bool,
    today: NaiveDate,
    p: &Pal,
    selected: bool,
    hovered: bool,
) {
    let r = CornerRadius::same(10);
    // shadow and body
    painter.rect_filled(rect.translate(Vec2::new(0.0, if hovered { 3.0 } else { 1.5 })), r, p.shadow);
    painter.rect_filled(rect, r, p.card);
    let border = if selected {
        Stroke::new(2.0, p.accent)
    } else if hovered {
        Stroke::new(1.0, crate::theme::lerp(p.border, p.accent, 0.45))
    } else {
        Stroke::new(1.0, p.border)
    };
    painter.rect_stroke(rect, r, border, StrokeKind::Inside);
    // priority strip on the left edge
    let strip = Rect::from_min_max(rect.min + Vec2::new(0.0, 0.0), Pos2::new(rect.min.x + 4.0, rect.max.y));
    painter.rect_filled(strip, CornerRadius { nw: 10, sw: 10, ne: 0, se: 0 }, task.priority.color());

    let left = rect.min.x + PAD + 2.0;
    let right = rect.max.x - PAD;
    let mut y = rect.min.y + PAD;

    // labels
    if !task.labels.is_empty() {
        let mut x = left;
        let labels: Vec<_> = task.labels.iter().filter_map(|l| project.label(*l)).collect();
        for (i, l) in labels.iter().enumerate() {
            let w = chip_width(painter, &l.name, 11.0);
            if x + w > right && i > 0 {
                let more = format!("+{}", labels.len() - i);
                if x + chip_width(painter, &more, 11.0) <= right + 2.0 {
                    paint_chip(painter, Pos2::new(x, y), &more, p.weak, p, 11.0);
                }
                break;
            }
            let rr = paint_chip(painter, Pos2::new(x, y), &l.name, rgb(l.color), p, 11.0);
            x = rr.max.x + 5.0;
        }
        y += 24.0;
    }

    // title
    let title_color = if in_done { p.weak } else { p.text };
    let title = painter.layout(card_title(task), FontId::proportional(14.5), title_color, right - left - 2.0);
    let th = title.size().y;
    painter.galley(Pos2::new(left, y), title, title_color);
    y += th + 10.0;

    // meta row
    let mid = y + 11.0;
    let mut x = left;
    let small = FontId::proportional(12.0);
    let pr = paint_chip(painter, Pos2::new(x, mid - 9.5), task.priority.name(), task.priority.color(), p, 11.5);
    x = pr.max.x + 8.0;
    if let Some(d) = task.due {
        let c = due_color(d, today, in_done, p);
        let txt = if in_done { format!("✔ {}", short_date(d, today)) } else { format!("📅 {}", short_date(d, today)) };
        let r = painter.text(Pos2::new(x, mid), Align2::LEFT_CENTER, txt, small.clone(), c);
        x = r.max.x + 10.0;
    }
    let (done, total) = task.checklist_progress();
    if total > 0 {
        let c = if done == total { p.success } else { p.weak };
        let r = painter.text(Pos2::new(x, mid), Align2::LEFT_CENTER, format!("☑ {done}/{total}"), small.clone(), c);
        x = r.max.x + 10.0;
    }
    if !task.comments.is_empty() {
        let r = painter.text(Pos2::new(x, mid), Align2::LEFT_CENTER, format!("💬 {}", task.comments.len()), small.clone(), p.weak);
        x = r.max.x + 10.0;
    }
    let avatar_space = if task.assignee.is_empty() { 0.0 } else { 26.0 };
    if task.points > 0 {
        let txt = format!("{} pt", task.points);
        let gw = chip_width(painter, &txt, 11.0);
        let start = right - avatar_space - gw - 4.0;
        if x - 4.0 <= start {
            paint_chip(painter, Pos2::new(start, mid - 9.0), &txt, p.weak, p, 11.0);
        }
    }
    if !task.assignee.is_empty() {
        paint_avatar(painter, Pos2::new(right - 11.0, mid), 11.0, &task.assignee);
    }
}
