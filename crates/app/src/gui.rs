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
use std::collections::HashSet;
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

const SETTINGS_GROUPS: &[&str] = &["fix", "typo", "undo", "keys", "admin", "look", "about"];

fn default_open_groups() -> HashSet<&'static str> {
    SETTINGS_GROUPS.iter().copied().collect()
}

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
    tab: Tab,
    vault: crate::vault_ui::VaultUi,
    capturing_hotkey: Option<HotkeySlot>,
    word_draft: String,
    icons: IconCache,
    /// Settings groups opened in this session (all open by default; the Settings
    /// section itself starts behind the home square).
    open_groups: HashSet<&'static str>,
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
    logo_textures: [Option<egui::TextureHandle>; 3],
    logo_fade_textures: [Option<egui::TextureHandle>; 3],
    /// «Запуск администратором» being switched: (wanted value, helper result when done).
    #[cfg_attr(not(windows), allow(dead_code))]
    admin_job: Option<(bool, AdminJobResult)>,
    /// Last check whether the daemon runs as administrator, and when it was made.
    #[cfg_attr(not(windows), allow(dead_code))]
    daemon_admin: (Option<bool>, bool, Option<Instant>),
}

type AdminJobResult = Arc<Mutex<Option<Result<(), String>>>>;

#[derive(PartialEq, Clone, Copy)]
enum HotkeySlot {
    Convert,
    Summon,
    Undo,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum AppMode {
    En,
    Ru,
    Off,
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
        self.set_app_mode(&entry, self.app_mode);
        self.app_draft.clear();
        true
    }

    fn set_app_mode(&mut self, id: &str, mode: AppMode) {
        let key_lower = id.to_lowercase();
        match mode {
            AppMode::En => {
                self.cfg.exception_apps.retain(|e| e.to_lowercase() != key_lower);
                self.cfg.set_layout(id, Lang::En);
            }
            AppMode::Ru => {
                self.cfg.exception_apps.retain(|e| e.to_lowercase() != key_lower);
                self.cfg.set_layout(id, Lang::Ru);
            }
            AppMode::Off => {
                self.cfg.remove_app(id);
                if !self.cfg.exception_apps.iter().any(|e| e.to_lowercase() == key_lower) {
                    self.cfg.exception_apps.push(id.to_string());
                }
            }
        }
    }

    fn remove_app_entry(&mut self, id: &str) {
        let key_lower = id.to_lowercase();
        self.cfg.remove_app(id);
        self.cfg.exception_apps.retain(|e| e.to_lowercase() != key_lower);
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

        // Add row: [ program / path ] [EN|RU|∅] [+] — all 32 px, gutters of 8.
        st::block(ui, |ui| {
            let mut go = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = st::GAP;
                let seg_w = st::HIT * 3.0;
                let fw = (ui.available_width() - seg_w - st::HIT - st::GAP * 2.0).max(st::HIT * 2.0);
                let r = st::field_bg(ui, &mut self.app_draft, "программа / путь", fw, accent);
                go |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                go |= st::segment_strip(
                    ui,
                    &mut self.app_mode,
                    &[
                        (AppMode::En, "EN", "Английская раскладка"),
                        (AppMode::Ru, "RU", "Русская раскладка"),
                        (AppMode::Off, "∅", "Выкл — исключить из автоисправления"),
                    ],
                    seg_w,
                    accent,
                );
                go |= st::plus_square(ui, accent, "Добавить").clicked();
            });
            if go {
                changed |= self.add_app_entry();
            }
        });

        // Merged per-app list: apps (EN / RU) first, then exception_apps (Выкл).
        let mut entries: Vec<(String, AppMode)> = self
            .cfg
            .apps
            .iter()
            .map(|(k, l)| {
                let m = match l {
                    Lang::En => AppMode::En,
                    Lang::Ru => AppMode::Ru,
                };
                (k.clone(), m)
            })
            .collect();
        entries.sort_by(|a, b| basename(&a.0).to_lowercase().cmp(&basename(&b.0).to_lowercase()).then_with(|| a.0.cmp(&b.0)));

        for item in &self.cfg.exception_apps {
            let lower = item.to_lowercase();
            if !entries.iter().any(|(k, _)| k.to_lowercase() == lower) {
                entries.push((item.clone(), AppMode::Off));
            }
        }

        let mut act: Option<(String, Option<AppMode>)> = None;
        for (id, cur) in entries {
            let icons = &mut self.icons;
            let mut mode = cur;
            st::row(
                ui,
                t.surface,
                |ui| {
                    icons.show(ui, &id);
                    ui.add(egui::Label::new(egui::RichText::new(&id).monospace().color(t.ink)).truncate())
                        .on_hover_text(&id);
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
                            (AppMode::Off, "∅", "Выкл — автоисправление выключено"),
                        ],
                        st::HIT * 3.0,
                        accent,
                    ) {
                        act = Some((id.clone(), Some(mode)));
                    }
                },
            );
        }

        if let Some((id, mode)) = act {
            match mode {
                Some(m) => self.set_app_mode(&id, m),
                None => self.remove_app_entry(&id),
            }
            changed = true;
        }
        if changed {
            self.save();
        }
    }

    fn words_tab(&mut self, ui: &mut egui::Ui) {
        let accent = Tab::Words.color();
        let mut changed = false;
        st::block(ui, |ui| {
            if st::add_line_plus(
                ui,
                &mut self.word_draft,
                "слово",
                "Добавить слово (без пробелов; можно цифры и знаки)",
                accent,
            ) {
                changed |= self.add_custom_word();
            }
        });
        let words = self.cfg.custom_words.clone();
        if !words.is_empty() {
            st::caption(ui, "Свои", "Приоритет выше встроенного словаря");
            let mut remove_at = None;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(st::GAP, st::GAP);
                for (i, w) in words.iter().enumerate() {
                    let lang = match userdict::classify(w) {
                        Ok(l) => lang_name(l),
                        Err(_) => "?",
                    };
                    if st::chip(ui, w, Some(lang)) {
                        remove_at = Some(i);
                    }
                }
            });
            if let Some(i) = remove_at {
                if i < self.cfg.custom_words.len() {
                    self.cfg.custom_words.remove(i);
                    changed = true;
                }
            }
        }

        let never = self.cfg.never_correct.clone();
        if !never.is_empty() {
            st::caption(ui, "Пропуск", "Слова, исправление которых вы отменили дважды");
            if let Some(i) = chips(ui, &never) {
                if i < self.cfg.never_correct.len() {
                    self.cfg.never_correct.remove(i);
                    changed = true;
                }
            }
        }

        let never_typo = self.cfg.never_typo_fix.clone();
        if !never_typo.is_empty() {
            st::caption(ui, "Без опечаток", "Отменена только правка опечаток; раскладка исправляется");
            if let Some(i) = chips(ui, &never_typo) {
                if i < self.cfg.never_typo_fix.len() {
                    self.cfg.never_typo_fix.remove(i);
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
            HotkeySlot::Undo => self.cfg.undo_hotkey,
        };
        let t = st::cur(ui);
        st::block(ui, |ui| {
            ui.spacing_mut().item_spacing.y = st::GAP;
            ui.horizontal(|ui| {
                let capturing = self.capturing_hotkey == Some(slot);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if capturing {
                        st::primary(ui, "Нажмите… Esc", accent);
                    } else if st::secondary(ui, "Назначить").clicked() {
                        self.capturing_hotkey = Some(slot);
                        self.status = "Нажмите сочетание…".into();
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(label).color(t.ink)).truncate());
                        ui.add(egui::Label::new(egui::RichText::new(current.to_string()).monospace().color(t.muted)).truncate());
                    });
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(st::GAP, st::GAP);
                for &(h, name) in presets {
                    if st::choice_tile(ui, current == h, name, 0.0, accent).clicked() {
                        match slot {
                            HotkeySlot::Convert => self.cfg.hotkey = h,
                            HotkeySlot::Summon => self.cfg.summon_hotkey = h,
                            HotkeySlot::Undo => self.cfg.undo_hotkey = h,
                        }
                        self.save();
                    }
                }
            });
        });
    }

    fn donate_tab(&mut self, ui: &mut egui::Ui) {
        st::block(ui, |ui| {
            st::row(
                ui,
                egui::Color32::TRANSPARENT,
                |ui| {
                    ui.label("Boosty");
                },
                |ui| {
                    if st::primary(ui, "Открыть", Tab::Donate.color()).on_hover_text(BOOSTY_URL).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(BOOSTY_URL));
                    }
                },
            );
        });
        self.address_block(ui, "USDT · TRC-20", USDT_TRC20, "Только сеть TRON (TRC-20)");
        self.address_block(ui, "GRAM · TON", GRAM_TON, "Только сеть TON");
    }

    fn address_block(&mut self, ui: &mut egui::Ui, title: &str, addr: &'static str, tip: &str) {
        let tex = self
            .qr
            .entry(addr)
            .or_insert_with(|| ui.ctx().load_texture(format!("qr-{addr}"), qr_image(addr), egui::TextureOptions::NEAREST))
            .clone();
        st::block(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(92.0, 92.0))).on_hover_text(tip);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    ui.label(title).on_hover_text(tip);
                    ui.add(egui::Label::new(egui::RichText::new(addr).monospace().small()).wrap_mode(egui::TextWrapMode::Wrap));
                    if st::secondary(ui, "Копировать").on_hover_text(tip).clicked() {
                        ui.ctx().copy_text(addr.to_owned());
                        self.status = "Скопировано".into();
                    }
                });
            });
        });
    }

    /// Collapsible group of the settings section. Closed until opened.
    fn group(&mut self, ui: &mut egui::Ui, key: &'static str, title: &str, mark: st::Mark, body: impl FnOnce(&mut Self, &mut egui::Ui)) {
        let open = self.open_groups.contains(key);
        if st::group_header(ui, open, title, &mark, Tab::Settings.color()).clicked() {
            if open {
                self.open_groups.remove(key);
            } else {
                self.open_groups.insert(key);
            }
        }
        if self.open_groups.contains(key) {
            body(self, ui);
        }
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let accent = Tab::Settings.color();
        let mut changed = false;
        changed |= st::toggle_row(
            ui,
            &mut self.cfg.autocorrect,
            "Автоисправление",
            "Исправляет слова не в той раскладке (зуфсу → peace)",
            accent,
        );

        let on = [self.cfg.convert_single_letters, self.cfg.fix_missing_spaces, self.cfg.auto_learn].iter().filter(|b| **b).count();
        let mut c = false;
        self.group(ui, "fix", "Исправления", st::Mark::Count { on: on as u8, of: 3 }, |s, ui| {
            c |= st::toggle_row(
                ui,
                &mut s.cfg.convert_single_letters,
                "Отдельные буквы",
                "d → в, z → я, ф → a, ш → i (настоящие a, I, в, и, я остаются)",
                accent,
            );
            c |= st::toggle_row(
                ui,
                &mut s.cfg.fix_missing_spaces,
                "Пропущенные пробелы",
                "После предлогов и «не»: незнаю → не знаю",
                accent,
            );
            c |= st::toggle_row(
                ui,
                &mut s.cfg.auto_learn,
                "Автозапоминание",
                "Запоминает раскладку новых программ",
                accent,
            );
        });

        let typo_mark = if self.cfg.fix_typos {
            st::Mark::Count { on: typo_rank(self.cfg.typo_level), of: 3 }
        } else {
            st::Mark::Toggle(false)
        };
        self.group(ui, "typo", "Опечатки", typo_mark, |s, ui| {
            c |= st::toggle_row(ui, &mut s.cfg.fix_typos, "Исправлять опечатки", "поечаток → опечаток", accent);
            ui.add_enabled_ui(s.cfg.fix_typos, |ui| {
                st::block(ui, |ui| {
                    st::caption(ui, "Смелость", "Насколько смело исправлять опечатки");
                    let typo_opts = [
                        (TypoLevel::Cautious, level_name(TypoLevel::Cautious), "1 ошибка от 4 букв, 2 от 8, только частые"),
                        (TypoLevel::Normal, level_name(TypoLevel::Normal), "1 от 3, 2 от 5, 3 от 9; раскладка + опечатка"),
                        (TypoLevel::Aggressive, level_name(TypoLevel::Aggressive), "Смелее при близких вариантах"),
                    ];
                    let strip_w = ui.available_width();
                    c |= st::segment_strip(ui, &mut s.cfg.typo_level, &typo_opts, strip_w, accent);
                });
                c |= st::toggle_row(
                    ui,
                    &mut s.cfg.use_context,
                    "Соседние слова",
                    "Учитывает 1–2 предыдущих слова предложения",
                    accent,
                );
            });
        });

        let undo_mark = if self.cfg.undo_on_backspace {
            st::Mark::Label("Backspace")
        } else {
            st::Mark::Value(self.cfg.undo_hotkey.to_string())
        };
        self.group(ui, "undo", "Отмена", undo_mark, |s, ui| {
            c |= st::toggle_row(
                ui,
                &mut s.cfg.undo_on_backspace,
                "Отмена по Backspace",
                "Первый Backspace снимает опечатку, второй — раскладку; двойная отмена добавляет в Пропуск",
                accent,
            );
        });

        self.group(ui, "keys", "Клавиши", st::Mark::Value(self.cfg.hotkey.to_string()), |s, ui| {
            s.hotkey_picker(
                ui,
                HotkeySlot::Convert,
                "Конвертация",
                &[
                    (Hotkey::Pause, "Pause"),
                    (Hotkey::ShiftShift, "Shift+Shift"),
                    (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space"),
                ],
            );
            s.hotkey_picker(
                ui,
                HotkeySlot::Summon,
                "Окно",
                &[
                    (Hotkey::CTRL_SHIFT_V, "Ctrl+Shift+V"),
                    (Hotkey::CTRL_SHIFT_K, "Ctrl+Shift+K"),
                    (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space"),
                ],
            );
            s.hotkey_picker(
                ui,
                HotkeySlot::Undo,
                "Отмена",
                &[
                    (Hotkey::CTRL_ALT_Z, if cfg!(target_os = "macos") { "Ctrl+Option+Z" } else { "Ctrl+Alt+Z" }),
                    (Hotkey::CTRL_ALT_BACKSPACE, if cfg!(target_os = "macos") { "Ctrl+Option+Delete" } else { "Ctrl+Alt+Backspace" }),
                    (Hotkey::SHIFT_PAUSE, "Shift+Pause"),
                ],
            );
            if s.cfg.hotkey == s.cfg.summon_hotkey
                || s.cfg.undo_hotkey == s.cfg.hotkey
                || s.cfg.undo_hotkey == s.cfg.summon_hotkey
            {
                st::block(ui, |ui| st::warn(ui, "Совпадение сочетаний"));
            }
        });

        #[cfg(windows)]
        {
            self.group(ui, "admin", "Запуск", st::Mark::Toggle(self.cfg.run_as_admin), |s, ui| s.admin_section(ui));
        }

        self.group(ui, "look", "Вид", st::Mark::Theme { light: self.cfg.light_theme }, |s, ui| {
            c |= st::toggle_row(ui, &mut s.cfg.light_theme, "Светлая тема", "", accent);
            st::block(ui, |ui| {
                c |= s.logo_picker(ui);
            });
        });

        self.group(ui, "about", "О программе", st::Mark::Label(env!("CARGO_PKG_VERSION")), |s, ui| {
            st::block(ui, |ui| {
                st::row(
                    ui,
                    egui::Color32::TRANSPARENT,
                    |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(WINDOW_TITLE).color(st::cur(ui).muted)).truncate())
                            .on_hover_text(format!("{}", s.path.display()));
                    },
                    |ui| {
                        if st::secondary(ui, "Приветствие").clicked() {
                            s.welcome_dont_show = s.cfg.hide_welcome;
                            s.welcome_open = true;
                        }
                    },
                );
            });
        });
        changed |= c;
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
        if let Ok(icon) = eframe::icon_data::from_png_bytes(crate::logos::icon_png(self.cfg.logo_style)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(icon))));
        }
        crate::platform::set_dock_icon(crate::logos::icon_png(self.cfg.logo_style));
        #[cfg(target_os = "macos")]
        let _ = ctx;
        if let Some(tray) = &self.tray {
            tray.set_logo(crate::logos::index(self.cfg.logo_style));
        }
    }

    fn logo_texture(&mut self, ctx: &egui::Context, style: LogoStyle) -> egui::TextureHandle {
        let i = crate::logos::index(style) as usize;
        self.logo_textures[i]
            .get_or_insert_with(|| crate::logos::texture(ctx, style))
            .clone()
    }

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
                } else if !self.cfg.fix_typos {
                    "Опечатки выключены".into()
                } else {
                    format!("Смелость: {}", level_name(self.cfg.typo_level))
                }
            }
            Tab::Donate => "Boosty, USDT, GRAM".into(),
        }
    }

    /// Home: five identical colour squares in one horizontal row, snapped to `U`.
    fn home(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_width();
        let g = st::GAP;
        let s = st::snap((full - 4.0 * g) / 5.0, st::HIT);
        let row_w = 5.0 * s + 4.0 * g;
        let x_pad = st::snap(((full - row_w) * 0.5).max(0.0), 0.0);
        let top_pad = st::snap(((ui.available_height() - s) * 0.5).max(0.0), 0.0);
        let origin = ui.cursor().min + egui::vec2(x_pad, top_pad);
        let mut clicked = None;
        for (i, &tab) in SECTIONS.iter().enumerate() {
            let x = origin.x + i as f32 * (s + g);
            let rect = egui::Rect::from_min_size(egui::pos2(x, origin.y), egui::vec2(s, s));
            let resp = ui.interact(rect, ui.id().with(("square", i)), egui::Sense::click());
            let color = tab.color();
            let p = ui.painter();
            p.rect_filled(rect, 0.0, if resp.is_pointer_button_down_on() { st::shade(color, -0.1) } else { color });
            if resp.has_focus() {
                p.rect_stroke(rect.shrink(0.5), 0.0, st::hair(st::INK_ON_COLOR));
            }
            let u = st::snap((s * 0.16).max(st::MARK / 2.0), st::MARK / 2.0).min(st::MARK);
            st::paint_home_icon(p, egui::pos2(rect.center().x, rect.top() + s * 0.42), u, i);
            let pad = st::GAP;
            let max_w = (s - pad * 2.0).max(st::U);
            // One size for all five labels: the largest scale step that fits the longest word.
            let longest = SECTIONS.iter().map(|tab| st::text_w(ui, tab.title(), st::font_md())).fold(0.0_f32, f32::max);
            let font = if longest > max_w { st::font_sm() } else { st::font_md() };
            let lg = st::line_galley(ui, tab.title(), font, st::INK_ON_COLOR, max_w);
            p.galley(egui::pos2(rect.center().x - lg.size().x / 2.0, rect.bottom() - pad - lg.size().y), lg, st::INK_ON_COLOR);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let tip = self.section_tooltip(tab);
            let resp = resp.on_hover_text(tip);
            if resp.clicked() {
                clicked = Some(tab);
            }
        }
        ui.allocate_rect(
            egui::Rect::from_min_max(origin - egui::vec2(x_pad, top_pad), egui::pos2(origin.x - x_pad + full, origin.y + s)),
            egui::Sense::hover(),
        );
        if let Some(tab) = clicked {
            self.go(tab);
        }
    }

    fn section_header(&mut self, ui: &mut egui::Ui) {
        let tab = self.tab;
        let color = tab.color();
        let mut target = None;
        ui.horizontal(|ui| {
            ui.set_min_height(st::ROW);
            ui.spacing_mut().item_spacing.x = st::GAP;
            if st::back_square(ui, color).clicked() {
                target = Some(Tab::Home);
            }
            let nav_w = 5.0 * st::HIT;
            let title_w = (ui.available_width() - nav_w - st::GAP).max(st::HIT);
            ui.add_sized(
                [title_w, st::HIT],
                egui::Label::new(egui::RichText::new(tab.title()).size(st::FONT_LG).color(st::INK_ON_COLOR)).truncate(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for &other in SECTIONS.iter().rev() {
                    // 32×32 hit target, 16×16 colour square centred in it.
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(st::HIT, st::HIT), egui::Sense::click());
                    let vis = egui::Rect::from_center_size(rect.center(), egui::vec2(st::MARK, st::MARK));
                    let p = ui.painter();
                    let fill = if other == tab { st::INK_ON_COLOR } else { other.color() };
                    p.rect_filled(vis, 0.0, fill);
                    if resp.has_focus() {
                        p.rect_stroke(rect.shrink(0.5), 0.0, st::hair(st::INK_ON_COLOR));
                    }
                    if resp.on_hover_text(other.title()).clicked() {
                        target = Some(other);
                    }
                }
            });
        });
        if let Some(t) = target {
            self.go(t);
        }
    }

    fn logo_picker(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        const TILE: f32 = st::ROW;
        let spacing = st::GAP * 2.0;
        let col = |ui: &egui::Ui, style| TILE.max(st::text_w(ui, crate::logos::label(style), st::font_sm()));
        let total: f32 = LogoStyle::ALL.iter().map(|&s| col(ui, s)).sum::<f32>() + spacing * (LogoStyle::ALL.len().saturating_sub(1) as f32);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            ui.add_space(((ui.available_width() - total) * 0.5).max(0.0));
            for style in LogoStyle::ALL {
                let col_w = col(ui, style);
                ui.allocate_ui_with_layout(egui::vec2(col_w, st::ROW + st::GAP + st::FONT_SM + st::U), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.y = st::GAP;
                    let tex = self.logo_texture(ui.ctx(), style);
                    let img = egui::Image::new(&tex)
                        .fit_to_exact_size(egui::vec2(TILE, TILE))
                        .maintain_aspect_ratio(false)
                        .rounding(0.0)
                        .sense(egui::Sense::click());
                    let response = ui.add(img);
                    let selected = self.cfg.logo_style == style;
                    if selected {
                        let bar = egui::Rect::from_min_size(egui::pos2(response.rect.left(), response.rect.bottom()), egui::vec2(TILE, st::HAIR));
                        ui.painter().rect_filled(bar, 0.0, st::cur(ui).ink);
                    }
                    if response.clicked() && !selected {
                        self.cfg.logo_style = style;
                        changed = true;
                        self.apply_logo(ui.ctx());
                    }
                    ui.label(egui::RichText::new(crate::logos::label(style)).size(st::FONT_SM));
                });
            }
        });
        changed
    }

    fn welcome_panel(&mut self, ui: &mut egui::Ui) {
        let heading = egui::Color32::from_rgb(18, 42, 48);
        let body = egui::Color32::from_rgb(36, 58, 64);
        ui.visuals_mut().override_text_color = Some(body);
        ui.visuals_mut().dark_mode = false;
        ui.vertical_centered(|ui| {
            let tex = self.faded_logo_texture(ui.ctx(), self.cfg.logo_style);
            let size = tex.size_vec2();
            if size.x > 0.0 && size.y > 0.0 {
                let logo_w = ui.available_width().min(200.0);
                let logo_h = logo_w * (size.y / size.x);
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(logo_w, logo_h)).rounding(0.0));
            }
            ui.add_space(st::GAP);
            let chip = st::MARK;
            let (strip, _) = ui.allocate_exact_size(egui::vec2(5.0 * chip + 4.0 * st::GAP, chip), egui::Sense::hover());
            for (i, tab) in SECTIONS.iter().enumerate() {
                let x = strip.left() + i as f32 * (chip + st::GAP);
                ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, strip.top()), egui::vec2(chip, chip)), 0.0, tab.color());
            }
            ui.add_space(st::GAP);
            ui.label(egui::RichText::new("Typed").color(heading).size(st::FONT_LG));
            ui.label(egui::RichText::new(concat!("ver. ", env!("CARGO_PKG_VERSION"))).color(body).size(st::FONT_SM));
            ui.add_space(st::GAP);
            if self.logo_picker(ui) {
                self.save();
            }
            ui.add_space(st::GAP);
        });
        let next_w = st::text_w(ui, "Далее", st::font_md()) + st::PAD * 2.0;
        let chk_w = st::HIT + st::text_w(ui, "Не показывать", st::font_md());
        let footer_w = next_w + st::PAD * 2.0 + chk_w;
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - footer_w) * 0.5).max(0.0));
            if st::primary(ui, "Далее", st::CYAN).clicked() {
                self.welcome_open = false;
                self.cfg.welcome_gen = layout_keeper_core::config::CURRENT_WELCOME_GEN;
                self.cfg.hide_welcome = self.welcome_dont_show;
                self.save();
            }
            ui.add_space(st::PAD);
            st::check(ui, &mut self.welcome_dont_show, "Не показывать", st::CYAN);
        });
        ui.add_space(st::GAP);
    }

    fn draw(&mut self, ctx: &egui::Context, focus: &Focus) {
        self.apply_theme(ctx);
        let t = st::theme(!self.cfg.light_theme);
        if self.welcome_open {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::WHITE).inner_margin(egui::Margin::same(st::PAD)))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        self.welcome_panel(ui);
                    });
                });
            return;
        }
        if self.tab != Tab::Home {
            let color = self.tab.color();
            egui::TopBottomPanel::top("section")
                .frame(egui::Frame::none().fill(color).inner_margin(egui::Margin::symmetric(st::PAD, st::GAP)))
                .show_separator_line(false)
                .show(ctx, |ui| self.section_header(ui));
        }
        if !self.status.is_empty() {
            egui::TopBottomPanel::bottom("status")
                .frame(egui::Frame::none().fill(t.surface).inner_margin(egui::Margin::symmetric(st::PAD, st::GAP)))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (sq, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().rect_filled(sq, 0.0, self.tab.color());
                        ui.add(egui::Label::new(egui::RichText::new(&self.status).small().color(t.ink)).truncate());
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
                let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::none().inner_margin(egui::Margin::same(st::PAD)).show(ui, |ui| {
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
            Tab::Home => st::CYAN,
            Tab::Apps => st::YELLOW,
            Tab::Words => st::ORANGE,
            Tab::Vault => st::LIME,
            Tab::Settings => st::CYAN,
            Tab::Donate => st::RED,
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

fn typo_rank(l: TypoLevel) -> u8 {
    match l {
        TypoLevel::Cautious => 1,
        TypoLevel::Normal => 2,
        TypoLevel::Aggressive => 3,
    }
}

fn chips(ui: &mut egui::Ui, words: &[String]) -> Option<usize> {
    let mut hit = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(st::GAP, st::GAP);
        for (i, w) in words.iter().enumerate() {
            if st::chip(ui, w, None) {
                hit = Some(i);
            }
        }
    });
    hit
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
            self.tab = Tab::Vault;
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
                    HotkeySlot::Undo => self.cfg.undo_hotkey = hk,
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
    let logo_index = crate::logos::index(cfg.logo_style);
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
        tab: if open_vault { Tab::Vault } else { Tab::Home },
        vault: crate::vault_ui::VaultUi::new(),
        capturing_hotkey: None,
        word_draft: String::new(),
        icons: IconCache::default(),
        open_groups: default_open_groups(),
        content_overflow: 0.0,
        welcome_open,
        welcome_dont_show: false,
        logo_textures: [None, None, None],
        logo_fade_textures: [None, None, None],
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
            .with_inner_size([512.0, 192.0])
            .with_min_inner_size([432.0, 160.0])
            .with_resizable(true);
        if start_hidden {
            vp = vp.with_visible(false);
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(icon) = eframe::icon_data::from_png_bytes(crate::logos::icon_png(app.cfg.logo_style)) {
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
