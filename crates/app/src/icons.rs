use eframe::egui;
use std::collections::HashMap;

pub use crate::app_id::{basename, has_path, is_hidden_app};

pub const ICON_SIZE: f32 = 16.0;

/// Compact trash-can button used instead of the word «Удалить».
pub fn delete_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let size = egui::vec2(18.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    if response.hovered() || response.is_pointer_button_down_on() {
        painter.rect_filled(rect.expand(1.0), 3.0, visuals.bg_fill);
    }
    let stroke = visuals.fg_stroke;
    let c = rect.center();
    painter.line_segment(
        [egui::pos2(c.x - 5.0, c.y - 2.5), egui::pos2(c.x + 5.0, c.y - 2.5)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(c.x - 2.0, c.y - 4.5), egui::pos2(c.x + 2.0, c.y - 4.5)],
        stroke,
    );
    let body = egui::Rect::from_min_max(egui::pos2(c.x - 3.5, c.y - 2.5), egui::pos2(c.x + 3.5, c.y + 5.0));
    painter.rect_stroke(body, 0.8, stroke);
    painter.line_segment([egui::pos2(c.x, c.y - 1.0), egui::pos2(c.x, c.y + 3.5)], stroke);
    response.on_hover_text("Удалить")
}

/// Small painted arrow button: down into a tray to hide an app in the collapsed group,
/// up out of it to bring the app back. Painted (like the trash icon) so it does not
/// depend on which symbols the bundled fonts have.
pub fn hide_icon_button(ui: &mut egui::Ui, hide: bool) -> egui::Response {
    let size = egui::vec2(18.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    if response.hovered() || response.is_pointer_button_down_on() {
        painter.rect_filled(rect.expand(1.0), 3.0, visuals.bg_fill);
    }
    let stroke = visuals.fg_stroke;
    let c = rect.center();
    // tray
    painter.line_segment([egui::pos2(c.x - 5.0, c.y + 5.0), egui::pos2(c.x + 5.0, c.y + 5.0)], stroke);
    // shaft
    painter.line_segment([egui::pos2(c.x, c.y - 5.0), egui::pos2(c.x, c.y + 2.5)], stroke);
    // head: pointing down (hide) or up (show again)
    let (tip, wing) = if hide { (c.y + 2.5, c.y - 0.5) } else { (c.y - 5.0, c.y - 2.0) };
    painter.line_segment([egui::pos2(c.x - 3.0, wing), egui::pos2(c.x, tip)], stroke);
    painter.line_segment([egui::pos2(c.x + 3.0, wing), egui::pos2(c.x, tip)], stroke);
    response.on_hover_text(if hide { "Скрыть в системные" } else { "Вернуть в список" })
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
        let size = egui::vec2(ICON_SIZE, ICON_SIZE);
        let rounding = 4.5;
        if let Some(tex) = self.get(ui.ctx(), id) {
            ui.add(egui::Image::new(&tex).fit_to_exact_size(size).rounding(rounding));
            return;
        }
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, rounding, ui.visuals().widgets.inactive.bg_fill);
        let letter = basename(id).chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
        p.text(rect.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(11.0), ui.visuals().text_color());
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
    if !has_path(id) {
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
