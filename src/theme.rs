//! Colours and widget styling for the light and dark themes.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Shadow, Stroke, TextStyle, Theme, Visuals,
};

#[derive(Clone, Copy)]
pub struct Pal {
    pub dark: bool,
    pub bg: Color32,
    pub sidebar: Color32,
    pub header: Color32,
    pub hover: Color32,
    pub column: Color32,
    pub card: Color32,
    pub border: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
    pub warn: Color32,
    pub success: Color32,
    pub shadow: Color32,
}

pub fn pal(dark: bool) -> Pal {
    if dark {
        Pal {
            dark,
            bg: Color32::from_rgb(20, 22, 29),
            sidebar: Color32::from_rgb(26, 28, 37),
            header: Color32::from_rgb(26, 28, 37),
            hover: Color32::from_rgb(38, 41, 53),
            column: Color32::from_rgb(31, 34, 44),
            card: Color32::from_rgb(42, 46, 58),
            border: Color32::from_rgb(54, 58, 72),
            text: Color32::from_rgb(230, 232, 240),
            weak: Color32::from_rgb(150, 156, 176),
            accent: Color32::from_rgb(129, 132, 245),
            accent_soft: Color32::from_rgb(50, 52, 92),
            danger: Color32::from_rgb(248, 113, 113),
            warn: Color32::from_rgb(251, 191, 36),
            success: Color32::from_rgb(74, 222, 128),
            shadow: Color32::from_black_alpha(90),
        }
    } else {
        Pal {
            dark,
            bg: Color32::from_rgb(243, 244, 248),
            sidebar: Color32::from_rgb(255, 255, 255),
            header: Color32::from_rgb(255, 255, 255),
            hover: Color32::from_rgb(238, 240, 246),
            column: Color32::from_rgb(232, 235, 242),
            card: Color32::from_rgb(255, 255, 255),
            border: Color32::from_rgb(220, 224, 233),
            text: Color32::from_rgb(30, 34, 50),
            weak: Color32::from_rgb(108, 114, 134),
            accent: Color32::from_rgb(91, 95, 227),
            accent_soft: Color32::from_rgb(228, 229, 252),
            danger: Color32::from_rgb(220, 38, 38),
            warn: Color32::from_rgb(217, 119, 6),
            success: Color32::from_rgb(22, 163, 74),
            shadow: Color32::from_black_alpha(18),
        }
    }
}

fn visuals(p: &Pal) -> Visuals {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    let r = CornerRadius::same(6);
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if p.dark { 120 } else { 40 }) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(if p.dark { 110 } else { 30 }) };
    v.extreme_bg_color = if p.dark { Color32::from_rgb(17, 19, 25) } else { Color32::WHITE };
    v.text_edit_bg_color = Some(v.extreme_bg_color);
    v.faint_bg_color = p.hover;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = if p.dark { Color32::from_rgb(70, 72, 140) } else { Color32::from_rgb(214, 216, 251) };
    v.selection.stroke = Stroke::new(1.0, if p.dark { Color32::WHITE } else { Color32::from_rgb(55, 58, 190) });

    let (inactive, hovered, active) = if p.dark {
        (Color32::from_rgb(44, 48, 61), Color32::from_rgb(54, 59, 75), Color32::from_rgb(64, 69, 88))
    } else {
        (Color32::from_rgb(236, 238, 244), Color32::from_rgb(225, 228, 237), Color32::from_rgb(212, 216, 228))
    };
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.weak_bg_fill = inactive;
    v.widgets.inactive.bg_fill = inactive;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.corner_radius = r;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.weak_bg_fill = hovered;
    v.widgets.hovered.bg_fill = hovered;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, p.text);
    v.widgets.active.weak_bg_fill = active;
    v.widgets.active.bg_fill = active;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.corner_radius = r;
    v.widgets.active.fg_stroke = Stroke::new(1.5, p.text);
    v.widgets.open.weak_bg_fill = hovered;
    v.widgets.open.bg_fill = hovered;
    v.widgets.open.corner_radius = r;
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    v
}

pub fn install(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals_of(Theme::Light, visuals(&pal(false)));
    ctx.set_visuals_of(Theme::Dark, visuals(&pal(true)));
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(21.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.menu_margin = egui::Margin::same(8);
        s.spacing.window_margin = egui::Margin::same(16);
        // Desktop-style scroll bars that stay visible, so it's clear when there's more to see.
        s.spacing.scroll = egui::style::ScrollStyle::thin();
    });
    set_dark(ctx, dark);
}

pub fn set_dark(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark { Theme::Dark } else { Theme::Light });
}

/// A softened version of a colour for chips and badges.
pub fn soft(c: Color32, p: &Pal) -> Color32 {
    let base = if p.dark { p.card } else { Color32::WHITE };
    let a = if p.dark { 0.28 } else { 0.16 };
    lerp(base, c, a)
}

/// Text colour to put on a soft chip of this colour.
pub fn on_soft(c: Color32, p: &Pal) -> Color32 {
    if p.dark {
        lerp(c, Color32::WHITE, 0.35)
    } else {
        lerp(c, Color32::BLACK, 0.25)
    }
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// A stable colour for a person's avatar, from their name.
pub fn person_color(name: &str) -> Color32 {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let colors = [
        Color32::from_rgb(99, 102, 241),
        Color32::from_rgb(14, 165, 233),
        Color32::from_rgb(20, 184, 166),
        Color32::from_rgb(234, 88, 12),
        Color32::from_rgb(219, 39, 119),
        Color32::from_rgb(124, 58, 237),
        Color32::from_rgb(22, 163, 74),
        Color32::from_rgb(202, 138, 4),
    ];
    colors[(h % colors.len() as u32) as usize]
}

pub fn initials(name: &str) -> String {
    let mut out = String::new();
    for part in name.split_whitespace().take(2) {
        if let Some(c) = part.chars().next() {
            out.extend(c.to_uppercase());
        }
    }
    if out.is_empty() {
        "?".into()
    } else {
        out
    }
}
