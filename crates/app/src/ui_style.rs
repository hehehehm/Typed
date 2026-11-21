//! Modular geometric look of the settings window.
//!
//! One base unit `U` = 4 px. Every size, gap and padding is an integer multiple
//! of `U`. Gutters are always `GAP` (= `U`). Interactive targets stay
//! `HIT` = 32 px (8`U`). Rows and headers are `ROW` = 40 px (10`U` = hit + two
//! gutters). Surfaces are monochrome; the five section colours are the only
//! accents. Hairlines are 1 px and only used where structure (a field, a focused
//! widget, a segmented cell) needs an edge.
//!
//! Layout rule: no widget asks for more width than `ui.available_width()`.
//! Long text is truncated with the full text on hover.

use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;

/// Base module, in logical pixels.
pub const U: f32 = 4.0;
/// Identical gutter everywhere.
pub const GAP: f32 = U;
/// Window and section padding (2U).
pub const PAD: f32 = U * 2.0;
/// Minimum hit target (8U), kept at 32 px for accessibility.
pub const HIT: f32 = U * 8.0;
/// List row / header / toggle row height (10U = 4 + 32 + 4).
pub const ROW: f32 = HIT + GAP * 2.0;
/// Visual marks, expanders, nav chips and plus/back arms (4U = 16 px).
pub const MARK: f32 = U * 4.0;
/// DIN-rail breaker housing (6U × 8U), centred in a 32×32 hit target.
pub const BREAKER_W: f32 = U * 6.0;
pub const BREAKER_H: f32 = HIT;
/// 1 px structural hairline.
pub const HAIR: f32 = 1.0;

/// Type scale: three sizes, one ratio 4:3 (12 : 16 : 21⅓).
pub const FONT_SM: f32 = 12.0;
pub const FONT_MD: f32 = FONT_SM * 4.0 / 3.0;
pub const FONT_LG: f32 = FONT_MD * 4.0 / 3.0;

/// Section colours (same in both themes; ink on them is `INK_ON_COLOR`).
pub const YELLOW: Color32 = Color32::from_rgb(0xE8, 0xC4, 0x68); // Программы
pub const ORANGE: Color32 = Color32::from_rgb(0xEE, 0x9F, 0x5E); // Словарь
pub const LIME: Color32 = Color32::from_rgb(0xA9, 0xC6, 0x6A); // Сейф
pub const CYAN: Color32 = Color32::from_rgb(0x6D, 0xB7, 0xC8); // Настройки
/// Slightly lifted from the reference red so ink stays clearly above 4.5:1.
pub const RED: Color32 = Color32::from_rgb(0xE4, 0x7A, 0x6A); // Поддержать
pub const INK_ON_COLOR: Color32 = Color32::from_rgb(0x23, 0x26, 0x2B);

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
            // Darker than the previous muted grey so text on `surface` stays ≥ 4.5:1.
            muted: Color32::from_rgb(0x5A, 0x5C, 0x62),
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

pub fn hair(color: Color32) -> Stroke {
    Stroke::new(HAIR, color)
}

/// Snap down to a multiple of `U` (never below `min`).
pub fn snap(v: f32, min: f32) -> f32 {
    let n = (v / U).floor() * U;
    n.max(min)
}

/// Proportional text falls back to Hack for symbols Ubuntu-Light lacks (→, ∅). Call once.
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
    v.window_stroke = hair(t.line);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.hyperlink_color = t.ink;
    v.selection.bg_fill = accent;
    v.selection.stroke = hair(INK_ON_COLOR);
    v.override_text_color = None;
    v.warn_fg_color = t.warn;
    for (w, fill) in [
        (&mut v.widgets.noninteractive, t.bg),
        (&mut v.widgets.inactive, t.surface),
        (&mut v.widgets.hovered, t.surface_hover),
        (&mut v.widgets.active, t.surface_hover),
        (&mut v.widgets.open, t.surface_hover),
    ] {
        w.rounding = egui::Rounding::ZERO;
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.expansion = 0.0;
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = hair(t.ink);
    }
    v.widgets.noninteractive.bg_stroke = hair(t.line);
    v.widgets.noninteractive.fg_stroke = hair(t.ink);
    v.widgets.inactive.fg_stroke = hair(t.ink);
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        use egui::{FontFamily::*, TextStyle};
        s.text_styles = [
            (TextStyle::Heading, FontId::new(FONT_LG, Proportional)),
            (TextStyle::Body, FontId::new(FONT_MD, Proportional)),
            (TextStyle::Button, FontId::new(FONT_MD, Proportional)),
            (TextStyle::Small, FontId::new(FONT_SM, Proportional)),
            (TextStyle::Monospace, FontId::new(FONT_MD, Monospace)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(GAP, GAP);
        s.spacing.button_padding = egui::vec2(GAP, GAP);
        s.spacing.interact_size = egui::vec2(HIT, HIT);
        s.spacing.icon_width = MARK;
        s.spacing.icon_width_inner = MARK / 2.0;
        s.spacing.window_margin = egui::Margin::same(PAD);
        let mut sc = egui::style::ScrollStyle::solid();
        sc.bar_width = U * 2.0;
        sc.bar_inner_margin = 0.0;
        s.spacing.scroll = sc;
    });
}

pub fn font_sm() -> FontId {
    FontId::proportional(FONT_SM)
}
pub fn font_md() -> FontId {
    FontId::proportional(FONT_MD)
}
pub fn font_lg() -> FontId {
    FontId::proportional(FONT_LG)
}
pub fn font_mono() -> FontId {
    FontId::monospace(FONT_MD)
}

/// Deprecated alias kept so older call sites compile during the pass; prefer `font_md`.
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn line_galley(ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(U));
    ui.fonts(|f| f.layout_job(job))
}

pub fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x)
}

fn focus_ring(ui: &Ui, rect: Rect, focused: bool) {
    if focused {
        ui.painter().rect_stroke(rect.shrink(0.5), 0.0, hair(cur(ui).ink));
    }
}

pub fn warn(ui: &mut Ui, text: &str) {
    let c = cur(ui).warn;
    ui.add(egui::Label::new(egui::RichText::new(text).color(c)).wrap());
}

pub fn caption(ui: &mut Ui, text: &str, tip: &str) {
    let c = cur(ui).muted;
    let r = ui.add(egui::Label::new(egui::RichText::new(text.to_uppercase()).size(FONT_SM).color(c).extra_letter_spacing(0.8)).truncate());
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
}

pub fn block<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = cur(ui);
    egui::Frame::none()
        .fill(t.surface)
        .inner_margin(egui::Margin::same(GAP))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn tile(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> egui::Response {
    let f = font_md();
    let natural = text_w(ui, text, f.clone()) + GAP * 2.0;
    let w = natural.min(ui.available_width().max(HIT)).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    if ui.is_rect_visible(rect) {
        let mut bg = fill;
        if resp.is_pointer_button_down_on() {
            bg = shade(fill, -0.12);
        } else if resp.hovered() {
            bg = shade(fill, if cur(ui).dark { 0.06 } else { -0.06 });
        }
        ui.painter().rect_filled(rect, 0.0, bg);
        let g = line_galley(ui, text, f, fg, w - GAP * 2.0);
        ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, rect, resp.has_focus());
    }
    let resp = if natural > w { resp.on_hover_text(text) } else { resp };
    resp
}

pub fn primary(ui: &mut Ui, text: &str, accent: Color32) -> egui::Response {
    tile(ui, text, accent, INK_ON_COLOR)
}

pub fn secondary(ui: &mut Ui, text: &str) -> egui::Response {
    let t = cur(ui);
    tile(ui, text, t.surface_hover, t.ink)
}

/// 32×32 `+` square in the section accent colour.
pub fn plus_square(ui: &mut Ui, accent: Color32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HIT), Sense::click());
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
        let s = hair(INK_ON_COLOR);
        let arm = MARK / 2.0;
        p.line_segment([egui::pos2(c.x - arm, c.y), egui::pos2(c.x + arm, c.y)], s);
        p.line_segment([egui::pos2(c.x, c.y - arm), egui::pos2(c.x, c.y + arm)], s);
        focus_ring(ui, rect, resp.has_focus());
    }
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// DIN-rail breaker: vertical housing, lever UP (accent, «I») when on,
/// DOWN (neutral grey, «O») when off. `rect` is the hit target (≥ 32×32).
fn paint_onoff(ui: &Ui, rect: Rect, on: bool, accent: Color32) {
    let t = cur(ui);
    let p = ui.painter();
    let w = BREAKER_W.min(rect.width());
    let h = BREAKER_H.min(rect.height()).max(MARK);
    let house = Rect::from_center_size(rect.center(), Vec2::new(w, h));
    p.rect_filled(house, 0.0, t.field);
    p.rect_stroke(house.shrink(0.5), 0.0, hair(t.line));
    let slot = Rect::from_center_size(house.center(), Vec2::new(w - U * 2.0, h - U * 2.0));
    p.rect_stroke(slot.shrink(0.5), 0.0, hair(t.line));
    let lever_h = (slot.height() / 2.0).max(U);
    let lever = if on {
        Rect::from_min_size(slot.min, Vec2::new(slot.width(), lever_h))
    } else {
        Rect::from_min_size(egui::pos2(slot.min.x, slot.max.y - lever_h), Vec2::new(slot.width(), lever_h))
    };
    let enabled = ui.is_enabled();
    // Off paddle is a mid-dark grey (≥ 3:1 vs the housing) so the down position
    // reads as a solid block in both themes; marks on it are light (≥ 4.5:1).
    let off_fill = if t.dark {
        Color32::from_rgb(0x6A, 0x6E, 0x76)
    } else {
        Color32::from_rgb(0x70, 0x6D, 0x68)
    };
    let light_mark = Color32::from_rgb(0xF7, 0xF6, 0xF2);
    let fill = if on && enabled { accent } else { off_fill };
    p.rect_filled(lever, 0.0, fill);
    let c = lever.center();
    if on {
        let mark = if enabled { INK_ON_COLOR } else { light_mark };
        p.rect_filled(Rect::from_center_size(c, Vec2::new(U, U * 2.0)), 0.0, mark);
    } else {
        p.rect_stroke(Rect::from_center_size(c, Vec2::new(U * 2.0, U * 2.0)).shrink(0.5), 0.0, hair(light_mark));
    }
}

fn state_tip(on: bool, tip: &str) -> String {
    let state = if on { "Вкл" } else { "Выкл" };
    if tip.is_empty() { state.into() } else { format!("{tip} · {state}") }
}

/// Full-width row, 10U tall, one-word label + DIN-rail breaker (32×32 hit).
pub fn toggle_row(ui: &mut Ui, on: &mut bool, title: &str, tip: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered()) && ui.is_enabled();
    let fill = if hovered { t.surface_hover } else { t.surface };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, fill);
    let resp = ui.interact(rect, id, Sense::click());
    let pad = GAP;
    let label_w = (rect.width() - pad * 3.0 - HIT).max(U);
    let g = line_galley(ui, title, font_md(), t.ink, label_w);
    ui.painter().galley(egui::pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0), g, t.ink);
    let box_r = Rect::from_center_size(egui::pos2(rect.right() - pad - HIT / 2.0, rect.center().y), Vec2::splat(HIT));
    paint_onoff(ui, box_r, *on, accent);
    focus_ring(ui, rect, resp.has_focus());
    let resp = resp.on_hover_text(state_tip(*on, tip));
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str, &str)], accent: Color32) -> bool {
    let n = options.len().max(1) as f32;
    let w = snap((ui.available_width() - GAP * (n - 1.0)) / n, HIT);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
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

/// Equal-width cells of a 3-way (or N-way) choice, height `HIT`, no gap (hairlines between).
pub fn segment_strip<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str, &str)], total_w: f32, accent: Color32) -> bool {
    let n = options.len().max(1);
    let w = (total_w / n as f32).max(HIT);
    let h = HIT;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w * n as f32, h), Sense::hover());
    let t = cur(ui);
    let p = ui.painter();
    let mut changed = false;
    for (i, &(v, name, tip)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(egui::pos2(rect.left() + i as f32 * w, rect.top()), egui::vec2(w, h));
        let resp = ui.interact(cell, ui.id().with(("seg", i, name)), Sense::click());
        let selected = *value == v;
        if selected {
            p.rect_filled(cell, 0.0, accent);
        } else if resp.hovered() {
            p.rect_filled(cell, 0.0, t.surface_hover);
        } else {
            p.rect_filled(cell, 0.0, t.surface);
        }
        if i > 0 && !selected {
            p.line_segment([cell.left_top(), cell.left_bottom()], hair(t.line));
        }
        let fg = if selected { INK_ON_COLOR } else { t.ink };
        let g = line_galley(ui, name, font_md(), fg, w - U);
        p.galley(cell.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, cell, resp.has_focus());
        let resp = resp.on_hover_text(if tip.is_empty() { name } else { tip });
        if resp.clicked() && *value != v {
            *value = v;
            changed = true;
        }
    }
    p.rect_stroke(rect.shrink(0.5), 0.0, hair(t.line));
    changed
}

pub fn choice_tile(ui: &mut Ui, selected: bool, text: &str, w: f32, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_md();
    let natural = text_w(ui, text, f.clone()) + GAP * 2.0;
    let w = if w > 0.0 { w } else { natural }.min(ui.available_width().max(HIT)).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if selected {
            accent
        } else if resp.hovered() {
            t.surface_hover
        } else {
            t.surface
        };
        ui.painter().rect_filled(rect, 0.0, bg);
        let fg = if selected { INK_ON_COLOR } else { t.ink };
        let g = line_galley(ui, text, f, fg, w - GAP);
        ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, rect, resp.has_focus());
    }
    if natural > w {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

pub fn field(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, password: bool, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let w = width.min(ui.available_width()).max(HIT);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.field);
    let resp = ui.put(
        rect,
        egui::TextEdit::singleline(text)
            .hint_text(hint_text)
            .password(password)
            .frame(false)
            .margin(egui::vec2(GAP, GAP))
            .desired_width(w - GAP * 2.0)
            .vertical_align(egui::Align::Center),
    );
    let stroke = if resp.has_focus() { hair(accent) } else { hair(t.line) };
    ui.painter().rect_stroke(rect.shrink(0.5), 0.0, stroke);
    resp
}

pub fn add_line_plus(ui: &mut Ui, text: &mut String, hint_text: &str, tip: &str, accent: Color32) -> bool {
    let mut go = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let fw = (ui.available_width() - HIT - GAP).max(HIT * 2.0);
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
    let margin = if fill == Color32::TRANSPARENT {
        egui::Margin::ZERO
    } else {
        egui::Margin::symmetric(GAP, GAP)
    };
    egui::Frame::none()
        .fill(fill)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(HIT);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
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
    let wf = font_mono();
    let tf = font_sm();
    let word_w = text_w(ui, word, wf.clone());
    let tag_w = tag.map_or(0.0, |s| text_w(ui, s, tf.clone()) + GAP);
    let icon = HIT;
    let pad = GAP;
    let fixed = pad + tag_w + icon;
    let max_w = ui.max_rect().width().max(fixed + HIT);
    let shown = word_w.min(max_w - fixed);
    let width = snap(fixed + shown, HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, HIT), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, t.surface);
    let g = line_galley(ui, word, wf, t.ink, shown + 0.5);
    p.galley(egui::pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0), g, t.ink);
    if let Some(s) = tag {
        p.text(egui::pos2(rect.left() + pad + shown + U, rect.center().y), egui::Align2::LEFT_CENTER, s, tf, t.muted);
    }
    if shown < word_w {
        resp.on_hover_text(word);
    }
    let irect = Rect::from_min_size(egui::pos2(rect.right() - icon, rect.top()), Vec2::splat(icon));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(irect));
    crate::icons::delete_icon_button(&mut child).clicked()
}

pub fn check(ui: &mut Ui, on: &mut bool, text: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let f = font_md();
    let w = (HIT + GAP + text_w(ui, text, f.clone())).min(ui.available_width()).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let b = Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - HIT / 2.0), Vec2::splat(HIT));
    paint_onoff(ui, b, *on, accent);
    let g = line_galley(ui, text, f, t.ink, w - HIT - GAP);
    ui.painter().galley(egui::pos2(b.right() + GAP, rect.center().y - g.size().y / 2.0), g, t.ink);
    focus_ring(ui, rect, resp.has_focus());
    let resp = resp.on_hover_text(state_tip(*on, text));
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

pub fn back_square(ui: &mut Ui, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HIT), Sense::click());
    let bg = if resp.is_pointer_button_down_on() {
        shade(color, -0.22)
    } else if resp.hovered() {
        shade(color, -0.16)
    } else {
        shade(color, -0.10)
    };
    let p = ui.painter();
    p.rect_filled(rect, 0.0, bg);
    let c = rect.center();
    let s = hair(INK_ON_COLOR);
    let arm = MARK / 2.0;
    p.line_segment([egui::pos2(c.x - arm, c.y), egui::pos2(c.x + arm, c.y)], s);
    p.line_segment([egui::pos2(c.x - arm, c.y), egui::pos2(c.x, c.y - arm)], s);
    p.line_segment([egui::pos2(c.x - arm, c.y), egui::pos2(c.x, c.y + arm)], s);
    focus_ring(ui, rect, resp.has_focus());
    resp.on_hover_text("Назад (Esc)")
}

/// Geometric summary drawn on a group header (values, not labels).
pub enum Mark {
    None,
    Label(&'static str),
    Value(String),
    /// `on` filled squares out of `of` (e.g. 2 of 3).
    Count { on: u8, of: u8 },
    /// DIN-rail breaker: lever up = on, down = off.
    Toggle(bool),
    /// Half-filled square: left half = light, right half = dark.
    Theme { light: bool },
}

fn mark_width(ui: &Ui, mark: &Mark) -> f32 {
    match mark {
        Mark::None => 0.0,
        Mark::Label(s) => text_w(ui, s, font_md()).max(U),
        Mark::Value(s) => text_w(ui, s, font_mono()).max(U),
        Mark::Count { of, .. } => *of as f32 * MARK + (*of as f32 - 1.0).max(0.0) * GAP,
        Mark::Toggle(_) => BREAKER_W,
        Mark::Theme { .. } => MARK,
    }
}

fn mark_tip(mark: &Mark) -> String {
    match mark {
        Mark::None => String::new(),
        Mark::Label(s) => (*s).to_string(),
        Mark::Value(s) => s.clone(),
        Mark::Count { on, of } => format!("{on} из {of}"),
        Mark::Toggle(true) => "вкл".into(),
        Mark::Toggle(false) => "выкл".into(),
        Mark::Theme { light: true } => "светлая".into(),
        Mark::Theme { light: false } => "тёмная".into(),
    }
}

fn paint_mark(ui: &Ui, right: f32, cy: f32, mark: &Mark, accent: Color32) -> f32 {
    let t = cur(ui);
    let p = ui.painter();
    let w = mark_width(ui, mark);
    let x0 = right - w;
    match mark {
        Mark::None => {}
        Mark::Label(s) => {
            let g = line_galley(ui, s, font_md(), t.muted, w);
            p.galley(egui::pos2(x0, cy - g.size().y / 2.0), g, t.muted);
        }
        Mark::Value(s) => {
            let g = line_galley(ui, s, font_mono(), t.muted, w);
            p.galley(egui::pos2(x0, cy - g.size().y / 2.0), g, t.muted);
        }
        Mark::Count { on, of } => {
            let side = MARK;
            let step = side + GAP;
            for i in 0..*of {
                let x = x0 + i as f32 * step;
                let r = Rect::from_min_size(egui::pos2(x, cy - side / 2.0), Vec2::splat(side));
                if i < *on {
                    p.rect_filled(r, 0.0, accent);
                } else {
                    p.rect_stroke(r.shrink(0.5), 0.0, hair(t.muted));
                }
            }
        }
        Mark::Toggle(on) => {
            let r = Rect::from_center_size(egui::pos2(right - BREAKER_W / 2.0, cy), Vec2::new(BREAKER_W, BREAKER_H));
            paint_onoff(ui, r, *on, accent);
        }
        Mark::Theme { light } => {
            let r = Rect::from_center_size(egui::pos2(right - MARK / 2.0, cy), Vec2::splat(MARK));
            p.rect_stroke(r.shrink(0.5), 0.0, hair(t.muted));
            let half = if *light {
                Rect::from_min_max(r.min, egui::pos2(r.center().x, r.max.y))
            } else {
                Rect::from_min_max(egui::pos2(r.center().x, r.min.y), r.max)
            };
            p.rect_filled(half.shrink(1.0), 0.0, t.ink);
        }
    }
    w
}

pub fn group_header(ui: &mut Ui, open: bool, title: &str, mark: &Mark, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, ROW), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if resp.hovered() { t.surface_hover } else { t.surface });
    // Expand marker: a 2U square. Closed = plus of two hairlines; open = one hairline (minus).
    let mark_box = Rect::from_center_size(egui::pos2(rect.right() - GAP - MARK / 2.0, rect.center().y), Vec2::splat(MARK));
    p.rect_stroke(mark_box.shrink(0.5), 0.0, hair(t.ink));
    let s = hair(t.ink);
    let arm = MARK / 2.0 - U / 2.0;
    p.line_segment([egui::pos2(mark_box.center().x - arm, mark_box.center().y), egui::pos2(mark_box.center().x + arm, mark_box.center().y)], s);
    if !open {
        p.line_segment([egui::pos2(mark_box.center().x, mark_box.center().y - arm), egui::pos2(mark_box.center().x, mark_box.center().y + arm)], s);
    }
    let summary_right = mark_box.left() - GAP;
    let mw = paint_mark(ui, summary_right, rect.center().y, mark, accent);
    let title_x = rect.left() + GAP;
    let title_max = (summary_right - mw - GAP - title_x).max(U);
    let tg = line_galley(ui, title, font_md(), t.ink, title_max);
    p.galley(egui::pos2(title_x, rect.center().y - tg.size().y / 2.0), tg, t.ink);
    focus_ring(ui, rect, resp.has_focus());
    let tip = mark_tip(mark);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// 32×32 letter square used as an app glyph when no icon is available.
pub fn letter_square(ui: &mut Ui, letter: &str, tip: &str) {
    let t = cur(ui);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HIT), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.surface_hover);
    let g = line_galley(ui, letter, font_md(), t.ink, HIT - U);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, t.ink);
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
}

/// Crisp geometric symbol painted inside each of the 5 home squares (`idx` 0..5).
pub fn paint_home_icon(p: &egui::Painter, c: egui::Pos2, u: f32, idx: usize) {
    let fg = INK_ON_COLOR;
    let s = Stroke::new(HAIR.max(u * 0.12), fg);
    match idx {
        0 => {
            let d = u * 0.42;
            let sz = Vec2::splat(u * 0.56);
            for (dx, dy) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
                p.rect_filled(Rect::from_center_size(c + egui::vec2(dx, dy), sz), 0.0, fg);
            }
        }
        1 => {
            for (dy, w) in [(-0.55, 1.4), (0.0, 1.05), (0.55, 1.25)] {
                let r = Rect::from_center_size(c + egui::vec2((w - 1.4) * u * 0.5, dy * u), egui::vec2(u * w, u * 0.22));
                p.rect_filled(r, 0.0, fg);
            }
        }
        2 => {
            let body = Rect::from_center_size(c + egui::vec2(0.0, u * 0.25), egui::vec2(u * 1.2, u * 0.9));
            p.rect_filled(body, 0.0, fg);
            let shackle = Rect::from_min_max(c + egui::vec2(-u * 0.36, -u * 0.72), c + egui::vec2(u * 0.36, -u * 0.15));
            p.rect_stroke(shackle, 0.0, s);
        }
        3 => {
            for (dy, kx) in [(-0.36, 0.26), (0.36, -0.26)] {
                let y = c.y + dy * u;
                p.line_segment([egui::pos2(c.x - u * 0.75, y), egui::pos2(c.x + u * 0.75, y)], s);
                p.rect_filled(Rect::from_center_size(egui::pos2(c.x + kx * u, y), Vec2::splat(u * 0.42)), 0.0, fg);
            }
        }
        _ => {
            let px = (u * 0.24).max(2.0);
            let rows: [(i8, &[i8]); 4] = [(-2, &[-2, -1, 1, 2]), (-1, &[-2, -1, 0, 1, 2]), (0, &[-1, 0, 1]), (1, &[0])];
            for (ry, cols) in rows {
                for &rx in cols {
                    let center = c + egui::vec2(rx as f32 * px, (ry as f32 + 0.3) * px);
                    p.rect_filled(Rect::from_center_size(center, Vec2::splat(px + 0.5)), 0.0, fg);
                }
            }
        }
    }
}

/// sRGB relative luminance (WCAG 2).
pub fn relative_luminance(c: Color32) -> f32 {
    fn lin(v: u8) -> f32 {
        let x = v as f32 / 255.0;
        if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (l1, l2) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    const AA: f32 = 4.5;

    fn assert_aa(name: &str, fg: Color32, bg: Color32) {
        let r = contrast_ratio(fg, bg);
        assert!(r + 1e-4 >= AA, "{name}: {r:.2} < {AA}");
    }

    #[test]
    fn text_on_surfaces_meets_wcag_aa() {
        for dark in [false, true] {
            let t = theme(dark);
            let tag = if dark { "dark" } else { "light" };
            assert_aa(&format!("{tag} ink on bg"), t.ink, t.bg);
            assert_aa(&format!("{tag} ink on surface"), t.ink, t.surface);
            assert_aa(&format!("{tag} ink on field"), t.ink, t.field);
            assert_aa(&format!("{tag} muted on bg"), t.muted, t.bg);
            assert_aa(&format!("{tag} muted on surface"), t.muted, t.surface);
            assert_aa(&format!("{tag} muted on field"), t.muted, t.field);
            assert_aa(&format!("{tag} warn on bg"), t.warn, t.bg);
            assert_aa(&format!("{tag} warn on surface"), t.warn, t.surface);
        }
    }

    #[test]
    fn ink_on_section_colours_meets_wcag_aa() {
        for (name, c) in [("yellow", YELLOW), ("orange", ORANGE), ("lime", LIME), ("cyan", CYAN), ("red", RED)] {
            assert_aa(&format!("ink on {name}"), INK_ON_COLOR, c);
        }
    }

    #[test]
    fn grid_constants_are_multiples_of_u() {
        for (n, v) in [("GAP", GAP), ("PAD", PAD), ("HIT", HIT), ("ROW", ROW), ("MARK", MARK), ("BREAKER_W", BREAKER_W), ("BREAKER_H", BREAKER_H)] {
            assert!((v / U).fract().abs() < 1e-6, "{n}={v} is not a multiple of {U}");
        }
        assert_eq!(U, 4.0);
        assert_eq!(HIT, 32.0);
        assert_eq!(ROW, 40.0);
        assert_eq!(MARK, 16.0);
        assert_eq!(BREAKER_W, 24.0);
        assert_eq!(BREAKER_H, 32.0);
        assert!((FONT_LG / FONT_MD - FONT_MD / FONT_SM).abs() < 1e-6, "type scale is not a single ratio");
        assert!((FONT_MD / FONT_SM - 4.0 / 3.0).abs() < 1e-6);
    }
}
