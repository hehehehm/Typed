//! Bundled app marks the user can pick in settings.

use eframe::egui::{self, Color32};
use layout_keeper_core::LogoStyle;

pub fn png(style: LogoStyle) -> &'static [u8] {
    match style {
        LogoStyle::Water => include_bytes!("../../../assets/typed-icon-256.png"),
        LogoStyle::Suprematism => include_bytes!("../../../assets/typed-logo-suprematism.png"),
        LogoStyle::Geometry => include_bytes!("../../../assets/typed-logo-geometry.png"),
    }
}

/// Opaque square for the Windows title bar, taskbar and tray.
/// The water painting keeps a rounded alpha fringe that Windows draws as white corners.
pub fn icon_png(style: LogoStyle) -> &'static [u8] {
    match style {
        LogoStyle::Water => include_bytes!("../../../assets/typed-icon-app.png"),
        LogoStyle::Suprematism => include_bytes!("../../../assets/typed-logo-suprematism-app.png"),
        LogoStyle::Geometry => include_bytes!("../../../assets/typed-logo-geometry-app.png"),
    }
}

pub fn uri(style: LogoStyle) -> &'static str {
    match style {
        LogoStyle::Water => "bytes://typed-logo-water",
        LogoStyle::Suprematism => "bytes://typed-logo-suprematism",
        LogoStyle::Geometry => "bytes://typed-logo-geometry",
    }
}

pub fn label(style: LogoStyle) -> &'static str {
    match style {
        LogoStyle::Water => "Море",
        LogoStyle::Suprematism => "Супреморе",
        LogoStyle::Geometry => "Геометроморе",
    }
}

pub fn index(style: LogoStyle) -> u8 {
    match style {
        LogoStyle::Water => 0,
        LogoStyle::Suprematism => 1,
        LogoStyle::Geometry => 2,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn from_index(index: u8) -> LogoStyle {
    match index {
        1 => LogoStyle::Suprematism,
        2 => LogoStyle::Geometry,
        _ => LogoStyle::Water,
    }
}

pub fn texture(ctx: &egui::Context, style: LogoStyle) -> egui::TextureHandle {
    // Use the opaque full-bleed marks so picker tiles are the same visual size.
    // The rounded/transparent water PNG looks smaller next to the other two.
    let icon = eframe::icon_data::from_png_bytes(icon_png(style)).expect("bundled logo png");
    let mut rgba = icon.rgba;
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u16;
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else if a < 255 {
            px[0] = ((px[0] as u16 * a) / 255) as u8;
            px[1] = ((px[1] as u16 * a) / 255) as u8;
            px[2] = ((px[2] as u16 * a) / 255) as u8;
        }
    }
    ctx.load_texture(
        uri(style),
        egui::ColorImage::from_rgba_premultiplied(
            [icon.width as usize, icon.height as usize],
            &rgba,
        ),
        egui::TextureOptions::LINEAR,
    )
}

/// Same mark as `texture`, with its own alpha faded to 0 from 85% of its height.
pub fn texture_faded(ctx: &egui::Context, style: LogoStyle) -> egui::TextureHandle {
    let icon = eframe::icon_data::from_png_bytes(png(style)).expect("bundled logo png");
    let w = icon.width as usize;
    let h = icon.height as usize;
    let mut rgba = icon.rgba;
    const FADE_START: f32 = 0.85;
    for y in 0..h {
        let t = if h <= 1 { 1.0 } else { y as f32 / (h as f32 - 1.0).max(1.0) };
        let fade = if t <= FADE_START {
            1.0
        } else {
            let u = ((t - FADE_START) / (1.0 - FADE_START)).clamp(0.0, 1.0);
            let s = u * u * (3.0 - 2.0 * u);
            1.0 - s
        };
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = (rgba[i + 3] as f32 * fade).round().clamp(0.0, 255.0) as u16;
            rgba[i + 3] = a as u8;
            if a == 0 {
                rgba[i] = 0;
                rgba[i + 1] = 0;
                rgba[i + 2] = 0;
            } else if a < 255 {
                rgba[i] = ((rgba[i] as u16 * a) / 255) as u8;
                rgba[i + 1] = ((rgba[i + 1] as u16 * a) / 255) as u8;
                rgba[i + 2] = ((rgba[i + 2] as u16 * a) / 255) as u8;
            }
        }
    }
    ctx.load_texture(
        format!("{}-fade", uri(style)),
        egui::ColorImage::from_rgba_premultiplied([w, h], &rgba),
        egui::TextureOptions::LINEAR,
    )
}

/// Water colors sampled from the selected mark, tuned so selected buttons stay readable.
pub struct Palette {
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub gold: Color32,
    /// Text drawn on accent fills (selected EN/RU pills and similar).
    pub on_accent: Color32,
}

pub fn palette(style: LogoStyle, light: bool) -> Palette {
    match (style, light) {
        (LogoStyle::Water, true) => Palette {
            accent: Color32::from_rgb(42, 152, 168),
            accent_hover: Color32::from_rgb(60, 180, 192),
            accent_soft: Color32::from_rgb(196, 232, 232),
            gold: Color32::from_rgb(212, 176, 72),
            on_accent: Color32::from_rgb(255, 255, 255),
        },
        (LogoStyle::Water, false) => Palette {
            accent: Color32::from_rgb(60, 180, 192),
            accent_hover: Color32::from_rgb(96, 204, 204),
            accent_soft: Color32::from_rgb(28, 72, 80),
            gold: Color32::from_rgb(232, 196, 96),
            on_accent: Color32::from_rgb(16, 32, 36),
        },
        (LogoStyle::Suprematism, true) => Palette {
            accent: Color32::from_rgb(0, 132, 144),
            accent_hover: Color32::from_rgb(0, 156, 180),
            accent_soft: Color32::from_rgb(188, 232, 232),
            gold: Color32::from_rgb(212, 168, 48),
            on_accent: Color32::from_rgb(255, 255, 255),
        },
        (LogoStyle::Suprematism, false) => Palette {
            accent: Color32::from_rgb(0, 192, 204),
            accent_hover: Color32::from_rgb(108, 216, 216),
            accent_soft: Color32::from_rgb(0, 72, 80),
            gold: Color32::from_rgb(232, 184, 64),
            on_accent: Color32::from_rgb(16, 32, 36),
        },
        (LogoStyle::Geometry, true) => Palette {
            accent: Color32::from_rgb(0, 120, 156),
            accent_hover: Color32::from_rgb(36, 180, 192),
            accent_soft: Color32::from_rgb(196, 228, 232),
            gold: Color32::from_rgb(196, 160, 48),
            on_accent: Color32::from_rgb(255, 255, 255),
        },
        (LogoStyle::Geometry, false) => Palette {
            accent: Color32::from_rgb(36, 180, 192),
            accent_hover: Color32::from_rgb(96, 192, 216),
            accent_soft: Color32::from_rgb(0, 56, 80),
            gold: Color32::from_rgb(212, 176, 72),
            on_accent: Color32::from_rgb(16, 32, 36),
        },
    }
}
