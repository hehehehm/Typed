
use eframe::egui;
use layout_keeper_core::LogoStyle;

pub fn png(style: LogoStyle) -> &'static [u8] {
    icon_png(style)
}

pub fn icon_png(style: LogoStyle) -> &'static [u8] {
    match style {
        LogoStyle::Beach => include_bytes!("../../../assets/logo-beach.png"),
        LogoStyle::Artdeco => include_bytes!("../../../assets/logo-artdeco.png"),
        LogoStyle::Ukiyoe => include_bytes!("../../../assets/logo-ukiyoe.png"),
        LogoStyle::Poster => include_bytes!("../../../assets/logo-poster.png"),
        LogoStyle::Porthole => include_bytes!("../../../assets/logo-porthole.png"),
    }
}

pub fn uri(style: LogoStyle) -> &'static str {
    match style {
        LogoStyle::Beach => "bytes://typed-logo-beach",
        LogoStyle::Artdeco => "bytes://typed-logo-artdeco",
        LogoStyle::Ukiyoe => "bytes://typed-logo-ukiyoe",
        LogoStyle::Poster => "bytes://typed-logo-poster",
        LogoStyle::Porthole => "bytes://typed-logo-porthole",
    }
}

pub fn label(style: LogoStyle) -> &'static str {
    match style {
        LogoStyle::Beach => "Пляж",
        LogoStyle::Artdeco => "Ар-деко",
        LogoStyle::Ukiyoe => "Укиё-э",
        LogoStyle::Poster => "Плакат",
        LogoStyle::Porthole => "Иллюминатор",
    }
}

pub fn index(style: LogoStyle) -> u8 {
    match style {
        LogoStyle::Beach => 0,
        LogoStyle::Artdeco => 1,
        LogoStyle::Ukiyoe => 2,
        LogoStyle::Poster => 3,
        LogoStyle::Porthole => 4,
    }
}

#[allow(dead_code)]
pub fn from_index(index: u8) -> LogoStyle {
    match index {
        1 => LogoStyle::Artdeco,
        2 => LogoStyle::Ukiyoe,
        3 => LogoStyle::Poster,
        4 => LogoStyle::Porthole,
        _ => LogoStyle::Beach,
    }
}

pub fn texture(ctx: &egui::Context, style: LogoStyle) -> egui::TextureHandle {
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
        egui::ColorImage::from_rgba_premultiplied([icon.width as usize, icon.height as usize], &rgba),
        egui::TextureOptions::LINEAR,
    )
}

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
