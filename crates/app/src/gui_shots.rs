//! Headless rendering of the settings window for layout tests and screenshots.
//! egui is run without a window; its meshes are rasterized on the CPU (premultiplied
//! alpha, gamma-space blending like egui_glow), so no GPU or display is needed.
//!
//! * `every_screen_fits_the_window` (always runs): every screen at the minimum and the
//!   default width, light and dark, never asks for more width than the window has.
//! * `render_ui_screenshots` (`--ignored`): writes PNGs to `$TYPED_UI_SHOTS`
//!   (default `/tmp/typed-ui-shots`).

use super::*;
use eframe::egui::{epaint, Color32, ImageData, Pos2, Rect, TextureId};
use std::collections::HashMap;

const LONG_PATH: &str = r"c:\users\hehehehm\appdata\local\temp\3m40e6uivkbbnon1nlp\setup.tmp";

fn tmp_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("typed-ui-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn sample_cfg() -> Config {
    let mut c = Config::default();
    c.hide_welcome = true;
    c.welcome_gen = layout_keeper_core::config::CURRENT_WELCOME_GEN;
    for (a, l) in [
        (r"C:\Program Files\Google\Chrome\Application\chrome.exe", Lang::En),
        (r"C:\Users\hehehehm\AppData\Local\Telegram Desktop\Telegram.exe", Lang::Ru),
        (r"C:\Program Files\Microsoft VS Code\Code.exe", Lang::En),
        ("WindowsTerminal.exe", Lang::En),
        (r"C:\Program Files\Microsoft Office\root\Office16\WINWORD.EXE", Lang::Ru),
        (LONG_PATH, Lang::En),
    ] {
        c.set_layout(a, l);
    }
    c.exception_apps = vec!["powershell".into(), "windowsterminal".into(), LONG_PATH.into(), "mobaxterm".into()];
    c.custom_words = ["exe", "ci/cd", "c++", "mp3", "гугл", "кубер", "деплой", "rustup", "egui"].map(String::from).to_vec();
    c.never_correct = ["Мандела", "actix", "kubectl"].map(String::from).to_vec();
    c.never_typo_fix = ["хранилка", "стеллажу"].map(String::from).to_vec();
    c
}

fn app_with(cfg: Config, tab: Tab, light: bool) -> App {
    let dir = tmp_dir();
    let mut cfg = cfg;
    cfg.light_theme = light;
    App {
        path: dir.join("config.json"),
        cfg,
        mtime: None,
        last_check: Instant::now(),
        focus: Arc::new(Mutex::new(Focus::default())),
        status: String::new(),
        qr: Default::default(),
        manual_id: String::new(),
        manual_lang: Lang::Ru,
        exception_draft: String::new(),
        tab,
        vault: crate::vault_ui::VaultUi::for_test(dir.join("no-vault.bin"), None),
        capturing_hotkey: None,
        word_draft: String::new(),
        expanded_paths: HashSet::new(),
        show_system_apps: false,
        icons: IconCache::default(),
        open_groups: HashSet::new(),
        content_overflow: 0.0,
        tray: None,
        allow_close: false,
        ui_visible: true,
        chrome_ready: true,
        last_idle_frame: Instant::now(),
        idle_frames: 0,
        welcome_open: false,
        welcome_dont_show: false,
        logo_textures: [None, None, None],
        logo_fade_textures: [None, None, None],
        admin_job: None,
        daemon_admin: (None, true, None),
    }
}

fn focus() -> Focus {
    Focus { app: Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe".into()), layout: Some(Lang::En) }
}

struct Shot {
    name: &'static str,
    app: App,
    focus: Focus,
    size: egui::Vec2,
}

fn all_groups(app: &mut App) {
    for k in ["fix", "typo", "undo", "keys", "admin", "look", "about"] {
        app.open_groups.insert(k);
    }
}

fn unlocked_vault(dir: &std::path::Path) -> crate::vault_ui::VaultUi {
    use layout_keeper_core::vault::{Card, Login, Vault};
    let mut v = Vault::create("screenshot-pass").expect("vault");
    let mut l = Login::default();
    l.label = "Google".into();
    l.username = "hehehehm@gmail.com".into();
    l.password = "x".into();
    l.app_match = "chrome".into();
    v.data.logins.push(l);
    let mut l = Login::default();
    l.label = "GitHub".into();
    l.username = "hehehehm".into();
    l.password = "y".into();
    v.data.logins.push(l);
    let mut c = Card::default();
    c.label = "Тинькофф".into();
    c.number = "5536 9100 0000 4242".into();
    c.expiry = "08/29".into();
    c.holder = "HEHEHE HM".into();
    v.data.cards.push(c);
    crate::vault_ui::VaultUi::for_test(dir.join("vault.bin"), Some(v))
}

fn shots(with_vault: bool) -> Vec<Shot> {
    let w = egui::vec2(440.0, 600.0);
    let mut v = Vec::new();
    for light in [true, false] {
        let sfx = |s: &'static str| -> &'static str {
            if light { s } else { Box::leak(format!("{s}-dark").into_boxed_str()) }
        };
        v.push(Shot { name: sfx("01-home"), app: app_with(sample_cfg(), Tab::Home, light), focus: focus(), size: w });
        v.push(Shot { name: sfx("02-layouts"), app: app_with(sample_cfg(), Tab::Layouts, light), focus: focus(), size: egui::vec2(440.0, 760.0) });
        v.push(Shot { name: sfx("03-exceptions-long-path"), app: app_with(sample_cfg(), Tab::Exceptions, light), focus: focus(), size: w });
        v.push(Shot { name: sfx("04-words"), app: app_with(sample_cfg(), Tab::Words, light), focus: focus(), size: w });
        v.push(Shot { name: sfx("05-vault-locked"), app: app_with(sample_cfg(), Tab::Vault, light), focus: focus(), size: w });
        v.push(Shot { name: sfx("07-settings"), app: app_with(sample_cfg(), Tab::Settings, light), focus: focus(), size: w });
        let mut s = app_with(sample_cfg(), Tab::Settings, light);
        all_groups(&mut s);
        v.push(Shot { name: sfx("08-settings-expanded"), app: s, focus: focus(), size: egui::vec2(440.0, 2000.0) });
        v.push(Shot { name: sfx("09-donate"), app: app_with(sample_cfg(), Tab::Donate, light), focus: focus(), size: egui::vec2(440.0, 640.0) });
        if with_vault {
            let mut a = app_with(sample_cfg(), Tab::Vault, light);
            a.vault = unlocked_vault(&tmp_dir());
            v.push(Shot { name: sfx("06-vault-open"), app: a, focus: focus(), size: egui::vec2(440.0, 700.0) });
        }
    }
    // Narrowest window with the long path, and the path expanded in Раскладки.
    v.push(Shot { name: "10-exceptions-min-width", app: app_with(sample_cfg(), Tab::Exceptions, true), focus: focus(), size: egui::vec2(380.0, 440.0) });
    let mut a = app_with(sample_cfg(), Tab::Layouts, true);
    a.expanded_paths.insert(LONG_PATH.into());
    a.show_system_apps = true;
    v.push(Shot { name: "11-layouts-min-width-path-open", app: a, focus: focus(), size: egui::vec2(380.0, 760.0) });
    v.push(Shot { name: "12-home-min-size", app: app_with(sample_cfg(), Tab::Home, true), focus: focus(), size: egui::vec2(380.0, 440.0) });
    let mut a = app_with(sample_cfg(), Tab::Exceptions, true);
    a.status = "Сохранено — демон применит изменения примерно за секунду.".into();
    v.push(Shot { name: "13-exceptions-status", app: a, focus: focus(), size: w });
    let mut a = app_with(sample_cfg(), Tab::Home, true);
    a.welcome_open = true;
    v.push(Shot { name: "14-welcome", app: a, focus: focus(), size: egui::vec2(440.0, 900.0) });
    v
}

/// Runs a few frames (egui settles sizes over two passes) and returns the final output.
fn run(shot: &mut Shot, ppp: f32) -> (egui::Context, egui::FullOutput, HashMap<TextureId, egui::ColorImage>) {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    st::install_fonts(&ctx);
    let mut textures: HashMap<TextureId, egui::ColorImage> = HashMap::new();
    let mut last = None;
    for frame in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, shot.size)),
            time: Some(frame as f64 * 0.1),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| shot.app.draw(ctx, &shot.focus));
        for (id, delta) in &out.textures_delta.set {
            let img = match &delta.image {
                ImageData::Color(c) => (**c).clone(),
                ImageData::Font(f) => egui::ColorImage { size: f.size, pixels: f.srgba_pixels(None).collect() },
            };
            match delta.pos {
                None => {
                    textures.insert(*id, img);
                }
                Some([x0, y0]) => {
                    let t = textures.get_mut(id).expect("partial update of a known texture");
                    for y in 0..img.size[1] {
                        for x in 0..img.size[0] {
                            t.pixels[(y0 + y) * t.size[0] + x0 + x] = img.pixels[y * img.size[0] + x];
                        }
                    }
                }
            }
        }
        last = Some(out);
    }
    (ctx, last.unwrap(), textures)
}

fn sample(img: &egui::ColorImage, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = (img.size[0] as i64, img.size[1] as i64);
    let x = u * w as f32 - 0.5;
    let y = v * h as f32 - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xi: i64, yi: i64| {
        let c = img.pixels[(yi.clamp(0, h - 1) * w + xi.clamp(0, w - 1)) as usize];
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1));
    let mut o = [0.0; 4];
    for i in 0..4 {
        o[i] = (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
    }
    o
}

fn rasterize(ctx: &egui::Context, out: egui::FullOutput, textures: &HashMap<TextureId, egui::ColorImage>, size: egui::Vec2, ppp: f32) -> (usize, usize, Vec<[f32; 4]>) {
    let (w, h) = ((size.x * ppp).round() as usize, (size.y * ppp).round() as usize);
    let mut buf = vec![[0.0f32, 0.0, 0.0, 255.0]; w * h];
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    for p in prims {
        let epaint::Primitive::Mesh(mesh) = p.primitive else { continue };
        let Some(tex) = textures.get(&mesh.texture_id) else { continue };
        let clip = p.clip_rect;
        let (cx0, cy0) = (((clip.min.x * ppp).floor().max(0.0)) as i64, ((clip.min.y * ppp).floor().max(0.0)) as i64);
        let (cx1, cy1) = (((clip.max.x * ppp).ceil().min(w as f32)) as i64, ((clip.max.y * ppp).ceil().min(h as f32)) as i64);
        for tri in mesh.indices.chunks_exact(3) {
            let mut v = [mesh.vertices[tri[0] as usize], mesh.vertices[tri[1] as usize], mesh.vertices[tri[2] as usize]];
            let pos = |i: usize, v: &[epaint::Vertex; 3]| (v[i].pos.x * ppp, v[i].pos.y * ppp);
            let area = {
                let (a, b, c) = (pos(0, &v), pos(1, &v), pos(2, &v));
                (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
            };
            if area.abs() < 1e-9 {
                continue;
            }
            if area < 0.0 {
                v.swap(1, 2);
            }
            let area = area.abs();
            let (a, b, c) = (pos(0, &v), pos(1, &v), pos(2, &v));
            let minx = (a.0.min(b.0).min(c.0).floor() as i64).max(cx0);
            let maxx = (a.0.max(b.0).max(c.0).ceil() as i64).min(cx1);
            let miny = (a.1.min(b.1).min(c.1).floor() as i64).max(cy0);
            let maxy = (a.1.max(b.1).max(c.1).ceil() as i64).min(cy1);
            // Edge function and the top-left rule so shared edges are drawn once.
            let edge = |p: (f32, f32), q: (f32, f32), x: f32, y: f32| (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0);
            let tl = |p: (f32, f32), q: (f32, f32)| {
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                (dy == 0.0 && dx > 0.0) || dy < 0.0
            };
            let (t0, t1, t2) = (tl(b, c), tl(c, a), tl(a, b));
            for y in miny..maxy {
                for x in minx..maxx {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = edge(b, c, px, py);
                    let w1 = edge(c, a, px, py);
                    let w2 = edge(a, b, px, py);
                    let inside = |w: f32, t: bool| w > 0.0 || (w == 0.0 && t);
                    if !(inside(w0, t0) && inside(w1, t1) && inside(w2, t2)) {
                        continue;
                    }
                    let (l0, l1, l2) = (w0 / area, w1 / area, w2 / area);
                    let u = l0 * v[0].uv.x + l1 * v[1].uv.x + l2 * v[2].uv.x;
                    let vv = l0 * v[0].uv.y + l1 * v[1].uv.y + l2 * v[2].uv.y;
                    let col = |i: usize| {
                        let c: Color32 = v[i].color;
                        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
                    };
                    let (c0, c1, c2) = (col(0), col(1), col(2));
                    let t = sample(tex, u, vv);
                    let mut s = [0.0f32; 4];
                    for i in 0..4 {
                        let vc = l0 * c0[i] + l1 * c1[i] + l2 * c2[i];
                        s[i] = vc * t[i] / 255.0;
                    }
                    let d = &mut buf[y as usize * w + x as usize];
                    let k = 1.0 - s[3] / 255.0;
                    for i in 0..4 {
                        d[i] = s[i] + d[i] * k;
                    }
                }
            }
        }
    }
    (w, h, buf)
}

fn write_png(path: &std::path::Path, w: usize, h: usize, buf: &[[f32; 4]]) {
    let f = std::fs::File::create(path).expect("png file");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().expect("png header");
    let data: Vec<u8> = buf.iter().flat_map(|p| [p[0], p[1], p[2]].map(|c| c.round().clamp(0.0, 255.0) as u8)).collect();
    wr.write_image_data(&data).expect("png data");
}

#[test]
fn every_screen_fits_the_window() {
    for mut shot in shots(false) {
        for width in [380.0, 440.0] {
            shot.size.x = width;
            let _ = run(&mut shot, 1.0);
            assert!(shot.app.content_overflow < 0.5, "{} at {width}px: content {}px wider than the window", shot.name, shot.app.content_overflow);
        }
    }
}

#[test]
#[ignore]
fn render_ui_screenshots() {
    let dir = std::env::var_os("TYPED_UI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp/typed-ui-shots"));
    std::fs::create_dir_all(&dir).unwrap();
    let ppp = 2.0;
    for mut shot in shots(true) {
        let (ctx, out, tex) = run(&mut shot, ppp);
        let (w, h, buf) = rasterize(&ctx, out, &tex, shot.size, ppp);
        write_png(&dir.join(format!("{}.png", shot.name)), w, h, &buf);
        println!("{} overflow={}", shot.name, shot.app.content_overflow);
    }
}
