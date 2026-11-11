//! Settings window (`layout-keeper gui`). A separate process that only edits the same
//! `config.json` the daemon watches; the daemon picks changes up within ~1 s.
//! The keyboard hook is never touched from here.

use crate::brand::{APP_NAME, WINDOW_TITLE};
use crate::platform;
use eframe::egui;
use layout_keeper_core::{Config, Hotkey, HotkeyCode, Lang, LogoStyle};
use crate::icons::{basename, delete_icon_button, has_path, hide_icon_button, is_hidden_app, IconCache};
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
    manual_id: String,
    manual_lang: Lang,
    exception_draft: String,
    tab: Tab,
    vault: crate::vault_ui::VaultUi,
    capturing_hotkey: Option<HotkeySlot>,
    word_draft: String,
    expanded_paths: HashSet<String>,
    show_system_apps: bool,
    icons: IconCache,
    /// Last inner height we applied (content + 10px, capped to the current monitor).
    fitted_height: Option<f32>,
    last_min_size: Option<egui::Vec2>,
    last_max_size: Option<egui::Vec2>,
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
    daemon_admin: (Option<bool>, Option<Instant>),
}

type AdminJobResult = Arc<Mutex<Option<Result<(), String>>>>;

#[derive(PartialEq, Clone, Copy)]
enum HotkeySlot {
    Convert,
    Summon,
    Undo,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Layouts,
    Exceptions,
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
                self.status = "Сохранено — демон применит изменения примерно за секунду.".into();
            }
            Err(e) => self.status = format!("Ошибка сохранения: {e}"),
        }
    }

    fn add_exception_entry(&mut self, raw: String) -> bool {
        let entry = raw.trim().to_string();
        if entry.is_empty() {
            return false;
        }
        let key = entry.to_lowercase();
        if self.cfg.exception_apps.iter().any(|e| e.to_lowercase() == key) {
            self.status = "Такая запись уже есть в списке.".into();
            return false;
        }
        self.cfg.exception_apps.push(entry);
        self.exception_draft.clear();
        true
    }

    fn add_custom_word(&mut self) -> bool {
        let w = self.word_draft.trim().to_string();
        if let Err(e) = userdict::classify(&w) {
            self.status = e.message_ru().into();
            return false;
        }
        if userdict::contains_word(&self.cfg.custom_words, &w) {
            self.status = "Такое слово уже есть в словаре.".into();
            return false;
        }
        self.cfg.custom_words.push(w);
        self.word_draft.clear();
        true
    }

    fn toggle_path(&mut self, id: &str) {
        if self.expanded_paths.contains(id) {
            self.expanded_paths.remove(id);
        } else {
            self.expanded_paths.insert(id.to_string());
        }
    }

    /// Icon + short exe name + "…" revealing the full path on the next line.
    fn app_label(&mut self, ui: &mut egui::Ui, id: &str) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                self.icons.show(ui, id);
                let title = platform::app_display_name(id).unwrap_or_else(|| basename(id).to_string());
                if has_path(id) {
                    let open = self.expanded_paths.contains(id);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(if open { "▾" } else { "…" })
                            .on_hover_text(if open { "Скрыть путь" } else { "Показать полный путь" })
                            .clicked()
                        {
                            self.toggle_path(id);
                        }
                        ui.add(egui::Label::new(egui::RichText::new(&title).strong()).truncate())
                            .on_hover_text(id);
                    });
                } else {
                    ui.add(egui::Label::new(egui::RichText::new(&title).strong()).truncate())
                        .on_hover_text(id);
                }
            });
            if self.expanded_paths.contains(id) {
                ui.add(
                    egui::Label::new(egui::RichText::new(id).monospace().small())
                        .wrap()
                        .selectable(true),
                );
            }
        });
    }

    fn app_actions(&mut self, ui: &mut egui::Ui, id: &str, cur: Lang) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.spacing_mut().item_spacing.x = 4.0;
            for l in [Lang::En, Lang::Ru] {
                if ui.selectable_label(cur == l, lang_name(l)).clicked() && cur != l {
                    self.cfg.set_layout(id, l);
                    changed = true;
                }
            }
            if delete_icon_button(ui).clicked() {
                self.cfg.remove_app(id);
                changed = true;
            }
            // Hide into the collapsed group / bring back. Built-in system apps stay
            // there without a button. Visual only: the layout is still applied.
            let user_hidden = self.cfg.is_user_hidden(id);
            if (user_hidden || !is_hidden_app(id)) && hide_icon_button(ui, !user_hidden).clicked() {
                self.cfg.set_user_hidden(id, !user_hidden);
                changed = true;
            }
        });
        changed
    }

    /// One compact row: name truncates, EN/RU, the trash and hide icons stay on the right.
    fn apps_list(&mut self, ui: &mut egui::Ui, apps: &[(String, Lang)]) -> bool {
        let mut changed = false;
        for (i, (id, cur)) in apps.iter().enumerate() {
            let bg = if i % 2 == 0 {
                ui.visuals().faint_bg_color
            } else {
                egui::Color32::TRANSPARENT
            };
            egui::Frame::none().fill(bg).inner_margin(egui::Margin::symmetric(2.0, 2.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        changed |= self.app_actions(ui, id, *cur);
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_max_width(ui.available_width());
                            self.app_label(ui, id);
                        });
                    });
                });
            });
        }
        changed
    }

    fn sort_apps(&self, apps: &mut [(String, Lang)]) {
        apps.sort_by(|a, b| {
            self.cfg
                .launch_count(&b.0)
                .cmp(&self.cfg.launch_count(&a.0))
                .then_with(|| basename(&a.0).to_lowercase().cmp(&basename(&b.0).to_lowercase()))
        });
    }

    fn words_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Свои слова");
        ui.label("Сокращения, жаргон, названия. Автозамена знает их и ставит выше встроенного словаря.");
        ui.small("Пример: слово «exe» заменит русское «учу». Обратная сторона защищена. Можно с цифрами и знаками (mp3, c++, ci/cd), но без пробелов. Однобуквенные — во вкладке «Настройки».");
        ui.add_space(6.0);
        let mut changed = false;
        let mut add_word = false;
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.label("Слово:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Добавить").clicked() {
                    add_word = true;
                }
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.word_draft).desired_width(ui.available_width().max(60.0)),
                );
                add_word = add_word || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            });
        });
        if add_word {
            changed |= self.add_custom_word();
        }
        ui.add_space(6.0);
        let words = self.cfg.custom_words.clone();
        if words.is_empty() {
            ui.label("(пока пусто)");
        }
        let mut remove_at = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (i, w) in words.iter().enumerate() {
                let lang = match userdict::classify(w) {
                    Ok(l) => lang_name(l),
                    Err(_) => "?",
                };
                if word_chip(ui, w, lang) {
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

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Не исправлять");
        ui.small("Слова, исправление которых вы отменили (сочетанием отмены, Backspace или набрав слово заново). Typed их больше не трогает. Корзина — снова разрешить исправление.");
        let never = self.cfg.never_correct.clone();
        if never.is_empty() {
            ui.label("(пока пусто)");
        }
        let mut never_remove = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (i, w) in never.iter().enumerate() {
                if chip(ui, w, None) {
                    never_remove = Some(i);
                }
            }
        });
        if let Some(i) = never_remove {
            if i < self.cfg.never_correct.len() {
                self.cfg.never_correct.remove(i);
                changed = true;
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
        let current = match slot {
            HotkeySlot::Convert => self.cfg.hotkey,
            HotkeySlot::Summon => self.cfg.summon_hotkey,
            HotkeySlot::Undo => self.cfg.undo_hotkey,
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(label);
            ui.strong(current.to_string());
            let capturing = self.capturing_hotkey == Some(slot);
            if capturing {
                ui.colored_label(
                    warn_color(ui),
                    "нажмите клавиши… Esc — отмена",
                );
            } else if ui.button("Назначить").clicked() {
                self.capturing_hotkey = Some(slot);
                self.status = "Нажмите нужное сочетание клавиш.".into();
            }
        });
        ui.horizontal_wrapped(|ui| {
            for &(h, name) in presets {
                if ui.selectable_label(current == h, name).clicked() {
                    match slot {
                        HotkeySlot::Convert => self.cfg.hotkey = h,
                        HotkeySlot::Summon => self.cfg.summon_hotkey = h,
                        HotkeySlot::Undo => self.cfg.undo_hotkey = h,
                    }
                    self.save();
                }
            }
        });
    }

    fn exceptions_tab(&mut self, ui: &mut egui::Ui, focus: &Focus) {
        ui.heading("Исключения автозамены");
        ui.label("Автозамена и клавиша конвертации здесь не работают. Назначенный язык по-прежнему ставится.");
        ui.small("Совпадение по фрагменту имени, без регистра: powershell, windowsterminal.");

        let mut changed = false;
        let mut remove_at: Option<usize> = None;
        let items = self.cfg.exception_apps.clone();
        if items.is_empty() {
            ui.label("(список пуст — автозамена работает везде)");
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (i, item) in items.iter().enumerate() {
                if chip(ui, item, None) {
                    remove_at = Some(i);
                }
            }
        });
        if let Some(i) = remove_at {
            if i < self.cfg.exception_apps.len() {
                self.cfg.exception_apps.remove(i);
                changed = true;
            }
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.label("Добавить:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Добавить").clicked() {
                    changed |= self.add_exception_entry(self.exception_draft.clone());
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.exception_draft).desired_width(ui.available_width().max(60.0)),
                );
            });
        });
        if let Some(a) = focus.app.clone() {
            ui.horizontal(|ui| {
                ui.label("Текущее:");
                ui.add(egui::Label::new(egui::RichText::new(basename(&a)).monospace()).truncate());
                if self.cfg.is_exception(&a) {
                    ui.colored_label(warn_color(ui), "уже в исключениях");
                } else {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Добавить текущее").clicked() {
                            changed |= self.add_exception_entry(a);
                        }
                    });
                }
            });
        }
        if changed {
            self.save();
        }
    }

    fn layouts_tab(&mut self, ui: &mut egui::Ui, focus: &Focus) {
        ui.heading("Текущее приложение");
        ui.label("Переключитесь в программу и вернитесь сюда — она останется в списке.");
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.label("Приложение:");
            match focus.app.clone() {
                Some(a) => self.app_label(ui, &a),
                None => {
                    ui.label("(не определено)");
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.label("Сейчас:");
            ui.monospace(focus.layout.map(lang_name).unwrap_or("?"));
            ui.label("Назначено:");
            let assigned = focus.app.as_deref().and_then(|a| self.cfg.layout_for(a));
            ui.monospace(assigned.map(lang_name).unwrap_or("—"));
        });
        if let Some(a) = focus.app.clone() {
            if self.cfg.is_exception(&a) {
                ui.colored_label(warn_color(ui), "В исключениях: автозамена выключена, раскладка ставится как обычно.");
            }
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label("Назначить:");
                for l in [Lang::En, Lang::Ru] {
                    if ui.button(lang_name(l)).clicked() {
                        self.cfg.set_layout(&a, l);
                        self.save();
                    }
                }
            });
        }

        ui.separator();
        ui.heading("Запомненные приложения");
        ui.label("Раскладка включается при каждом входе в приложение. Ручное переключение её не меняет.");
        let (mut hidden_apps, mut apps): (Vec<(String, Lang)>, Vec<(String, Lang)>) = self
            .cfg
            .apps
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .partition(|(id, _)| is_hidden_app(id) || self.cfg.is_user_hidden(id));
        self.sort_apps(&mut apps);
        self.sort_apps(&mut hidden_apps);
        if apps.is_empty() {
            ui.label("(пока пусто)");
        }
        let mut changed = self.apps_list(ui, &apps);
        if !hidden_apps.is_empty() {
            let arrow = if self.show_system_apps { "▾" } else { "▸" };
            if ui
                .small_button(format!("{arrow} Скрытые и системные ({})", hidden_apps.len()))
                .on_hover_text("Системные программы, установщики и те, что вы скрыли. Раскладка для них всё равно ставится.")
                .clicked()
            {
                self.show_system_apps = !self.show_system_apps;
            }
            if self.show_system_apps {
                changed |= self.apps_list(ui, &hidden_apps);
            }
        }
        ui.add_space(6.0);
        ui.label("Добавить вручную:");
        ui.add(egui::TextEdit::singleline(&mut self.manual_id).desired_width(ui.available_width().max(60.0)));
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            for l in [Lang::En, Lang::Ru] {
                ui.selectable_value(&mut self.manual_lang, l, lang_name(l));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Добавить").clicked() && !self.manual_id.trim().is_empty() {
                    let id = std::mem::take(&mut self.manual_id);
                    self.cfg.set_layout(&id, self.manual_lang);
                    changed = true;
                }
            });
        });

        if changed {
            self.save();
        }
    }

    fn donate_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Поддержать Typed");
        ui.label("Typed бесплатный и ничего не отправляет в сеть. Если он вам помогает — можно поддержать.");
        ui.add_space(8.0);
        ui.strong("Boosty");
        if ui.button("Открыть Boosty").on_hover_text(BOOSTY_URL).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(BOOSTY_URL));
        }
        ui.add_space(8.0);
        self.address_block(ui, "USDT (TRC-20)", USDT_TRC20, "Только сеть TRON (TRC-20). USDT из другой сети (ERC-20, BEP-20) потеряется.");
        ui.add_space(8.0);
        self.address_block(ui, "GRAM (TON)", GRAM_TON, "Только сеть TON. Из другой сети перевод потеряется.");
    }

    fn address_block(&mut self, ui: &mut egui::Ui, title: &str, addr: &'static str, note: &str) {
        let tex = self
            .qr
            .entry(addr)
            .or_insert_with(|| ui.ctx().load_texture(format!("qr-{addr}"), qr_image(addr), egui::TextureOptions::NEAREST))
            .clone();
        ui.strong(title);
        ui.horizontal_top(|ui| {
            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(110.0, 110.0)));
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(addr).monospace().small())
                            .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                });
                if ui.small_button("🗐").on_hover_text("Скопировать адрес").clicked() {
                    ui.ctx().copy_text(addr.to_owned());
                    self.status = format!("Адрес {title} скопирован.");
                }
                ui.add(egui::Label::new(egui::RichText::new(note).small()).wrap());
            });
        });
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Настройки");
        let mut changed = false;
        changed |= ui.checkbox(&mut self.cfg.autocorrect, "Автоисправление (зуфсу → peace)").changed();
        changed |= ui
            .checkbox(
                &mut self.cfg.convert_single_letters,
                "Исправлять отдельные буквы",
            )
            .changed();
        ui.small("Буква, которая сама по себе слово только в другом языке, переводится: d → в, z → я, ф → a, ш → i. Настоящие a, I, в, и, я остаются. Остальные (q/й) — в язык приложения.");
        changed |= ui.checkbox(&mut self.cfg.fix_typos, "Исправлять опечатки (поечаток → опечаток)").changed();
        ui.small("Одна ошибка — в словах от 4 букв, две — от 8 букв. Только когда вариант один и частый. Комбинация конвертации возвращает как было.");
        changed |= ui.checkbox(&mut self.cfg.fix_missing_spaces, "Расставлять пропущенные пробелы").changed();
        ui.small("После предлогов и «не»: снейросеткой → с нейросеткой, вофисе → в офисе, незнаю → не знаю. Только когда слова нет в словарях. Отменяется как исправление.");
        changed |= ui.checkbox(&mut self.cfg.undo_on_backspace, "Backspace сразу после исправления отменяет его").changed();
        ui.small("Только первый Backspace, пока не нажато ничего другого. Слово возвращается как было и запоминается, если вы его оставите.");
        changed |= ui.checkbox(&mut self.cfg.auto_learn, "Автозапоминание").changed();
        ui.small("Запоминает раскладку, только если её ещё нет. Назначенную не меняет.");
        ui.separator();
        ui.heading("Горячие клавиши");
        ui.small("Удобно начинать с Ctrl+Shift. Букву без Ctrl, Alt или Win назначить нельзя.");
        self.hotkey_picker(
            ui,
            HotkeySlot::Convert,
            "Конвертация слова:",
            &[
                (Hotkey::Pause, "Pause"),
                (Hotkey::ShiftShift, "Shift+Shift"),
                (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space"),
            ],
        );
        self.hotkey_picker(
            ui,
            HotkeySlot::Summon,
            "Вызов окна:",
            &[
                (Hotkey::CTRL_SHIFT_V, "Ctrl+Shift+V"),
                (Hotkey::CTRL_SHIFT_K, "Ctrl+Shift+K"),
                (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space"),
            ],
        );
        self.hotkey_picker(
            ui,
            HotkeySlot::Undo,
            "Отмена автоисправления:",
            &[
                (Hotkey::CTRL_ALT_Z, if cfg!(target_os = "macos") { "Ctrl+Option+Z" } else { "Ctrl+Alt+Z" }),
                (Hotkey::CTRL_ALT_BACKSPACE, if cfg!(target_os = "macos") { "Ctrl+Option+Delete" } else { "Ctrl+Alt+Backspace" }),
                (Hotkey::SHIFT_PAUSE, "Shift+Pause"),
            ],
        );
        ui.small("Возвращает слово как было набрано и больше никогда его не исправляет (список — во вкладке «Свои слова»).");
        if self.cfg.hotkey == self.cfg.summon_hotkey {
            ui.colored_label(
                warn_color(ui),
                "Сочетания совпадают — сработает только конвертация слова.",
            );
        }
        if self.cfg.undo_hotkey == self.cfg.hotkey || self.cfg.undo_hotkey == self.cfg.summon_hotkey {
            ui.colored_label(
                warn_color(ui),
                "Отмена совпадает с другим сочетанием — сработает только отмена.",
            );
        }
        #[cfg(windows)]
        self.admin_section(ui);
        ui.separator();
        ui.heading("Оформление");
        changed |= ui.checkbox(&mut self.cfg.light_theme, "Светлый фон").changed();
        ui.small("Тёмный стиль остаётся по умолчанию. Можно переключить в любой момент.");
        ui.add_space(8.0);
        ui.label("Значок");
        ui.small("Он появится в окне, в трее и в переключателе окон.");
        changed |= self.logo_picker(ui);
        ui.separator();
        ui.add(egui::Label::new(egui::RichText::new(format!("Конфиг: {}", self.path.display())).small()).wrap());
        ui.small(WINDOW_TITLE);
        if changed {
            self.save();
        }
    }

    /// «Запуск администратором» (Windows): see `platform::win_admin`.
    #[cfg(windows)]
    fn admin_section(&mut self, ui: &mut egui::Ui) {
        use crate::platform::win_admin;
        ui.separator();
        // Finished helper (the UAC prompt runs off the UI thread).
        let done = self.admin_job.as_ref().and_then(|(on, r)| r.lock().ok().and_then(|mut g| g.take()).map(|r| (*on, r)));
        if let Some((on, result)) = done {
            self.admin_job = None;
            self.daemon_admin = (None, None);
            match result {
                Ok(()) => {
                    self.cfg.run_as_admin = on;
                    self.save();
                    self.status = if on {
                        "Готово: Typed перезапущен с правами администратора и так будет запускаться при входе в Windows.".into()
                    } else {
                        "Готово: задача удалена. Со следующего входа в Windows Typed запускается как обычно.".into()
                    };
                }
                Err(e) if e == "cancelled" => self.status = "Windows не дал прав администратора — настройка не изменена.".into(),
                Err(e) => self.status = format!("Не получилось изменить запуск администратором: {e}"),
            }
        }
        let busy = self.admin_job.is_some();
        let mut want = self.cfg.run_as_admin;
        let resp = ui.add_enabled(!busy, egui::Checkbox::new(&mut want, "Запуск администратором"));
        if resp.changed() && !busy {
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
            self.status = "Windows спросит разрешение (один раз)…".into();
        }
        ui.small("Нужно, чтобы раскладка переключалась и в программах, запущенных от имени администратора (MobaXterm, PowerShell и т. п.): Windows не даёт обычной программе менять в них раскладку. Typed будет запускаться при входе через Планировщик заданий, без окна с вопросом. Windows спросит разрешение только при включении и выключении.");
        if self.cfg.run_as_admin && !busy {
            let stale = self.daemon_admin.1.map_or(true, |t| t.elapsed() > Duration::from_secs(3));
            if stale {
                self.daemon_admin = (win_admin::other_instance_elevated(), Some(Instant::now()));
            }
            if self.daemon_admin.0 == Some(false) {
                ui.colored_label(warn_color(ui), "Typed сейчас работает без прав администратора — в окнах администратора раскладка не переключится.");
                if ui.button("Перезапустить с правами администратора").clicked() {
                    crate::platform::stop_daemon();
                    match win_admin::run_task() {
                        Ok(()) => self.status = "Перезапускаю Typed с правами администратора…".into(),
                        Err(e) => self.status = format!("Не удалось запустить задачу «{}»: {e}. Снимите и снова поставьте галочку.", win_admin::TASK_NAME),
                    }
                    self.daemon_admin = (None, None);
                }
            }
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let mut visuals = if self.cfg.light_theme {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        let p = crate::logos::palette(self.cfg.logo_style, self.cfg.light_theme);
        visuals.selection.bg_fill = p.accent;
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, p.on_accent);
        visuals.hyperlink_color = p.accent;
        visuals.widgets.hovered.bg_stroke.color = p.accent_hover;
        visuals.widgets.active.bg_stroke.color = p.accent;
        visuals.widgets.active.bg_fill = p.accent_soft;
        visuals.widgets.hovered.weak_bg_fill = p.accent_soft;
        ctx.set_visuals(visuals);
    }

    fn apply_logo(&mut self, ctx: &egui::Context) {
        // On macOS, ViewportCommand::Icon replaces Typed.icns with a raw PNG and the
        // Dock icon loses the system squircle. Set the Dock mark via AppKit instead.
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

    fn tab_strip(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let before = self.tab;
        let r = ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.selectable_value(&mut self.tab, Tab::Layouts, "Раскладки");
            ui.selectable_value(&mut self.tab, Tab::Exceptions, "Исключения");
            ui.selectable_value(&mut self.tab, Tab::Words, "Словарь");
            let t = if self.vault.is_unlocked() {
                "Сейф 🔓"
            } else {
                "Сейф 🔒"
            };
            ui.selectable_value(&mut self.tab, Tab::Vault, t);
            ui.selectable_value(&mut self.tab, Tab::Settings, "Настройки");
            ui.selectable_value(&mut self.tab, Tab::Donate, "💰").on_hover_text("Поддержать Typed");
        })
        .response
        .rect;
        if self.tab != before {
            // A message belongs to the tab that produced it.
            self.status.clear();
        }
        r
    }

    fn logo_picker(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        const TILE: f32 = 64.0;
        const ROUND: f32 = 16.0;
        let spacing = ui.spacing().item_spacing.x;
        let mut total = 0.0_f32;
        for style in LogoStyle::ALL {
            let w = TILE.max(ui_text_width(ui, crate::logos::label(style), egui::TextStyle::Body));
            total += w;
        }
        total += spacing * (LogoStyle::ALL.len().saturating_sub(1) as f32);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - total) * 0.5).max(0.0));
            for style in LogoStyle::ALL {
                let col_w = TILE.max(ui_text_width(ui, crate::logos::label(style), egui::TextStyle::Body));
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, 88.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        let tex = self.logo_texture(ui.ctx(), style);
                        let img = egui::Image::new(&tex)
                            .fit_to_exact_size(egui::vec2(TILE, TILE))
                            .maintain_aspect_ratio(false)
                            .rounding(ROUND)
                            .sense(egui::Sense::click());
                        let response = ui.add(img);
                        if self.cfg.logo_style == style {
                            let gold = crate::logos::palette(style, self.cfg.light_theme).gold;
                            ui.painter().rect_stroke(
                                response.rect,
                                ROUND,
                                egui::Stroke::new(2.0_f32, gold),
                            );
                        }
                        if response.clicked() && self.cfg.logo_style != style {
                            self.cfg.logo_style = style;
                            changed = true;
                            self.apply_logo(ui.ctx());
                        }
                        ui.add(egui::Label::new(crate::logos::label(style)));
                    },
                );
            }
        });
        changed
    }

    fn welcome_panel(&mut self, ui: &mut egui::Ui) -> (egui::Rect, egui::Rect) {
        let tab_rect = self.tab_strip(ui);
        let full = ui.max_rect();
        let heading = egui::Color32::from_rgb(18, 42, 48);
        let body = egui::Color32::from_rgb(36, 58, 64);

        ui.painter().rect_filled(
            egui::Rect::from_min_max(egui::pos2(full.left(), ui.cursor().top()), full.max),
            0.0,
            egui::Color32::WHITE,
        );

        let tex = self.faded_logo_texture(ui.ctx(), self.cfg.logo_style);
        let size = tex.size_vec2();
        if size.x > 0.0 && size.y > 0.0 {
            let logo_w = ui.available_width();
            let logo_h = logo_w * (size.y / size.x);
            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(logo_w, logo_h)));
        }

        ui.add_space(10.0);
        ui.style_mut().visuals.override_text_color = Some(body);
        ui.add(egui::Label::new(egui::RichText::new("Typed").color(heading).size(28.0)).halign(egui::Align::Center));
        ui.add(egui::Label::new(egui::RichText::new(concat!("ver. ", env!("CARGO_PKG_VERSION"))).color(body).small()).halign(egui::Align::Center));
        ui.add_space(8.0);
        ui.add(
            egui::Label::new(egui::RichText::new(
                "Typed запоминает, какая раскладка нужна в каждой программе, и сам исправляет слова, набранные не в той раскладке: «зуфсу» становится «peace».",
            ).color(body))
            .wrap()
            .halign(egui::Align::Center),
        );
        ui.add_space(8.0);
        for line in [
            "• Память раскладки для каждого приложения",
            "• Автоисправление при пробеле, знаке препинания и Enter",
            "• Свой словарь",
            "• Сейф паролей и карт — только на этом компьютере, открывается мастер-паролем",
            "• Ctrl+Shift+V открывает окно сразу на вкладке «Сейф»",
            "• Окно сворачивается в трей",
        ] {
            ui.add(egui::Label::new(egui::RichText::new(line).color(body)).wrap().halign(egui::Align::Center));
        }
        ui.add_space(10.0);
        ui.add(egui::Label::new(egui::RichText::new("Выберите значок: он задаст цвета кнопок и акцентов.").color(body)).wrap().halign(egui::Align::Center));
        ui.add_space(10.0);
        if self.logo_picker(ui) {
            self.save();
        }
        ui.add_space(14.0);
        let btn_w = ui_text_width(ui, "Далее", egui::TextStyle::Button) + ui.spacing().button_padding.x * 2.0;
        let chk_w = ui.spacing().icon_width
            + ui.spacing().icon_spacing
            + ui_text_width(ui, "Не показывать снова", egui::TextStyle::Body);
        let footer_w = btn_w + 12.0 + chk_w;
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - footer_w) * 0.5).max(0.0));
            ui.style_mut().visuals.override_text_color = None;
            let pal = crate::logos::palette(self.cfg.logo_style, true);
            let next = egui::Button::new(egui::RichText::new("Далее").color(pal.on_accent)).fill(pal.accent);
            if ui.add(next).clicked() {
                self.welcome_open = false;
                self.cfg.welcome_gen = layout_keeper_core::config::CURRENT_WELCOME_GEN;
                if self.welcome_dont_show {
                    self.cfg.hide_welcome = true;
                }
                self.save();
            }
            ui.add_space(12.0);
            ui.style_mut().visuals.override_text_color = Some(body);
            ui.checkbox(&mut self.welcome_dont_show, "Не показывать снова");
        });
        ui.add_space(8.0);
        let used = egui::Rect::from_min_max(tab_rect.min, egui::pos2(full.max.x, ui.cursor().top()));
        (tab_rect, used)
    }

    fn apply_window_limits(&mut self, ctx: &egui::Context, tab_rect: egui::Rect, used: egui::Rect) {
        const PAD: f32 = 10.0;
        let origin = ctx.screen_rect().min;
        let inset = (tab_rect.min.x - origin.x).max(0.0);
        let width = (tab_rect.width() + inset * 2.0).max(1.0);
        let min = egui::vec2(width, (tab_rect.bottom() - origin.y).max(1.0));
        let mut target_h = (used.bottom() - origin.y + PAD).max(min.y);
        let (monitor, inner_rect, outer_rect) = ctx.input(|i| {
            (
                i.viewport().monitor_size,
                i.viewport().inner_rect,
                i.viewport().outer_rect,
            )
        });
        if let Some(monitor) = monitor {
            if monitor.y > 80.0 {
                // InnerSize is the client area; subtract the title bar so the outer
                // window stays on the monitor the window is currently on.
                let frame = match (outer_rect, inner_rect) {
                    (Some(outer), Some(inner)) => (outer.height() - inner.height()).max(0.0),
                    _ => 32.0,
                };
                target_h = target_h.min((monitor.y - frame).max(min.y));
            }
        }
        let max = egui::vec2(width, target_h).max(min);
        let limits_changed = self.last_min_size.map(|v| (v - min).length() > 1.0).unwrap_or(true)
            || self.last_max_size.map(|v| (v - max).length() > 1.0).unwrap_or(true);
        if limits_changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(max));
            self.last_min_size = Some(min);
            self.last_max_size = Some(max);
        }

        let current_h = inner_rect.map(|r| r.height()).unwrap_or(0.0);
        let last = self.fitted_height.unwrap_or(f32::NAN);
        if last.is_nan() || (target_h - last).abs() > 1.0 || current_h > target_h + 1.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(max));
            self.fitted_height = Some(target_h);
        }
    }
}

fn warn_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(232, 196, 72)
    } else {
        egui::Color32::from_rgb(148, 90, 0)
    }
}

fn ui_text_width(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> f32 {
    let font_id = style.resolve(ui.style());
    ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font_id, egui::Color32::WHITE).size().x)
}


/// One dictionary word as a compact chip so many words wrap across the window width.
fn word_chip(ui: &mut egui::Ui, word: &str, lang: &str) -> bool {
    chip(ui, word, Some(lang))
}

/// A compact chip: monospace text, optional small tag, trash icon. Returns true when deleted.
fn chip(ui: &mut egui::Ui, word: &str, lang: Option<&str>) -> bool {
    let spacing = ui.spacing().item_spacing.x.max(4.0);
    let color = ui.visuals().text_color();
    let word_font = egui::TextStyle::Monospace.resolve(ui.style());
    let lang_font = egui::TextStyle::Small.resolve(ui.style());
    let word_w = ui.fonts(|f| f.layout_no_wrap(word.to_owned(), word_font, color)).size().x;
    let lang_w = lang.map_or(0.0, |l| ui.fonts(|f| f.layout_no_wrap(l.to_owned(), lang_font, color)).size().x + spacing);
    let icon = 18.0;
    let pad_x = 6.0;
    let width = pad_x + word_w + spacing + lang_w + icon + pad_x;
    let height = ui.spacing().interact_size.y.max(22.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, ui.visuals().faint_bg_color);
    let mut deleted = false;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = spacing;
    child.add_space(pad_x * 0.5);
    child.add(egui::Label::new(egui::RichText::new(word).monospace())).on_hover_text(word);
    if let Some(l) = lang {
        child.small(l);
    }
    if delete_icon_button(&mut child).clicked() {
        deleted = true;
    }
    deleted
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
        }
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        // Only on the transition: a hidden window can keep reporting `minimized`, and
        // re-sending the hide commands every frame woke eframe again and again (≈6% CPU
        // while minimized, 0% while open).
        if can_hide && minimized && self.ui_visible {
            hide_window(ctx);
            self.ui_visible = false;
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
        self.apply_theme(ctx);
        ctx.request_repaint_after(Duration::from_millis(200));
        if let Some(slot) = self.capturing_hotkey {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.capturing_hotkey = None;
                self.status = "Назначение сочетания отменено.".into();
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
        let focus = self.focus.lock().map(|f| f.clone()).unwrap_or_default();
        self.vault.tick(ctx, self.cfg.vault_idle_minutes);

        let (tab_rect, used) = if self.welcome_open {
            egui::CentralPanel::default().show(ctx, |ui| self.welcome_panel(ui)).inner
        } else {
            let inner = egui::CentralPanel::default().show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let tab_rect = self.tab_strip(ui);
                if !platform::accessibility_trusted() {
                    ui.colored_label(
                        warn_color(ui),
                        "Нет разрешения «Универсальный доступ» — без него не работают автозамена и горячие клавиши.",
                    );
                    if ui.button("Открыть настройки доступа").clicked() {
                        platform::open_accessibility_settings();
                    }
                    ui.add_space(6.0);
                }

                let scroll = egui::ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.set_max_width(tab_rect.width());
                        match self.tab {
                            Tab::Vault => {
                                if self.vault.show(ui, focus.app.clone(), &mut self.cfg.vault_idle_minutes) {
                                    self.save();
                                }
                            }
                            Tab::Words => self.words_tab(ui),
                            Tab::Exceptions => self.exceptions_tab(ui, &focus),
                            Tab::Layouts => self.layouts_tab(ui, &focus),
                            Tab::Settings => self.settings_tab(ui),
                            Tab::Donate => self.donate_tab(ui),
                        }
                        ui.add_space(6.0);
                        if !self.status.is_empty() {
                            ui.add(egui::Label::new(&self.status).wrap());
                        }
                    });

                let used = egui::Rect::from_min_max(
                    tab_rect.min.min(scroll.inner_rect.min),
                    egui::pos2(
                        tab_rect.right().max(scroll.inner_rect.min.x + scroll.content_size.x),
                        tab_rect.bottom().max(scroll.inner_rect.min.y + scroll.content_size.y),
                    ),
                );
                (tab_rect, used)
            });
            inner.inner
        };
        self.apply_window_limits(ctx, tab_rect, used);
    }
}

pub fn run(open_vault: bool, start_hidden: bool) -> Result<(), String> {
    #[cfg(windows)]
    let _gui_mutex = match platform::acquire_gui_mutex() {
        Ok(None) => {
            if open_vault {
                platform::request_open_vault();
            } else {
                platform::request_show_gui();
            }
            platform::raise_settings_window();
            return Ok(());
        }
        Ok(Some(guard)) => Some(guard),
        Err(e) => {
            log::warn!("cannot take GUI single-instance lock: {e}");
            None
        }
    };
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
            "Не удалось запустить демон. Запустите layout-keeper без параметров.".into()
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
        manual_id: String::new(),
        manual_lang: Lang::Ru,
        exception_draft: String::new(),
        tab: if open_vault { Tab::Vault } else { Tab::Layouts },
        vault: crate::vault_ui::VaultUi::new(),
        capturing_hotkey: None,
        word_draft: String::new(),
        expanded_paths: HashSet::new(),
        show_system_apps: false,
        icons: IconCache::default(),
        fitted_height: None,
        last_min_size: None,
        last_max_size: None,
        welcome_open,
        welcome_dont_show: false,
        logo_textures: [None, None, None],
        logo_fade_textures: [None, None, None],
        admin_job: None,
        daemon_admin: (None, None),
        tray: crate::tray::start(logo_index),
        allow_close: false,
        ui_visible: !start_hidden,
        chrome_ready: false,
        last_idle_frame: Instant::now(),
        idle_frames: 0,
    };
    let viewport = {
        let mut vp = egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size([480.0, 360.0]);
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
    eframe::run_native(APP_NAME, opts, Box::new(|_cc| Ok(Box::new(app)))).map_err(|e| e.to_string())
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
mod qr_tests {
    #[test]
    fn donation_addresses_make_qr_codes() {
        for a in [super::USDT_TRC20, super::GRAM_TON] {
            let img = super::qr_image(a);
            assert!(img.size[0] >= 25 && img.size[0] == img.size[1]);
        }
    }
}
