//! Settings window (`typed gui`). A separate process that only edits the same
//! `config.json` the daemon watches; the daemon picks changes up within ~1 s.
//! The keyboard hook is never touched from here.

use crate::brand::{APP_NAME, WINDOW_TITLE};
use crate::platform;
use eframe::egui;
use layout_keeper_core::{typo::Level as TypoLevel, Config, Hotkey, HotkeyCode, Lang, LogoStyle};
use crate::icons::{basename, delete_icon_button, IconCache};
use crate::ui_style as st;
use layout_keeper_core::userdict;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

#[derive(Default, Clone)]
struct Focus {
    /// Last focused app that is not this settings window.
    app: Option<String>,
    layout: Option<Lang>,
}

pub(crate) fn is_self(app: &str) -> bool {
    let a = app.to_lowercase();
    let name = basename(app).to_lowercase();
    if name.contains("layout-keeper")
        || name.contains("layout_keeper")
        || name.contains("layoutkeeper")
        || name == "typed"
        || name == "typed.exe"
        || a == "app.layoutkeeper"
    {
        return true;
    }
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_lowercase())
        .is_some_and(|p| p == a)
}

/// Shortest gap between frames while the window is hidden or minimized.
const IDLE_FRAME: Duration = Duration::from_millis(250);

/// Home window: five squares in one short row.
const HOME_INNER: [f32; 2] = [512.0, 192.0];
/// Section window: settings (and other tabs) fit this height without scrolling.
const SECTION_INNER: [f32; 2] = [512.0, if cfg!(windows) { 536.0 } else { 496.0 }];

pub(crate) fn hide_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
}

pub(crate) fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

struct App {
    path: PathBuf,
    cfg: Config,
    mtime: Option<SystemTime>,
    last_check: Instant,
    focus: Arc<Mutex<Focus>>,
    status: String,
    qr: std::collections::HashMap<&'static str, egui::TextureHandle>,
    app_draft: String,
    app_mode: AppMode,
    apps_pane: AppsPane,
    tab: Tab,
    vault: crate::vault_ui::VaultUi,
    capturing_hotkey: Option<HotkeySlot>,
    word_draft: String,
    icons: IconCache,
    /// Grow the window to `SECTION_INNER` the next time a non-home tab is shown.
    grow_for_section: bool,
    /// How much the last frame's content was wider than the window (should stay 0).
    content_overflow: f32,
    tray: Option<crate::tray::TrayHandle>,
    /// When true, the close button / tray "Выход" actually quits instead of hiding.
    allow_close: bool,
    /// False after hide-to-tray. The close button clears `minimized`, so we cannot
    /// use that flag to detect a hidden window — it would keep repainting forever.
    ui_visible: bool,
    /// Dock icon / reopen hook applied once after the native window exists.
    chrome_ready: bool,
    /// Throttle for frames while hidden or minimized (see `IDLE_FRAME`).
    last_idle_frame: Instant,
    idle_frames: u64,
    welcome_open: bool,
    welcome_dont_show: bool,
    logo_textures: [Option<egui::TextureHandle>; 5],
    logo_fade_textures: [Option<egui::TextureHandle>; 3],
    sea_textures: [Option<egui::TextureHandle>; 2],
    /// «Запуск администратором» being switched: (wanted value, helper result when done).
    #[cfg_attr(not(windows), allow(dead_code))]
    admin_job: Option<(bool, AdminJobResult)>,
    /// Last check whether the daemon runs as administrator, and when it was made.
    #[cfg_attr(not(windows), allow(dead_code))]
    daemon_admin: (Option<bool>, bool, Option<Instant>),
}

type AdminJobResult = Arc<Mutex<Option<Result<(), String>>>>;

#[derive(PartialEq, Clone, Copy)]
#[repr(u8)]
enum HotkeySlot {
    Convert,
    Summon,
    Undo,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum AppMode {
    En,
    Ru,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum AppsPane {
    Programs,
    Exceptions,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    /// Five identical squares in one row.
    Home,
    Apps,
    Words,
    Vault,
    Settings,
    Donate,
}

const BOOSTY_URL: &str = "https://boosty.to/hehehehm/donate";
const USDT_TRC20: &str = "TExcg4dfgq4BnaEE9ZzdvcATwWQb51vrwx";
const GRAM_TON: &str = "UQCLfvmcU8sMYWE-xya2syD5N39du0qRuZ0ssnaSRi33IkG9";

fn mtime(p: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(p).ok()?.modified().ok()
}

impl App {
    fn save(&mut self) {
        match self.cfg.save(&self.path) {
            Ok(()) => {
                self.mtime = mtime(&self.path);
                self.status.clear();
            }
            Err(e) => self.status = format!("Ошибка: {e}"),
        }
    }

    fn add_app_entry(&mut self) -> bool {
        let entry = self.app_draft.trim().to_string();
        if entry.is_empty() {
            return false;
        }
        match self.apps_pane {
            AppsPane::Programs => self.set_app_mode(&entry, self.app_mode),
            AppsPane::Exceptions => {
                self.cfg.add_exception(&entry);
            }
        }
        self.app_draft.clear();
        true
    }

    fn set_app_mode(&mut self, id: &str, mode: AppMode) {
        self.cfg.remove_exception(id);
        match mode {
            AppMode::En => {
                self.cfg.set_layout(id, Lang::En);
            }
            AppMode::Ru => {
                self.cfg.set_layout(id, Lang::Ru);
            }
        }
    }

    fn remove_app_entry(&mut self, id: &str) {
        self.cfg.remove_app(id);
        self.cfg.remove_exception(id);
    }

    fn add_custom_word(&mut self) -> bool {
        let w = self.word_draft.trim().to_string();
        if let Err(e) = userdict::classify(&w) {
            self.status = e.message_ru().into();
            return false;
        }
        if userdict::contains_word(&self.cfg.custom_words, &w) {
            self.status = "Уже есть".into();
            return false;
        }
        self.cfg.custom_words.push(w);
        self.word_draft.clear();
        true
    }

    fn apps_tab(&mut self, ui: &mut egui::Ui) {
        let accent = Tab::Apps.color();
        let t = st::cur(ui);
        let mut changed = false;

        // Программы | Исключения at the top of the section; each pane has its own
        // add button and the header count follows the open pane.
        let n_prog = self.cfg.apps.len();
        let n_exc = self.cfg.exception_apps.len();
        let prog = format!("Программы  {n_prog}");
        let exc = format!("Исключения  {n_exc}");
        st::group(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(st::ROW);
                st::segment_strip(
                    ui,
                    &mut self.apps_pane,
                    &[
                        (AppsPane::Programs, prog.as_str(), "Раскладка EN / RU при переходе в программу"),
                        (AppsPane::Exceptions, exc.as_str(), "Typed здесь полностью выключен"),
                    ],
                    ui.available_width(),
                    accent,
                );
            });
        });
        ui.add_space(st::PAD);

        match self.apps_pane {
            AppsPane::Programs => {
                let mut go = false;
                st::group(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(st::ROW);
                        ui.spacing_mut().item_spacing.x = st::PAD;
                        let fw = (ui.available_width() - st::HIT * 3.0 - st::PAD * 2.0).max(st::HIT * 2.0);
                        let r = st::field_bg(ui, &mut self.app_draft, "программа или путь", fw, accent);
                        go |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        go |= st::segment_strip(
                            ui,
                            &mut self.app_mode,
                            &[
                                (AppMode::En, "EN", "Английская раскладка"),
                                (AppMode::Ru, "RU", "Русская раскладка"),
                            ],
                            st::HIT * 2.0,
                            accent,
                        );
                        go |= st::plus_square(ui, accent, "Добавить").clicked();
                    });
                });
                if go {
                    changed |= self.add_app_entry();
                }

                let mut entries: Vec<(String, AppMode)> = self
                    .cfg
                    .apps
                    .iter()
                    .map(|(k, l)| {
                        (k.clone(), match l {
                            Lang::En => AppMode::En,
                            Lang::Ru => AppMode::Ru,
                        })
                    })
                    .collect();
                entries.sort_by(|a, b| basename(&a.0).to_lowercase().cmp(&basename(&b.0).to_lowercase()).then_with(|| a.0.cmp(&b.0)));

                let mut act: Option<(String, Option<AppMode>)> = None;
                if !entries.is_empty() {
                    ui.add_space(st::PAD);
                    st::group_title(ui, "Раскладка", Some(entries.len()), "EN / RU — раскладка при переключении в программу");
                    let icons = &mut self.icons;
                    st::group(ui, |ui| {
                        for (k, (id, cur)) in entries.into_iter().enumerate() {
                            if k > 0 {
                                st::sep(ui);
                            }
                            let mut mode = cur;
                            st::line_row(
                                ui,
                                |ui| {
                                    icons.show(ui, &id);
                                    let name = basename(&id);
                                    let shown = if name.is_empty() { id.as_str() } else { name };
                                    ui.add(egui::Label::new(egui::RichText::new(shown).color(t.ink)).truncate()).on_hover_text(&id);
                                },
                                |ui| {
                                    if delete_icon_button(ui).clicked() {
                                        act = Some((id.clone(), None));
                                    }
                                    if st::segment_strip(
                                        ui,
                                        &mut mode,
                                        &[
                                            (AppMode::En, "EN", "Английская раскладка"),
                                            (AppMode::Ru, "RU", "Русская раскладка"),
                                        ],
                                        st::HIT * 2.0,
                                        accent,
                                    ) {
                                        act = Some((id.clone(), Some(mode)));
                                    }
                                },
                            );
                        }
                    });
                }
                if let Some((id, mode)) = act {
                    match mode {
                        Some(m) => self.set_app_mode(&id, m),
                        None => self.remove_app_entry(&id),
                    }
                    changed = true;
                }
            }
            AppsPane::Exceptions => {
                st::group(ui, |ui| {
                    egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, st::GAP)).show(ui, |ui| {
                        if st::add_line_plus(ui, &mut self.app_draft, "программа или путь", "Добавить исключение", accent) {
                            changed |= self.add_app_entry();
                        }
                    });
                });

                let mut entries: Vec<String> = self.cfg.exception_apps.clone();
                entries.sort_by(|a, b| basename(a).to_lowercase().cmp(&basename(b).to_lowercase()).then_with(|| a.cmp(b)));
                let mut remove = None;
                if !entries.is_empty() {
                    ui.add_space(st::PAD);
                    st::group_title(ui, "Исключения", Some(entries.len()), "Typed полностью выключен");
                    let icons = &mut self.icons;
                    st::group(ui, |ui| {
                        for (k, id) in entries.into_iter().enumerate() {
                            if k > 0 {
                                st::sep(ui);
                            }
                            st::line_row(
                                ui,
                                |ui| {
                                    icons.show(ui, &id);
                                    let name = basename(&id);
                                    let shown = if name.is_empty() { id.as_str() } else { name };
                                    ui.add(egui::Label::new(egui::RichText::new(shown).color(t.ink)).truncate()).on_hover_text(&id);
                                },
                                |ui| {
                                    if delete_icon_button(ui).clicked() {
                                        remove = Some(id.clone());
                                    }
                                },
                            );
                        }
                    });
                }
                if let Some(id) = remove {
                    self.cfg.remove_exception(&id);
                    changed = true;
                }
            }
        }

        if changed {
            self.save();
        }
    }

    fn words_tab(&mut self, ui: &mut egui::Ui) {
        let accent = Tab::Words.color();
        let mut changed = false;
        st::group(ui, |ui| {
            egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, st::GAP)).show(ui, |ui| {
                if st::add_line_plus(ui, &mut self.word_draft, "слово", "Добавить слово (без пробелов; можно цифры и знаки)", accent) {
                    changed |= self.add_custom_word();
                }
            });
        });

        let groups: [(&str, &str, u8); 3] = [
            ("Свои", "Приоритет выше встроенного словаря", 0),
            ("Пропуск", "Слова, исправление которых вы отменили дважды", 1),
            ("Без опечаток", "Отменена только правка опечаток; раскладка исправляется", 2),
        ];
        for (title, tip, which) in groups {
            let words = match which {
                0 => self.cfg.custom_words.clone(),
                1 => self.cfg.never_correct.clone(),
                _ => self.cfg.never_typo_fix.clone(),
            };
            if words.is_empty() {
                continue;
            }
            ui.add_space(st::PAD);
            st::group_title(ui, title, Some(words.len()), tip);
            let mut remove_at = None;
            st::group(ui, |ui| {
                egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, st::PAD)).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(st::PAD, st::GAP);
                        for (i, w) in words.iter().enumerate() {
                            let tag = if which == 0 {
                                Some(match userdict::classify(w) {
                                    Ok(l) => lang_name(l),
                                    Err(_) => "?",
                                })
                            } else {
                                None
                            };
                            if st::chip(ui, w, tag) {
                                remove_at = Some(i);
                            }
                        }
                    });
                });
            });
            if let Some(i) = remove_at {
                let list = match which {
                    0 => &mut self.cfg.custom_words,
                    1 => &mut self.cfg.never_correct,
                    _ => &mut self.cfg.never_typo_fix,
                };
                if i < list.len() {
                    list.remove(i);
                    changed = true;
                }
            }
        }
        if changed {
            self.save();
        }
    }

    fn maybe_reload(&mut self) {
        if self.last_check.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last_check = Instant::now();
        let m = mtime(&self.path);
        if m.is_some() && m != self.mtime {
            if let Ok(c) = std::fs::read_to_string(&self.path).and_then(|s| Config::from_json(&s)) {
                self.cfg = c;
            }
            self.mtime = m;
        }
    }

    fn hotkey_picker(&mut self, ui: &mut egui::Ui, slot: HotkeySlot, label: &str, presets: &[(Hotkey, &str)]) {
        let accent = Tab::Settings.color();
        let current = match slot {
            HotkeySlot::Convert => self.cfg.hotkey,
            HotkeySlot::Summon => self.cfg.summon_hotkey,
            HotkeySlot::Undo => self.cfg.undo_key,
        };
        let t = st::cur(ui);
        let capturing = self.capturing_hotkey == Some(slot);
        let mut picked = None;
        let mut start_capture = false;
        let label_w = st::text_w(ui, label, st::font_md()) + st::PAD * 2.0;
        st::line_row(
            ui,
            |ui| {
                ui.add(egui::Label::new(egui::RichText::new(label).color(t.ink)).truncate());
            },
            |ui| {
                let max = (ui.available_width() - label_w).max(st::HIT * 2.0);
                if capturing {
                    st::keycaps(ui, "Нажмите сочетание", max, true, accent).on_hover_text("Esc — отмена");
                    return;
                }
                let shown = presets.iter().find(|p| p.0 == current).map(|p| p.1.to_string()).unwrap_or_else(|| current.to_string());
                let r = st::keycaps(ui, &shown, max, false, accent).on_hover_text("Выбрать или назначить");
                let popup_id = ui.make_persistent_id(("hotkey", slot as u8));
                if r.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                egui::popup::popup_below_widget(ui, popup_id, &r, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(st::HIT * 6.0);
                    ui.spacing_mut().item_spacing.y = st::GAP;
                    for &(h, name) in presets {
                        ui.horizontal(|ui| {
                            if st::keycaps(ui, name, st::HIT * 8.0, current == h, accent).clicked() {
                                picked = Some(h);
                            }
                        });
                    }
                    if st::link(ui, "Назначить…", accent).clicked() {
                        start_capture = true;
                    }
                });
            },
        );
        if start_capture {
            self.capturing_hotkey = Some(slot);
            self.status = "Нажмите сочетание…".into();
        }
        if let Some(h) = picked {
            match slot {
                HotkeySlot::Convert => self.cfg.hotkey = h,
                HotkeySlot::Summon => self.cfg.summon_hotkey = h,
                HotkeySlot::Undo => {
                    self.cfg.undo_key = h;
                    self.cfg.undo_hotkey = h;
                }
            }
            self.save();
        }
    }

    fn donate_tab(&mut self, ui: &mut egui::Ui) {
        let t = st::cur(ui);
        st::group(ui, |ui| {
            st::line_row(
                ui,
                |ui| {
                    ui.label(egui::RichText::new("Boosty").color(t.ink));
                },
                |ui| {
                    if st::primary(ui, "Открыть", Tab::Donate.color()).on_hover_text(BOOSTY_URL).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(BOOSTY_URL));
                    }
                },
            );
        });
        ui.add_space(st::MARGIN);
        st::group(ui, |ui| {
            self.address_block(ui, "USDT · TRC-20", USDT_TRC20, "Только сеть TRON (TRC-20)");
            st::sep(ui);
            self.address_block(ui, "GRAM · TON", GRAM_TON, "Только сеть TON");
        });
    }

    fn address_block(&mut self, ui: &mut egui::Ui, title: &str, addr: &'static str, tip: &str) {
        let t = st::cur(ui);
        let tex = self
            .qr
            .entry(addr)
            .or_insert_with(|| ui.ctx().load_texture(format!("qr-{addr}"), qr_image(addr), egui::TextureOptions::NEAREST))
            .clone();
        let side = st::U * 20.0;
        egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, st::INSET)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = st::MARGIN;
                // QR on a white plate with a quiet zone, readable in both themes.
                let (plate, _) = ui.allocate_exact_size(egui::vec2(side + st::PAD, side + st::PAD), egui::Sense::hover());
                ui.painter().rect_filled(plate, st::RADIUS_SM, egui::Color32::WHITE);
                ui.painter().rect_stroke(plate.shrink(0.5), st::RADIUS_SM, st::hair(t.line));
                egui::Image::new(&tex).fit_to_exact_size(egui::vec2(side, side)).paint_at(ui, plate.shrink(st::GAP));
                ui.interact(plate, ui.id().with(("qr", addr)), egui::Sense::hover()).on_hover_text(tip);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = st::GAP;
                    ui.set_max_width(ui.available_width());
                    ui.label(egui::RichText::new(title).color(t.ink)).on_hover_text(tip);
                    ui.add(egui::Label::new(egui::RichText::new(addr).monospace().size(st::FONT_SM).color(t.muted)).wrap_mode(egui::TextWrapMode::Wrap));
                    ui.add_space(st::GAP);
                    if st::secondary(ui, "Копировать").on_hover_text(tip).clicked() {
                        ui.ctx().copy_text(addr.to_owned());
                        self.status = "Скопировано".into();
                    }
                });
            });
        });
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let accent = Tab::Settings.color();
        let t = st::cur(ui);
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut changed = false;

        // I. Correction.
        let typo_opts = [
            (TypoLevel::Cautious, level_name(TypoLevel::Cautious), "1 ошибка от 4 букв, 2 от 8, только частые"),
            (TypoLevel::Normal, level_name(TypoLevel::Normal), "1 от 3, 2 от 5, 3 от 9; раскладка + опечатка"),
            (TypoLevel::Aggressive, level_name(TypoLevel::Aggressive), "Смелее при близких вариантах"),
        ];
        let mut typo = self.cfg.typo_level;
        st::group(ui, |ui| {
            changed |= st::toggle_row(ui, &mut self.cfg.autocorrect, "Автоисправление", "Исправляет слова не в той раскладке (зуфсу → peace)", accent);
            st::sep(ui);
            st::line_row(
                ui,
                |ui| {
                    ui.add(egui::Label::new(egui::RichText::new("Смелость").color(t.ink)).truncate())
                        .on_hover_text("Насколько смело исправлять опечатки");
                },
                |ui| {
                    let enabled = self.cfg.autocorrect;
                    let w = (ui.available_width() - st::text_w(ui, "Смелость", st::font_md()) - st::MARGIN).min(st::U * 60.0);
                    ui.add_enabled_ui(enabled, |ui| {
                        if st::segment_strip(ui, &mut typo, &typo_opts, w, accent) {
                            changed = true;
                        }
                    });
                },
            );
        });
        self.cfg.typo_level = typo;

        // II. Keys.
        ui.add_space(st::MARGIN);
        st::group(ui, |ui| {
            self.hotkey_picker(
                ui,
                HotkeySlot::Convert,
                "Конвертация",
                &[(Hotkey::Pause, "Pause"), (Hotkey::ShiftShift, "Shift+Shift"), (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space")],
            );
            st::sep(ui);
            self.hotkey_picker(
                ui,
                HotkeySlot::Summon,
                "Окно",
                &[(Hotkey::CTRL_SHIFT_V, "Ctrl+Shift+V"), (Hotkey::CTRL_SHIFT_K, "Ctrl+Shift+K"), (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space")],
            );
            st::sep(ui);
            changed |= st::toggle_row(
                ui,
                &mut self.cfg.undo_on_backspace,
                "Отмена исправления",
                "Сразу после исправления выбранная клавиша возвращает набранное слово",
                accent,
            );
            st::sep(ui);
            self.hotkey_picker(
                ui,
                HotkeySlot::Undo,
                "Клавиша",
                &[
                    (Hotkey::BACKSPACE, if cfg!(target_os = "macos") { "Delete" } else { "Backspace" }),
                    (Hotkey::CTRL_ALT_Z, if cfg!(target_os = "macos") { "Ctrl+Option+Z" } else { "Ctrl+Alt+Z" }),
                    (Hotkey::CTRL_ALT_BACKSPACE, if cfg!(target_os = "macos") { "Ctrl+Option+Delete" } else { "Ctrl+Alt+Backspace" }),
                    (Hotkey::SHIFT_PAUSE, "Shift+Pause"),
                ],
            );
        });
        let undo = self.cfg.undo_key;
        if self.cfg.hotkey == self.cfg.summon_hotkey || undo == self.cfg.hotkey || undo == self.cfg.summon_hotkey {
            ui.add_space(st::GAP);
            ui.horizontal(|ui| {
                ui.add_space(st::INSET);
                st::warn(ui, "Совпадение сочетаний");
            });
        }

        // III. Look.
        ui.add_space(st::MARGIN);
        st::group(ui, |ui| {
            changed |= st::toggle_row(ui, &mut self.cfg.light_theme, "Светлая тема", "", accent);
            st::sep(ui);
            changed |= self.wallpaper_row(ui);
            #[cfg(windows)]
            {
                st::sep(ui);
                self.admin_section(ui);
            }
        });

        // IV. About.
        ui.add_space(st::MARGIN);
        st::group(ui, |ui| {
            st::line_row(
                ui,
                |ui| {
                    ui.label(egui::RichText::new(APP_NAME).color(t.ink));
                    ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).color(t.muted).size(st::FONT_SM))
                        .on_hover_text(format!("{}", self.path.display()));
                },
                |ui| {
                    if st::link(ui, "Приветствие", accent).clicked() {
                        self.welcome_dont_show = self.cfg.hide_welcome;
                        self.welcome_open = true;
                    }
                },
            );
        });

        if changed {
            self.save();
        }
    }

    /// «Запуск администратором» (Windows): see `platform::win_admin`.
    #[cfg(windows)]
    fn admin_section(&mut self, ui: &mut egui::Ui) {
        use crate::platform::win_admin;
        let accent = Tab::Settings.color();
        let done = self.admin_job.as_ref().and_then(|(on, r)| r.lock().ok().and_then(|mut g| g.take()).map(|r| (*on, r)));
        if let Some((on, result)) = done {
            self.admin_job = None;
            self.daemon_admin = (None, true, None);
            match result {
                Ok(()) => {
                    self.cfg.run_as_admin = on;
                    self.save();
                }
                Err(e) if e == "cancelled" => self.status = "Отменено".into(),
                Err(e) => self.status = format!("Ошибка: {e}"),
            }
        }
        let busy = self.admin_job.is_some();
        let mut want = self.cfg.run_as_admin;
        let toggled = ui
            .add_enabled_ui(!busy, |ui| {
                st::toggle_row(
                    ui,
                    &mut want,
                    "Администратор",
                    "Переключение раскладки в окнах от имени администратора (через Планировщик заданий)",
                    accent,
                )
            })
            .inner;
        if toggled && !busy {
            let result: AdminJobResult = Arc::new(Mutex::new(None));
            let slot = result.clone();
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || {
                let r = win_admin::run_helper_elevated(want);
                if let Ok(mut g) = slot.lock() {
                    *g = Some(r);
                }
                ctx.request_repaint();
            });
            self.admin_job = Some((want, result));
        }
        if self.cfg.run_as_admin && !busy {
            let stale = self.daemon_admin.2.map_or(true, |t| t.elapsed() > Duration::from_secs(5));
            if stale {
                self.daemon_admin = (win_admin::other_instance_elevated(), win_admin::task_points_here(), Some(Instant::now()));
            }
            if !self.daemon_admin.1 {
                let mut refresh = false;
                st::block(ui, |ui| {
                    st::row(
                        ui,
                        egui::Color32::TRANSPARENT,
                        |ui| st::warn(ui, "Задача устарела"),
                        |ui| refresh = st::primary(ui, "Обновить", accent).clicked(),
                    );
                });
                if refresh {
                    let result: AdminJobResult = Arc::new(Mutex::new(None));
                    let slot = result.clone();
                    let ctx = ui.ctx().clone();
                    std::thread::spawn(move || {
                        let r = win_admin::run_helper_elevated(true);
                        if let Ok(mut g) = slot.lock() {
                            *g = Some(r);
                        }
                        ctx.request_repaint();
                    });
                    self.admin_job = Some((true, result));
                }
            } else if self.daemon_admin.0 == Some(false) {
                let mut restart = false;
                st::block(ui, |ui| {
                    st::row(
                        ui,
                        egui::Color32::TRANSPARENT,
                        |ui| st::warn(ui, "Без прав администратора"),
                        |ui| restart = st::primary(ui, "Перезапустить", accent).clicked(),
                    );
                });
                if restart {
                    crate::platform::stop_daemon();
                    if let Err(e) = win_admin::run_task() {
                        self.status = format!("Ошибка: {e}");
                    }
                    self.daemon_admin = (None, true, None);
                }
            }
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        st::apply(ctx, !self.cfg.light_theme, self.tab.color());
    }

    fn apply_logo(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_os = "macos"))]
        if let Ok(icon) = eframe::icon_data::from_png_bytes(crate::logos::icon_png(self.cfg.logo)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(icon))));
        }
        crate::platform::set_dock_icon(crate::logos::icon_png(self.cfg.logo));
        #[cfg(target_os = "macos")]
        let _ = ctx;
        if let Some(tray) = &self.tray {
            tray.set_logo(crate::logos::index(self.cfg.logo));
        }
    }

    fn sea_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        let i = if self.cfg.light_theme { 0 } else { 1 };
        self.sea_textures[i]
            .get_or_insert_with(|| st::load_sea_texture(ctx, !self.cfg.light_theme))
            .clone()
    }

    fn logo_texture(&mut self, ctx: &egui::Context, style: LogoStyle) -> egui::TextureHandle {
        let i = crate::logos::index(style) as usize;
        self.logo_textures[i]
            .get_or_insert_with(|| crate::logos::texture(ctx, style))
            .clone()
    }

    #[allow(dead_code)]
    fn faded_logo_texture(&mut self, ctx: &egui::Context, style: LogoStyle) -> egui::TextureHandle {
        let i = crate::logos::index(style) as usize;
        self.logo_fade_textures[i]
            .get_or_insert_with(|| crate::logos::texture_faded(ctx, style))
            .clone()
    }

    fn go(&mut self, tab: Tab) {
        if self.tab != tab {
            self.status.clear();
            self.tab = tab;
            if tab != Tab::Home {
                self.grow_for_section = true;
            }
        }
    }

    fn section_tooltip(&self, tab: Tab) -> String {
        match tab {
            Tab::Home => String::new(),
            Tab::Apps => format!("Программы: {} · Исключения: {}", self.cfg.apps.len(), self.cfg.exception_apps.len()),
            Tab::Words => format!("Слова: {}", self.cfg.custom_words.len()),
            Tab::Vault => format!("Сейф: {}", self.vault.state_text()),
            Tab::Settings => {
                if !self.cfg.autocorrect {
                    "Автоисправление выключено".into()
                } else {
                    format!("Смелость: {}", level_name(self.cfg.typo_level))
                }
            }
            Tab::Donate => "Boosty, USDT, GRAM".into(),
        }
    }

    /// The live number a section stands for (none = draw its glyph).
    fn section_count(&self, tab: Tab) -> Option<usize> {
        match tab {
            Tab::Apps => Some(self.cfg.apps.len()),
            Tab::Words => Some(self.cfg.custom_words.len()),
            Tab::Vault => self.vault.entry_count(),
            _ => None,
        }
    }

    /// Home: five identical neutral cards in one row. Each card holds one coloured
    /// circle with the section glyph; the live number sits small in the top-right
    /// corner; the caption is centred under the card.
    fn home(&mut self, ui: &mut egui::Ui) {
        let t = st::cur(ui);
        let full = ui.available_width();
        let g = st::PAD;
        let s = st::snap((full - 4.0 * g) / 5.0, st::HIT);
        let row_w = 5.0 * s + 4.0 * g;
        let caption_font = st::font_sm();
        let cap_h = st::MARGIN + st::GAP;
        let block_h = s + st::PAD + cap_h;
        let x_pad = st::snap(((full - row_w) * 0.5).max(0.0), 0.0);
        let top_pad = st::snap(((ui.available_height() - block_h) * 0.5).max(0.0), 0.0);
        let origin = ui.cursor().min + egui::vec2(x_pad, top_pad);
        let mut clicked = None;
        let d = st::snap(s * 0.44, st::U * 6.0).min(st::U * 12.0);
        for (i, &tab) in SECTIONS.iter().enumerate() {
            let x = origin.x + i as f32 * (s + g);
            let rect = egui::Rect::from_min_size(egui::pos2(x, origin.y), egui::vec2(s, s));
            let hit = egui::Rect::from_min_size(rect.min, egui::vec2(s, block_h));
            let resp = ui.interact(hit, ui.id().with(("card", i)), egui::Sense::click());
            let color = tab.color();
            let p = ui.painter();
            let down = resp.is_pointer_button_down_on();
            let fill = if down { t.surface_hover } else if resp.hovered() { st::mix(t.surface, t.surface_hover, 0.6) } else { t.surface };
            let edge = if resp.hovered() { st::mix(t.line, t.muted, 0.45) } else { t.line };
            let r = if down { rect.shrink(1.0) } else { rect };
            st::card_shadow(ui, r);
            p.rect_filled(r, st::RADIUS, fill);
            p.rect_stroke(r.shrink(0.5), st::RADIUS, st::hair(edge));
            st::focus_ring(ui, rect, st::RADIUS, resp.has_focus());
            let c = r.center();
            st::paint_mark(p, c, if down { d - 2.0 } else { d }, color, i);
            if let Some(n) = self.section_count(tab) {
                let ng = st::line_galley(ui, &n.to_string(), caption_font.clone(), t.muted, s / 2.0);
                p.galley(egui::pos2(r.right() - st::PAD - ng.size().x, r.top() + st::PAD - 2.0), ng, t.muted);
            } else if tab == Tab::Settings && !self.cfg.autocorrect {
                // Micro-state: a hollow dot = Автоисправление off.
                p.circle_stroke(egui::pos2(r.right() - st::PAD - 3.0, r.top() + st::PAD + 3.0), 3.0, st::hair(t.muted));
            }
            let lg = st::line_galley(ui, tab.title(), caption_font.clone(), t.ink, s + g - st::GAP);
            let lp = egui::pos2(rect.center().x - lg.size().x / 2.0, rect.bottom() + st::PAD);
            st::caption_plate(ui, lp, lg.size());
            p.galley(lp, lg, t.ink);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let resp = resp.on_hover_text(self.section_tooltip(tab));
            if resp.clicked() {
                clicked = Some(tab);
            }
        }
        ui.allocate_rect(
            egui::Rect::from_min_max(origin - egui::vec2(x_pad, top_pad), egui::pos2(origin.x - x_pad + full, origin.y + block_h)),
            egui::Sense::hover(),
        );
        if let Some(tab) = clicked {
            self.go(tab);
        }
    }

    /// Section header, 48 px: back chevron · colour dot · title · count, and on the
    /// right five small dots to jump between sections (the current one ringed).
    fn header(&mut self, ui: &mut egui::Ui) {
        let tab = self.tab;
        let t = st::cur(ui);
        let rect = ui.max_rect();
        // A thin crop of the same photograph (the water, aligned to the top).
        let sea = self.sea_texture(ui.ctx());
        st::paint_photo(ui, rect, &sea, 0.0, egui::Color32::WHITE);
        let p = ui.painter().clone();
        let mut target = None;
        let cy = rect.center().y;

        // Navigation, right: 5 × 24 px targets with 10 px dots.
        let cell = st::U * 6.0;
        let nav_w = cell * SECTIONS.len() as f32;
        let nav_left = rect.right() - st::MARGIN - nav_w + st::GAP;
        let nav_plate = egui::Rect::from_x_y_ranges(
            (nav_left - st::GAP)..=(rect.right() - st::PAD),
            (cy - st::HIT / 2.0)..=(cy + st::HIT / 2.0),
        );
        st::frost_plate(ui, nav_plate);

        let back = egui::Rect::from_center_size(egui::pos2(rect.left() + st::PAD + st::HIT / 2.0, cy), egui::Vec2::splat(st::HIT));
        st::frost_plate(ui, back.shrink(1.0));
        let r = ui.interact(back, ui.id().with("back"), egui::Sense::click());
        if r.hovered() {
            p.rect_filled(back.shrink(2.0), st::RADIUS_SM, t.surface_hover);
        }
        st::paint_chevron(&p, back.center(), if r.hovered() { t.ink } else { t.muted });
        st::focus_ring(ui, back.shrink(2.0), st::RADIUS_SM, r.has_focus());
        if r.on_hover_text("Назад (Esc)").clicked() {
            target = Some(Tab::Home);
        }

        for (i, &other) in SECTIONS.iter().enumerate() {
            let c = egui::Rect::from_center_size(egui::pos2(nav_left + cell * (i as f32 + 0.5), cy), egui::vec2(cell, st::HIT));
            let resp = ui.interact(c, ui.id().with(("nav", i)), egui::Sense::click());
            let center = c.center();
            if other == tab {
                p.circle_stroke(center, 7.5, egui::Stroke::new(1.5, st::mix(t.ink, t.bg, 0.2)));
            }
            let rad = if resp.hovered() && other != tab { 5.5 } else { 4.5 };
            p.circle_filled(center, rad, other.color());
            p.circle_stroke(center, rad - 0.25, st::hair(st::shade(other.color(), -0.15)));
            st::focus_ring(ui, egui::Rect::from_center_size(center, egui::Vec2::splat(16.0)), 8.0, resp.has_focus());
            if resp.on_hover_text(other.title()).clicked() && other != tab {
                target = Some(other);
            }
        }

        // Title block: dot · title · count, truncated before the navigation.
        let dot_c = egui::pos2(back.right() + st::GAP + 6.0, cy);
        let tx = dot_c.x + 6.0 + st::PAD;
        let max_w = (nav_left - st::PAD - tx).max(st::HIT);
        let title_font = egui::FontId::proportional(st::FONT_LG);
        let g = st::line_galley(ui, tab.title(), title_font, t.ink, max_w);
        let tw = g.size().x;
        let th = g.size().y;
        let mut title_right = tx + tw;
        let count_galley = self.section_count(tab).and_then(|n| {
            let ng = st::line_galley(ui, &n.to_string(), st::font_md(), t.muted, (max_w - tw - st::PAD).max(0.0));
            if tw + st::PAD + ng.size().x <= max_w {
                title_right = tx + tw + st::PAD + ng.size().x;
                Some(ng)
            } else {
                None
            }
        });
        let title_plate = egui::Rect::from_min_max(
            egui::pos2(dot_c.x - 10.0, cy - st::HIT / 2.0),
            egui::pos2(title_right + st::PAD, cy + st::HIT / 2.0),
        );
        st::frost_plate(ui, title_plate);
        p.circle_filled(dot_c, 6.0, tab.color());
        p.circle_stroke(dot_c, 5.75, st::hair(st::shade(tab.color(), -0.15)));
        p.galley(egui::pos2(tx, cy - th / 2.0), g, t.ink);
        if let Some(ng) = count_galley {
            p.galley(egui::pos2(tx + tw + st::PAD, cy - ng.size().y / 2.0 + 1.0), ng, t.muted);
        }
        let title_rect = egui::Rect::from_min_max(egui::pos2(dot_c.x - 6.0, rect.top()), egui::pos2(title_right, rect.bottom()));
        ui.interact(title_rect, ui.id().with("title"), egui::Sense::hover()).on_hover_text(self.section_tooltip(tab));

        // Bottom hairline.
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.bottom() - st::HAIR), egui::vec2(rect.width(), st::HAIR)), 0.0, t.line);
        if let Some(t) = target {
            self.go(t);
        }
    }

    /// «Логотип»: five marks to pick for the window, tray and welcome picture.
    fn wallpaper_row(&mut self, ui: &mut egui::Ui) -> bool {
        let t = st::cur(ui);
        let mut changed = false;
        st::line_row(
            ui,
            |ui| {
                ui.label(egui::RichText::new("Логотип").color(t.ink));
            },
            |ui| {
                ui.spacing_mut().item_spacing.x = st::PAD;
                // Actions sit in a right-to-left row, so walk ALL backwards
                // and the default (beach) lands on the left.
                for &style in LogoStyle::ALL.iter().rev() {
                    if self.wallpaper_tile(ui, style, st::U * 7.0) {
                        changed = true;
                    }
                }
            },
        );
        changed
    }

    fn wallpaper_tile(&mut self, ui: &mut egui::Ui, style: LogoStyle, side: f32) -> bool {
        let t = st::cur(ui);
        let tex = self.logo_texture(ui.ctx(), style);
        let (cell, resp) = ui.allocate_exact_size(egui::vec2(side.max(st::HIT), side.max(st::HIT)), egui::Sense::click());
        let rect = egui::Rect::from_center_size(cell.center(), egui::Vec2::splat(side));
        let selected = self.cfg.logo == style;
        egui::Image::new(&tex).maintain_aspect_ratio(false).rounding(st::RADIUS_SM).paint_at(ui, rect);
        ui.painter().rect_stroke(rect.shrink(0.5), st::RADIUS_SM, st::hair(t.line));
        if selected {
            ui.painter().rect_stroke(rect.expand(2.5), st::RADIUS_SM + 2.5, egui::Stroke::new(2.0, Tab::Settings.color()));
        } else if resp.hovered() {
            ui.painter().rect_stroke(rect.expand(2.0), st::RADIUS_SM + 2.0, st::hair(t.muted));
        }
        st::focus_ring(ui, rect.expand(3.0), st::RADIUS_SM + 3.0, resp.has_focus());
        let resp = resp.on_hover_text(crate::logos::label(style));
        if resp.clicked() && !selected {
            self.cfg.logo = style;
            self.apply_logo(ui.ctx());
            return true;
        }
        false
    }

    /// Welcome: the name, the chosen logo as a large rounded picture, the
    /// five marks to pick in a group, and a footer.
    fn welcome_panel(&mut self, ui: &mut egui::Ui) {
        let t = st::cur(ui);
        let full = ui.max_rect();
        let m = st::MARGIN;
        let footer = m + st::ROW + st::HAIR * 2.0 + m + st::HIT + m;
        let art = st::snap((full.width() * 0.5).min(full.height() - m - footer), st::HIT * 3.0);
        let art_rect = egui::Rect::from_min_size(egui::pos2(full.right() - m - art, full.top() + m), egui::Vec2::splat(art));
        let tex = self.logo_texture(ui.ctx(), self.cfg.logo);
        egui::Image::new(&tex).maintain_aspect_ratio(false).rounding(st::RADIUS).paint_at(ui, art_rect);
        ui.painter().rect_stroke(art_rect.shrink(0.5), st::RADIUS, st::hair(t.line));

        let left = full.left() + m;
        let col_w = (art_rect.left() - m - left).max(st::HIT * 3.0);
        let p = ui.painter().clone();
        let dot = st::U * 3.0;
        for (i, tab) in SECTIONS.iter().enumerate() {
            let c = egui::pos2(left + dot / 2.0 + i as f32 * (dot + st::GAP * 1.5), full.top() + m + dot / 2.0 + st::GAP);
            p.circle_filled(c, dot / 2.0, tab.color());
        }
        let name_top = full.top() + m + dot + st::MARGIN;
        let g = st::line_galley(ui, APP_NAME, egui::FontId::proportional(st::FONT_XXL * 1.44), t.ink, col_w);
        let name_h = g.size().y;
        p.galley(egui::pos2(left - st::U * 0.5, name_top), g, t.ink);
        let sub = st::line_galley(ui, "Раскладка, опечатки, сейф", st::font_md(), t.muted, col_w);
        let sub_h = sub.size().y;
        p.galley(egui::pos2(left, name_top + name_h + st::GAP), sub, t.muted);
        let vg = st::line_galley(ui, env!("CARGO_PKG_VERSION"), st::font_sm(), t.muted, col_w);
        p.galley(egui::pos2(left, name_top + name_h + st::GAP * 2.0 + sub_h), vg, t.muted);

        let row_top = art_rect.bottom() + m;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(egui::pos2(left, row_top), egui::pos2(full.right() - m, full.bottom()))));
        child.spacing_mut().item_spacing = egui::vec2(st::PAD, 0.0);
        let mut changed = false;
        st::group(&mut child, |ui| {
            changed |= self.wallpaper_row(ui);
        });
        child.add_space(m);
        let mut next = false;
        child.horizontal(|ui| {
            ui.set_min_height(st::HIT);
            st::check(ui, &mut self.welcome_dont_show, "Не показывать", st::SEA);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                next = st::primary(ui, "Далее", st::SEA).clicked();
            });
        });
        let used = child.min_rect();
        ui.allocate_rect(egui::Rect::from_min_max(full.min, egui::pos2(full.right(), used.bottom() + m)), egui::Sense::hover());
        if changed {
            self.save();
        }
        if next {
            self.welcome_open = false;
            self.cfg.welcome_gen = layout_keeper_core::config::CURRENT_WELCOME_GEN;
            self.cfg.hide_welcome = self.welcome_dont_show;
            self.save();
        }
    }

    fn draw(&mut self, ctx: &egui::Context, focus: &Focus) {
        self.apply_theme(ctx);
        if self.grow_for_section {
            let rect = ctx.input(|i| i.screen_rect());
            let w = rect.width().max(SECTION_INNER[0]);
            let h = rect.height().max(SECTION_INNER[1]);
            if h > rect.height() + 0.5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            }
            self.grow_for_section = false;
        }
        let t = st::theme(!self.cfg.light_theme);
        if self.welcome_open {
            let lt = st::theme(false);
            st::apply(ctx, false, st::SEA);
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(lt.bg).inner_margin(egui::Margin::ZERO))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.set_min_height(ui.available_height());
                        self.welcome_panel(ui);
                    });
                });
            return;
        }
        if self.tab != Tab::Home {
            egui::TopBottomPanel::top("header")
                .exact_height(st::HEADER)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::none().fill(t.bg))
                .show(ctx, |ui| self.header(ui));
        }
        if !self.status.is_empty() {
            egui::TopBottomPanel::bottom("status")
                .frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(st::MARGIN, st::PAD)))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    let r = ui.max_rect();
                    ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(r.left(), r.top() - st::PAD), egui::vec2(r.width(), st::HAIR)), 0.0, t.line);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = st::PAD;
                        let (sq, _) = ui.allocate_exact_size(egui::vec2(st::PAD, st::PAD), egui::Sense::hover());
                        ui.painter().circle_filled(sq.center(), st::GAP, self.tab.color());
                        ui.add(egui::Label::new(egui::RichText::new(&self.status).small().color(t.ink)).truncate());
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
                if self.tab == Tab::Home {
                    let sea = self.sea_texture(ctx);
                    st::paint_photo(ui, ui.max_rect(), &sea, 0.0, egui::Color32::WHITE);
                }
                let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let margin = if self.tab == Tab::Home {
                        egui::Margin::same(st::PAD)
                    } else {
                        egui::Margin { left: st::MARGIN, right: st::MARGIN, top: st::INSET, bottom: st::MARGIN }
                    };
                    egui::Frame::none().inner_margin(margin).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if self.tab == Tab::Home {
                            ui.set_min_height((ui.available_height() - st::PAD * 2.0).max(60.0));
                        }
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        if !platform::accessibility_trusted() {
                            st::block(ui, |ui| {
                                st::row(
                                    ui,
                                    egui::Color32::TRANSPARENT,
                                    |ui| st::warn(ui, "Нет разрешения «Универсальный доступ»"),
                                    |ui| {
                                        if st::primary(ui, "Открыть", self.tab.color()).clicked() {
                                            platform::open_accessibility_settings();
                                        }
                                    },
                                );
                            });
                            ui.add_space(st::GAP);
                        }
                        match self.tab {
                            Tab::Home => self.home(ui),
                            Tab::Apps => self.apps_tab(ui),
                            Tab::Words => self.words_tab(ui),
                            Tab::Vault => {
                                if self.vault.show(ui, focus.app.clone(), &mut self.cfg.vault_idle_minutes) {
                                    self.save();
                                }
                            }
                            Tab::Settings => self.settings_tab(ui),
                            Tab::Donate => self.donate_tab(ui),
                        }
                    });
                });
                self.content_overflow = (scroll.content_size.x - scroll.inner_rect.width()).max(0.0);
            });
    }
}

const SECTIONS: [Tab; 5] = [Tab::Apps, Tab::Words, Tab::Vault, Tab::Settings, Tab::Donate];

impl Tab {
    fn color(self) -> egui::Color32 {
        match self {
            Tab::Home => st::SEA,
            Tab::Apps => st::SUN,
            Tab::Words => st::SAND,
            Tab::Vault => st::SEAFOAM,
            Tab::Settings => st::SEA,
            Tab::Donate => st::CORAL,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Home => APP_NAME,
            Tab::Apps => "Программы",
            Tab::Words => "Словарь",
            Tab::Vault => "Сейф",
            Tab::Settings => "Настройки",
            Tab::Donate => "Поддержать",
        }
    }
}

fn level_name(l: TypoLevel) -> &'static str {
    match l {
        TypoLevel::Cautious => "Осторожно",
        TypoLevel::Normal => "Обычно",
        TypoLevel::Aggressive => "Смело",
    }
}



fn lang_name(l: Lang) -> &'static str {
    match l {
        Lang::En => "EN",
        Lang::Ru => "RU",
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.vault.lock();
        // Windows (since 0.3.69): the settings process exits on every close / minimize
        // and the daemon (with the tray icon) keeps running; the tray's «Выход» stops both.
        // macOS / Linux: the window is the app (macOS) or there is no tray (Linux), so
        // quitting it stops the key handling too, as before.
        #[cfg(not(windows))]
        crate::platform::stop_daemon();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.maybe_reload();
        if !self.chrome_ready {
            crate::platform::install_dock_reopen_handler();
            crate::platform::ensure_login_item();
            self.apply_logo(ctx);
            self.chrome_ready = true;
        }
        if let Some(tray) = &self.tray {
            tray.set_repaint(ctx.clone());
            while let Some(cmd) = tray.try_recv() {
                match cmd {
                    crate::tray::TrayCmd::Show => {
                        show_window(ctx);
                        self.ui_visible = true;
                    }
                    crate::tray::TrayCmd::ShowVault => {
                        self.tab = Tab::Vault;
                        self.grow_for_section = true;
                        show_window(ctx);
                        self.ui_visible = true;
                    }
                    crate::tray::TrayCmd::Quit => {
                        crate::platform::stop_daemon();
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        let can_hide = self.tray.as_ref().is_some_and(|t| t.is_active()) && !self.allow_close;
        if can_hide && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            hide_window(ctx);
            self.ui_visible = false;
            self.vault.lock();
        }
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        // Windows: minimizing quits the settings process like the close button does
        // (the daemon's tray icon / summon hotkey start it again).
        if cfg!(windows) && minimized && !self.allow_close {
            self.allow_close = true;
            self.vault.lock();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Only on the transition: a hidden window can keep reporting `minimized`, and
        // re-sending the hide commands every frame woke eframe again and again (≈6% CPU
        // while minimized, 0% while open).
        if can_hide && minimized && self.ui_visible {
            hide_window(ctx);
            self.ui_visible = false;
            self.vault.lock();
        }
        let want_vault = crate::platform::take_open_vault_request();
        let want_show = crate::platform::take_show_gui_request();
        if want_vault {
            self.go(Tab::Vault);
        }
        if want_vault || want_show {
            show_window(ctx);
            self.ui_visible = true;
        }
        if !self.ui_visible || (minimized && !(want_vault || want_show)) {
            // Minimized to the taskbar (no tray) is idle too: nothing to paint, so no
            // timed repaint; restoring the window sends its own events.
            // Hidden to the tray: do not paint or resize. A 200ms repaint here was
            // leaking gigabytes of GPU memory on Windows while the window was invisible.
            // Something can still wake eframe in a loop while the window is hidden or
            // minimized (Windows keeps sending events; eframe paints on each). Whatever the
            // source, cap idle frames at 4 per second so it stays near 0% CPU / GPU.
            let since = self.last_idle_frame.elapsed();
            if since < IDLE_FRAME {
                std::thread::sleep(IDLE_FRAME - since);
            }
            self.last_idle_frame = Instant::now();
            self.idle_frames += 1;
            if self.idle_frames % 240 == 0 {
                log::debug!("hidden GUI woke {} times", self.idle_frames);
            }
            // An unlocked vault still needs its idle lock: one cheap wake a minute.
            self.vault.tick(ctx, self.cfg.vault_idle_minutes);
            if self.vault.is_unlocked() {
                ctx.request_repaint_after(Duration::from_secs(60));
            }
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(200));
        // Esc in a section goes back to the squares (not while assigning a hotkey).
        let capturing = self.capturing_hotkey.is_some();
        if let Some(slot) = self.capturing_hotkey {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.capturing_hotkey = None;
                self.status.clear();
            } else if let Some(hk) = read_captured_hotkey(ctx) {
                match slot {
                    HotkeySlot::Convert => self.cfg.hotkey = hk,
                    HotkeySlot::Summon => self.cfg.summon_hotkey = hk,
                    HotkeySlot::Undo => {
                        self.cfg.undo_key = hk;
                        self.cfg.undo_hotkey = hk;
                    }
                }
                self.capturing_hotkey = None;
                self.save();
            }
        }
        if !capturing && !self.welcome_open && self.tab != Tab::Home && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.go(Tab::Home);
        }
        let focus = self.focus.lock().map(|f| f.clone()).unwrap_or_default();
        self.vault.tick(ctx, self.cfg.vault_idle_minutes);
        self.draw(ctx, &focus);
    }
}

pub fn run(open_vault: bool, start_hidden: bool) -> Result<(), String> {
    // Windows (since 0.3.69): a hidden settings window is never kept — the tray lives in
    // the daemon. `gui --hidden` (older shortcuts) only makes sure the daemon runs.
    #[cfg(windows)]
    if start_hidden {
        return platform::ensure_daemon_running();
    }
    #[cfg(windows)]
    let _gui_mutex = {
        let mut guard = None;
        // A settings process that is just exiting (window closed a moment ago) still holds
        // the lock for a few milliseconds: wait for it instead of asking it to show itself.
        for attempt in 0..20 {
            match platform::acquire_gui_mutex() {
                Ok(Some(g)) => {
                    guard = Some(g);
                    break;
                }
                Ok(None) => {
                    if platform::raise_settings_window() || attempt == 19 {
                        if open_vault {
                            platform::request_open_vault();
                        } else {
                            platform::request_show_gui();
                        }
                        platform::raise_settings_window();
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    log::warn!("cannot take GUI single-instance lock: {e}");
                    break;
                }
            }
        }
        guard
    };
    // A fresh settings process answers its own launch: stale requests are not needed.
    #[cfg(windows)]
    {
        let _ = platform::take_show_gui_request();
        let _ = platform::take_open_vault_request();
    }
    #[cfg(target_os = "macos")]
    let _gui_lock = match platform::acquire_gui_lock() {
        Ok(None) => {
            if open_vault {
                platform::request_open_vault();
            } else {
                platform::request_show_gui();
            }
            return Ok(());
        }
        Ok(Some(guard)) => Some(guard),
        Err(e) => {
            log::warn!("cannot take GUI single-instance lock: {e}");
            None
        }
    };
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_millis(250));
        if platform::gui_wake_pending() {
            crate::tray::wake_gui();
        }
    });
    let daemon_status = match platform::ensure_daemon_running() {
        Ok(()) => String::new(),
        Err(e) => {
            log::warn!("cannot start background daemon: {e}");
            "Не удалось запустить демон. Запустите typed.exe без параметров.".into()
        }
    };
    let path = Config::default_path();
    let cfg = Config::load_or_create(&path).map_err(|e| format!("cannot load config {}: {e}", path.display()))?;
    let focus = Arc::new(Mutex::new(Focus::default()));
    {
        // Poll the focused window off the UI thread; skip our own window so the
        // "assign current app" buttons target the app the user came from.
        let focus = focus.clone();
        std::thread::spawn(move || loop {
            let (app, layout) = platform::probe();
            if let Ok(mut f) = focus.lock() {
                match app {
                    Some(a) if !is_self(&a) => {
                        f.app = Some(a);
                        f.layout = layout;
                    }
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        });
    }
    let welcome_open = !cfg.hide_welcome || cfg.welcome_gen < layout_keeper_core::config::CURRENT_WELCOME_GEN;
    let logo_index = crate::logos::index(cfg.logo);
    let app = App {
        mtime: mtime(&path),
        path,
        cfg,
        last_check: Instant::now(),
        focus,
        status: daemon_status,
        qr: Default::default(),
        app_draft: String::new(),
        app_mode: AppMode::Ru,
        apps_pane: AppsPane::Programs,
        tab: if open_vault { Tab::Vault } else { Tab::Home },
        vault: crate::vault_ui::VaultUi::new(),
        capturing_hotkey: None,
        word_draft: String::new(),
        icons: IconCache::default(),
        grow_for_section: false,
        content_overflow: 0.0,
        welcome_open,
        welcome_dont_show: false,
        logo_textures: [None, None, None, None, None],
        logo_fade_textures: [None, None, None],
        sea_textures: [None, None],
        admin_job: None,
        daemon_admin: (None, true, None),
        // Windows: the tray icon belongs to the daemon (see `tray`).
        tray: if cfg!(windows) { None } else { crate::tray::start(logo_index) },
        allow_close: false,
        ui_visible: !start_hidden,
        chrome_ready: false,
        last_idle_frame: Instant::now(),
        idle_frames: 0,
    };
    let viewport = {
        let mut vp = egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(HOME_INNER)
            .with_min_inner_size([432.0, 160.0])
            .with_resizable(true);
        if start_hidden {
            vp = vp.with_visible(false);
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(icon) = eframe::icon_data::from_png_bytes(crate::logos::icon_png(app.cfg.logo)) {
                vp.with_icon(std::sync::Arc::new(icon))
            } else {
                vp
            }
        }
        #[cfg(target_os = "macos")]
        {
            // Empty IconData = "use the OS default". If we omit the icon entirely,
            // eframe injects its own white-e-on-black mark and overwrites Typed.icns.
            vp.with_icon(egui::IconData::default())
        }
    };
    let opts = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        opts,
        Box::new(|cc| {
            st::install_fonts(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())
}


fn read_captured_hotkey(ctx: &egui::Context) -> Option<Hotkey> {
    ctx.input(|i| {
        for ev in &i.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = ev
            {
                if *key == egui::Key::Escape {
                    continue;
                }
                if let Some(hk) = hotkey_from_egui(*key, *modifiers) {
                    return Some(hk);
                }
            }
        }
        None
    })
}

fn hotkey_from_egui(key: egui::Key, m: egui::Modifiers) -> Option<Hotkey> {
    use egui::Key;
    let code = match key {
        Key::Space => HotkeyCode::Space,
        Key::Tab => HotkeyCode::Tab,
        Key::Enter => HotkeyCode::Enter,
        Key::Backspace => HotkeyCode::Backspace,
        Key::F1 => HotkeyCode::F(1),
        Key::F2 => HotkeyCode::F(2),
        Key::F3 => HotkeyCode::F(3),
        Key::F4 => HotkeyCode::F(4),
        Key::F5 => HotkeyCode::F(5),
        Key::F6 => HotkeyCode::F(6),
        Key::F7 => HotkeyCode::F(7),
        Key::F8 => HotkeyCode::F(8),
        Key::F9 => HotkeyCode::F(9),
        Key::F10 => HotkeyCode::F(10),
        Key::F11 => HotkeyCode::F(11),
        Key::F12 => HotkeyCode::F(12),
        Key::A => HotkeyCode::Letter('A'),
        Key::B => HotkeyCode::Letter('B'),
        Key::C => HotkeyCode::Letter('C'),
        Key::D => HotkeyCode::Letter('D'),
        Key::E => HotkeyCode::Letter('E'),
        Key::F => HotkeyCode::Letter('F'),
        Key::G => HotkeyCode::Letter('G'),
        Key::H => HotkeyCode::Letter('H'),
        Key::I => HotkeyCode::Letter('I'),
        Key::J => HotkeyCode::Letter('J'),
        Key::K => HotkeyCode::Letter('K'),
        Key::L => HotkeyCode::Letter('L'),
        Key::M => HotkeyCode::Letter('M'),
        Key::N => HotkeyCode::Letter('N'),
        Key::O => HotkeyCode::Letter('O'),
        Key::P => HotkeyCode::Letter('P'),
        Key::Q => HotkeyCode::Letter('Q'),
        Key::R => HotkeyCode::Letter('R'),
        Key::S => HotkeyCode::Letter('S'),
        Key::T => HotkeyCode::Letter('T'),
        Key::U => HotkeyCode::Letter('U'),
        Key::V => HotkeyCode::Letter('V'),
        Key::W => HotkeyCode::Letter('W'),
        Key::X => HotkeyCode::Letter('X'),
        Key::Y => HotkeyCode::Letter('Y'),
        Key::Z => HotkeyCode::Letter('Z'),
        Key::Num0 => HotkeyCode::Digit(0),
        Key::Num1 => HotkeyCode::Digit(1),
        Key::Num2 => HotkeyCode::Digit(2),
        Key::Num3 => HotkeyCode::Digit(3),
        Key::Num4 => HotkeyCode::Digit(4),
        Key::Num5 => HotkeyCode::Digit(5),
        Key::Num6 => HotkeyCode::Digit(6),
        Key::Num7 => HotkeyCode::Digit(7),
        Key::Num8 => HotkeyCode::Digit(8),
        Key::Num9 => HotkeyCode::Digit(9),
        _ => return None,
    };
    let ctrl = if cfg!(target_os = "macos") {
        m.ctrl
    } else {
        m.ctrl || m.command
    };
    let meta = m.mac_cmd || (cfg!(target_os = "macos") && m.command);
    let hk = Hotkey {
        ctrl,
        alt: m.alt,
        shift: m.shift,
        meta,
        key: code,
    };
    hk.is_safe().then_some(hk)
}

/// Black-on-white QR code with a 2-module quiet zone, one pixel per module.
fn qr_image(text: &str) -> egui::ColorImage {
    use qrcodegen::{QrCode, QrCodeEcc};
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).expect("address fits in a QR code");
    let border = 2;
    let n = qr.size() + border * 2;
    let mut px = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        for x in 0..n {
            let dark = qr.get_module(x - border, y - border);
            px.push(if dark { egui::Color32::BLACK } else { egui::Color32::WHITE });
        }
    }
    egui::ColorImage { size: [n as usize, n as usize], pixels: px }
}

#[cfg(test)]
#[path = "gui_shots.rs"]
mod shots;

#[cfg(test)]
mod qr_tests {
    #[test]
    fn donation_addresses_make_qr_codes() {
        for a in [super::USDT_TRC20, super::GRAM_TON] {
            let img = super::qr_image(a);
            assert!(img.size[0] >= 25 && img.size[0] == img.size[1]);
        }
    }
}
