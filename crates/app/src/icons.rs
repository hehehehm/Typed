use eframe::egui;
use std::collections::HashMap;

pub use crate::app_id::basename;

pub const ICON_SIZE: f32 = crate::ui_style::HIT;

fn icon_frame(ui: &mut egui::Ui) -> (egui::Rect, egui::Response, egui::Stroke) {
    use crate::ui_style as st;
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(st::HIT), egui::Sense::click());
    let t = st::cur(ui);
    let bg = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(st::CTRL_H));
    if response.is_pointer_button_down_on() {
        ui.painter().rect_filled(bg, st::RADIUS_SM, st::mix(t.surface_hover, t.line, 0.5));
    } else if response.hovered() {
        ui.painter().rect_filled(bg, st::RADIUS_SM, t.surface_hover);
    }
    st::focus_ring(ui, bg, st::RADIUS_SM, response.has_focus());
    let color = if response.hovered() { t.ink } else { st::mix(t.muted, t.ink, 0.35) };
    (rect, response, egui::Stroke::new(1.3, color))
}

/// Trash-can button instead of the word «Удалить».
pub fn delete_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response, stroke) = icon_frame(ui);
    let p = ui.painter();
    let c = rect.center();
    p.line_segment([egui::pos2(c.x - 5.5, c.y - 3.5), egui::pos2(c.x + 5.5, c.y - 3.5)], stroke);
    p.line_segment([egui::pos2(c.x - 2.0, c.y - 5.5), egui::pos2(c.x + 2.0, c.y - 5.5)], stroke);
    let body = egui::Rect::from_min_max(egui::pos2(c.x - 4.0, c.y - 3.5), egui::pos2(c.x + 4.0, c.y + 5.5));
    p.rect_stroke(body, 1.5, stroke);
    p.line_segment([egui::pos2(c.x - 1.5, c.y - 0.5), egui::pos2(c.x - 1.5, c.y + 3.0)], stroke);
    p.line_segment([egui::pos2(c.x + 1.5, c.y - 0.5), egui::pos2(c.x + 1.5, c.y + 3.0)], stroke);
    response.on_hover_text("Удалить")
}

/// Pencil button instead of the word «Изменить».
pub fn edit_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response, stroke) = icon_frame(ui);
    let p = ui.painter();
    let c = rect.center();
    let a = egui::pos2(c.x - 4.5, c.y + 4.5);
    let b = egui::pos2(c.x + 3.0, c.y - 3.0);
    let n = egui::vec2(1.6, 1.6);
    let perp = egui::vec2(1.6, -1.6);
    p.add(egui::Shape::closed_line(vec![a, b - n + perp, b + n - n + perp + n, b + n - perp, a], stroke));
    p.line_segment([egui::pos2(c.x - 1.0, c.y + 5.5), egui::pos2(c.x + 5.5, c.y + 5.5)], stroke);
    response.on_hover_text("Изменить")
}

/// Eye button to show / hide a password.
pub fn eye_icon_button(ui: &mut egui::Ui, revealed: bool) -> egui::Response {
    let (rect, response, stroke) = icon_frame(ui);
    let p = ui.painter();
    let c = rect.center();
    let (w, h) = (7.0, 4.5);
    let mut pts = Vec::new();
    for k in 0..=16 {
        let t = k as f32 / 16.0;
        pts.push(egui::pos2(c.x - w + 2.0 * w * t, c.y - h * (std::f32::consts::PI * t).sin()));
    }
    for k in (1..16).rev() {
        let t = k as f32 / 16.0;
        pts.push(egui::pos2(c.x - w + 2.0 * w * t, c.y + h * (std::f32::consts::PI * t).sin()));
    }
    p.add(egui::Shape::closed_line(pts, stroke));
    p.circle_filled(c, 2.0, stroke.color);
    if revealed {
        p.line_segment([egui::pos2(c.x - 6.0, c.y + 5.5), egui::pos2(c.x + 6.0, c.y - 5.5)], stroke);
    }
    response.on_hover_text(if revealed { "Скрыть" } else { "Показать" })
}

/// Texture cache keyed by app id. `None` = extraction failed, use the placeholder.
#[derive(Default)]
pub struct IconCache {
    map: HashMap<String, Option<egui::TextureHandle>>,
}

impl IconCache {
    pub fn get(&mut self, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
        if let Some(t) = self.map.get(id) {
            return t.clone();
        }
        let tex = load_rgba(id).map(|img| ctx.load_texture(format!("icon:{id}"), img, egui::TextureOptions::LINEAR));
        self.map.insert(id.to_string(), tex.clone());
        tex
    }

    /// Draw the icon (or placeholder) for `id`.
    pub fn show(&mut self, ui: &mut egui::Ui, id: &str) {
        use crate::ui_style as st;
        let (cell, _) = ui.allocate_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE), egui::Sense::hover());
        let rect = egui::Rect::from_center_size(cell.center(), egui::Vec2::splat(st::CAP_H));
        if let Some(tex) = self.get(ui.ctx(), id) {
            egui::Image::new(&tex).maintain_aspect_ratio(true).paint_at(ui, rect);
            return;
        }
        let t = st::cur(ui);
        let p = ui.painter();
        p.rect_filled(rect, st::RADIUS_SM + 1.0, if t.dark { t.surface_hover } else { t.bg });
        p.rect_stroke(rect.shrink(0.5), st::RADIUS_SM + 1.0, st::hair(t.line));
        let letter = basename(id).chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
        p.text(rect.center(), egui::Align2::CENTER_CENTER, letter, st::font_sm(), t.ink);
    }
}

#[cfg(target_os = "macos")]
fn load_rgba(id: &str) -> Option<egui::ColorImage> {
    crate::platform::app_icon_rgba(id)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn load_rgba(_id: &str) -> Option<egui::ColorImage> {
    None
}

#[cfg(target_os = "windows")]
fn load_rgba(id: &str) -> Option<egui::ColorImage> {
    if !crate::app_id::has_path(id) {
        return None;
    }
    std::panic::catch_unwind(|| unsafe { win::extract(id) }).ok().flatten()
}

#[cfg(target_os = "windows")]
mod win {
    use eframe::egui;
    use std::mem::{size_of, zeroed};
    use std::ptr;
    use windows_sys::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HBITMAP, HDC,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    unsafe fn bits(dc: HDC, bmp: HBITMAP, w: i32, h: i32) -> Option<Vec<u8>> {
        let mut bi: BITMAPINFO = zeroed();
        bi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..zeroed()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let n = GetDIBits(dc, bmp, 0, h as u32, buf.as_mut_ptr().cast(), &mut bi, DIB_RGB_COLORS);
        (n != 0).then_some(buf)
    }

    pub unsafe fn extract(path: &str) -> Option<egui::ColorImage> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut info: SHFILEINFOW = zeroed();
        let ok = SHGetFileInfoW(wide.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_SMALLICON);
        if ok == 0 || info.hIcon.is_null() {
            return None;
        }
        let hicon = info.hIcon;
        let mut ii: ICONINFO = zeroed();
        let mut result = None;
        if GetIconInfo(hicon, &mut ii) != 0 {
            let mut bm: BITMAP = zeroed();
            let color = ii.hbmColor;
            if !color.is_null() && GetObjectW(color, size_of::<BITMAP>() as i32, (&mut bm as *mut BITMAP).cast()) != 0 {
                let (w, h) = (bm.bmWidth, bm.bmHeight);
                if w > 0 && h > 0 && w <= 256 && h <= 256 {
                    let dc = GetDC(ptr::null_mut());
                    if let Some(mut px) = bits(dc, color, w, h) {
                        let has_alpha = px.chunks(4).any(|p| p[3] != 0);
                        let mask = if has_alpha || ii.hbmMask.is_null() { None } else { bits(dc, ii.hbmMask, w, h) };
                        for (i, p) in px.chunks_mut(4).enumerate() {
                            p.swap(0, 2); // BGRA → RGBA
                            if !has_alpha {
                                let transparent = mask.as_ref().is_some_and(|m| m[i * 4] != 0);
                                p[3] = if transparent { 0 } else { 255 };
                            }
                        }
                        result = Some(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px));
                    }
                    ReleaseDC(ptr::null_mut(), dc);
                }
            }
            if !ii.hbmColor.is_null() {
                DeleteObject(ii.hbmColor);
            }
            if !ii.hbmMask.is_null() {
                DeleteObject(ii.hbmMask);
            }
        }
        DestroyIcon(hicon);
        result
    }
}
