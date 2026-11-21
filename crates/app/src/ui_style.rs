//! Look of the settings window: a quiet cross-platform desktop utility.
//!
//! One base unit `U` = 4 px; spacing is an integer multiple of `U`. Neutral surfaces,
//! 1 px hairlines, one small radius family (6 px containers, 4 px controls). The five
//! section colours are accents only, taken from the original water logo: sun gold, sand,
//! seafoam, sea turquoise, coral. A switch that is on is always sea turquoise. The home
//! screen sits on a photographic sea backdrop (day / dusk). Text uses the platform UI
//! font when it can be found (Segoe UI on Windows, SF / Helvetica Neue on macOS), else
//! the bundled one. Type scale is one ratio, 6:5: 12 · 14.4 · 17.3 · 20.7 · 24.9.
//! Layout rule: no widget asks for more width than `ui.available_width()`.

use eframe::egui::{self, Color32, FontId, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use std::cell::Cell;
use std::sync::Arc;

/// Base module, in logical pixels.
pub const U: f32 = 4.0;
pub const GAP: f32 = U;
pub const PAD: f32 = U * 2.0;
/// Horizontal inset of rows inside a group (3U).
pub const INSET: f32 = U * 3.0;
/// Window margin (4U).
pub const MARGIN: f32 = U * 4.0;
/// Minimum hit target (8U).
pub const HIT: f32 = U * 8.0;
/// Row height inside groups (10U).
pub const ROW: f32 = HIT + GAP * 2.0;
pub const MARK: f32 = U * 4.0;
/// Section header height (12U).
pub const HEADER: f32 = U * 12.0;
/// Toggle: 36 × 20 track, 9:5 like the native ones, round knob.
pub const SWITCH_W: f32 = U * 9.0;
pub const SWITCH_H: f32 = U * 5.0;
pub const KNOB: f32 = 14.0;
/// Visual height of buttons, fields and keycaps inside a 32 px target.
pub const CTRL_H: f32 = 28.0;
pub const CAP_H: f32 = U * 6.0;
/// Corner radii: containers and controls.
pub const RADIUS: f32 = 6.0;
pub const RADIUS_SM: f32 = U;
pub const HAIR: f32 = 1.0;

pub const FONT_SM: f32 = 12.0;
pub const FONT_MD: f32 = FONT_SM * 1.2;
pub const FONT_LG: f32 = FONT_MD * 1.2;
pub const FONT_XL: f32 = FONT_LG * 1.2;
pub const FONT_XXL: f32 = FONT_XL * 1.2;

/// Accents taken from the original water logo (sun sparkles, sand, shallow
/// sea, coral at sunset). Dark ink `INK_ON_COLOR` sits on all of them (WCAG AA).
pub const SUN: Color32 = Color32::from_rgb(0xE8, 0xC4, 0x68); // Программы
pub const SAND: Color32 = Color32::from_rgb(0xD4, 0xA0, 0x6E); // Словарь
pub const SEAFOAM: Color32 = Color32::from_rgb(0x72, 0xC2, 0xB9); // Сейф
pub const SEA: Color32 = Color32::from_rgb(0x5B, 0xB8, 0xC4); // Настройки; switch that is on
pub const CORAL: Color32 = Color32::from_rgb(0xE4, 0x7A, 0x6A); // Поддержать
pub const INK_ON_COLOR: Color32 = Color32::from_rgb(0x23, 0x26, 0x2B);

/// Older names kept so existing call sites compile.
#[allow(dead_code)]
pub const YELLOW: Color32 = SUN;
#[allow(dead_code)]
pub const ORANGE: Color32 = SAND;
#[allow(dead_code)]
pub const LIME: Color32 = SEAFOAM;
#[allow(dead_code)]
pub const CYAN: Color32 = SEA;
#[allow(dead_code)]
pub const RED: Color32 = CORAL;


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
    /// Stronger than `muted` for counts and short marks that must stay AA
    /// even on a slightly mixed plate (card corner, group caption).
    pub muted_strong: Color32,
    pub warn: Color32,
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        // Night sea: deep teal water, warm sand-white ink, gold as the warning.
        Theme {
            dark,
            bg: Color32::from_rgb(0x15, 0x1E, 0x21),
            surface: Color32::from_rgb(0x22, 0x2C, 0x30),
            surface_hover: Color32::from_rgb(0x2C, 0x39, 0x3E),
            field: Color32::from_rgb(0x12, 0x1A, 0x1D),
            line: Color32::from_rgb(0x33, 0x40, 0x45),
            ink: Color32::from_rgb(0xF2, 0xF0, 0xE8),
            muted: Color32::from_rgb(0xA3, 0xAD, 0xAD),
            muted_strong: Color32::from_rgb(0xC8, 0xD0, 0xCF),
            warn: Color32::from_rgb(0xE8, 0xC4, 0x48),
        }
    } else {
        // Sun-warm sand, from the geometry mark's cream and the water's gold light.
        Theme {
            dark,
            bg: Color32::from_rgb(0xF4, 0xEF, 0xE6),
            surface: Color32::from_rgb(0xFF, 0xFC, 0xFA),
            surface_hover: Color32::from_rgb(0xF4, 0xEE, 0xE6),
            field: Color32::from_rgb(0xFF, 0xFC, 0xFA),
            line: Color32::from_rgb(0xE4, 0xDC, 0xD0),
            ink: Color32::from_rgb(0x1D, 0x1D, 0x1F),
            muted: Color32::from_rgb(0x5F, 0x5A, 0x54),
            muted_strong: Color32::from_rgb(0x45, 0x41, 0x3C),
            warn: Color32::from_rgb(0x8F, 0x56, 0x00),
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

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

pub fn hair(color: Color32) -> Stroke {
    Stroke::new(HAIR, color)
}

/// Snap down to a multiple of `U` (never below `min`).
pub fn snap(v: f32, min: f32) -> f32 {
    ((v / U).floor() * U).max(min)
}

thread_local! {
    static MAC_PREVIEW: Cell<bool> = const { Cell::new(false) };
}

/// macOS conventions (⌃⌥⇧⌘ keycaps). True on macOS, or in a screenshot preview.
pub fn mac_style() -> bool {
    cfg!(target_os = "macos") || MAC_PREVIEW.with(|c| c.get())
}

/// Screenshot tests: render this thread with macOS conventions.
#[cfg(test)]
pub fn set_mac_preview(on: bool) {
    MAC_PREVIEW.with(|c| c.set(on));
}

/// Hotkey label parts for display: `Ctrl+Alt+Z` → `[Ctrl, Alt, Z]`, or on macOS
/// `[⌃, ⌥, Z]`. Only the label changes, never the stored hotkey.
pub fn combo_parts(combo: &str) -> Vec<String> {
    let parts: Vec<&str> = if combo.len() > 1 && combo.contains('+') { combo.split('+').collect() } else { vec![combo] };
    parts
        .into_iter()
        .map(|p| {
            if !mac_style() {
                return p.to_string();
            }
            match p.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => "⌃".into(),
                "alt" | "option" | "opt" => "⌥".into(),
                "shift" => "⇧".into(),
                "cmd" | "command" | "win" | "super" | "meta" => "⌘".into(),
                "backspace" | "delete" => "⌫".into(),
                _ => p.to_string(),
            }
        })
        .collect()
}

/// Platform UI font candidates (path, face index in a collection).
fn system_font_candidates() -> Vec<(std::path::PathBuf, u32)> {
    let mut v = Vec::new();
    if cfg!(windows) {
        let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
        v.push((dir.join("segoeui.ttf"), 0));
        v.push((dir.join("SegUIVar.ttf"), 0));
    }
    if cfg!(target_os = "macos") {
        v.push(("/System/Library/Fonts/SFNS.ttf".into(), 0));
        v.push(("/System/Library/Fonts/HelveticaNeue.ttc".into(), 0));
        v.push(("/System/Library/Fonts/Helvetica.ttc".into(), 0));
    }
    v
}

/// A font file is used only if it parses and covers Latin, Cyrillic and digits,
/// so a missing or odd file never breaks the window (egui would panic on a bad font).
fn usable_font(bytes: &[u8], index: u32) -> bool {
    use ab_glyph::Font;
    match ab_glyph::FontRef::try_from_slice_and_index(bytes, index) {
        Ok(f) => "AzЖж09".chars().all(|c| f.glyph_id(c).0 != 0),
        Err(_) => false,
    }
}

fn load_font(path: &std::path::Path, index: u32) -> Option<egui::FontData> {
    let bytes = std::fs::read(path).ok()?;
    if !usable_font(&bytes, index) {
        return None;
    }
    let mut d = egui::FontData::from_owned(bytes);
    d.index = index;
    Some(d)
}

/// Fonts: the platform UI font first (if found), then the bundled ones; Hack and
/// the emoji fonts catch symbols (→, ∅, ⌘). Call once per context.
pub fn install_fonts(ctx: &egui::Context) {
    let found = system_font_candidates().into_iter().find_map(|(p, i)| load_font(&p, i));
    install_fonts_with(ctx, found);
}

/// Same as `install_fonts` with an explicit primary font (tests: a macOS-like preview).
pub fn install_fonts_with(ctx: &egui::Context, primary: Option<egui::FontData>) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(d) = primary {
        fonts.font_data.insert("system".to_owned(), d);
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            list.insert(0, "system".to_owned());
        }
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        if !list.iter().any(|f| f == "Hack") {
            list.push("Hack".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
pub fn font_file(path: &str) -> Option<egui::FontData> {
    load_font(std::path::Path::new(path), 0)
}

pub fn apply(ctx: &egui::Context, dark: bool, accent: Color32) {
    let t = theme(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = t.bg;
    v.window_fill = t.surface;
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.surface;
    v.code_bg_color = t.surface;
    v.window_rounding = Rounding::same(RADIUS);
    v.menu_rounding = Rounding::same(RADIUS);
    v.window_stroke = hair(t.line);
    v.window_shadow = egui::epaint::Shadow { offset: egui::vec2(0.0, 4.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(if dark { 90 } else { 28 }) };
    v.popup_shadow = v.window_shadow;
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
        w.rounding = Rounding::same(RADIUS_SM);
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.expansion = 0.0;
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = hair(t.ink);
    }
    v.widgets.noninteractive.bg_stroke = hair(t.line);
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        use egui::{FontFamily::*, TextStyle};
        s.text_styles = [
            (TextStyle::Heading, FontId::new(FONT_LG, Proportional)),
            (TextStyle::Body, FontId::new(FONT_MD, Proportional)),
            (TextStyle::Button, FontId::new(FONT_MD, Proportional)),
            (TextStyle::Small, FontId::new(FONT_SM, Proportional)),
            (TextStyle::Monospace, FontId::new(FONT_SM * 1.1, Monospace)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(GAP, GAP);
        s.spacing.button_padding = egui::vec2(PAD, GAP);
        s.spacing.interact_size = egui::vec2(HIT, HIT);
        s.spacing.icon_width = MARK;
        s.spacing.icon_width_inner = MARK / 2.0;
        s.spacing.window_margin = egui::Margin::same(PAD);
        s.spacing.menu_margin = egui::Margin::same(GAP);
        let mut sc = egui::style::ScrollStyle::thin();
        sc.bar_width = U * 1.5;
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
    FontId::monospace(FONT_SM * 1.1)
}

pub fn line_galley(ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(U));
    ui.fonts(|f| f.layout_job(job))
}

pub fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x)
}

/// Focus: a 2 px ring just outside the control, same corner family.
pub fn focus_ring(ui: &Ui, rect: Rect, radius: f32, focused: bool) {
    if focused {
        let t = cur(ui);
        ui.painter().rect_stroke(rect.expand(2.0), radius + 2.0, Stroke::new(2.0, mix(t.ink, t.bg, 0.25)));
    }
}

pub fn warn(ui: &mut Ui, text: &str) {
    let c = cur(ui).warn;
    ui.add(egui::Label::new(egui::RichText::new(text).color(c)).wrap());
}

/// Small muted caption above a group, inset like the rows.
pub fn group_title(ui: &mut Ui, text: &str, count: Option<usize>, tip: &str) {
    let t = cur(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(MARGIN + U);
        ui.add_space(INSET);
        let r = ui.add(egui::Label::new(egui::RichText::new(text).size(FONT_SM).color(t.muted)).truncate());
        if !tip.is_empty() {
            r.on_hover_text(tip);
        }
        if let Some(n) = count {
            ui.label(egui::RichText::new(n.to_string()).size(FONT_SM).color(t.muted_strong));
        }
    });
}

/// Inset grouped list: rounded surface, hairline border; rows inside are
/// separated with `sep`.
pub fn group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = cur(ui);
    egui::Frame::none()
        .fill(t.surface)
        .stroke(hair(t.line))
        .rounding(RADIUS)
        .inner_margin(egui::Margin::symmetric(INSET, 0.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner
}

/// Hairline between rows of a group (inset by the group margin).
pub fn sep(ui: &mut Ui) {
    let t = cur(ui);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, HAIR), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.line);
}

/// Plain block (kept for callers that just need vertical padding).
pub fn block<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(0.0, GAP))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn control_rect(rect: Rect) -> Rect {
    Rect::from_center_size(rect.center(), egui::vec2(rect.width(), CTRL_H.min(rect.height())))
}

/// Button: 28 px visual in a 32 px target. `fill` TRANSPARENT = neutral button.
pub fn tile(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_md();
    let natural = text_w(ui, text, f.clone()) + INSET * 2.0;
    let w = natural.min(ui.available_width().max(HIT)).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    if ui.is_rect_visible(rect) {
        let r = control_rect(rect);
        let p = ui.painter();
        if fill == Color32::TRANSPARENT {
            let bg = if resp.is_pointer_button_down_on() {
                mix(t.surface_hover, t.line, 0.5)
            } else if resp.hovered() {
                t.surface_hover
            } else if t.dark {
                t.surface_hover
            } else {
                t.surface
            };
            p.rect_filled(r, RADIUS_SM, bg);
            p.rect_stroke(r.shrink(0.5), RADIUS_SM, hair(t.line));
        } else {
            let bg = if resp.is_pointer_button_down_on() {
                shade(fill, -0.12)
            } else if resp.hovered() {
                shade(fill, -0.06)
            } else {
                fill
            };
            p.rect_filled(r, RADIUS_SM, bg);
        }
        let g = line_galley(ui, text, f, fg, w - INSET * 2.0);
        p.galley(r.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, r, RADIUS_SM, resp.has_focus());
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

/// Rounded `+` button in the accent colour.
pub fn plus_square(ui: &mut Ui, accent: Color32, tip: &str) -> egui::Response {
    let t = cur(ui);
    let enabled = ui.is_enabled();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HIT), sense);
    if ui.is_rect_visible(rect) {
        let r = Rect::from_center_size(rect.center(), Vec2::splat(CTRL_H));
        let bg = if !enabled {
            mix(t.muted, t.surface, 0.55)
        } else if resp.is_pointer_button_down_on() {
            shade(accent, -0.14)
        } else if resp.hovered() {
            shade(accent, -0.06)
        } else {
            accent
        };
        let p = ui.painter();
        p.rect_filled(r, RADIUS_SM, bg);
        let c = r.center();
        let arm = 6.0;
        let s = Stroke::new(1.5, if enabled { INK_ON_COLOR } else { t.muted });
        p.line_segment([egui::pos2(c.x - arm, c.y), egui::pos2(c.x + arm, c.y)], s);
        p.line_segment([egui::pos2(c.x, c.y - arm), egui::pos2(c.x, c.y + arm)], s);
        if enabled {
            focus_ring(ui, r, RADIUS_SM, resp.has_focus());
        }
    }
    if tip.is_empty() || !enabled { resp } else { resp.on_hover_text(tip) }
}

/// The toggle: 36 × 20 pill, round 14 px knob, animated.
/// Off — hairline outline, muted knob on the left. On — accent fill, white knob on the right.
pub fn paint_switch(ui: &Ui, rect: Rect, id: egui::Id, on: bool, hovered: bool, accent: Color32) -> Rect {
    let t = cur(ui);
    let k = ui.ctx().animate_bool_with_time(id.with("sw"), on, 0.14);
    let track = Rect::from_center_size(egui::pos2(rect.right() - SWITCH_W / 2.0, rect.center().y), egui::vec2(SWITCH_W, SWITCH_H));
    let p = ui.painter();
    let enabled = ui.is_enabled();
    let accent = if enabled { accent } else { mix(t.muted, t.surface, 0.4) };
    let r = SWITCH_H / 2.0;
    let off_fill = if hovered { t.surface_hover } else if t.dark { t.field } else { t.surface };
    let on_fill = if hovered { shade(accent, -0.06) } else { accent };
    p.rect_filled(track, r, mix(off_fill, on_fill, k));
    if k < 1.0 {
        p.rect_stroke(track.shrink(0.5), r, hair(mix(t.muted, on_fill, k)));
    } else {
        p.rect_stroke(track.shrink(0.5), r, hair(shade(accent, -0.18)));
    }
    let inset = (SWITCH_H - KNOB) / 2.0;
    let x0 = track.left() + inset + KNOB / 2.0;
    let x1 = track.right() - inset - KNOB / 2.0;
    let c = egui::pos2(x0 + (x1 - x0) * k, track.center().y);
    let knob_on = Color32::WHITE;
    let knob_off = t.muted;
    let grow = if hovered { 1.0 } else { 0.0 };
    if k > 0.0 {
        p.circle_filled(c + egui::vec2(0.0, 0.5), KNOB / 2.0 + grow, Color32::from_black_alpha((40.0 * k) as u8));
    }
    p.circle_filled(c, KNOB / 2.0 - 2.0 * (1.0 - k) + grow, mix(knob_off, knob_on, k));
    track
}

fn state_tip(on: bool, tip: &str) -> String {
    let state = if on { "Вкл" } else { "Выкл" };
    if tip.is_empty() { state.into() } else { format!("{tip} · {state}") }
}

/// Row with a label and a toggle on the right; the whole row toggles.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, title: &str, tip: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let id = ui.next_auto_id();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let label_w = (rect.width() - SWITCH_W - PAD).max(U);
    let g = line_galley(ui, title, font_md(), t.ink, label_w);
    ui.painter().galley(egui::pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, t.ink);
    let track = paint_switch(ui, rect, id, *on, resp.hovered(), accent);
    focus_ring(ui, track, SWITCH_H / 2.0, resp.has_focus());
    let resp = resp.on_hover_text(state_tip(*on, tip));
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

/// Segmented control: rounded track, the chosen segment filled with the accent.
pub fn segment_strip<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str, &str)], total_w: f32, accent: Color32) -> bool {
    let n = options.len().max(1);
    let w = (total_w / n as f32).max(U * 7.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w * n as f32, HIT), Sense::hover());
    let t = cur(ui);
    let track = control_rect(rect);
    let p = ui.painter();
    p.rect_filled(track, RADIUS_SM, if t.dark { t.field } else { t.bg });
    p.rect_stroke(track.shrink(0.5), RADIUS_SM, hair(t.line));
    let mut changed = false;
    for (i, &(v, name, tip)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(egui::pos2(track.left() + i as f32 * w, track.top()), egui::vec2(w, track.height()));
        let resp = ui.interact(cell, ui.id().with(("seg", i, (cell.min.x as i32, cell.min.y as i32))), Sense::click());
        let selected = *value == v;
        let p = ui.painter();
        if selected {
            p.rect_filled(cell.shrink(2.0), RADIUS_SM - 1.0, accent);
        } else if resp.hovered() {
            p.rect_filled(cell.shrink(2.0), RADIUS_SM - 1.0, t.surface_hover);
        }
        let fg = if selected { INK_ON_COLOR } else { t.muted };
        let g = line_galley(ui, name, font_sm(), fg, w - U);
        p.galley(cell.center() - g.size() / 2.0, g, fg);
        focus_ring(ui, cell, RADIUS_SM, resp.has_focus());
        let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip) };
        if resp.clicked() && !selected {
            *value = v;
            changed = true;
        }
    }
    changed
}

/// Keycaps for a combo; on macOS with ⌃⌥⇧⌘ symbols.
pub fn keycaps(ui: &mut Ui, combo: &str, max_w: f32, active: bool, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_sm();
    let mut parts = combo_parts(combo);
    if mac_style() && parts.len() > 1 {
        // macOS writes shortcuts as one run of symbols: ⌃⇧V.
        let joined: String = parts.concat();
        if joined.chars().count() <= 6 {
            parts = vec![joined.chars().map(|c| c.to_string()).collect::<Vec<_>>().join("\u{2009}")];
        }
    }
    let widths: Vec<f32> = parts.iter().map(|s| (text_w(ui, s, f.clone()) + PAD * 2.0).max(CAP_H)).collect();
    let gap = if mac_style() { 2.0 } else { GAP };
    let total: f32 = widths.iter().sum::<f32>() + gap * (parts.len().saturating_sub(1) as f32);
    let fits = total <= max_w;
    let w = if fits { total } else { max_w.max(HIT) };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let p = ui.painter();
    let fill = if active { accent } else if resp.hovered() { t.surface_hover } else if t.dark { t.field } else { t.bg };
    let fg = if active { INK_ON_COLOR } else { t.ink };
    let edge = if active { shade(accent, -0.2) } else { t.line };
    let cap = |r: Rect, s: &str, max: f32| {
        p.rect_filled(r, RADIUS_SM, fill);
        p.rect_stroke(r.shrink(0.5), RADIUS_SM, hair(edge));
        // Keycap base: a slightly darker lower edge.
        p.line_segment([egui::pos2(r.left() + 2.0, r.bottom() - 0.5), egui::pos2(r.right() - 2.0, r.bottom() - 0.5)], hair(mix(edge, t.ink, 0.25)));
        let g = line_galley(ui, s, f.clone(), fg, max);
        p.galley(r.center() - g.size() / 2.0, g, fg);
    };
    let top = rect.center().y - CAP_H / 2.0;
    if fits {
        let mut x = rect.left();
        for (s, cw) in parts.iter().zip(&widths) {
            cap(Rect::from_min_size(egui::pos2(x, top), egui::vec2(*cw, CAP_H)), s, cw - PAD);
            x += cw + gap;
        }
    } else {
        cap(Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(w, CAP_H)), &parts.join(if mac_style() { "" } else { "+" }), w - PAD);
    }
    focus_ring(ui, Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(w, CAP_H)), RADIUS_SM, resp.has_focus());
    if fits { resp } else { resp.on_hover_text(combo) }
}

/// Text field: rounded, hairline border, accent border on focus.
pub fn field(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, password: bool, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let w = width.min(ui.available_width()).max(HIT);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::hover());
    let r = control_rect(rect);
    ui.painter().rect_filled(r, RADIUS_SM, t.field);
    let resp = ui.put(
        r,
        egui::TextEdit::singleline(text)
            .hint_text(egui::RichText::new(hint_text).color(t.muted))
            .password(password)
            .frame(false)
            .margin(egui::vec2(PAD, GAP))
            .desired_width(w - PAD * 2.0)
            .vertical_align(egui::Align::Center),
    );
    if resp.has_focus() {
        ui.painter().rect_stroke(r.shrink(0.5), RADIUS_SM, hair(shade(accent, -0.3)));
        ui.painter().rect_stroke(r.expand(1.5), RADIUS_SM + 1.5, Stroke::new(2.0, accent.gamma_multiply(0.6)));
    } else {
        ui.painter().rect_stroke(r.shrink(0.5), RADIUS_SM, hair(if resp.hovered() { t.muted } else { t.line }));
    }
    resp
}

pub fn add_line_plus(ui: &mut Ui, text: &mut String, hint_text: &str, tip: &str, accent: Color32) -> bool {
    add_line_plus_focus(ui, text, hint_text, tip, accent, false)
}

/// Like [`add_line_plus`]; when `request_focus` is set the field takes focus this frame.
/// `+` is disabled and Enter is ignored while the field is empty.
pub fn add_line_plus_focus(
    ui: &mut Ui,
    text: &mut String,
    hint_text: &str,
    tip: &str,
    accent: Color32,
    request_focus: bool,
) -> bool {
    let mut go = false;
    let can = !text.trim().is_empty();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = PAD;
        let fw = (ui.available_width() - HIT - PAD).max(HIT * 2.0);
        let r = field(ui, text, hint_text, fw, false, accent);
        if request_focus {
            r.request_focus();
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        go |= can && enter && (r.has_focus() || r.lost_focus());
        ui.add_enabled_ui(can, |ui| {
            go |= plus_square(ui, accent, tip).clicked();
        });
    });
    go
}

pub fn field_bg(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, accent: Color32) -> egui::Response {
    field(ui, text, hint_text, width, false, accent)
}

pub fn password_bg(ui: &mut Ui, text: &mut String, hint_text: &str, width: f32, accent: Color32) -> egui::Response {
    field(ui, text, hint_text, width, true, accent)
}

/// Row: label left, actions right (right-to-left), 32 px; `fill` TRANSPARENT = no
/// vertical padding, otherwise a 40 px row.
pub fn row(ui: &mut Ui, fill: Color32, label: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) -> egui::Response {
    let margin = if fill != Color32::TRANSPARENT { egui::Margin::symmetric(0.0, GAP) } else { egui::Margin::ZERO };
    egui::Frame::none()
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
        .response
}

/// 40 px row inside a group.
pub fn line_row(ui: &mut Ui, label: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) -> egui::Response {
    let t = cur(ui);
    row(ui, t.surface, label, actions)
}

/// Text action (no box), accent-dark underline on hover.
pub fn link(ui: &mut Ui, text: &str, accent: Color32) -> egui::Response {
    let t = cur(ui);
    let f = font_md();
    let w = (text_w(ui, text, f.clone()) + PAD).min(ui.available_width()).max(HIT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let g = line_galley(ui, text, f, t.ink, w);
    let pos = egui::pos2(rect.right() - g.size().x, rect.center().y - g.size().y / 2.0);
    let gw = g.size().x;
    let base = pos.y + g.size().y;
    ui.painter().galley(pos, g, t.ink);
    let c = if resp.hovered() { shade(accent, -0.3) } else { t.line };
    ui.painter().line_segment([egui::pos2(pos.x, base), egui::pos2(pos.x + gw, base)], hair(c));
    focus_ring(ui, rect, RADIUS_SM, resp.has_focus());
    resp
}

/// Word token: rounded, hairline, mono word, muted tag, × on the right.
pub fn chip(ui: &mut Ui, word: &str, tag: Option<&str>) -> bool {
    chip_with_tip(ui, word, tag, None)
}

/// Like [`chip`], but `tip` is shown on hover (e.g. a full path). If `tip` is
/// `None` and the label does not fit, the full `word` is used as the tooltip.
pub fn chip_with_tip(ui: &mut Ui, word: &str, tag: Option<&str>, tip: Option<&str>) -> bool {
    let t = cur(ui);
    let wf = font_mono();
    let tf = font_sm();
    let word_w = text_w(ui, word, wf.clone());
    let tag_w = tag.map_or(0.0, |s| text_w(ui, s, tf.clone()) + PAD);
    let x_w = U * 6.0;
    let pad = INSET;
    let fixed = pad + tag_w + x_w;
    let max_w = ui.max_rect().width().max(fixed + HIT);
    let shown = word_w.min(max_w - fixed);
    let width = ((fixed + shown) / U).ceil() * U;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, CAP_H + U * 2.0), Sense::hover());
    let r = Rect::from_center_size(rect.center(), egui::vec2(width, CAP_H + U));
    let p = ui.painter();
    p.rect_filled(r, RADIUS_SM, if t.dark { t.field } else { t.bg });
    p.rect_stroke(r.shrink(0.5), RADIUS_SM, hair(t.line));
    let g = line_galley(ui, word, wf, t.ink, shown + 0.5);
    p.galley(egui::pos2(r.left() + pad, r.center().y - g.size().y / 2.0), g, t.ink);
    if let Some(s) = tag {
        p.text(egui::pos2(r.left() + pad + shown + PAD, r.center().y), egui::Align2::LEFT_CENTER, s, tf, t.muted);
    }
    let hover = tip.filter(|s| !s.is_empty()).or_else(|| (shown < word_w - 0.5).then_some(word));
    if let Some(msg) = hover {
        resp.clone().on_hover_text(msg);
    }
    let xr = Rect::from_min_size(egui::pos2(r.right() - x_w, r.top()), egui::vec2(x_w, r.height()));
    let xresp = ui.interact(xr, ui.id().with(("chip-x", word)), Sense::click());
    let c = xr.center();
    let arm = 3.5;
    let col = if xresp.hovered() { t.ink } else { t.muted };
    if xresp.hovered() {
        p.circle_filled(c, 8.0, t.surface_hover);
    }
    p.line_segment([c + egui::vec2(-arm, -arm), c + egui::vec2(arm, arm)], Stroke::new(1.2, col));
    p.line_segment([c + egui::vec2(-arm, arm), c + egui::vec2(arm, -arm)], Stroke::new(1.2, col));
    xresp.on_hover_text("Удалить").clicked()
}

/// Toggle followed by its label.
pub fn check(ui: &mut Ui, on: &mut bool, text: &str, accent: Color32) -> bool {
    let t = cur(ui);
    let f = font_md();
    let w = (SWITCH_W + PAD + text_w(ui, text, f.clone())).min(ui.available_width()).max(SWITCH_W);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HIT), Sense::click());
    let b = Rect::from_min_size(rect.min, egui::vec2(SWITCH_W, HIT));
    let track = paint_switch(ui, b, resp.id, *on, resp.hovered(), accent);
    let g = line_galley(ui, text, f, t.ink, w - SWITCH_W - PAD);
    ui.painter().galley(egui::pos2(b.right() + PAD, rect.center().y - g.size().y / 2.0), g, t.ink);
    focus_ring(ui, track, SWITCH_H / 2.0, resp.has_focus());
    let resp = resp.on_hover_text(state_tip(*on, text));
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

/// Day (light theme) and dusk (dark theme) photographic backdrops.
pub fn sea_jpeg(dark: bool) -> &'static [u8] {
    if dark {
        include_bytes!("../../../assets/bg-sea-dusk.jpg")
    } else {
        include_bytes!("../../../assets/bg-sea-day.jpg")
    }
}

pub fn load_sea_texture(ctx: &egui::Context, dark: bool) -> egui::TextureHandle {
    let (w, h, rgb) = sea_srgb(dark);
    ctx.load_texture(
        if dark { "sea-dusk" } else { "sea-day" },
        egui::ColorImage::from_rgb([w, h], &rgb),
        egui::TextureOptions::LINEAR,
    )
}

/// Decode a backdrop JPEG to packed sRGB bytes (no gamma conversion, no premultiply).
pub fn sea_srgb(dark: bool) -> (usize, usize, Vec<u8>) {
    let img = image::load_from_memory(sea_jpeg(dark)).expect("sea backdrop jpeg").to_rgb8();
    (img.width() as usize, img.height() as usize, img.into_raw())
}

/// Tight frosted plate behind header controls so they stay readable on the photo
/// without washing the rest of the strip.
pub fn frost_plate(ui: &Ui, rect: Rect) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let t = cur(ui);
    let a = if t.dark { 200 } else { 215 };
    ui.painter().rect_filled(
        rect,
        RADIUS_SM,
        Color32::from_rgba_unmultiplied(t.surface.r(), t.surface.g(), t.surface.b(), a),
    );
    ui.painter().rect_stroke(rect.shrink(0.5), RADIUS_SM, hair(t.line));
}

/// Draw `tex` to cover `rect` (crop overflow, never letterbox). `align_y` 0 = keep
/// the top of the photo (the water), 1 = the bottom (the sand).
pub fn paint_photo(ui: &Ui, rect: Rect, tex: &egui::TextureHandle, align_y: f32, tint: Color32) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let size = tex.size_vec2();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let tex_a = size.x / size.y;
    let rec_a = rect.width() / rect.height();
    let draw = if tex_a > rec_a {
        let h = rect.height();
        let w = h * tex_a;
        Rect::from_center_size(rect.center(), egui::vec2(w, h))
    } else {
        let w = rect.width();
        let h = w / tex_a;
        let y = rect.top() - (h - rect.height()) * align_y.clamp(0.0, 1.0);
        Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(w, h))
    };
    let mut mesh = egui::Mesh::with_texture(tex.id());
    mesh.add_rect_with_uv(draw, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
    ui.painter().with_clip_rect(rect).add(egui::Shape::mesh(mesh));
}

/// Soft shadow under a home card, so it sits on the photo instead of melting into it.
pub fn card_shadow(ui: &Ui, rect: Rect) {
    let t = cur(ui);
    let color = Color32::from_black_alpha(if t.dark { 90 } else { 40 });
    let shadow = egui::epaint::Shadow { offset: egui::vec2(0.0, 3.0), blur: 10.0, spread: 0.0, color };
    ui.painter().add(shadow.as_shape(rect, RADIUS));
}

/// Subtle dark frost under a caption on the dusk sand. The pale day sand already
/// keeps dark ink well above WCAG AA, so the light theme has no plate.
pub fn caption_plate(ui: &Ui, pos: egui::Pos2, size: Vec2) {
    let t = cur(ui);
    if !t.dark {
        return;
    }
    let pad_x = INSET;
    let pad_y = 3.0;
    let r = Rect::from_min_size(egui::pos2(pos.x - pad_x, pos.y - pad_y), egui::vec2(size.x + pad_x * 2.0, size.y + pad_y * 2.0));
    ui.painter().rect_filled(r, RADIUS_SM, Color32::from_rgba_unmultiplied(t.surface.r(), t.surface.g(), t.surface.b(), 170));
}

/// Section mark: a solid circle in the section colour with the glyph in ink.

pub fn paint_mark(p: &egui::Painter, c: egui::Pos2, d: f32, color: Color32, idx: usize) {
    p.circle_filled(c, d / 2.0, color);
    p.circle_stroke(c, d / 2.0 - 0.5, hair(shade(color, -0.12)));
    paint_home_icon(p, c, (d * 0.22).round(), idx);
}

/// Chevron «‹» for the back button.
pub fn paint_chevron(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let s = Stroke::new(1.6, color);
    p.line_segment([egui::pos2(c.x + 3.0, c.y - 6.0), egui::pos2(c.x - 3.0, c.y)], s);
    p.line_segment([egui::pos2(c.x - 3.0, c.y), egui::pos2(c.x + 3.0, c.y + 6.0)], s);
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
            assert_aa(&format!("{tag} muted on surface_hover"), t.muted, t.surface_hover);
            assert_aa(&format!("{tag} muted_strong on bg"), t.muted_strong, t.bg);
            assert_aa(&format!("{tag} muted_strong on surface"), t.muted_strong, t.surface);
            assert_aa(&format!("{tag} muted_strong on surface_hover"), t.muted_strong, t.surface_hover);
            assert_aa(&format!("{tag} muted_strong on field"), t.muted_strong, t.field);
        }
        // Sampled plates from the UX tester: counts sit near a rounded card
        // corner or on a slightly mixed group caption.
        let dark = theme(true);
        assert_aa("dark count on mixed card #404A4D", dark.muted_strong, Color32::from_rgb(0x40, 0x4A, 0x4D));
        let light = theme(false);
        assert_aa("light group count on mixed #D5D0C8", light.muted_strong, Color32::from_rgb(0xD5, 0xD0, 0xC8));
        assert_aa("light letter ink on mixed plate #BCB7AF", light.ink, Color32::from_rgb(0xBC, 0xB7, 0xAF));
        assert_aa("dark letter ink on surface_hover", dark.ink, dark.surface_hover);
    }

    #[test]
    fn ink_on_section_colours_meets_wcag_aa() {
        for (name, c) in [("sun", SUN), ("sand", SAND), ("seafoam", SEAFOAM), ("sea", SEA), ("coral", CORAL)] {
            assert_aa(&format!("ink on {name}"), INK_ON_COLOR, c);
        }
        // Aliases still point at the same swatches.
        assert_eq!(YELLOW, SUN);
        assert_eq!(ORANGE, SAND);
        assert_eq!(LIME, SEAFOAM);
        assert_eq!(CYAN, SEA);
        assert_eq!(RED, CORAL);
    }

    #[test]
    fn sea_jpegs_load() {
        for dark in [false, true] {
            let bytes = sea_jpeg(dark);
            assert!(bytes.len() < 200 * 1024, "backdrop should stay under 200 KB");
            let img = image::load_from_memory(bytes).expect("jpeg");
            let (w, h) = (img.width(), img.height());
            assert!(w >= 1024 && h >= 384, "backdrop is too small for a 2× window, got {w}×{h}");
        }
    }

    #[test]
    fn mac_keycaps_use_symbols() {
        set_mac_preview(false);
        assert_eq!(combo_parts("Ctrl+Alt+Z"), ["Ctrl", "Alt", "Z"]);
        assert_eq!(combo_parts("+"), ["+"]);
        set_mac_preview(true);
        assert_eq!(combo_parts("Ctrl+Option+Delete"), ["⌃", "⌥", "⌫"]);
        assert_eq!(combo_parts("Ctrl+Shift+V"), ["⌃", "⇧", "V"]);
        assert_eq!(combo_parts("Cmd+Space"), ["⌘", "Space"]);
        set_mac_preview(false);
    }

    #[test]
    fn broken_font_files_are_ignored() {
        assert!(!usable_font(b"not a font", 0));
        assert!(load_font(std::path::Path::new("/nonexistent/SFNS.ttf"), 0).is_none());
        // Whatever the platform offers, installing never panics.
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        install_fonts_with(&ctx, None);
    }

    #[test]
    fn grid_constants_are_multiples_of_u() {
        for (n, v) in [("GAP", GAP), ("PAD", PAD), ("INSET", INSET), ("MARGIN", MARGIN), ("HIT", HIT), ("ROW", ROW), ("MARK", MARK), ("HEADER", HEADER), ("SWITCH_W", SWITCH_W), ("SWITCH_H", SWITCH_H), ("CAP_H", CAP_H), ("RADIUS_SM", RADIUS_SM)] {
            assert!((v / U).fract().abs() < 1e-6, "{n}={v} is not a multiple of {U}");
        }
        assert_eq!(U, 4.0);
        assert_eq!(HIT, 32.0);
        assert_eq!(ROW, 40.0);
        assert!((SWITCH_W / SWITCH_H - 1.8).abs() < 1e-6, "switch track is 9:5 like the macOS / Windows 11 toggles");
        assert!(RADIUS_SM < RADIUS && RADIUS <= 6.0, "one small radius family");
        assert!((FONT_LG / FONT_MD - FONT_MD / FONT_SM).abs() < 1e-6, "type scale is not a single ratio");
        for (a, b) in [(FONT_SM, FONT_MD), (FONT_MD, FONT_LG), (FONT_LG, FONT_XL), (FONT_XL, FONT_XXL)] {
            assert!((b / a - 1.2).abs() < 1e-6);
        }
    }

    #[test]
    fn sea_jpeg_is_loaded_as_srgb() {
        for dark in [false, true] {
            let (w, h, rgb) = sea_srgb(dark);
            assert!(w > 0 && h > 0 && rgb.len() == w * h * 3, "dark={dark}");
            let img = egui::ColorImage::from_rgb([w, h], &rgb);
            // from_rgb must keep the JPEG's 8-bit sRGB bytes (no extra gamma, no premultiply).
            for (i, px) in img.pixels.iter().enumerate().step_by((w * h / 64).max(1)) {
                let o = i * 3;
                assert_eq!([px.r(), px.g(), px.b()], [rgb[o], rgb[o + 1], rgb[o + 2]], "px {i} dark={dark}");
                assert_eq!(px.a(), 255);
            }
        }
    }
}
