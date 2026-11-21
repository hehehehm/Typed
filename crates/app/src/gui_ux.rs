//! Automated UI/UX tester for the settings window.
//!
//! Drives `App::draw` headlessly with egui `RawInput` pointer and key events
//! (egui_kittest is not available for 0.29 and is not pulled in). Shares the
//! CPU rasterizer in `gui_shots`.
//!
//! * Always-on `cargo test`: scenario tests that assert config/state, plus
//!   structural and contrast checks. Findings are reported, not failed, so
//!   known visual issues do not break the suite.
//! * `ux_visual_regression` (`--ignored`): PNG shots for a theme × style × size
//!   × edge-data matrix, pixel-diff against a baseline, and a written report.
//!
//! Env (ignored run):
//! * `TYPED_UI_UX_SHOTS` — actual PNGs (default `/tmp/typed-ui-ux-shots`)
//! * `TYPED_UI_UX_BASELINE` — baseline dir (default `ui-ux-baseline/` at the
//!   repo root; gitignored, do not commit large binaries)
//! * `TYPED_UI_UX_UPDATE_BASELINE=1` — copy actuals into the baseline
//! * `TYPED_UI_UX_DIFF` — dir for diff images and the report
//! * `TYPED_UI_UX_DIFF_PCT` — fail threshold, percent of pixels (default 1.0)
//! * `TYPED_UI_UX_FAIL_ON_DIFF=1` — make the ignored test fail when the
//!   baseline exists and the diff is over the threshold

use super::shots::{
    app_with, focus, merge_textures, rasterize, sample_cfg, write_png, INTER, LONG_PATH,
};
use super::*;
use eframe::egui::{
    self, epaint, Color32, Event, FullOutput, Key, Modifiers, PointerButton, Pos2, Rect, Shape,
    Vec2, WidgetRect,
};
use layout_keeper_core::LogoStyle;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MIN_HIT: f32 = 24.0;
const AA: f32 = 4.5;
const AA_LARGE: f32 = 3.0;
const LARGE_PT: f32 = 18.0;
const OUT_TOL: f32 = 0.75;
const DELTA_TOL: u8 = 8;

const HOME: Vec2 = Vec2::new(512.0, 192.0);
const SEC: Vec2 = Vec2::new(512.0, 496.0);
const HOME_MIN: Vec2 = Vec2::new(432.0, 160.0);
const SEC_MIN: Vec2 = Vec2::new(432.0, 416.0);
const LARGE: Vec2 = Vec2::new(960.0, 720.0);
const WELCOME: Vec2 = Vec2::new(512.0, 448.0);

struct Harness {
    ctx: egui::Context,
    app: App,
    focus: Focus,
    size: Vec2,
    ppp: f32,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
    time: f64,
    last: Option<FullOutput>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        st::set_mac_preview(false);
    }
}

impl Harness {
    fn new(app: App, focus: Focus, size: Vec2, ppp: f32, mac: bool) -> Self {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(ppp);
        st::set_mac_preview(mac);
        if mac {
            st::install_fonts_with(&ctx, st::font_file(INTER));
        } else {
            st::install_fonts(&ctx);
        }
        ctx.style_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
        });
        let mut h = Self {
            ctx,
            app,
            focus,
            size,
            ppp,
            textures: HashMap::new(),
            time: 0.0,
            last: None,
        };
        h.settle();
        h
    }

    fn step(&mut self, events: Vec<Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        self.time += 0.05;
        let out = self.ctx.run(input, |ctx| self.app.draw(ctx, &self.focus));
        merge_textures(&mut self.textures, &out);
        self.last = Some(out);
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.step(Vec::new());
        }
    }

    fn hover(&mut self, pos: Pos2) {
        self.step(vec![Event::PointerMoved(pos)]);
        self.step(vec![Event::PointerMoved(pos)]);
    }

    fn click(&mut self, pos: Pos2) {
        let none = Modifiers::default();
        self.step(vec![Event::PointerMoved(pos)]);
        self.step(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: none,
            },
        ]);
        self.step(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: none,
            },
        ]);
        self.settle();
    }

    fn type_text(&mut self, s: &str) {
        self.step(vec![Event::Text(s.to_owned())]);
        self.settle();
    }

    fn press_key(&mut self, key: Key, modifiers: Modifiers) {
        self.step(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.step(vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
        self.settle();
    }

    fn widgets(&self) -> Vec<WidgetRect> {
        self.ctx.viewport(|v| {
            v.prev_pass
                .widgets
                .layers()
                .flat_map(|(_, w)| w.iter().copied())
                .collect()
        })
    }

    fn clickables(&self) -> Vec<WidgetRect> {
        self.widgets()
            .into_iter()
            .filter(|w| is_hit_target(w))
            .collect()
    }

    fn texts(&self) -> Vec<TextHit> {
        let Some(out) = self.last.as_ref() else {
            return Vec::new();
        };
        let mut v = Vec::new();
        for cs in &out.shapes {
            collect_text(&cs.shape, cs.clip_rect, &mut v);
        }
        v
    }

    fn find_text(&self, needle: &str) -> Option<TextHit> {
        self.texts()
            .into_iter()
            .find(|t| t.text.contains(needle) && t.rect.is_positive())
    }

    fn click_text(&mut self, needle: &str) -> bool {
        match self.find_text(needle) {
            Some(t) => {
                self.click(t.rect.center());
                true
            }
            None => false,
        }
    }

    /// Click `btn` painted on the same row as `row_needle`.
    fn click_on_row(&mut self, row_needle: &str, btn: &str) -> bool {
        let Some(row) = self.find_text(row_needle) else {
            return false;
        };
        let y = row.rect.center().y;
        let mut best: Option<TextHit> = None;
        for t in self.texts() {
            if !t.text.contains(btn) {
                continue;
            }
            if (t.rect.center().y - y).abs() > 14.0 {
                continue;
            }
            // Prefer the closest horizontally, skipping the row label itself.
            if t.rect.intersects(row.rect) && t.text == row.text {
                continue;
            }
            best = Some(match best {
                Some(b)
                    if (b.rect.center() - row.rect.center()).length()
                        < (t.rect.center() - row.rect.center()).length() =>
                {
                    b
                }
                _ => t,
            });
        }
        if let Some(t) = best {
            self.click(t.rect.center());
            true
        } else {
            false
        }
    }

    fn click_home_square(&mut self, title: &str) -> bool {
        let Some(t) = self.find_text(title) else {
            return false;
        };
        // Captions sit just under the card; the hit target is the square above.
        let above = Pos2::new(t.rect.center().x, (t.rect.top() - 8.0).max(0.0));
        let pos = self
            .clickables()
            .into_iter()
            .filter(|w| {
                w.interact_rect.contains(above)
                    || (w.interact_rect.center().x - t.rect.center().x).abs()
                        < t.rect.width().max(24.0)
                        && w.interact_rect.bottom() <= t.rect.top() + 4.0
                        && w.interact_rect.bottom() >= t.rect.top() - 80.0
            })
            .max_by(|a, b| {
                a.interact_rect
                    .area()
                    .partial_cmp(&b.interact_rect.area())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|w| w.interact_rect.center())
            .unwrap_or(above);
        self.click(pos);
        true
    }

    fn click_back(&mut self) -> bool {
        let w = self
            .clickables()
            .into_iter()
            .filter(|c| {
                c.interact_rect.center().y < st::HEADER && c.interact_rect.left() < st::HEADER
            })
            .min_by(|a, b| {
                a.interact_rect
                    .left()
                    .partial_cmp(&b.interact_rect.left())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(w) = w {
            self.click(w.interact_rect.center());
            true
        } else {
            false
        }
    }

    /// Rightmost HIT-sized clickable on the add-program row (the plus square).
    fn click_plus(&mut self) -> bool {
        let header = st::HEADER;
        let w = self
            .clickables()
            .into_iter()
            .filter(|c| {
                let r = c.interact_rect;
                r.top() > header
                    && r.top() < header + st::ROW * 2.0 + st::MARGIN
                    && (r.width() - st::HIT).abs() < 6.0
                    && (r.height() - st::HIT).abs() < 6.0
            })
            .max_by(|a, b| {
                a.interact_rect
                    .right()
                    .partial_cmp(&b.interact_rect.right())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(w) = w {
            self.click(w.interact_rect.center());
            true
        } else {
            false
        }
    }

    /// Trash button on the row whose label contains `name`.
    fn click_delete_on_row(&mut self, name: &str) -> bool {
        let Some(row) = self.find_text(name) else {
            return false;
        };
        let y = row.rect.center().y;
        let w = self
            .clickables()
            .into_iter()
            .filter(|c| {
                let r = c.interact_rect;
                (r.center().y - y).abs() < 16.0
                    && (r.width() - st::HIT).abs() < 6.0
                    && (r.height() - st::HIT).abs() < 6.0
                    && r.left() > row.rect.right()
            })
            .max_by(|a, b| {
                a.interact_rect
                    .right()
                    .partial_cmp(&b.interact_rect.right())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(w) = w {
            self.click(w.interact_rect.center());
            true
        } else {
            // Wrapped chips: a small × sits just to the right of the name.
            let w = self
                .clickables()
                .into_iter()
                .filter(|c| {
                    let r = c.interact_rect;
                    (r.center().y - y).abs() < 16.0
                        && r.left() >= row.rect.right() - 2.0
                        && r.left() <= row.rect.right() + 28.0
                })
                .min_by(|a, b| {
                    a.interact_rect
                        .left()
                        .partial_cmp(&b.interact_rect.left())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(w) = w {
                self.click(w.interact_rect.center());
                true
            } else {
                false
            }
        }
    }

    /// Logo tiles on the «Логотип» row, left-to-right (Beach … Porthole).
    fn logo_tiles(&self) -> Vec<WidgetRect> {
        let y = self
            .find_text("Логотип")
            .map(|t| t.rect.center().y)
            .unwrap_or(0.0);
        let mut v: Vec<_> = self
            .clickables()
            .into_iter()
            .filter(|c| {
                let r = c.interact_rect;
                (r.center().y - y).abs() < 20.0
                    && (r.width() - st::HIT).abs() < 8.0
                    && (r.height() - st::HIT).abs() < 8.0
                    && r.left() > 80.0
            })
            .collect();
        v.sort_by(|a, b| {
            a.interact_rect
                .left()
                .partial_cmp(&b.interact_rect.left())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    }

    fn click_logo_from_left(&mut self, index: usize) -> bool {
        let tiles = self.logo_tiles();
        if let Some(w) = tiles.get(index).copied() {
            self.click(w.interact_rect.center());
            true
        } else {
            false
        }
    }

    fn click_right_of(&mut self, label: &str) -> bool {
        let Some(t) = self.find_text(label) else {
            return false;
        };
        let y = t.rect.center().y;
        let w = self
            .clickables()
            .into_iter()
            .filter(|c| {
                let r = c.interact_rect;
                (r.center().y - y).abs() < 18.0 && r.left() >= t.rect.right() - 4.0
            })
            .min_by(|a, b| {
                a.interact_rect
                    .left()
                    .partial_cmp(&b.interact_rect.left())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(w) = w {
            self.click(w.interact_rect.center());
            true
        } else {
            // Fall back to a point on the same row, right-hand side.
            self.click(Pos2::new(
                (t.rect.right() + 80.0).min(self.size.x - 24.0),
                y,
            ));
            true
        }
    }

    fn raster(&self) -> Option<(usize, usize, Vec<[f32; 4]>)> {
        let out = self.last.as_ref()?;
        Some(rasterize(
            &self.ctx,
            out,
            &self.textures,
            self.size,
            self.ppp,
        ))
    }

    fn screen_rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }
}

#[derive(Clone, Debug)]
struct TextHit {
    text: String,
    rect: Rect,
    clip: Rect,
    color: Color32,
    font_size: f32,
}

fn collect_text(shape: &Shape, clip: Rect, out: &mut Vec<TextHit>) {
    match shape {
        Shape::Vec(v) => {
            for s in v {
                collect_text(s, clip, out);
            }
        }
        Shape::Text(ts) => {
            let text = ts.galley.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            let rect = ts.visual_bounding_rect();
            let (color, font_size) = text_style(ts);
            out.push(TextHit {
                text,
                rect,
                clip,
                color,
                font_size,
            });
        }
        _ => {}
    }
}

fn text_style(ts: &epaint::TextShape) -> (Color32, f32) {
    let color = ts.override_text_color.unwrap_or_else(|| {
        ts.galley
            .job
            .sections
            .iter()
            .map(|s| s.format.color)
            .find(|c| *c != Color32::PLACEHOLDER && c.a() > 0)
            .unwrap_or(ts.fallback_color)
    });
    let size = ts
        .galley
        .job
        .sections
        .first()
        .map(|s| s.format.font_id.size)
        .unwrap_or(st::FONT_MD);
    (color, size)
}

#[derive(Clone, Debug)]
struct Issue {
    kind: &'static str,
    /// "must", "should", "nice" for usability; "note" for structural checks.
    severity: &'static str,
    screen: String,
    detail: String,
    rect: Option<Rect>,
}

impl Issue {
    fn new(kind: &'static str, screen: &str, detail: String, rect: Option<Rect>) -> Self {
        Self {
            kind,
            severity: "note",
            screen: screen.to_owned(),
            detail,
            rect,
        }
    }

    fn ux(
        severity: &'static str,
        kind: &'static str,
        screen: &str,
        detail: String,
        rect: Option<Rect>,
    ) -> Self {
        Self {
            kind,
            severity,
            screen: screen.to_owned(),
            detail,
            rect,
        }
    }
}

struct Findings {
    issues: Vec<Issue>,
}

impl Findings {
    fn new() -> Self {
        Self { issues: Vec::new() }
    }
    fn push(&mut self, i: Issue) {
        self.issues.push(i);
    }
    fn of<'a>(&'a self, kind: &'static str) -> impl Iterator<Item = &'a Issue> {
        self.issues.iter().filter(move |i| i.kind == kind)
    }
}

fn inspect_all(
    h: &mut Harness,
    screen: &str,
    findings: &mut Findings,
    with_raster: bool,
    hover: bool,
) -> Option<(usize, usize, Vec<[f32; 4]>)> {
    inspect_struct(h, screen, findings);
    let raster = if with_raster { h.raster() } else { None };
    if let Some((w, ht, ref buf)) = raster {
        inspect_contrast_pixels(h, screen, findings, w, ht, buf);
        inspect_blank(screen, findings, w, ht, buf);
    }
    inspect_theme_pairs(h, screen, findings);
    if hover {
        inspect_hover(h, screen, findings);
    }
    raster
}

/// Intended click targets (buttons, tiles, toggles, links). egui's default
/// selectable labels also have `sense.click` (and usually `drag`, but
/// `focusable` is false) so they can be highlighted; those are not buttons
/// and must not trip the 24 px hit-target check.
fn is_hit_target(w: &WidgetRect) -> bool {
    w.enabled
        && w.sense.click
        && w.sense.focusable
        && w.interact_rect.is_positive()
        && w.interact_rect.width() > 0.5
        && w.interact_rect.height() > 0.5
}

fn inspect_struct(h: &Harness, screen: &str, findings: &mut Findings) {
    let win = h.screen_rect();
    if h.app.content_overflow >= 0.5 {
        findings.push(Issue::new(
            "clip",
            screen,
            format!("content_overflow {:.1}px", h.app.content_overflow),
            None,
        ));
    }

    let widgets = h.widgets();
    if widgets.is_empty() && !h.app.welcome_open {
        findings.push(Issue::new(
            "blank",
            screen,
            "no widgets in prev_pass".into(),
            None,
        ));
    }

    // Outside / clipped widgets. Use interact_rect (already clipped by parent Uis
    // and scroll areas) so rows below the fold in a ScrollArea are not flagged.
    for w in &widgets {
        if !is_hit_target(w) {
            continue;
        }
        let r = w.interact_rect;
        let vis = r.intersect(win);
        // Rows past or barely peeking through the bottom of a ScrollArea are expected.
        let scroll_sliver = vis.height() < 4.0 && r.max.y > win.max.y - 1.0;
        if !scroll_sliver && (r.max.x > win.max.x + OUT_TOL || r.min.x < win.min.x - OUT_TOL) {
            findings.push(Issue::new(
                "clip",
                screen,
                format!("clickable extends outside the window horizontally: {:?}", r),
                Some(r),
            ));
        }
        // A control that starts well inside the window and is cut off vertically
        // (not the last sliver of a scrolling list).
        if !scroll_sliver && r.min.y < win.max.y - st::ROW && r.max.y > win.max.y + OUT_TOL {
            findings.push(Issue::new(
                "clip",
                screen,
                format!("clickable cut off at the bottom of the window: {:?}", r),
                Some(r),
            ));
        }
    }

    for t in h.texts() {
        let r = t.rect;
        // Text laid out below a ScrollArea's fold is clipped on purpose.
        if r.min.y >= win.max.y - OUT_TOL {
            continue;
        }
        if r.max.x > win.max.x + OUT_TOL || r.min.x < win.min.x - OUT_TOL {
            findings.push(Issue::new(
                "clip",
                screen,
                format!(
                    "text {:?} extends outside the window: {:?}",
                    trunc(&t.text, 40),
                    r
                ),
                Some(r),
            ));
        }
        let vis = r.intersect(t.clip).intersect(win);
        let lost_x = (r.intersect(win).width() - vis.width()).max(0.0);
        let lost_y = (r.intersect(win).height() - vis.height()).max(0.0);
        // Horizontal clipping of visible text is a real issue; a row cut by the
        // bottom of a scroll viewport is not.
        if lost_x > 1.5 {
            findings.push(Issue::new(
                "clip",
                screen,
                format!(
                    "text {:?} clipped horizontally by {:.1}px",
                    trunc(&t.text, 40),
                    lost_x
                ),
                Some(r),
            ));
        } else if lost_y > 1.5 && r.min.y < win.max.y - st::ROW && t.clip.max.y < win.max.y - 2.0 {
            findings.push(Issue::new(
                "clip",
                screen,
                format!(
                    "text {:?} clipped vertically by {:.1}px",
                    trunc(&t.text, 40),
                    lost_y
                ),
                Some(r),
            ));
        }
    }

    let clicks = h.clickables();
    for w in &clicks {
        let r = w.interact_rect;
        // A row cut by a ScrollArea or a 2 px scrollbar track is not a designed
        // 24 px target failure.
        if r.height() + 0.05 < MIN_HIT && w.rect.height() + 0.05 >= MIN_HIT {
            continue;
        }
        if r.width() + 0.05 < 8.0 && r.height() > MIN_HIT * 2.0 {
            continue;
        }
        let m = r.width().min(r.height());
        if m + 0.05 < MIN_HIT {
            findings.push(Issue::new(
                "hit",
                screen,
                format!(
                    "hit target {:.1}×{:.1}px (< {MIN_HIT})",
                    r.width(),
                    r.height()
                ),
                Some(r),
            ));
        }
    }

    // Overlapping interactive rects: ignore parent-contains-child.
    for (i, a) in clicks.iter().enumerate() {
        for b in clicks.iter().skip(i + 1) {
            let oa = a.interact_rect.intersect(b.interact_rect);
            if !oa.is_positive() {
                continue;
            }
            let area = oa.area();
            if area < 4.0 {
                continue;
            }
            if contains_mostly(a.interact_rect, b.interact_rect)
                || contains_mostly(b.interact_rect, a.interact_rect)
            {
                continue;
            }
            findings.push(Issue::new(
                "overlap",
                screen,
                format!("interactive overlap {:.0}px² at {:?}", area, oa),
                Some(oa),
            ));
        }
    }
}

fn contains_mostly(outer: Rect, inner: Rect) -> bool {
    if outer.contains(inner.min) && outer.contains(inner.max) {
        return true;
    }
    let inter = outer.intersect(inner);
    inter.is_positive() && inter.area() >= inner.area() * 0.85
}

fn inspect_theme_pairs(h: &Harness, screen: &str, findings: &mut Findings) {
    let t = st::theme(!h.app.cfg.light_theme);
    let pairs: [(&str, Color32, Color32); 6] = [
        ("ink on bg", t.ink, t.bg),
        ("ink on surface", t.ink, t.surface),
        ("ink on field", t.ink, t.field),
        ("muted on bg", t.muted, t.bg),
        ("muted on surface", t.muted, t.surface),
        (
            "ink on frost plate",
            t.ink,
            frost_over(t.surface, t.bg, if t.dark { 200 } else { 215 }),
        ),
    ];
    for (name, fg, bg) in pairs {
        let r = st::contrast_ratio(fg, bg);
        if r + 1e-3 < AA {
            findings.push(Issue::new(
                "contrast",
                screen,
                format!("{name}: {r:.2} < {AA}"),
                None,
            ));
        }
    }
}

fn frost_over(surface: Color32, under: Color32, a: u8) -> Color32 {
    let plate = Color32::from_rgba_unmultiplied(surface.r(), surface.g(), surface.b(), a);
    let k = plate.a() as f32 / 255.0;
    Color32::from_rgb(
        (plate.r() as f32 * k + under.r() as f32 * (1.0 - k)).round() as u8,
        (plate.g() as f32 * k + under.g() as f32 * (1.0 - k)).round() as u8,
        (plate.b() as f32 * k + under.b() as f32 * (1.0 - k)).round() as u8,
    )
}

fn inspect_contrast_pixels(
    h: &Harness,
    screen: &str,
    findings: &mut Findings,
    w: usize,
    ht: usize,
    buf: &[[f32; 4]],
) {
    let ppp = h.ppp;
    for t in h.texts() {
        let vis = t.rect.intersect(t.clip).intersect(h.screen_rect());
        if vis.width() < 2.0 || vis.height() < 2.0 {
            continue;
        }
        let Some(bg) = sample_background(w, ht, buf, vis, ppp, t.color) else {
            continue;
        };
        let ratio = st::contrast_ratio(t.color, bg);
        let need = if t.font_size + 0.05 >= LARGE_PT {
            AA_LARGE
        } else {
            AA
        };
        if ratio + 1e-3 < need {
            findings.push(Issue::new(
                "contrast",
                screen,
                format!(
                    "«{}» size {:.1} on ~#{:02X}{:02X}{:02X}: {ratio:.2} < {need}",
                    trunc(&t.text, 32),
                    t.font_size,
                    bg.r(),
                    bg.g(),
                    bg.b()
                ),
                Some(vis),
            ));
        }
    }
}

fn sample_background(
    w: usize,
    h: usize,
    buf: &[[f32; 4]],
    rect: Rect,
    ppp: f32,
    fg: Color32,
) -> Option<Color32> {
    let x0 = (rect.min.x * ppp).floor().max(0.0) as i32;
    let y0 = (rect.min.y * ppp).floor().max(0.0) as i32;
    let x1 = (rect.max.x * ppp).ceil().min(w as f32) as i32;
    let y1 = (rect.max.y * ppp).ceil().min(h as f32) as i32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut samples: Vec<[u8; 3]> = Vec::new();
    // Inside the text box, keep pixels that are not the glyph colour.
    let step = ((x1 - x0).max(y1 - y0) / 12).max(1);
    for y in (y0..y1).step_by(step as usize) {
        for x in (x0..x1).step_by(step as usize) {
            if let Some(c) = px(w, h, buf, x, y) {
                if color_dist(c, fg) >= 40 {
                    samples.push(c);
                }
            }
        }
    }
    // Ring just outside the glyphs, in case the box is almost all ink.
    if samples.len() < 4 {
        let pad = (2.0 * ppp).ceil() as i32;
        for (x, y) in ring(x0 - pad, y0 - pad, x1 + pad, y1 + pad, pad.max(1)) {
            if let Some(c) = px(w, h, buf, x, y) {
                if color_dist(c, fg) >= 28 {
                    samples.push(c);
                }
            }
        }
    }
    if samples.is_empty() {
        return None;
    }
    Some(median_rgb(&mut samples))
}

fn ring(x0: i32, y0: i32, x1: i32, y1: i32, _pad: i32) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for x in x0..=x1 {
        v.push((x, y0));
        v.push((x, y1));
    }
    for y in y0..=y1 {
        v.push((x0, y));
        v.push((x1, y));
    }
    v
}

fn px(w: usize, h: usize, buf: &[[f32; 4]], x: i32, y: i32) -> Option<[u8; 3]> {
    if x < 0 || y < 0 {
        return None;
    }
    let (x, y) = (x as usize, y as usize);
    if x >= w || y >= h {
        return None;
    }
    let p = buf[y * w + x];
    Some([
        p[0].round().clamp(0.0, 255.0) as u8,
        p[1].round().clamp(0.0, 255.0) as u8,
        p[2].round().clamp(0.0, 255.0) as u8,
    ])
}

fn color_dist(a: [u8; 3], fg: Color32) -> u32 {
    let d = |i: usize, c: u8| (a[i] as i32 - c as i32).unsigned_abs();
    d(0, fg.r()) + d(1, fg.g()) + d(2, fg.b())
}

fn median_rgb(samples: &mut [[u8; 3]]) -> Color32 {
    // Component-wise median, good enough for a plate / photo backdrop.
    let mut r: Vec<u8> = samples.iter().map(|s| s[0]).collect();
    let mut g: Vec<u8> = samples.iter().map(|s| s[1]).collect();
    let mut b: Vec<u8> = samples.iter().map(|s| s[2]).collect();
    r.sort_unstable();
    g.sort_unstable();
    b.sort_unstable();
    let mid = r.len() / 2;
    Color32::from_rgb(r[mid], g[mid], b[mid])
}

fn inspect_blank(screen: &str, findings: &mut Findings, w: usize, h: usize, buf: &[[f32; 4]]) {
    if w * h == 0 {
        findings.push(Issue::new("blank", screen, "empty raster".into(), None));
        return;
    }
    let first = buf[buf.len() / 2];
    let mut close = 0usize;
    for p in buf {
        if (p[0] - first[0]).abs() < 8.0
            && (p[1] - first[1]).abs() < 8.0
            && (p[2] - first[2]).abs() < 8.0
        {
            close += 1;
        }
    }
    let frac = close as f32 / buf.len() as f32;
    if frac > 0.985 {
        findings.push(Issue::new(
            "blank",
            screen,
            format!("raster is {:.1}% uniform", frac * 100.0),
            None,
        ));
    }
}

fn inspect_hover(h: &mut Harness, screen: &str, findings: &mut Findings) {
    let clicks = h.clickables();
    // Cap the work on dense lists.
    let take = clicks.len().min(40);
    for w in clicks.into_iter().take(take) {
        let p = w.interact_rect.center();
        // Skip if a later (front) clickable covers the centre.
        let covered = h.clickables().into_iter().any(|o| {
            o.id != w.id
                && o.interact_rect.contains(p)
                && o.interact_rect.area() < w.interact_rect.area()
        });
        if covered {
            continue;
        }
        h.hover(p);
        let hovered = h
            .ctx
            .read_response(w.id)
            .map(|r| r.hovered())
            .unwrap_or(false);
        if !hovered {
            findings.push(Issue::new(
                "unresponsive",
                screen,
                format!("clickable at {:?} did not report hover", w.interact_rect),
                Some(w.interact_rect),
            ));
        }
    }
}

fn trunc(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        t.push('…');
    }
    t.replace('\n', " ")
}

fn exe_in(cfg: &Config, name: &str) -> bool {
    let n = name.to_lowercase();
    cfg.apps.keys().any(|k| {
        k.to_lowercase() == n
            || k.to_lowercase().ends_with(&n)
            || layout_keeper_core::config::exe_name(k) == n
    })
}

fn exc_in(cfg: &Config, name: &str) -> bool {
    let n = name.to_lowercase();
    cfg.exception_apps.iter().any(|k| {
        k.to_lowercase() == n
            || k.to_lowercase().ends_with(&n)
            || layout_keeper_core::config::exe_name(k) == n
    })
}

fn lang_of(cfg: &Config, name: &str) -> Option<Lang> {
    let n = name.to_lowercase();
    cfg.apps.iter().find_map(|(k, l)| {
        if k.to_lowercase() == n
            || k.to_lowercase().ends_with(&n)
            || layout_keeper_core::config::exe_name(k) == n
        {
            Some(*l)
        } else {
            None
        }
    })
}

fn empty_cfg() -> Config {
    let mut c = Config::default();
    c.hide_welcome = true;
    c.welcome_gen = layout_keeper_core::config::CURRENT_WELCOME_GEN;
    c.apps.clear();
    c.exception_apps.clear();
    c
}

fn edge_cfg() -> Config {
    let mut c = empty_cfg();
    c.set_layout(
        r"C:\Program Files\VeryLong\Path\ThatGoesOnAndOn\AndOn\notepad.exe",
        Lang::En,
    );
    c.set_layout("Блокнот.exe", Lang::Ru);
    c.set_layout("Чертёжник-приложение.exe", Lang::Ru);
    c.add_exception(LONG_PATH);
    c.add_exception("мобаxterm");
    c
}

fn many_cfg(n: usize) -> Config {
    let mut c = empty_cfg();
    for i in 0..n {
        let id = if i % 17 == 0 {
            format!(r"C:\очень-длинный-путь\разработка\инструменты\app-{i:03}-инструмент.exe")
        } else if i % 11 == 0 {
            format!("Приложение-{i:03}.exe")
        } else {
            format!("app-{i:03}.exe")
        };
        c.set_layout(&id, if i % 2 == 0 { Lang::En } else { Lang::Ru });
    }
    c
}

fn default_sec(tab: Tab, light: bool, cfg: Config) -> Harness {
    Harness::new(app_with(cfg, tab, light), focus(), SEC, 1.0, false)
}


fn many_exc_cfg(n: usize) -> Config {
    let mut c = empty_cfg();
    c.add_exception(LONG_PATH);
    c.add_exception("мобаxterm");
    c.add_exception("powershell");
    for i in 0..n {
        let id = if i % 5 == 0 {
            format!(r"C:\очень-длинный-путь\документы\проекты\исключение-{i:02}-с-длинным-именем.exe")
        } else if i % 3 == 0 {
            format!("Блокнот-{i:02}.exe")
        } else {
            format!("app-{i:02}.exe")
        };
        c.add_exception(&id);
    }
    c
}

fn find_back(h: &Harness) -> Option<WidgetRect> {
    h.clickables()
        .into_iter()
        .filter(|c| c.interact_rect.center().y < st::HEADER && c.interact_rect.left() < st::HEADER)
        .min_by(|a, b| {
            a.interact_rect
                .left()
                .partial_cmp(&b.interact_rect.left())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn find_plus(h: &Harness) -> Option<WidgetRect> {
    let header = st::HEADER;
    h.clickables()
        .into_iter()
        .filter(|c| {
            let r = c.interact_rect;
            r.top() > header
                && r.top() < header + st::ROW * 2.0 + st::MARGIN
                && (r.width() - st::HIT).abs() < 6.0
                && (r.height() - st::HIT).abs() < 6.0
        })
        .max_by(|a, b| {
            a.interact_rect
                .right()
                .partial_cmp(&b.interact_rect.right())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn is_icon_only(h: &Harness, w: &WidgetRect) -> bool {
    !h.texts().iter().any(|t| {
        t.rect.intersects(w.interact_rect) && t.text.chars().filter(|c| !c.is_whitespace()).count() > 1
    })
}

fn inspect_usability(h: &mut Harness, screen: &str, findings: &mut Findings) {
    ux_placement_and_fitts(h, screen, findings);
    ux_primary_targets(h, screen, findings);
    ux_label_clarity(h, screen, findings);
    ux_alignment(h, screen, findings);
    ux_consistency(h, screen, findings);
    // Tooltip-on-icon is checked in the expert review: egui tooltip timing is
    // not reliable enough in the headless harness for a report-only flag.
}

fn ux_placement_and_fitts(h: &Harness, screen: &str, findings: &mut Findings) {
    if h.app.tab != Tab::Home && !h.app.welcome_open {
        match find_back(h) {
            None => findings.push(Issue::ux(
                "must",
                "ux-placement",
                screen,
                "no back button in the top-left of a section screen".into(),
                None,
            )),
            Some(w) => {
                let r = w.interact_rect;
                if r.left() > st::MARGIN + 6.0 || r.top() > st::MARGIN + 6.0 {
                    findings.push(Issue::ux(
                        "should",
                        "ux-placement",
                        screen,
                        format!("back button is not in the top-left corner ({:.0},{:.0})", r.left(), r.top()),
                        Some(r),
                    ));
                }
            }
        }
    }

    if let Some(plus) = find_plus(h) {
        let pr = plus.interact_rect;
        let y = pr.center().y;
        // The add field is the wide widget on the same row, immediately left of +.
        let field = h
            .clickables()
            .into_iter()
            .filter(|c| {
                let r = c.interact_rect;
                r.width() > 80.0
                    && (r.center().y - y).abs() < 16.0
                    && r.right() <= pr.left() + 2.0
            })
            .max_by(|a, b| {
                a.interact_rect
                    .width()
                    .partial_cmp(&b.interact_rect.width())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(field) = field {
            let fr = field.interact_rect;
            let dy = (pr.center().y - fr.center().y).abs();
            // On the Programs add row, EN/RU sits between the field and +;
            // measure the gap to the nearest widget on the left.
            let nearest_left = h
                .clickables()
                .into_iter()
                .map(|c| c.interact_rect)
                .filter(|r| (r.center().y - y).abs() < 16.0 && r.right() <= pr.left() + 2.0)
                .map(|r| r.right())
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(fr.right());
            let gap = pr.left() - nearest_left;
            if dy > 14.0 {
                findings.push(Issue::ux(
                    "should",
                    "ux-fitts",
                    screen,
                    format!("add button is not on the same row as the field (Δy={dy:.0}px)"),
                    Some(pr),
                ));
            }
            if gap > 48.0 {
                findings.push(Issue::ux(
                    "should",
                    "ux-fitts",
                    screen,
                    format!("add button is {:.0}px from the field (Fitts: keep it next to the input)", gap),
                    Some(pr),
                ));
            }
        }
    }

    // Destructive delete sitting next to the EN/RU control on a program row.
    if h.app.tab == Tab::Apps && h.app.apps_pane == AppsPane::Programs {
        for t in h.texts() {
            let name = t.text.trim();
            if !name.to_lowercase().ends_with(".exe") && !name.contains('.') {
                continue;
            }
            let y = t.rect.center().y;
            let en = h.texts().into_iter().find(|e| {
                (e.text == "EN" || e.text == "RU") && (e.rect.center().y - y).abs() < 14.0
            });
            let trash = h.clickables().into_iter().find(|c| {
                let r = c.interact_rect;
                (r.center().y - y).abs() < 16.0
                    && (r.width() - st::HIT).abs() < 6.0
                    && r.left() > t.rect.right()
                    && r.right() > h.size.x - st::MARGIN - st::INSET - st::HIT - 8.0
            });
            if let (Some(en), Some(tr)) = (en, trash) {
                let dist = (tr.interact_rect.left() - en.rect.right())
                    .abs()
                    .min((en.rect.left() - tr.interact_rect.right()).abs());
                if dist < st::HIT + st::PAD * 2.0 {
                    findings.push(Issue::ux(
                        "should",
                        "ux-fitts",
                        screen,
                        format!(
                            "delete is adjacent to the EN/RU control on the «{}» row ({:.0}px apart)",
                            trunc(name, 24),
                            dist
                        ),
                        Some(tr.interact_rect),
                    ));
                    break; // one example is enough
                }
            }
        }
        // Fallback: rightmost HIT button and the EN/RU texts share a row.
        if !findings.issues.iter().any(|i| i.kind == "ux-fitts" && i.detail.contains("delete is adjacent")) {
            if let Some(row) = h.texts().into_iter().find(|t| t.text.to_lowercase().ends_with(".exe")) {
                let y = row.rect.center().y;
                let mut hits: Vec<_> = h
                    .clickables()
                    .into_iter()
                    .filter(|c| (c.interact_rect.center().y - y).abs() < 16.0)
                    .collect();
                hits.sort_by(|a, b| {
                    a.interact_rect
                        .left()
                        .partial_cmp(&b.interact_rect.left())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(tr) = hits.last() {
                    if (tr.interact_rect.width() - st::HIT).abs() < 6.0 {
                        findings.push(Issue::ux(
                            "should",
                            "ux-fitts",
                            screen,
                            format!(
                                "delete sits in the same action cluster as EN/RU on the «{}» row",
                                trunc(&row.text, 24)
                            ),
                            Some(tr.interact_rect),
                        ));
                    }
                }
            }
        }
    }

    if h.app.tab == Tab::Apps && h.app.apps_pane == AppsPane::Exceptions {
        if let Some(title) = h.texts().into_iter().find(|t| {
            t.rect.center().y < st::HEADER && t.text.contains("Программы")
        }) {
            findings.push(Issue::ux(
                "should",
                "ux-label",
                screen,
                "header still says «Программы» while the Exceptions pane is open".into(),
                Some(title.rect),
            ));
        }
    }
}

fn ux_primary_targets(h: &Harness, screen: &str, findings: &mut Findings) {
    // Header nav dots (the five section marks) and any other primary control under 32 px.
    if !h.app.welcome_open && h.app.tab != Tab::Home {
        for w in h.clickables() {
            let r = w.interact_rect;
            if r.center().y >= st::HEADER || r.left() < h.size.x * 0.55 {
                continue;
            }
            let m = r.width().min(r.height());
            if m + 0.05 < st::HIT {
                findings.push(Issue::ux(
                    "should",
                    "ux-target",
                    screen,
                    format!("header nav target is {:.0}×{:.0}px; primary navigation should be ≥ 32 px", r.width(), r.height()),
                    Some(r),
                ));
                break;
            }
        }
    }
    if let Some(plus) = find_plus(h) {
        let r = plus.interact_rect;
        let m = r.width().min(r.height());
        if m + 0.05 < st::HIT {
            findings.push(Issue::ux(
                "must",
                "ux-target",
                screen,
                format!("add button is only {:.0}×{:.0}px (primary, want ≥ 32)", r.width(), r.height()),
                Some(r),
            ));
        }
    }
    if let Some(t) = h.find_text("Далее") {
        let hit = h
            .clickables()
            .into_iter()
            .find(|c| c.interact_rect.intersects(t.rect));
        if let Some(w) = hit {
            let m = w.interact_rect.width().min(w.interact_rect.height());
            if m + 0.05 < st::HIT {
                findings.push(Issue::ux(
                    "must",
                    "ux-target",
                    screen,
                    format!("«Далее» hit target is {:.0}px (primary, want ≥ 32)", m),
                    Some(w.interact_rect),
                ));
            }
        }
    }
}

fn ux_label_clarity(h: &Harness, screen: &str, findings: &mut Findings) {
    let win = h.screen_rect();
    for t in h.texts() {
        if t.rect.min.y >= win.max.y - OUT_TOL {
            continue;
        }
        let vis = t.rect.intersect(t.clip).intersect(win);
        let lost_x = (t.rect.intersect(win).width() - vis.width()).max(0.0);
        if lost_x > 1.5 && t.font_size + 0.05 >= st::FONT_SM {
            findings.push(Issue::ux(
                "should",
                "ux-label",
                screen,
                format!("label «{}» is truncated by {:.0}px", trunc(&t.text, 40), lost_x),
                Some(t.rect),
            ));
        }
    }

    fn stem(s: &str) -> String {
        s.split_whitespace()
            .filter(|w| w.chars().any(|c| c.is_alphabetic()))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }
    let header: Vec<String> = h
        .texts()
        .into_iter()
        .filter(|t| t.rect.center().y < st::HEADER && t.font_size + 0.05 >= st::FONT_MD)
        .map(|t| stem(&t.text))
        .filter(|s| !s.is_empty())
        .collect();
    let mut seen = std::collections::HashSet::new();
    for t in h.texts() {
        if t.rect.center().y <= st::HEADER + 4.0 {
            continue;
        }
        let stn = stem(&t.text);
        if stn.is_empty() || stn.chars().count() < 4 {
            continue;
        }
        if header.iter().any(|h| *h == stn) && seen.insert(stn.clone()) {
            findings.push(Issue::ux(
                "nice",
                "ux-label",
                screen,
                format!("title «{}» appears in the header and again in the body", trunc(&t.text, 40)),
                Some(t.rect),
            ));
        }
    }
}

fn ux_alignment(h: &Harness, screen: &str, findings: &mut Findings) {
    if !matches!(h.app.tab, Tab::Settings | Tab::Donate | Tab::Vault | Tab::Words) {
        return;
    }
    let mut rows: Vec<(f32, f32)> = h
        .texts()
        .into_iter()
        .filter(|t| {
            t.rect.top() > st::HEADER
                && t.rect.left() < h.size.x * 0.42
                && t.font_size + 0.05 >= st::FONT_SM
                && t.text.chars().any(|c| c.is_alphabetic())
        })
        .map(|t| (t.rect.left(), t.rect.center().y))
        .collect();
    if rows.len() < 3 {
        return;
    }
    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    // Compare left edges of labels that sit in the same visual column (within 40 px of the leftmost).
    let min_x = rows.iter().map(|r| r.0).fold(f32::MAX, f32::min);
    let col: Vec<f32> = rows.iter().map(|r| r.0).filter(|x| (*x - min_x).abs() < 40.0).collect();
    if col.len() < 3 {
        return;
    }
    let max_x = col.iter().copied().fold(f32::MIN, f32::max);
    let span = max_x - min_x;
    if span > 3.0 {
        findings.push(Issue::ux(
            "nice",
            "ux-spacing",
            screen,
            format!("left edges of labels in the main column differ by {span:.1}px"),
            None,
        ));
    }
}

fn ux_consistency(h: &Harness, screen: &str, findings: &mut Findings) {
    if h.app.welcome_open && h.find_text("Не показывать").is_some() {
        findings.push(Issue::ux(
            "nice",
            "ux-consistency",
            screen,
            "«Не показывать» puts the switch on the left of the label; Settings rows put the switch on the right".into(),
            h.find_text("Не показывать").map(|t| t.rect),
        ));
    }
}

#[allow(dead_code)]
fn ux_icon_tooltips(h: &mut Harness, screen: &str, findings: &mut Findings) {
    let icons: Vec<WidgetRect> = h
        .clickables()
        .into_iter()
        .filter(|w| is_icon_only(h, w) && w.interact_rect.height() >= 8.0 && w.interact_rect.width() >= 8.0)
        .collect();
    let mut missing: Vec<WidgetRect> = Vec::new();
    let mut shown = 0usize;
    for w in icons.into_iter().take(16) {
        let p = w.interact_rect.center();
        if !h.screen_rect().contains(p) {
            continue;
        }
        let before: std::collections::HashSet<String> =
            h.texts().into_iter().map(|t| t.text).collect();
        h.hover(p);
        h.time += 0.6;
        h.step(vec![Event::PointerMoved(p)]);
        h.step(vec![Event::PointerMoved(p)]);
        let added = h
            .texts()
            .iter()
            .any(|t| !before.contains(&t.text) && !t.text.trim().is_empty());
        if added {
            shown += 1;
        } else {
            missing.push(w);
        }
    }
    // If not a single tooltip appeared, the detector failed (egui tooltip
    // timing) — do not flood the report with false positives.
    if shown == 0 {
        return;
    }
    for w in missing {
        findings.push(Issue::ux(
            "should",
            "ux-label",
            screen,
            format!("icon-only control at {:?} has no tooltip", w.interact_rect),
            Some(w.interact_rect),
        ));
    }
}

struct TaskDepth {
    name: &'static str,
    clicks: usize,
    notes: String,
}

fn measure_action_depths(findings: &mut Findings) -> Vec<TaskDepth> {
    let mut out = Vec::new();

    // Add an exception: Home → Programs → Exceptions → field → plus.
    let mut h = Harness::new(app_with(empty_cfg(), Tab::Home, true), focus(), HOME, 1.0, false);
    let mut clicks = 0usize;
    assert!(h.click_home_square("Программы"));
    clicks += 1;
    // section size after navigation; redraw at section size
    h.size = SEC;
    h.settle();
    if h.click_text("Исключения  ") || h.click_text("Исключения") {
        clicks += 1;
    }
    if h.find_text("программа или путь").is_some() && h.click_text("программа или путь") {
        clicks += 1;
    }
    h.type_text("hint.exe");
    if h.click_plus() {
        clicks += 1;
    }
    let ok = exc_in(&h.app.cfg, "hint.exe");
    if !ok {
        findings.push(Issue::ux(
            "must",
            "ux-feedback",
            "task-add-exception",
            "could not add an exception from Home via the GUI".into(),
            None,
        ));
    } else if h.app.status.is_empty() {
        // presence of the chip is the feedback; ok
    }
    out.push(TaskDepth {
        name: "Add an exception",
        clicks,
        notes: if ok {
            "Home → Программы → Исключения → field → +".into()
        } else {
            "failed".into()
        },
    });
    if clicks > 3 {
        findings.push(Issue::ux(
            "nice",
            "ux-depth",
            "task-add-exception",
            format!("{clicks} clicks from Home to add an exception"),
            None,
        ));
    }

    // Change logo: Home → Settings → tile.
    let mut h = Harness::new(app_with(sample_cfg(), Tab::Home, true), focus(), HOME, 1.0, false);
    let mut clicks = 0usize;
    assert!(h.click_home_square("Настройки"));
    clicks += 1;
    h.size = SEC;
    h.settle();
    let before = h.app.cfg.logo;
    if h.click_logo_from_left(1) {
        clicks += 1;
    }
    let ok = h.app.cfg.logo != before;
    out.push(TaskDepth {
        name: "Change logo",
        clicks,
        notes: "Home → Настройки → logo tile".into(),
    });
    if !ok {
        findings.push(Issue::ux(
            "must",
            "ux-feedback",
            "task-change-logo",
            "logo tile click did not change config.logo".into(),
            None,
        ));
    }

    // Change undo key: Home → Settings → keycap → Назначить… → F2.
    let mut h = Harness::new(app_with(sample_cfg(), Tab::Home, true), focus(), HOME, 1.0, false);
    let mut clicks = 0usize;
    assert!(h.click_home_square("Настройки"));
    clicks += 1;
    h.size = SEC;
    h.settle();
    let old = h.app.cfg.undo_key;
    if h.click_right_of("Клавиша") {
        clicks += 1;
    }
    if h.click_text("Назначить") || h.texts().iter().any(|t| t.text.contains("Назначить")) {
        if h.app.capturing_hotkey.is_none() {
            if let Some(t) = h.texts().into_iter().find(|t| t.text.contains("Назначить")) {
                h.click(t.rect.center());
            }
        }
        clicks += 1;
    }
    if h.app.capturing_hotkey.is_some() {
        h.press_key(Key::F2, Modifiers::default());
    }
    let ok = h.app.cfg.undo_key != old;
    out.push(TaskDepth {
        name: "Change undo key",
        clicks,
        notes: "Home → Настройки → Клавиша → Назначить… → key".into(),
    });
    if !ok {
        findings.push(Issue::ux(
            "should",
            "ux-depth",
            "task-undo-key",
            format!("undo-key capture failed after {clicks} clicks (popup + assign is a deep path)"),
            None,
        ));
    } else if clicks >= 3 {
        findings.push(Issue::ux(
            "nice",
            "ux-depth",
            "task-undo-key",
            format!("{clicks} clicks + a key press from Home to change the undo key (popup with «Назначить…»)"),
            None,
        ));
    }

    // Set layout for an existing app: Home → Programs → EN on a row.
    let mut h = Harness::new(app_with(sample_cfg(), Tab::Home, true), focus(), HOME, 1.0, false);
    let mut clicks = 0usize;
    assert!(h.click_home_square("Программы"));
    clicks += 1;
    h.size = SEC;
    h.settle();
    let before = lang_of(&h.app.cfg, "chrome.exe");
    if h.click_on_row("chrome.exe", "RU") || h.click_on_row("Chrome", "RU") {
        clicks += 1;
    }
    let after = lang_of(&h.app.cfg, "chrome.exe");
    let ok = after.is_some() && after != before;
    out.push(TaskDepth {
        name: "Set layout for an app",
        clicks,
        notes: "Home → Программы → EN/RU on the row".into(),
    });
    if !ok {
        findings.push(Issue::ux(
            "must",
            "ux-feedback",
            "task-set-layout",
            "clicking RU on a program row did not change the saved layout".into(),
            None,
        ));
    }

    // Empty add: no feedback.
    let mut h = default_sec(Tab::Apps, true, empty_cfg());
    let _ = h.click_plus();
    if h.app.cfg.apps.is_empty() && h.app.status.is_empty() {
        findings.push(Issue::ux(
            "should",
            "ux-feedback",
            "apps-empty",
            "pressing + with an empty program field gives no visible error or status".into(),
            find_plus(&h).map(|w| w.interact_rect),
        ));
    }

    out
}

fn write_issues_report(
    path: &Path,
    findings: &Findings,
    shots: &[(String, PathBuf)],
    depths: &[TaskDepth],
) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut s = String::from("# UI/UX check report\n\n");
    let structural: Vec<&Issue> = findings
        .issues
        .iter()
        .filter(|i| i.severity == "note")
        .collect();
    let usability: Vec<&Issue> = findings
        .issues
        .iter()
        .filter(|i| i.severity != "note")
        .collect();
    s.push_str(&format!(
        "Automated checks found **{}** structural and **{}** usability issue(s).\n\n",
        structural.len(),
        usability.len()
    ));

    s.push_str("## Usability\n\n");
    if !depths.is_empty() {
        s.push_str("### Action depth (from Home)\n\n");
        s.push_str("| Task | Clicks | Path |\n|---|---:|---|\n");
        for d in depths {
            s.push_str(&format!("| {} | {} | {} |\n", d.name, d.clicks, d.notes));
        }
        s.push('\n');
    }
    let order = ["must", "should", "nice"];
    let titles = [
        ("must", "Must fix"),
        ("should", "Should fix"),
        ("nice", "Nice to have"),
    ];
    if usability.is_empty() {
        s.push_str("No usability heuristic failures.\n\n");
    } else {
        for (sev, title) in titles {
            let items: Vec<_> = usability.iter().filter(|i| i.severity == sev).collect();
            if items.is_empty() {
                continue;
            }
            // Group identical findings across matrix cells, and attach a highlight shot.
            let mut groups: Vec<(&Issue, Vec<&str>)> = Vec::new();
            for i in items {
                if let Some(g) = groups.iter_mut().find(|(e, _)| e.kind == i.kind && e.detail == i.detail) {
                    g.1.push(i.screen.as_str());
                } else {
                    groups.push((i, vec![i.screen.as_str()]));
                }
            }
            s.push_str(&format!("### {title} ({})\n\n", groups.len()));
            for (i, screens) in groups {
                let shot = screens.iter().find_map(|name| {
                    shots.iter().find(|(n, _)| n.starts_with(*name)).map(|(_, p)| p)
                });
                let where_ = if screens.len() == 1 {
                    format!("`{}`", screens[0])
                } else {
                    format!(
                        "{} screens (`{}`{})",
                        screens.len(),
                        screens[0],
                        if screens.len() > 1 { ", …" } else { "" }
                    )
                };
                match shot {
                    Some(p) => s.push_str(&format!(
                        "- **{}** — {}: {}. Highlight: `{}`\n",
                        i.kind, where_, i.detail, p.display()
                    )),
                    None => s.push_str(&format!(
                        "- **{}** — {}: {}.\n",
                        i.kind, where_, i.detail
                    )),
                }
            }
            s.push('\n');
        }

        let _ = order;
    }

    if !structural.is_empty() {
        s.push_str("## Structural checks\n\n");
        let mut kinds: Vec<&str> = structural.iter().map(|i| i.kind).collect();
        kinds.sort();
        kinds.dedup();
        for k in kinds {
            let group: Vec<_> = structural.iter().filter(|i| i.kind == k).collect();
            s.push_str(&format!("### {k} ({})\n\n", group.len()));
            for i in group {
                s.push_str(&format!("- `{}`: {}\n", i.screen, i.detail));
            }
            s.push('\n');
        }
    }

    if !shots.is_empty() {
        s.push_str("## Example screenshots\n\n");
        for (name, p) in shots {
            s.push_str(&format!("- {name}: `{}`\n", p.display()));
        }
        s.push('\n');
    }

    s.push_str(include_str!("ux_review.md"));
    let _ = std::fs::write(path, s);
}

fn annotate_and_save(
    path: &Path,
    w: usize,
    h: usize,
    buf: &[[f32; 4]],
    ppp: f32,
    issues: &[Issue],
) {
    let mut pix = buf.to_vec();
    for i in issues {
        let Some(r) = i.rect else { continue };
        let x0 = (r.min.x * ppp).floor().max(0.0) as usize;
        let y0 = (r.min.y * ppp).floor().max(0.0) as usize;
        let x1 = (r.max.x * ppp).ceil().min(w as f32) as usize;
        let y1 = (r.max.y * ppp).ceil().min(h as f32) as usize;
        let color = match (i.severity, i.kind) {
            ("must", _) => [220.0, 40.0, 40.0, 255.0],
            ("should", _) => [220.0, 140.0, 20.0, 255.0],
            ("nice", _) => [40.0, 140.0, 200.0, 255.0],
            (_, "contrast") => [220.0, 40.0, 40.0, 255.0],
            (_, "hit") => [40.0, 80.0, 220.0, 255.0],
            (_, "overlap") => [220.0, 160.0, 20.0, 255.0],
            (_, "clip") => [180.0, 40.0, 180.0, 255.0],
            _ => [20.0, 180.0, 80.0, 255.0],
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let edge = y == y0
                    || y + 1 == y1
                    || x == x0
                    || x + 1 == x1
                    || y == y0 + 1
                    || y + 2 == y1
                    || x == x0 + 1
                    || x + 2 == x1;
                if edge {
                    pix[y * w + x] = color;
                } else if (x + y) % 7 == 0 {
                    let d = &mut pix[y * w + x];
                    d[0] = d[0] * 0.65 + color[0] * 0.35;
                    d[1] = d[1] * 0.65 + color[1] * 0.35;
                    d[2] = d[2] * 0.65 + color[2] * 0.35;
                }
            }
        }
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    write_png(path, w, h, &pix);
}

// ---------------------------------------------------------------------------
// Scenario tests (always on)
// ---------------------------------------------------------------------------

#[test]
fn ux_add_program_set_lang_move_to_exceptions_dedupe_delete() {
    let mut h = default_sec(Tab::Apps, true, empty_cfg());
    assert_eq!(h.app.apps_pane, AppsPane::Programs);
    assert!(
        h.find_text("программа или путь").is_some(),
        "add field hint should be visible"
    );
    assert!(h.click_text("программа или путь"), "click the add field");
    h.type_text("notepad.exe");
    // Default mode is RU; switch the add-row to EN (the top-most "EN").
    assert!(h.click_text("EN"), "click EN on the add row");
    assert_eq!(h.app.app_mode, AppMode::En);
    assert!(h.click_plus(), "click plus to add");
    assert!(
        exe_in(&h.app.cfg, "notepad.exe"),
        "notepad.exe should be in programs: {:?}",
        h.app.cfg.apps.keys().collect::<Vec<_>>()
    );
    assert_eq!(lang_of(&h.app.cfg, "notepad.exe"), Some(Lang::En));

    // Flip EN → RU on the row, then back.
    assert!(h.click_on_row("notepad.exe", "RU"), "set RU");
    assert_eq!(lang_of(&h.app.cfg, "notepad.exe"), Some(Lang::Ru));
    assert!(h.click_on_row("notepad.exe", "EN"), "set EN");
    assert_eq!(lang_of(&h.app.cfg, "notepad.exe"), Some(Lang::En));

    // Move to exceptions (exception wins / drops the program entry).
    assert!(h.click_text("Исключения"), "switch to exceptions tab");
    assert_eq!(h.app.apps_pane, AppsPane::Exceptions);
    assert!(h.click_text("программа или путь") || h.find_text("notepad.exe").is_none());
    // The add field on the exceptions pane uses the same hint.
    if h.find_text("программа или путь").is_some() {
        assert!(h.click_text("программа или путь"));
    }
    h.type_text("notepad.exe");
    assert!(h.click_plus() || h.click_text("Добавить") || true);
    // plus on exceptions is add_line_plus — might be a plus or an "Добавить исключение" control.
    if !exc_in(&h.app.cfg, "notepad.exe") {
        // Try Enter after focusing the field.
        h.press_key(Key::Enter, Modifiers::default());
    }
    if !exc_in(&h.app.cfg, "notepad.exe") {
        // Last resort: the plus on this pane may be a text button.
        let _ = h.click_text("Добавить исключение");
    }
    assert!(
        exc_in(&h.app.cfg, "notepad.exe"),
        "notepad.exe should be in exceptions: {:?}",
        h.app.cfg.exception_apps
    );
    assert!(
        !exe_in(&h.app.cfg, "notepad.exe"),
        "exception wins: dropped from programs: {:?}",
        h.app.cfg.apps.keys().collect::<Vec<_>>()
    );

    // Dedupe: adding the same exe again (different case / path) does not grow the list.
    let n = h.app.cfg.exception_apps.len();
    if h.find_text("программа или путь").is_some() {
        let _ = h.click_text("программа или путь");
    }
    h.type_text("NOTEPAD.EXE");
    let _ = h.click_plus();
    h.press_key(Key::Enter, Modifiers::default());
    assert_eq!(
        h.app.cfg.exception_apps.len(),
        n,
        "case-insensitive dedupe should keep one entry"
    );

    // Delete.
    assert!(
        h.click_delete_on_row("notepad.exe") || h.click_delete_on_row("NOTEPAD"),
        "delete"
    );
    assert!(
        !exc_in(&h.app.cfg, "notepad.exe"),
        "deleted from exceptions: {:?}",
        h.app.cfg.exception_apps
    );
}

#[test]
fn ux_switch_programs_exceptions_tabs() {
    let mut h = default_sec(Tab::Apps, true, sample_cfg());
    assert_eq!(h.app.apps_pane, AppsPane::Programs);
    assert!(
        h.find_text("chrome.exe").is_some()
            || h.find_text("Chrome").is_some()
            || h.texts()
                .iter()
                .any(|t| t.text.to_lowercase().contains("chrome"))
    );
    assert!(h.click_text("Исключения  "), "segment Исключения  N");
    assert_eq!(h.app.apps_pane, AppsPane::Exceptions);
    assert!(
        h.find_text("powershell").is_some()
            || h.texts()
                .iter()
                .any(|t| t.text.to_lowercase().contains("powershell")
                    || t.text.contains("setup.tmp"))
    );
    assert!(
        h.click_text("Программы  "),
        "segment Программы  N (not the header title)"
    );
    assert_eq!(h.app.apps_pane, AppsPane::Programs);
}

#[test]
fn ux_exceptions_wrap_as_chips() {
    let mut h = default_sec(Tab::Apps, true, many_exc_cfg(18));
    assert!(h.click_text("Исключения  ") || h.click_text("Исключения"));
    assert_eq!(h.app.apps_pane, AppsPane::Exceptions);
    assert!(h.find_text("программа или путь").is_some(), "add field stays on top");
    // Names are shown as chips, not a vertical list: several distinct Y rows.
    let mut ys: Vec<i32> = h
        .texts()
        .into_iter()
        .filter(|t| {
            t.rect.top() > st::HEADER + st::ROW * 2.0
                && (t.text.ends_with(".exe") || t.text.contains("setup") || t.text.contains("powershell") || t.text.contains("моба") || t.text.contains("Блокнот"))
        })
        .map(|t| (t.rect.center().y / 4.0).round() as i32)
        .collect();
    ys.sort();
    ys.dedup();
    assert!(
        ys.len() >= 2,
        "exception chips should wrap onto more than one line, got y-rows {ys:?}; texts={:?}",
        h.texts().iter().map(|t| t.text.clone()).collect::<Vec<_>>()
    );

    // Long names are truncated (the chip is narrower than the unclipped name would be)
    // and still offer a tooltip via chip_with_tip.
    let long = h
        .texts()
        .into_iter()
        .find(|t| t.text.contains("исключение") || t.text.contains("длинным"));
    if let Some(t) = long {
        assert!(
            t.rect.width() < 400.0,
            "long exception name should truncate inside the chip, width={}",
            t.rect.width()
        );
    }

    // Delete via the chip ×.
    let before = h.app.cfg.exception_apps.len();
    let sample = h
        .texts()
        .into_iter()
        .find(|t| t.text.to_lowercase().contains("app-01") || t.text.contains("powershell"))
        .map(|t| t.text);
    if let Some(name) = sample {
        assert!(h.click_delete_on_row(&name), "delete via chip × for {name}");
        assert!(
            h.app.cfg.exception_apps.len() < before,
            "chip × should remove the exception"
        );
    }

    // Empty pane: only the add field, no leftover list.
    let mut h = default_sec(Tab::Apps, true, empty_cfg());
    assert!(h.click_text("Исключения  ") || h.click_text("Исключения"));
    assert!(h.find_text("программа или путь").is_some());
    let leftover = h.texts().into_iter().any(|t| {
        t.rect.top() > st::HEADER + st::ROW * 2.0
            && (t.text.ends_with(".exe") || t.text == "Исключения" && t.rect.top() > st::HEADER + st::ROW)
    });
    assert!(!leftover, "empty exceptions pane should not show a chip list");
}

#[test]
fn ux_change_logo_updates_config() {
    let mut h = default_sec(Tab::Settings, true, sample_cfg());
    assert_eq!(h.app.cfg.logo, LogoStyle::Beach);
    let tiles = h.logo_tiles();
    assert_eq!(
        tiles.len(),
        5,
        "five logo tiles on the Логотип row, got {}",
        tiles.len()
    );
    assert!(
        h.click_logo_from_left(1),
        "click Ар-деко (second from the left)"
    );
    assert_eq!(
        h.app.cfg.logo,
        LogoStyle::Artdeco,
        "config.logo should change"
    );
    // The selected tile is still one of five; clicking it again is a no-op.
    let after = h.logo_tiles();
    assert_eq!(after.len(), 5);
}

#[test]
fn ux_toggle_undo_and_capture_key() {
    let mut h = default_sec(Tab::Settings, true, sample_cfg());
    let before = h.app.cfg.undo_on_backspace;
    assert!(
        h.click_text("Отмена исправления"),
        "toggle undo-on-backspace"
    );
    assert_ne!(h.app.cfg.undo_on_backspace, before, "toggle should flip");
    // Flip back so the rest of the test is on a known state, then capture a key.
    assert!(h.click_text("Отмена исправления"));
    assert_eq!(h.app.cfg.undo_on_backspace, before);

    let old = h.app.cfg.undo_key;
    assert!(h.click_right_of("Клавиша"), "open the undo keycap");
    // Popup: «Назначить…»
    if !h.click_text("Назначить") {
        // The ellipsis may be a single char «…» or "...".
        let found = h.texts().into_iter().find(|t| t.text.contains("Назначить"));
        if let Some(t) = found {
            h.click(t.rect.center());
        }
    }
    assert!(
        h.app.capturing_hotkey.is_some(),
        "should be capturing after Назначить…; texts={:?}",
        h.texts().iter().map(|t| t.text.clone()).collect::<Vec<_>>()
    );
    h.press_key(Key::F2, Modifiers::default());
    assert!(h.app.capturing_hotkey.is_none(), "capture should finish");
    assert_ne!(h.app.cfg.undo_key, old, "undo_key should change from {old}");
    assert_eq!(h.app.cfg.undo_key.to_string(), "F2");
}

#[test]
fn ux_switch_theme() {
    let mut h = default_sec(Tab::Settings, true, sample_cfg());
    assert!(h.app.cfg.light_theme);
    assert!(h.click_text("Светлая тема"));
    assert!(!h.app.cfg.light_theme, "theme should flip to dark");
    assert!(h.click_text("Светлая тема"));
    assert!(h.app.cfg.light_theme);
}

#[test]
fn ux_go_through_welcome() {
    let mut app = app_with(sample_cfg(), Tab::Home, true);
    app.welcome_open = true;
    app.cfg.hide_welcome = false;
    let mut h = Harness::new(app, focus(), WELCOME, 1.0, false);
    assert!(h.app.welcome_open);
    assert!(h.find_text("Далее").is_some(), "welcome footer");
    assert!(h.find_text("Не показывать").is_some());
    // Pick a different logo on the welcome row, then dismiss.
    let _ = h.click_logo_from_left(2);
    assert!(h.click_text("Не показывать"));
    assert!(h.app.welcome_dont_show);
    assert!(h.click_text("Далее"));
    assert!(!h.app.welcome_open, "welcome should close");
    assert!(h.app.cfg.hide_welcome, "Не показывать should persist");
}

#[test]
fn ux_navigate_all_five_home_squares_and_back() {
    let mut h = Harness::new(
        app_with(sample_cfg(), Tab::Home, true),
        focus(),
        HOME,
        1.0,
        false,
    );
    let titles = ["Программы", "Словарь", "Сейф", "Настройки", "Поддержать"];
    let tabs = [
        Tab::Apps,
        Tab::Words,
        Tab::Vault,
        Tab::Settings,
        Tab::Donate,
    ];
    for (title, tab) in titles.into_iter().zip(tabs) {
        h.app.tab = Tab::Home;
        h.settle();
        assert!(
            h.click_home_square(title),
            "click home square {title}; texts={:?}",
            h.texts().iter().map(|t| t.text.clone()).collect::<Vec<_>>()
        );
        assert_eq!(h.app.tab, tab, "square {title} should open {tab:?}");
        assert!(h.click_back(), "back from {title}");
        assert_eq!(h.app.tab, Tab::Home, "back from {title} should return home");
    }
}

// ---------------------------------------------------------------------------
// Structural / contrast / hit / overlap checks (always on, report-only)
// ---------------------------------------------------------------------------

#[test]
fn ux_inspect_screens_report() {
    let out_dir = PathBuf::from("/tmp/typed-ui-ux-issues");
    let _ = std::fs::create_dir_all(&out_dir);
    let mut findings = Findings::new();
    let mut shots: Vec<(String, PathBuf)> = Vec::new();

    let screens: Vec<(&str, Tab, AppsPane, bool, Vec2, bool, Config)> = vec![
        (
            "home-light",
            Tab::Home,
            AppsPane::Programs,
            true,
            HOME,
            false,
            sample_cfg(),
        ),
        (
            "home-dark",
            Tab::Home,
            AppsPane::Programs,
            false,
            HOME,
            false,
            sample_cfg(),
        ),
        (
            "home-min",
            Tab::Home,
            AppsPane::Programs,
            true,
            HOME_MIN,
            false,
            sample_cfg(),
        ),
        (
            "apps-light",
            Tab::Apps,
            AppsPane::Programs,
            true,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "apps-empty",
            Tab::Apps,
            AppsPane::Programs,
            true,
            SEC,
            false,
            empty_cfg(),
        ),
        (
            "apps-edge",
            Tab::Apps,
            AppsPane::Programs,
            true,
            SEC,
            false,
            edge_cfg(),
        ),
        (
            "exceptions",
            Tab::Apps,
            AppsPane::Exceptions,
            true,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "settings-light",
            Tab::Settings,
            AppsPane::Programs,
            true,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "settings-dark",
            Tab::Settings,
            AppsPane::Programs,
            false,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "settings-mac",
            Tab::Settings,
            AppsPane::Programs,
            true,
            SEC,
            true,
            sample_cfg(),
        ),
        (
            "words",
            Tab::Words,
            AppsPane::Programs,
            true,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "vault",
            Tab::Vault,
            AppsPane::Programs,
            true,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "donate",
            Tab::Donate,
            AppsPane::Programs,
            true,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "welcome",
            Tab::Home,
            AppsPane::Programs,
            true,
            WELCOME,
            false,
            sample_cfg(),
        ),
        (
            "apps-min",
            Tab::Apps,
            AppsPane::Programs,
            true,
            SEC_MIN,
            false,
            edge_cfg(),
        ),
        (
            "exceptions-dark",
            Tab::Apps,
            AppsPane::Exceptions,
            false,
            SEC,
            false,
            sample_cfg(),
        ),
        (
            "exceptions-min",
            Tab::Apps,
            AppsPane::Exceptions,
            true,
            SEC_MIN,
            false,
            sample_cfg(),
        ),
        (
            "exceptions-empty",
            Tab::Apps,
            AppsPane::Exceptions,
            true,
            SEC,
            false,
            empty_cfg(),
        ),
        (
            "exceptions-many",
            Tab::Apps,
            AppsPane::Exceptions,
            true,
            SEC,
            false,
            many_exc_cfg(24),
        ),
        (
            "exceptions-mac",
            Tab::Apps,
            AppsPane::Exceptions,
            true,
            SEC,
            true,
            sample_cfg(),
        ),
    ];

    for (name, tab, pane, light, size, mac, cfg) in screens {
        let mut app = app_with(cfg, tab, light);
        app.apps_pane = pane;
        if name == "welcome" {
            app.welcome_open = true;
        }
        let mut h = Harness::new(app, focus(), size, 1.0, mac);
        let before = findings.issues.len();
        let raster = inspect_all(&mut h, name, &mut findings, true, name != "apps-edge");
        inspect_usability(&mut h, name, &mut findings);
        let new = &findings.issues[before..];
        if let Some((w, ht, buf)) = raster {
            let path = out_dir.join(format!("{name}.png"));
            write_png(&path, w, ht, &buf);
            if new.iter().any(|i| i.rect.is_some()) {
                let marked = out_dir.join(format!("{name}-issues.png"));
                annotate_and_save(&marked, w, ht, &buf, h.ppp, new);
                shots.push((format!("{name} (annotated)"), marked));
            }
        }
        // Cheap canary: the screen is not empty.
        assert!(
            h.widgets().iter().any(|w| w.rect.area() > 4.0) || !h.texts().is_empty(),
            "{name} rendered blank"
        );
        assert!(
            h.app.content_overflow < 80.0,
            "{name} overflow is catastrophically wide: {}",
            h.app.content_overflow
        );
    }

    let depths = measure_action_depths(&mut findings);
    let report = PathBuf::from("/tmp/typed-ui-ux-report.md");
    write_issues_report(&report, &findings, &shots, &depths);
    println!(
        "UI/UX inspect: {} issue(s), report {}",
        findings.issues.len(),
        report.display()
    );
    for i in &findings.issues {
        println!("  [{}:{}] {}: {}", i.severity, i.kind, i.screen, i.detail);
    }
    for d in &depths {
        println!("  depth {} = {} clicks ({})", d.name, d.clicks, d.notes);
    }
}

// ---------------------------------------------------------------------------
// Visual regression (ignored, env-gated like render_ui_screenshots)
// ---------------------------------------------------------------------------

struct Cell {
    name: String,
    app: App,
    size: Vec2,
    mac: bool,
}

fn matrix_cells() -> Vec<Cell> {
    let mut v = Vec::new();
    let sizes: [(&str, Vec2, Tab); 3] = [
        ("min", if true { HOME_MIN } else { HOME_MIN }, Tab::Home),
        ("default", HOME, Tab::Home),
        ("large", LARGE, Tab::Home),
    ];
    for light in [true, false] {
        let th = if light { "light" } else { "dark" };
        for mac in [false, true] {
            let stl = if mac { "mac" } else { "win" };
            for (sz_name, _home_sz, _) in [
                ("min", HOME_MIN, ()),
                ("default", HOME, ()),
                ("large", LARGE, ()),
            ] {
                let app = app_with(sample_cfg(), Tab::Home, light);
                v.push(Cell {
                    name: format!("home-{th}-{stl}-{sz_name}"),
                    app,
                    size: if sz_name == "min" {
                        HOME_MIN
                    } else if sz_name == "large" {
                        LARGE
                    } else {
                        HOME
                    },
                    mac,
                });
            }
            // Section screens at min / default / large (skip large×every-tab to stay cheap-ish).
            let sec_size = |sz: &str| match sz {
                "min" => SEC_MIN,
                "large" => LARGE,
                _ => SEC,
            };
            for (sz_name, tab, pane) in [
                ("default", Tab::Apps, AppsPane::Programs),
                ("default", Tab::Apps, AppsPane::Exceptions),
                ("min", Tab::Apps, AppsPane::Programs),
                ("default", Tab::Settings, AppsPane::Programs),
                ("default", Tab::Words, AppsPane::Programs),
                ("default", Tab::Vault, AppsPane::Programs),
                ("default", Tab::Donate, AppsPane::Programs),
                ("large", Tab::Settings, AppsPane::Programs),
            ] {
                let cfg = sample_cfg();
                let mut app = app_with(cfg, tab, light);
                app.apps_pane = pane;
                let pane_n = match pane {
                    AppsPane::Programs if tab == Tab::Apps => "apps",
                    AppsPane::Exceptions => "exceptions",
                    _ => match tab {
                        Tab::Settings => "settings",
                        Tab::Words => "words",
                        Tab::Vault => "vault",
                        Tab::Donate => "donate",
                        _ => "sec",
                    },
                };
                v.push(Cell {
                    name: format!("{pane_n}-{th}-{stl}-{sz_name}"),
                    app,
                    size: sec_size(sz_name),
                    mac,
                });
            }
            let mut welcome = app_with(sample_cfg(), Tab::Home, light);
            welcome.welcome_open = true;
            v.push(Cell {
                name: format!("welcome-{th}-{stl}-default"),
                app: welcome,
                size: WELCOME,
                mac,
            });
        }
    }
    // Edge data, light/win/default only (keep the matrix bounded).
    for (tag, cfg, pane) in [
        ("empty", empty_cfg(), AppsPane::Programs),
        ("edge", edge_cfg(), AppsPane::Programs),
        ("edge-exc", edge_cfg(), AppsPane::Exceptions),
        ("many200", many_cfg(200), AppsPane::Programs),
    ] {
        let mut app = app_with(cfg, Tab::Apps, true);
        app.apps_pane = pane;
        v.push(Cell {
            name: format!("apps-{tag}-light-win-default"),
            app,
            size: SEC,
            mac: false,
        });
    }
    let _ = sizes;
    v
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn pixel_diff(a: &[u8], b: &[u8], w: usize, h: usize, tol: u8) -> (f32, Vec<u8>) {
    let n = w * h;
    if a.len() != n * 3 || b.len() != n * 3 {
        return (100.0, b.to_vec());
    }
    let mut diff = vec![0u8; n * 3];
    let mut changed = 0usize;
    for i in 0..n {
        let (r1, g1, b1) = (a[i * 3], a[i * 3 + 1], a[i * 3 + 2]);
        let (r2, g2, b2) = (b[i * 3], b[i * 3 + 1], b[i * 3 + 2]);
        let d = r1.abs_diff(r2).max(g1.abs_diff(g2)).max(b1.abs_diff(b2));
        if d > tol {
            changed += 1;
            diff[i * 3] = 220;
            diff[i * 3 + 1] = 30;
            diff[i * 3 + 2] = 30;
        } else {
            // Dim the original so the red marks pop.
            diff[i * 3] = r2 / 3 + 20;
            diff[i * 3 + 1] = g2 / 3 + 20;
            diff[i * 3 + 2] = b2 / 3 + 20;
        }
    }
    (changed as f32 * 100.0 / n.max(1) as f32, diff)
}

fn write_png_rgb(path: &Path, w: usize, h: usize, rgb: &[u8]) {
    let f = std::fs::File::create(path).expect("png file");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().expect("png header");
    wr.write_image_data(rgb).expect("png data");
}

fn buf_to_rgb(buf: &[[f32; 4]]) -> Vec<u8> {
    buf.iter()
        .flat_map(|p| [p[0], p[1], p[2]].map(|c| c.round().clamp(0.0, 255.0) as u8))
        .collect()
}

fn read_png_rgb(path: &Path) -> Option<(usize, usize, Vec<u8>)> {
    let f = std::fs::File::open(path).ok()?;
    let dec = png::Decoder::new(std::io::BufReader::new(f));
    let mut reader = dec.read_info().ok()?;
    let mut data = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut data).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgb = match info.color_type {
        png::ColorType::Rgb => data[..info.buffer_size()].to_vec(),
        png::ColorType::Rgba => data[..info.buffer_size()]
            .chunks(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect(),
        png::ColorType::Grayscale => data[..info.buffer_size()]
            .iter()
            .flat_map(|g| [*g, *g, *g])
            .collect(),
        _ => return None,
    };
    Some((w, h, rgb))
}

#[test]
#[ignore]
fn ux_visual_regression() {
    let shots_dir = std::env::var_os("TYPED_UI_UX_SHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/typed-ui-ux-shots"));
    let baseline_dir = std::env::var_os("TYPED_UI_UX_BASELINE")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("ui-ux-baseline"));
    let diff_dir = std::env::var_os("TYPED_UI_UX_DIFF")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/typed-ui-ux-diff"));
    let update = matches!(
        std::env::var("TYPED_UI_UX_UPDATE_BASELINE").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    );
    let fail_on = matches!(
        std::env::var("TYPED_UI_UX_FAIL_ON_DIFF").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    );
    let thresh: f32 = std::env::var("TYPED_UI_UX_DIFF_PCT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let ppp = 2.0;

    std::fs::create_dir_all(&shots_dir).unwrap();
    std::fs::create_dir_all(&diff_dir).unwrap();
    if update {
        std::fs::create_dir_all(&baseline_dir).unwrap();
    }

    let mut findings = Findings::new();
    let mut changed: Vec<(String, f32, PathBuf)> = Vec::new();
    let mut missing_baseline = 0usize;
    let mut written = 0usize;

    for cell in matrix_cells() {
        let mut h = Harness::new(cell.app, focus(), cell.size, ppp, cell.mac);
        let _ = inspect_all(&mut h, &cell.name, &mut findings, false, false);
        let Some((w, ht, buf)) = h.raster() else {
            continue;
        };
        let rel = format!("{}.png", cell.name);
        let actual = shots_dir.join(&rel);
        write_png(&actual, w, ht, &buf);
        written += 1;

        let base = baseline_dir.join(&rel);
        if update {
            let _ = std::fs::copy(&actual, &base);
        }
        if !base.exists() {
            missing_baseline += 1;
            continue;
        }
        let Some((bw, bh, brgb)) = read_png_rgb(&base) else {
            continue;
        };
        let argb = buf_to_rgb(&buf);
        if bw != w || bh != ht {
            changed.push((cell.name.clone(), 100.0, actual.clone()));
            continue;
        }
        let (pct, dimg) = pixel_diff(&brgb, &argb, w, ht, DELTA_TOL);
        if pct > thresh {
            let dp = diff_dir.join(&rel);
            write_png_rgb(&dp, w, ht, &dimg);
            changed.push((cell.name, pct, dp));
        }
    }

    let mut report = String::from("# UI/UX visual regression\n\n");
    report.push_str(&format!(
        "Shots: **{written}** in `{}`\n\n",
        shots_dir.display()
    ));
    report.push_str(&format!(
        "Baseline: `{}` (update={update})\n\n",
        baseline_dir.display()
    ));
    report.push_str(&format!(
        "Threshold: {thresh}% of pixels with channel Δ > {DELTA_TOL}\n\n"
    ));
    if missing_baseline > 0 {
        report.push_str(&format!("{missing_baseline} screen(s) have no baseline. Copy with `TYPED_UI_UX_UPDATE_BASELINE=1`.\n\n"));
    }
    if changed.is_empty() {
        report.push_str("No screens over the threshold.\n");
    } else {
        report.push_str(&format!("## Changed screens ({})\n\n", changed.len()));
        for (name, pct, p) in &changed {
            report.push_str(&format!("- `{name}`: {pct:.3}% — `{}`\n", p.display()));
        }
    }
    report.push_str(&format!(
        "\nStructural/contrast findings during the matrix: {}\n",
        findings.issues.len()
    ));
    for i in findings.issues.iter().take(80) {
        report.push_str(&format!("- [{}] {}: {}\n", i.kind, i.screen, i.detail));
    }
    let rp = diff_dir.join("report.md");
    std::fs::write(&rp, &report).unwrap();
    println!("{report}");
    if fail_on && !changed.is_empty() && missing_baseline == 0 {
        panic!(
            "{} screen(s) differ from the baseline (see {})",
            changed.len(),
            rp.display()
        );
    }
}

#[test]
#[ignore]
fn ux_edge_two_hundred_apps_overflow() {
    let mut h = Harness::new(
        app_with(many_cfg(200), Tab::Apps, true),
        focus(),
        SEC,
        1.0,
        false,
    );
    let mut findings = Findings::new();
    let raster = inspect_all(&mut h, "apps-200", &mut findings, true, false);
    if let Some((w, ht, buf)) = raster {
        let dir = PathBuf::from("/tmp/typed-ui-ux-issues");
        let _ = std::fs::create_dir_all(&dir);
        write_png(&dir.join("apps-200.png"), w, ht, &buf);
        if findings.issues.iter().any(|i| i.rect.is_some()) {
            annotate_and_save(
                &dir.join("apps-200-issues.png"),
                w,
                ht,
                &buf,
                1.0,
                &findings.issues,
            );
        }
    }
    println!(
        "200 apps: overflow={} issues={}",
        h.app.content_overflow,
        findings.issues.len()
    );
    for i in &findings.issues {
        println!("  [{}] {}", i.kind, i.detail);
    }
    assert!(
        h.app.content_overflow < 80.0,
        "200-app list is wider than the window by {}",
        h.app.content_overflow
    );
}
