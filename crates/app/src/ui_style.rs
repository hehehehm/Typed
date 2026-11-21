//! Flat geometric look of the settings window: five equal colour squares in one
//! row on the home screen, and the same square vocabulary inside the sections —
//! flat blocks, square switches, tile buttons, no rounded corners, minimal text.
//!
//! Layout rule: no widget asks for more width than `ui.available_width()`. Long
//! text is truncated with the full text on hover, so content never widens the window.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;

/// Section colours (same in both themes; text and icons on them are `INK_ON_COLOR`).
pub const YELLOW: Color32 = Color32::from_rgb(0xE8, 0xC4, 0x68); // Программы
pub const ORANGE: Color32 = Color32::from_rgb(0xEE, 0x9F, 0x5E); // Словарь
pub const LIME: Color32 = Color32::from_rgb(0xA9, 0xC6, 0x6A);   // Сейф
pub const CYAN: Color32 = Color32::from_rgb(0x6D, 0xB7, 0xC8);   // Настройки
pub const RED: Color32 = Color32::from_rgb(0xDE, 0x6B, 0x5A);    // Поддержать
pub const INK_ON_COLOR: Color32 = Color32::from_rgb(0x23, 0x26, 0x2B);

pub const PAD: f32 = 16.0;
pub const GAP: f32 = 8.0;

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub field: Color32,
    pub line: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub warn: Color32,
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            dark,
            bg: Color32::from_rgb(0x1C, 0x1E, 0x22),
            surface: Color32::from_rgb(0x27, 0x2A, 0x30),
            surface_hover: Color32::from_rgb(0x32, 0x36, 0x3D),
            field: Color32::from_rgb(0x15, 0x17, 0x1A),
            line: Color32::from_rgb(0x3A, 0x3E, 0x45),
            ink: Color32::from_rgb(0xEC, 0xEA, 0xE5),
            muted: Color32::from_rgb(0x9E, 0xA2, 0xA8),
            warn: Color32::from_rgb(0xE8, 0xC4, 0x48),
        }
    } else {
        Theme {
            dark,
            bg: Color32::from_rgb(0xF7, 0xF6, 0xF2),
            surface: Color32::from_rgb(0xEC, 0xEA, 0xE4),
            surface_hover: Color32::from_rgb(0xE1, 0xDE, 0xD6),
            field: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            line: Color32::from_rgb(0xD6, 0xD3, 0xCB),
            ink: Color32::from_rgb(0x23, 0x26, 0x2B),
            muted: Color32::from_rgb(0x6E, 0x71, 0x78),
            warn: Color32::from_rgb(0x94, 0x5A, 0x00),
        }
    }
}

pub fn cur(ui: &Ui) -> Theme {
    theme(ui.visuals().dark_mode)
}

pub fn shade(c: Color32, f: f32) -> Color32 {
    let t = if f < 0.0 { 0.0 } else { 255.0 };
    let k = f.abs().clamp(0.0, 1.0);
    let m = |v: u8| (v as f32 + (t - v as f32) * k).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Proportional text falls back to Hack for symbols Ubuntu-Light lacks (→, ←). Call once.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        if !list.iter().any(|f| f == "Hack") {
            list.push("Hack".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, dark: bool, accent: Color32) {
    let t = theme(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = t.bg;
    v.window_fill = t.bg;
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.surface;
    v.code_bg_color = t.surface;
    v.window_rounding = egui::Rounding::ZERO;
    v.menu_rounding = egui::Rounding::ZERO;
    v.window_stroke = Stroke::new(1.0, t.line);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.hyperlink_color = shade(accent, -0.35);
    v.selection.bg_fill = accent.gamma_multiply(if dark { 0.55 } else { 0.6 });
    v.selection.stroke = Stroke::new(1.5, if dark { accent } else { shade(accent, -0.25) });
    v.override_text_color = None;
    v.warn_fg_color = t.warn;
    for (w, fill) in [
        (&mut v.widgets.noninteractive, t.bg),
        (&mut v.widgets.inactive, t.surface),
        (&mut v.widgets.hovered, t.surface_hover),
        (&mut v.widgets.active, shade(t.surface_hover, if dark { 0.06 } else { -0.06 })),
        (&mut v.widgets.open, t.surface_hover),
    ] {
        w.rounding = egui::Rounding::ZERO;
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.expansion = 0.0;
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = Stroke::new(1.0, t.ink);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.ink);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.ink);
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        use egui::{FontFamily::*, TextStyle};
        s.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, Proportional)),
            (TextStyle::Body, FontId::new(14.0, Proportional)),
            (TextStyle::Button, FontId::new(14.0, Proportional)),
            (TextStyle::Small, FontId::new(11.5, Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, Monospace)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(GAP, GAP);
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
        s.spacing.interact_size = egui::vec2(30.0, 28.0);
        s.spacing.icon_width = 16.0;
        s.spacing.icon_width_inner = 9.0;
        s.spacing.window_margin = egui::Margin::same(PAD);
        let mut sc = egui::style::ScrollStyle::solid();
        sc.bar_width = 6.0;
        sc.bar_inner_margin = 2.0;
        s.spacing.scroll = sc;
    });
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn line_galley(ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(8.0));
    ui.fonts(|f| f.layout_job(job))
}

pub fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x)
}

pub fn warn(ui: &mut Ui, text: &str) {
    let c = cur(ui).warn;
    ui.add(egui::Label::new(egui::RichText::new(text).color(c)).wrap());
}

pub fn caption(ui: &mut Ui, text: &str, tip: &str) {
    let c = cur(ui).muted;
    let r = ui.add(egui::Label::new(egui::RichText::new(text.to_uppercase()).size(11.0).color(c).extra_letter_spacing(0.8)).truncate());
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
}

pub fn block<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = cur(ui);
    egui::Frame::none()
        .fill(t.surface)
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn tile(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> egui::Response {
    let f = font(13.5);
    let pad = 10.0;
    let natural = text_w(ui, text, f.clone()) + pad * 2.0;
    let w = natural.min(ui.available_width().max(36.0));
    let h = 28.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let mut bg = fill;
        if resp.is_pointer_button_down_on() {
            bg = shade(fill, -0.12);
        } else if resp.hovered() {
            bg = shade(fill, if cur(ui).dark && fill != CYAN { 0.06 } else { -0.06 });
        }
        ui.painter().rect_filled(rect, 0.0, bg);
        let g = line_galley(ui, text, f, fg, w - pad * 2.0);
        let pos = egui::pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0);
        ui.painter().galley(pos, g, fg);
    }
    if natural > w {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

pub fn primary(ui: &mut Ui, text: &str, accent: Color32) -> egui::Response {
    tile(ui, text, accent, INK_ON_COLOR)
}

pub fn secondary(ui: &mut Ui, text: &str) -> egui::Response {
    let t = cur(ui);
    tile(ui, text, t.surface_hover, t.ink)
}

/// Square 28x28 `+` button in the section accent colour.
pub fn plus_square(ui: &mut Ui, accent: Color32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if resp.is_pointer_button_down_on() {
            shade(accent, -0.14)
        } else if resp.hovered() {
            shade(accent, -0.06)
        } else {
            accent
        };
        let p = ui.painter();
        p.rect_filled(rect, 0.0, bg);
        let c = rect.center();
        let s = Stroke::new(1.8, INK_ON_COLOR);
        p.line_segment([egui::pos2(c.x - 5.5, c.y), egui::pos2(c.x + 5.5, c.y)], s);
        p.line_segment([egui::pos2(c.x, c.y - 5.5), egui::pos2(c.x, c.y + 5.5)], s);
    }
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

fn paint_switch(ui: &Ui, rect: Rect, on: bool, accent: Color32, hovered: bool) {
    let t = cur(ui);
    let p = ui.painter();
    let track = if on {
        accent
    } else if hovered {
        t.line
    } else {
        shade(t.line, if t.dark { -0.1 } else { 0.0 })
    };
    p.rect_filled(rect, 0.0, track);
    let k = rect.height() - 6.0;
    let x = if on { rect.right() - 3.0 - k } else { rect.left() + 3.0 };
    let knob = Rect::from_min_size(egui::pos2(x, rect.top() + 3.0), egui::vec2(k, k));
    let kc = if on { INK_ON_COLOR } else if t.dark { t.muted } else { Color32::WHITE };
    p.rect_filled(knob, 0.0, kc);
}

/// Compact full-width toggle row with a square switch and optional hover tooltip.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, title: &str, tip: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered()) && ui.is_enabled();
    let fill = if hovered { t.surface_hover } else { t.surface };
    let sw = egui::vec2(36.0, 20.0);
    let frame = egui::Frame::none().fill(fill).inner_margin(egui::Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let text_w = (ui.available_width() - sw.x - 10.0).max(40.0);
            ui.set_width(text_w);
            ui.add(egui::Label::new(egui::RichText::new(title).color(t.ink)).truncate());
        });
    });
    let rect = frame.response.rect;
    let resp = ui.interact(rect, id, Sense::click());
    let sw_rect = Rect::from_min_size(egui::pos2(rect.right() - 10.0 - sw.x, rect.center().y - sw.y / 2.0), sw);
    paint_switch(ui, sw_rect, *on, accent, resp.hovered());
    let clicked = resp.clicked();
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    if clicked {
        *on = !*on;
        true
    } else {
        false
    }
}

pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str, &str)], accent: Color32) -> bool {
    let n = options.len().max(1) as f32;
    let gap = 4.0;
    let w = ((ui.available_width() - gap * (n - 1.0)) / n).max(24.0);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for &(v, name, tip) in options {
            let r = choice_tile(ui, *value == v, name, w, accent);
            let clicked = r.clicked();
            if !tip.is_empty() {
                r.on_hover_text(tip);
            }
            if clicked && *value != v {
                *value = v;
                changed = true;
            }
        }
    });
    changed
}

pub fn choice_tile(ui: &mut Ui, selected: bool, text: &str, w: f32, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font(13.0);
    let natural = text_w(ui, text, f.clone()) + 16.0;
    let w = if w > 0.0 { w } else { natural }.min(ui.available_width().max(24.0));
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let base = if t.dark { t.surface_hover } else { Color32::from_rgb(0xE1, 0xDE, 0xD6) };
        let bg = if selected {
            accent
        } else if resp.hovered() {
            shade(base, if t.dark { 0.06 } else { -0.05 })
        } else {
            base
        };
        ui.painter().rect_filled(rect, 0.0, bg);
        let fg = if selected { INK_ON_COLOR } else { t.ink };
        let g = line_galley(ui, text, f, fg, w - 8.0);
        ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
    }
    if natural > w {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

pub fn field(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, password: bool, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let w = width.min(ui.available_width()).max(40.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 28.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.field);
    let resp = ui.put(
        rect,
        egui::TextEdit::singleline(text)
            .hint_text(hint_text)
            .password(password)
            .frame(false)
            .margin(egui::vec2(8.0, 5.0))
            .desired_width(w - 16.0)
            .vertical_align(egui::Align::Center),
    );
    let stroke = if resp.has_focus() { Stroke::new(1.5, if t.dark { accent } else { shade(accent, -0.2) }) } else { Stroke::new(1.0, t.line) };
    ui.painter().rect_stroke(rect.shrink(0.5), 0.0, stroke);
    resp
}

/// Field + square `+` button on one line.
pub fn add_line_plus(ui: &mut Ui, text: &mut String, hint_text: &str, tip: &str, accent: Color32) -> bool {
    let mut go = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let fw = (ui.available_width() - 28.0 - 6.0).max(60.0);
        let r = field(ui, text, hint_text, fw, false, accent);
        go |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        go |= plus_square(ui, accent, tip).clicked();
    });
    go
}

pub fn field_bg(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, accent: Color32) -> egui::Response {
    field(ui, text, hint_text, width, false, accent)
}

pub fn password_bg(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, accent: Color32) -> egui::Response {
    field(ui, text, hint_text, width, true, accent)
}

pub fn row(ui: &mut Ui, fill: Color32, label: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) -> egui::Response {
    let margin = if fill == Color32::TRANSPARENT { egui::Margin::ZERO } else { egui::Margin::symmetric(10.0, 4.0) };
    egui::Frame::none()
        .fill(fill)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    actions(ui);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_max_width(ui.available_width());
                        label(ui);
                    });
                });
            });
        })
        .response
}

pub fn chip(ui: &mut Ui, word: &str, tag: Option<&str>) -> bool {
    let t = cur(ui);
    let wf = FontId::monospace(13.0);
    let tf = font(10.5);
    let word_w = text_w(ui, word, wf.clone());
    let tag_w = tag.map_or(0.0, |s| text_w(ui, s, tf.clone()) + 6.0);
    let icon = 22.0;
    let pad = 8.0;
    let fixed = pad + tag_w + 4.0 + icon + 2.0;
    let max_w = ui.max_rect().width().max(fixed + 40.0);
    let shown = word_w.min(max_w - fixed);
    let width = fixed + shown;
    let h = 26.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, t.surface);
    let g = line_galley(ui, word, wf, t.ink, shown + 0.5);
    p.galley(egui::pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0), g, t.ink);
    if let Some(s) = tag {
        p.text(egui::pos2(rect.left() + pad + shown + 6.0, rect.center().y), Align2::LEFT_CENTER, s, tf, t.muted);
    }
    if shown < word_w {
        resp.on_hover_text(word);
    }
    let irect = Rect::from_min_size(egui::pos2(rect.right() - icon - 2.0, rect.top() + (h - icon) / 2.0), Vec2::splat(icon));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(irect));
    crate::icons::delete_icon_button(&mut child).clicked()
}

pub fn check(ui: &mut Ui, on: &mut bool, text: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let f = font(13.5);
    let w = (18.0 + 8.0 + text_w(ui, text, f.clone())).min(ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 28.0), Sense::click());
    let b = Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 9.0), Vec2::splat(18.0));
    let p = ui.painter();
    if *on {
        p.rect_filled(b, 0.0, accent);
        p.rect_filled(b.shrink(5.0), 0.0, INK_ON_COLOR);
    } else {
        p.rect_stroke(b.shrink(0.75), 0.0, Stroke::new(1.5, if resp.hovered() { t.ink } else { t.muted }));
    }
    let g = line_galley(ui, text, f, t.ink, w - 26.0);
    p.galley(egui::pos2(b.right() + 8.0, rect.center().y - g.size().y / 2.0), g, t.ink);
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

pub fn back_square(ui: &mut Ui, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let bg = if resp.hovered() { shade(color, -0.22) } else { shade(color, -0.12) };
    let p = ui.painter();
    p.rect_filled(rect, 0.0, bg);
    let c = rect.center();
    let s = Stroke::new(2.0, INK_ON_COLOR);
    p.line_segment([egui::pos2(c.x - 6.0, c.y), egui::pos2(c.x + 6.0, c.y)], s);
    p.line_segment([egui::pos2(c.x - 6.0, c.y), egui::pos2(c.x - 1.5, c.y - 4.5)], s);
    p.line_segment([egui::pos2(c.x - 6.0, c.y), egui::pos2(c.x - 1.5, c.y + 4.5)], s);
    resp.on_hover_text("Назад (Esc)")
}

pub fn group_header(ui: &mut Ui, open: bool, title: &str, summary: &str, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let w = ui.available_width();
    let h = 38.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if resp.hovered() { t.surface_hover } else { t.surface });
    let sq = if open { 12.0 } else { 9.0 };
    let sq_rect = Rect::from_center_size(egui::pos2(rect.left() + 18.0, rect.center().y), Vec2::splat(sq));
    p.rect_filled(sq_rect, 0.0, accent);
    let x0 = rect.left() + 34.0;
    let sx = rect.right() - 18.0;
    let s = Stroke::new(1.5, t.ink);
    p.line_segment([egui::pos2(sx - 4.5, rect.center().y), egui::pos2(sx + 4.5, rect.center().y)], s);
    if !open {
        p.line_segment([egui::pos2(sx, rect.center().y - 4.5), egui::pos2(sx, rect.center().y + 4.5)], s);
    }
    let avail = (sx - 14.0 - x0).max(20.0);
    let tg = line_galley(ui, title, font(14.5), t.ink, avail);
    let title_w = tg.size().x;
    p.galley(egui::pos2(x0, rect.center().y - tg.size().y / 2.0), tg, t.ink);
    if !summary.is_empty() && avail - title_w > 36.0 {
        let sg = line_galley(ui, summary, font(12.0), t.muted, avail - title_w - 14.0);
        p.galley(egui::pos2(sx - 14.0 - sg.size().x, rect.center().y - sg.size().y / 2.0), sg, t.muted);
    }
    resp
}

/// Crisp geometric symbol painted inside each of the 5 home squares (`idx` 0..5).
pub fn paint_home_icon(p: &egui::Painter, c: egui::Pos2, u: f32, idx: usize) {
    let fg = INK_ON_COLOR;
    let s = Stroke::new((u * 0.14).max(1.5), fg);
    match idx {
        // 0: 2x2 squares (Программы)
        0 => {
            let d = u * 0.42;
            let sz = Vec2::splat(u * 0.56);
            for (dx, dy) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
                p.rect_filled(Rect::from_center_size(c + egui::vec2(dx, dy), sz), 0.0, fg);
            }
        }
        // 1: 3 horizontal bars (Словарь)
        1 => {
            for (dy, w) in [(-0.55, 1.4), (0.0, 1.05), (0.55, 1.25)] {
                let r = Rect::from_center_size(c + egui::vec2((w - 1.4) * u * 0.5, dy * u), egui::vec2(u * w, u * 0.22));
                p.rect_filled(r, 0.0, fg);
            }
        }
        // 2: Square padlock (Сейф)
        2 => {
            let body = Rect::from_center_size(c + egui::vec2(0.0, u * 0.25), egui::vec2(u * 1.2, u * 0.9));
            p.rect_filled(body, 0.0, fg);
            let shackle = Rect::from_min_max(
                c + egui::vec2(-u * 0.36, -u * 0.72),
                c + egui::vec2(u * 0.36, -u * 0.15),
            );
            p.rect_stroke(shackle, 0.0, s);
        }
        // 3: 2 slider tracks with square thumbs (Настройки)
        3 => {
            for (dy, kx) in [(-0.36, 0.26), (0.36, -0.26)] {
                let y = c.y + dy * u;
                p.line_segment([egui::pos2(c.x - u * 0.75, y), egui::pos2(c.x + u * 0.75, y)], s);
                p.rect_filled(Rect::from_center_size(egui::pos2(c.x + kx * u, y), Vec2::splat(u * 0.42)), 0.0, fg);
            }
        }
        // 4: Geometric pixel heart (Поддержать)
        _ => {
            let px = (u * 0.24).max(2.0);
            let cells: [(i8, i8); 15] = [
                (-2, -1), (-1, -1), (1, -1), (2, -1),
                (-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0),
                (-1, 1), (0, 1), (1, 1),
                (-1, 2), (0, 2), (1, 2),
            ];
            let _ = cells;
            // Clean 5-col pixel heart:
            let rows: [(i8, &[i8]); 4] = [
                (-2, &[-2, -1, 1, 2]),
                (-1, &[-2, -1, 0, 1, 2]),
                (0, &[-1, 0, 1]),
                (1, &[0]),
            ];
            for (ry, cols) in rows {
                for &rx in cols {
                    let center = c + egui::vec2(rx as f32 * px, (ry as f32 + 0.3) * px);
                    p.rect_filled(Rect::from_center_size(center, Vec2::splat(px + 0.5)), 0.0, fg);
                }
            }
        }
    }
}
