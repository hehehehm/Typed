//! Geometric look of the settings window: «Супрематический» pass.
//!
//! One base unit `U` = 4 px; every size is an integer multiple of `U`.
//! Composition: each section screen is its home square enlarged into a full-height
//! colour *spine* on the left (glyph or large numeral, vertical title, five-square
//! navigation); content sits on the plain ground to the right on a strict column,
//! separated by 1 px rules, never by grey cards. Controls live in one right column.
//!
//! Type scale is one ratio, 4:3: 12 · 16 · 21⅓ · 28⁴⁄₉ · 37.9 · 50.6.
//! Layout rule: no widget asks for more width than `ui.available_width()`.

use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;

/// Base module, in logical pixels.
pub const U: f32 = 4.0;
/// Small gutter (U).
pub const GAP: f32 = U;
/// Padding (2U).
pub const PAD: f32 = U * 2.0;
/// Content margin from the spine and the window edge (4U).
pub const MARGIN: f32 = U * 4.0;
/// Minimum hit target (8U), 32 px for accessibility.
pub const HIT: f32 = U * 8.0;
/// Row height (10U = 4 + 32 + 4).
pub const ROW: f32 = HIT + GAP * 2.0;
/// Visual marks, nav chips (4U).
pub const MARK: f32 = U * 4.0;
/// Width of the colour spine on section screens (24U).
pub const SPINE: f32 = U * 24.0;
/// Switch: 2:1 track (12U × 6U) with a 4U square knob.
pub const SWITCH_W: f32 = U * 12.0;
pub const SWITCH_H: f32 = U * 6.0;
pub const KNOB: f32 = U * 4.0;
/// Keycap height (6U).
pub const CAP_H: f32 = U * 6.0;
/// 1 px structural hairline.
pub const HAIR: f32 = 1.0;

/// Type scale, ratio 4:3.
pub const FONT_SM: f32 = 12.0;
pub const FONT_MD: f32 = FONT_SM * 4.0 / 3.0;
pub const FONT_LG: f32 = FONT_MD * 4.0 / 3.0;
pub const FONT_XL: f32 = FONT_LG * 4.0 / 3.0;
pub const FONT_XXL: f32 = FONT_XL * 4.0 / 3.0;
pub const FONT_HUGE: f32 = FONT_XXL * 4.0 / 3.0;

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
    #[allow(dead_code)]
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
pub fn font_mono() -> FontId {
    FontId::monospace(FONT_MD)
}


pub fn line_galley(ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(U));
    ui.fonts(|f| f.layout_job(job))
}

pub fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x)
}

pub fn focus_ring(ui: &Ui, rect: Rect, focused: bool) {
    if focused {
        ui.painter().rect_stroke(rect.expand(1.5), 0.0, hair(cur(ui).ink));
    }
}

/// Linear blend of two colours, `t` in 0..1.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

pub fn warn(ui: &mut Ui, text: &str) {
    let c = cur(ui).warn;
    ui.add(egui::Label::new(egui::RichText::new(text).color(c)).wrap());
}

/// Small spaced capitals, muted.
pub fn caption(ui: &mut Ui, text: &str, tip: &str) {
    let c = cur(ui).muted;
    let r = ui.add(egui::Label::new(egui::RichText::new(text.to_uppercase()).size(FONT_SM).color(c).extra_letter_spacing(1.2)).truncate());
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
}

/// Full-width 1 px rule.
pub fn rule(ui: &mut Ui) {
    let t = cur(ui);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, HAIR), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.line);
}

/// Plain group: no fill, content padded by GAP vertically.
pub fn block<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(0.0, GAP))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Flat text tile, 32 high. `fill` transparent = outlined.
pub fn tile(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_md();
    let natural = text_w(ui, text, f.clone()) + PAD * 2.0;
    let w = natural.min(ui.available_width().max(HIT)).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if fill == Color32::TRANSPARENT {
            if resp.hovered() {
                p.rect_filled(rect, 0.0, t.surface);
            }
            p.rect_stroke(rect.shrink(0.5), 0.0, hair(t.muted));
        } else {
            let mut bg = fill;
            if resp.is_pointer_button_down_on() {
                bg = shade(fill, -0.12);
            } else if resp.hovered() {
                bg = shade(fill, -0.06);
            }
            p.rect_filled(rect, 0.0, bg);
        }
        let g = line_galley(ui, text, f, fg, w - PAD * 2.0);
        p.galley(rect.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, rect, resp.has_focus());
    }
    if natural > w { resp.on_hover_text(text) } else { resp }
}

pub fn primary(ui: &mut Ui, text: &str, accent: Color32) -> egui::Response {
    tile(ui, text, accent, INK_ON_COLOR)
}

pub fn secondary(ui: &mut Ui, text: &str) -> egui::Response {
    let t = cur(ui);
    tile(ui, text, Color32::TRANSPARENT, t.ink)
}

/// Text-only action (no box): ink text, accent underline on hover.
pub fn link(ui: &mut Ui, text: &str, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_md();
    let w = (text_w(ui, text, f.clone()) + PAD).min(ui.available_width()).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let g = line_galley(ui, text, f, t.ink, w);
    let pos = egui::pos2(rect.right() - g.size().x - U, rect.center().y - g.size().y / 2.0);
    let base = pos.y + g.size().y + 1.0;
    let gw = g.size().x;
    ui.painter().galley(pos, g, t.ink);
    let c = if resp.hovered() { accent } else { t.line };
    ui.painter().line_segment([egui::pos2(pos.x, base), egui::pos2(pos.x + gw, base)], Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, c));
    focus_ring(ui, rect, resp.has_focus());
    resp
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
        let arm = MARK / 2.0;
        p.rect_filled(Rect::from_center_size(c, egui::vec2(arm * 2.0, 2.0)), 0.0, INK_ON_COLOR);
        p.rect_filled(Rect::from_center_size(c, egui::vec2(2.0, arm * 2.0)), 0.0, INK_ON_COLOR);
        focus_ring(ui, rect, resp.has_focus());
    }
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// The switch: a 2:1 flat track with a square knob that slides.
/// Off — ink outline, ink knob on the left. On — accent track, ink knob on the right.
/// `rect` is the hit target; the track is centred in it.
pub fn paint_switch(ui: &Ui, rect: Rect, id: egui::Id, on: bool, hovered: bool, accent: Color32) {
    let t = cur(ui);
    let k = ui.ctx().animate_bool_with_time(id.with("sw"), on, 0.12);
    let track = Rect::from_center_size(rect.center(), egui::vec2(SWITCH_W, SWITCH_H));
    let p = ui.painter();
    let enabled = ui.is_enabled();
    let accent = if enabled { accent } else { t.muted };
    let off_fill = if hovered { t.surface } else { t.bg };
    let on_fill = if hovered { shade(accent, -0.06) } else { accent };
    p.rect_filled(track, 0.0, mix(off_fill, on_fill, k));
    if k < 1.0 {
        let c = mix(t.ink, on_fill, k);
        p.rect_stroke(track.shrink(0.5), 0.0, hair(c));
    }
    let inset = (SWITCH_H - KNOB) / 2.0;
    let x0 = track.left() + inset;
    let x1 = track.right() - inset - KNOB;
    let x = x0 + (x1 - x0) * k;
    let knob = Rect::from_min_size(egui::pos2(x, track.top() + inset), Vec2::splat(KNOB));
    let knob_c = mix(t.ink, INK_ON_COLOR, k);
    p.rect_filled(knob, 0.0, knob_c);
}

fn state_tip(on: bool, tip: &str) -> String {
    let state = if on { "Вкл" } else { "Выкл" };
    if tip.is_empty() { state.into() } else { format!("{tip} · {state}") }
}

/// Ruled row: label left, switch in the right column; the whole row toggles.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, title: &str, tip: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let id = ui.next_auto_id();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let label_w = (rect.width() - SWITCH_W - GAP).max(U);
    let g = line_galley(ui, title, font_md(), t.ink, label_w);
    ui.painter().galley(egui::pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, t.ink);
    let sw = Rect::from_min_size(egui::pos2(rect.right() - SWITCH_W, rect.center().y - HIT / 2.0), egui::vec2(SWITCH_W, HIT));
    paint_switch(ui, sw, id, *on, resp.hovered(), accent);
    focus_ring(ui, Rect::from_center_size(sw.center(), egui::vec2(SWITCH_W, SWITCH_H)), resp.has_focus());
    let resp = resp.on_hover_text(state_tip(*on, tip));
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

/// Compact flag of cells (e.g. EN · RU · ∅): outlined strip, the chosen cell filled.
pub fn segment_strip<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str, &str)], total_w: f32, accent: Color32) -> bool {
    let n = options.len().max(1);
    let w = (total_w / n as f32).max(HIT);
    let h = HIT;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w * n as f32, h), Sense::hover());
    let t = cur(ui);
    let mut changed = false;
    for (i, &(v, name, tip)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(egui::pos2(rect.left() + i as f32 * w, rect.top()), egui::vec2(w, h));
        let resp = ui.interact(cell, ui.id().with(("seg", i, name)), Sense::click());
        let selected = *value == v;
        let p = ui.painter();
        if selected {
            p.rect_filled(cell, 0.0, accent);
        } else if resp.hovered() {
            p.rect_filled(cell, 0.0, t.surface);
        }
        let fg = if selected { INK_ON_COLOR } else { t.muted };
        let g = line_galley(ui, name, font_sm(), fg, w - U);
        p.galley(cell.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, cell, resp.has_focus());
        let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip) };
        if resp.clicked() && !selected {
            *value = v;
            changed = true;
        }
    }
    ui.painter().rect_stroke(rect.shrink(0.5), 0.0, hair(t.line));
    changed
}

/// Three-step geometric scale: squares of 2U, 4U, 6U on one baseline;
/// steps up to the chosen one are filled with the accent.
pub fn scale3<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str, &str); 3], accent: Color32) -> bool {
    let t = cur(ui);
    let mut changed = false;
    let chosen = options.iter().position(|o| o.0 == *value).unwrap_or(1);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(HIT * 3.0, HIT), Sense::hover());
    for (i, &(v, name, tip)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(egui::pos2(rect.left() + i as f32 * HIT, rect.top()), Vec2::splat(HIT));
        let resp = ui.interact(cell, ui.id().with(("scale", i, name)), Sense::click());
        let side = U * 2.0 * (i as f32 + 1.0);
        // Equal pitch (one hit target per step); the strip is shifted so the
        // largest square ends flush with the right column.
        let shift = (HIT - U * 6.0) / 2.0;
        let sq = Rect::from_min_size(egui::pos2(cell.center().x + shift - side / 2.0, cell.bottom() - U - side), Vec2::splat(side));
        let p = ui.painter();
        if i <= chosen {
            p.rect_filled(sq, 0.0, if resp.hovered() { shade(accent, -0.08) } else { accent });
        } else {
            p.rect_stroke(sq.shrink(0.5), 0.0, hair(if resp.hovered() { t.ink } else { t.muted }));
        }
        focus_ring(ui, cell, resp.has_focus());
        let resp = resp.on_hover_text(format!("{name} — {tip}"));
        if resp.clicked() && *value != v {
            *value = v;
            changed = true;
        }
    }
    changed
}

/// Flat keycaps for a combo such as `Ctrl+Alt+Z`, right-aligned in `max_w`.
/// Returns the clickable response covering all caps.
pub fn keycaps(ui: &mut Ui, combo: &str, max_w: f32, active: bool, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_sm();
    let parts: Vec<&str> = if combo.contains('+') && combo.len() > 1 { combo.split('+').collect() } else { vec![combo] };
    let pad = PAD;
    let widths: Vec<f32> = parts.iter().map(|s| (text_w(ui, s, f.clone()) + pad * 2.0).max(CAP_H)).collect();
    let total: f32 = widths.iter().sum::<f32>() + GAP * (parts.len().saturating_sub(1) as f32);
    let fits = total <= max_w;
    let w = if fits { total } else { max_w.max(HIT) };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let p = ui.painter();
    let fill = if active { accent } else if resp.hovered() { t.surface } else { t.bg };
    let fg = if active { INK_ON_COLOR } else { t.ink };
    let edge = if active { INK_ON_COLOR } else { t.muted };
    let cap = |r: Rect, s: &str, max: f32| {
        p.rect_filled(r, 0.0, fill);
        p.rect_stroke(r.shrink(0.5), 0.0, hair(edge));
        // Keycap base: a 2 px foot.
        p.rect_filled(Rect::from_min_max(egui::pos2(r.left(), r.bottom() - 2.0), r.max), 0.0, edge);
        let g = line_galley(ui, s, f.clone(), fg, max);
        p.galley(egui::pos2(r.center().x - g.size().x / 2.0, r.center().y - 1.0 - g.size().y / 2.0), g, fg);
    };
    let top = rect.center().y - CAP_H / 2.0;
    if fits {
        let mut x = rect.left();
        for (s, cw) in parts.iter().zip(&widths) {
            cap(Rect::from_min_size(egui::pos2(x, top), egui::vec2(*cw, CAP_H)), s, cw - pad);
            x += cw + GAP;
        }
    } else {
        cap(Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(w, CAP_H)), combo, w - pad);
    }
    focus_ring(ui, rect, resp.has_focus());
    if fits { resp } else { resp.on_hover_text(combo) }
}

/// Text field: no box, a baseline rule; focus thickens it to 2 px accent.
pub fn field(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, password: bool, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let w = width.min(ui.available_width()).max(HIT);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::hover());
    let resp = ui.put(
        rect,
        egui::TextEdit::singleline(text)
            .hint_text(egui::RichText::new(hint_text).color(t.muted))
            .password(password)
            .frame(false)
            .margin(egui::vec2(0.0, GAP))
            .desired_width(w)
            .vertical_align(egui::Align::Center),
    );
    let (c, h) = if resp.has_focus() { (accent, 2.0) } else if resp.hovered() { (t.ink, 1.0) } else { (t.muted, 1.0) };
    ui.painter().rect_filled(Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - h), rect.max), 0.0, c);
    resp
}

pub fn add_line_plus(ui: &mut Ui, text: &mut String, hint_text: &str, tip: &str, accent: Color32) -> bool {
    let mut go = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = PAD;
        let fw = (ui.available_width() - HIT - PAD).max(HIT * 2.0);
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

/// Row: label left, actions right (right-to-left), 32 high. `fill` ignored except
/// TRANSPARENT = no vertical padding; otherwise a ruled 40 px row.
pub fn row(ui: &mut Ui, fill: Color32, label: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) -> egui::Response {
    let ruled = fill != Color32::TRANSPARENT;
    let margin = if ruled { egui::Margin::symmetric(0.0, GAP) } else { egui::Margin::ZERO };
    let r = egui::Frame::none()
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(HIT);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = PAD;
                    actions(ui);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = PAD;
                        ui.set_max_width(ui.available_width());
                        label(ui);
                    });
                });
            });
        })
        .response;
    r
}

/// 40 px row (rules between rows are drawn by the caller with `rule`).
pub fn line_row(ui: &mut Ui, label: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) -> egui::Response {
    let t = cur(ui);
    row(ui, t.bg, label, actions)
}

/// Group heading: spaced caption, a count in tabular figures, optional `+`.
pub fn heading(ui: &mut Ui, text: &str, count: usize, tip: &str) {
    let t = cur(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(HIT);
        caption(ui, text, tip);
        ui.label(egui::RichText::new(count.to_string()).size(FONT_SM).color(t.ink).monospace());
    });
}

/// Word token: outlined, mono word, muted tag, trash on the right.
pub fn chip(ui: &mut Ui, word: &str, tag: Option<&str>) -> bool {
    let t = cur(ui);
    let wf = font_mono();
    let tf = font_sm();
    let word_w = text_w(ui, word, wf.clone());
    let tag_w = tag.map_or(0.0, |s| text_w(ui, s, tf.clone()) + PAD);
    let icon = HIT;
    let pad = PAD;
    let fixed = pad + tag_w + icon;
    let max_w = ui.max_rect().width().max(fixed + HIT);
    let shown = word_w.min(max_w - fixed);
    let width = (fixed + shown).ceil();
    let width = (width / U).ceil() * U;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, HIT), Sense::hover());
    let p = ui.painter();
    p.rect_stroke(rect.shrink(0.5), 0.0, hair(t.line));
    let g = line_galley(ui, word, wf, t.ink, shown + 0.5);
    p.galley(egui::pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0), g, t.ink);
    if let Some(s) = tag {
        p.text(egui::pos2(rect.left() + pad + shown + PAD, rect.center().y), egui::Align2::LEFT_CENTER, s, tf, t.muted);
    }
    if shown < word_w {
        resp.on_hover_text(word);
    }
    let irect = Rect::from_min_size(egui::pos2(rect.right() - icon, rect.top()), Vec2::splat(icon));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(irect));
    crate::icons::delete_icon_button(&mut child).clicked()
}

/// Switch followed by its label.
pub fn check(ui: &mut Ui, on: &mut bool, text: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let f = font_md();
    let w = (SWITCH_W + PAD + text_w(ui, text, f.clone())).min(ui.available_width()).max(SWITCH_W);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let b = Rect::from_min_size(rect.min, egui::vec2(SWITCH_W, HIT));
    paint_switch(ui, b, resp.id, *on, resp.hovered(), accent);
    let g = line_galley(ui, text, f, t.ink, w - SWITCH_W - PAD);
    ui.painter().galley(egui::pos2(b.right() + PAD, rect.center().y - g.size().y / 2.0), g, t.ink);
    focus_ring(ui, Rect::from_center_size(b.center(), egui::vec2(SWITCH_W, SWITCH_H)), resp.has_focus());
    let resp = resp.on_hover_text(state_tip(*on, text));
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

/// Text painted rotated 90° counter-clockwise (reads bottom → top), its left end at `bottom_left`.
pub fn vertical_text(p: &egui::Painter, bottom_left: egui::Pos2, galley: Arc<egui::Galley>, color: Color32) {
    let shape = egui::epaint::TextShape::new(bottom_left, galley, color).with_angle(-std::f32::consts::FRAC_PI_2);
    p.add(shape);
}

/// Back arrow in ink, 2 px strokes, inside a 32×32 hit target.
pub fn paint_back_arrow(p: &egui::Painter, rect: Rect, hovered: bool) {
    let c = rect.center();
    let s = Stroke::new(2.0, INK_ON_COLOR);
    let arm = MARK / 2.0;
    if hovered {
        p.rect_stroke(rect.shrink(0.5), 0.0, hair(INK_ON_COLOR));
    }
    p.line_segment([egui::pos2(c.x - arm, c.y), egui::pos2(c.x + arm, c.y)], s);
    p.line_segment([egui::pos2(c.x - arm + 0.7, c.y), egui::pos2(c.x - arm / 4.0, c.y - arm * 0.75)], s);
    p.line_segment([egui::pos2(c.x - arm + 0.7, c.y), egui::pos2(c.x - arm / 4.0, c.y + arm * 0.75)], s);
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn relative_luminance(c: Color32) -> f32 {
    fn lin(v: u8) -> f32 {
        let x = v as f32 / 255.0;
        if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

#[cfg_attr(not(test), allow(dead_code))]
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
        for (n, v) in [("GAP", GAP), ("PAD", PAD), ("MARGIN", MARGIN), ("HIT", HIT), ("ROW", ROW), ("MARK", MARK), ("SPINE", SPINE), ("SWITCH_W", SWITCH_W), ("SWITCH_H", SWITCH_H), ("KNOB", KNOB), ("CAP_H", CAP_H)] {
            assert!((v / U).fract().abs() < 1e-6, "{n}={v} is not a multiple of {U}");
        }
        assert_eq!(U, 4.0);
        assert_eq!(HIT, 32.0);
        assert_eq!(ROW, 40.0);
        assert_eq!(MARK, 16.0);
        assert_eq!(SWITCH_W, 2.0 * SWITCH_H, "switch track is 2:1");
        assert_eq!(SWITCH_H - KNOB, 2.0 * U, "knob inset is one U on each side");
        assert!((FONT_LG / FONT_MD - FONT_MD / FONT_SM).abs() < 1e-6, "type scale is not a single ratio");
        assert!((FONT_MD / FONT_SM - 4.0 / 3.0).abs() < 1e-6);
        for (a, b) in [(FONT_LG, FONT_XL), (FONT_XL, FONT_XXL), (FONT_XXL, FONT_HUGE)] {
            assert!((b / a - 4.0 / 3.0).abs() < 1e-6);
        }
    }
}
