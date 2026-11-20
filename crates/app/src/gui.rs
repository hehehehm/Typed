//! Settings window (`typed gui`). A separate process that only edits the same
//! `config.json` the daemon watches; the daemon picks changes up within ~1 s.
//! The keyboard hook is never touched from here.

use crate::brand::{APP_NAME, WINDOW_TITLE};
use crate::platform;
use eframe::egui;
use layout_keeper_core::{typo::Level as TypoLevel, Config, Hotkey, HotkeyCode, Lang, LogoStyle};
use crate::icons::{basename, delete_icon_button, has_path, hide_icon_button, is_hidden_app, IconCache};
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
    /// Settings groups opened in this session (all closed at start).
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

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    /// The five squares.
    Home,
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

    /// Remembered apps: one flat row each — name (cut to fit), EN/RU tiles, hide, trash.
    fn apps_list(&mut self, ui: &mut egui::Ui, apps: &[(String, Lang)]) -> bool {
        let mut changed = false;
        let accent = Tab::Layouts.color();
        let t = st::cur(ui);
        ui.spacing_mut().item_spacing.y = 2.0;
        for (id, cur) in apps {
            let mut act: Option<AppAction> = None;
            let cfg = &self.cfg;
            let icons = &mut self.icons;
            let expanded = &mut self.expanded_paths;
            st::row(
                ui,
                t.surface,
                |ui| app_label(ui, icons, expanded, id),
                |ui| {
                    if delete_icon_button(ui).clicked() {
                        act = Some(AppAction::Remove);
                    }
                    // Hide into the collapsed group / bring back. Built-in system apps stay
                    // there without a button. Visual only: the layout is still applied.
                    let user_hidden = cfg.is_user_hidden(id);
                    if (user_hidden || !is_hidden_app(id)) && hide_icon_button(ui, !user_hidden).clicked() {
                        act = Some(AppAction::Hide(!user_hidden));
                    }
                    ui.add_space(4.0);
                    for l in [Lang::Ru, Lang::En] {
                        if st::choice_tile(ui, *cur == l, lang_name(l), 36.0, accent).clicked() && *cur != l {
                            act = Some(AppAction::Set(l));
                        }
                    }
                },
            );
            match act {
                Some(AppAction::Remove) => {
                    self.cfg.remove_app(id);
                    changed = true;
                }
                Some(AppAction::Hide(h)) => {
                    self.cfg.set_user_hidden(id, h);
                    changed = true;
                }
                Some(AppAction::Set(l)) => {
                    self.cfg.set_layout(id, l);
                    changed = true;
                }
                None => {}
            }
        }
        ui.spacing_mut().item_spacing.y = st::GAP;
        ui.add_space(4.0);
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
        let accent = Tab::Words.color();
        let mut changed = false;
        st::hint(ui, "Сокращения, жаргон, названия. Автозамена знает их и ставит выше встроенного словаря. Пример: слово «exe» заменит русское «учу». Можно с цифрами и знаками (mp3, c++, ci/cd), но без пробелов. Однобуквенные — в «Настройки → Исправления».");
        st::block(ui, |ui| {
            if st::add_line(ui, &mut self.word_draft, "новое слово", "Добавить", accent) {
                changed |= self.add_custom_word();
            }
        });
        ui.add_space(4.0);
        let words = self.cfg.custom_words.clone();
        st::caption(ui, &format!("Свои слова · {}", words.len()));
        if words.is_empty() {
            st::hint(ui, "Пока пусто.");
        }
        let mut remove_at = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
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

        ui.add_space(8.0);
        let never = self.cfg.never_correct.clone();
        st::caption(ui, &format!("Не исправлять · {}", never.len()));
        st::hint(ui, "Слова, исправление которых вы отменили (сочетанием отмены, дважды Backspace или стерев и набрав слово заново). Typed их больше не трогает. Корзина — снова разрешить исправление.");
        if let Some(i) = chips(ui, &never) {
            if i < self.cfg.never_correct.len() {
                self.cfg.never_correct.remove(i);
                changed = true;
            }
        }
        ui.add_space(8.0);
        let never_typo = self.cfg.never_typo_fix.clone();
        st::caption(ui, &format!("Не исправлять опечатки · {}", never_typo.len()));
        st::hint(ui, "Слова, у которых вы дважды отменили только исправление опечатки. Раскладку в них Typed по-прежнему исправляет.");
        if let Some(i) = chips(ui, &never_typo) {
            if i < self.cfg.never_typo_fix.len() {
                self.cfg.never_typo_fix.remove(i);
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
        let accent = Tab::Settings.color();
        let current = match slot {
            HotkeySlot::Convert => self.cfg.hotkey,
            HotkeySlot::Summon => self.cfg.summon_hotkey,
            HotkeySlot::Undo => self.cfg.undo_hotkey,
        };
        let t = st::cur(ui);
        st::block(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal(|ui| {
                let capturing = self.capturing_hotkey == Some(slot);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if capturing {
                        st::primary(ui, "Нажмите клавиши… Esc — отмена", accent);
                    } else if st::secondary(ui, "Назначить").clicked() {
                        self.capturing_hotkey = Some(slot);
                        self.status = "Нажмите нужное сочетание клавиш.".into();
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(egui::RichText::new(label).color(t.ink)).truncate());
                            ui.add(egui::Label::new(egui::RichText::new(current.to_string()).monospace().color(t.muted)).truncate());
                        });
                    });
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
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

    fn exceptions_tab(&mut self, ui: &mut egui::Ui, focus: &Focus) {
        let accent = Tab::Exceptions.color();
        let t = st::cur(ui);
        st::hint(ui, "Здесь автозамена и клавиша конвертации не работают; назначенная раскладка по-прежнему ставится. Совпадение по фрагменту имени, без регистра: powershell, windowsterminal.");
        let mut changed = false;
        // The add line comes first, so no list length or entry width can push it away.
        st::block(ui, |ui| {
            if st::add_line(ui, &mut self.exception_draft, "фрагмент имени или путь", "Добавить", accent) {
                changed |= self.add_exception_entry(self.exception_draft.clone());
            }
            if let Some(a) = focus.app.clone() {
                let already = self.cfg.is_exception(&a);
                let mut add_current = false;
                st::row(
                    ui,
                    egui::Color32::TRANSPARENT,
                    |ui| {
                        ui.label(egui::RichText::new("Текущее:").color(t.muted));
                        ui.add(egui::Label::new(egui::RichText::new(basename(&a)).monospace()).truncate()).on_hover_text(&a);
                    },
                    |ui| {
                        if already {
                            ui.label(egui::RichText::new("уже в списке").color(t.warn));
                        } else if st::secondary(ui, "Добавить текущее").clicked() {
                            add_current = true;
                        }
                    },
                );
                if add_current {
                    changed |= self.add_exception_entry(a);
                }
            }
        });
        ui.add_space(4.0);
        let items = self.cfg.exception_apps.clone();
        st::caption(ui, &format!("Список · {}", items.len()));
        if items.is_empty() {
            st::hint(ui, "Список пуст — автозамена работает везде.");
        }
        let mut remove_at: Option<usize> = None;
        ui.spacing_mut().item_spacing.y = 2.0;
        for (i, item) in items.iter().enumerate() {
            st::row(
                ui,
                t.surface,
                |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(item).monospace()).truncate()).on_hover_text(item);
                },
                |ui| {
                    if delete_icon_button(ui).clicked() {
                        remove_at = Some(i);
                    }
                },
            );
        }
        ui.spacing_mut().item_spacing.y = st::GAP;
        if let Some(i) = remove_at {
            if i < self.cfg.exception_apps.len() {
                self.cfg.exception_apps.remove(i);
                changed = true;
            }
        }
        if changed {
            self.save();
        }
    }

    fn layouts_tab(&mut self, ui: &mut egui::Ui, focus: &Focus) {
        let accent = Tab::Layouts.color();
        let t = st::cur(ui);
        st::caption(ui, "Текущее приложение");
        st::block(ui, |ui| match focus.app.clone() {
            Some(a) => {
                ui.horizontal(|ui| {
                    ui.set_max_width(ui.available_width());
                    app_label(ui, &mut self.icons, &mut self.expanded_paths, &a);
                });
                let assigned = self.cfg.layout_for(&a);
                ui.label(
                    egui::RichText::new(format!(
                        "Сейчас {} · назначено {}",
                        focus.layout.map(lang_name).unwrap_or("?"),
                        assigned.map(lang_name).unwrap_or("—"),
                    ))
                    .color(t.muted),
                );
                if self.cfg.is_exception(&a) {
                    st::warn(ui, "В исключениях: автозамена выключена, раскладка ставится как обычно.");
                }
                let mut sel = assigned;
                if st::segmented(ui, &mut sel, &[(Some(Lang::En), "Назначить EN"), (Some(Lang::Ru), "Назначить RU")], accent) {
                    if let Some(l) = sel {
                        self.cfg.set_layout(&a, l);
                        self.save();
                    }
                }
            }
            None => st::hint(ui, "Переключитесь в нужную программу и вернитесь сюда — она появится здесь и останется в списке."),
        });

        ui.add_space(4.0);
        let (mut hidden_apps, mut apps): (Vec<(String, Lang)>, Vec<(String, Lang)>) = self
            .cfg
            .apps
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .partition(|(id, _)| is_hidden_app(id) || self.cfg.is_user_hidden(id));
        self.sort_apps(&mut apps);
        self.sort_apps(&mut hidden_apps);
        st::caption(ui, &format!("Запомненные · {}", apps.len()));
        st::hint(ui, "Раскладка включается при каждом входе в приложение. Ручное переключение её не меняет.");
        if apps.is_empty() {
            st::hint(ui, "Пока пусто.");
        }
        let mut changed = self.apps_list(ui, &apps);
        if !hidden_apps.is_empty() {
            if st::group_header(ui, self.show_system_apps, "Скрытые и системные", &hidden_apps.len().to_string(), accent)
                .on_hover_text("Системные программы, установщики и те, что вы скрыли. Раскладка для них всё равно ставится.")
                .clicked()
            {
                self.show_system_apps = !self.show_system_apps;
            }
            if self.show_system_apps {
                changed |= self.apps_list(ui, &hidden_apps);
            }
        }
        ui.add_space(4.0);
        st::caption(ui, "Добавить вручную");
        st::block(ui, |ui| {
            let w = ui.available_width();
            st::field_bg(ui, &mut self.manual_id, "имя программы или путь", w, accent);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for l in [Lang::En, Lang::Ru] {
                    if st::choice_tile(ui, self.manual_lang == l, lang_name(l), 44.0, accent).clicked() {
                        self.manual_lang = l;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st::primary(ui, "Добавить", accent).clicked() && !self.manual_id.trim().is_empty() {
                        let id = std::mem::take(&mut self.manual_id);
                        self.cfg.set_layout(id.trim(), self.manual_lang);
                        changed = true;
                    }
                });
            });
        });

        if changed {
            self.save();
        }
    }

    fn donate_tab(&mut self, ui: &mut egui::Ui) {
        st::hint(ui, "Typed бесплатный и ничего не отправляет в сеть. Если он вам помогает — можно поддержать.");
        st::block(ui, |ui| {
            ui.label("Boosty");
            if st::primary(ui, "Открыть Boosty", Tab::Donate.color()).on_hover_text(BOOSTY_URL).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(BOOSTY_URL));
            }
        });
        self.address_block(ui, "USDT (TRC-20)", USDT_TRC20, "Только сеть TRON (TRC-20). USDT из другой сети (ERC-20, BEP-20) потеряется.");
        self.address_block(ui, "GRAM (TON)", GRAM_TON, "Только сеть TON. Из другой сети перевод потеряется.");
    }

    fn address_block(&mut self, ui: &mut egui::Ui, title: &str, addr: &'static str, note: &str) {
        let tex = self
            .qr
            .entry(addr)
            .or_insert_with(|| ui.ctx().load_texture(format!("qr-{addr}"), qr_image(addr), egui::TextureOptions::NEAREST))
            .clone();
        st::block(ui, |ui| {
            ui.label(title);
            ui.horizontal_top(|ui| {
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(104.0, 104.0)));
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    ui.add(egui::Label::new(egui::RichText::new(addr).monospace().small()).wrap_mode(egui::TextWrapMode::Wrap));
                    if st::secondary(ui, "Скопировать адрес").clicked() {
                        ui.ctx().copy_text(addr.to_owned());
                        self.status = format!("Адрес {title} скопирован.");
                    }
                    st::hint(ui, note);
                });
            });
        });
    }

    /// Collapsible group of the settings section. Closed until opened (state kept
    /// for the session only).
    fn group(&mut self, ui: &mut egui::Ui, key: &'static str, title: &str, summary: &str, body: impl FnOnce(&mut Self, &mut egui::Ui)) {
        let open = self.open_groups.contains(key);
        if open {
            ui.spacing_mut().item_spacing.y = 2.0;
        }
        if st::group_header(ui, open, title, summary, Tab::Settings.color()).clicked() {
            if open {
                self.open_groups.remove(key);
            } else {
                self.open_groups.insert(key);
            }
        }
        ui.spacing_mut().item_spacing.y = st::GAP;
        if self.open_groups.contains(key) {
            ui.spacing_mut().item_spacing.y = 2.0;
            body(self, ui);
            ui.spacing_mut().item_spacing.y = st::GAP;
            ui.add_space(6.0);
        }
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let accent = Tab::Settings.color();
        let mut changed = false;
        changed |= st::toggle_row(
            ui,
            &mut self.cfg.autocorrect,
            "Автоисправление",
            "Слово, набранное не в той раскладке, исправляется при пробеле, знаке препинания и Enter: зуфсу → peace.",
            accent,
        );
        ui.add_space(4.0);
        st::caption(ui, "Подробнее");

        let on = [self.cfg.convert_single_letters, self.cfg.fix_missing_spaces, self.cfg.auto_learn].iter().filter(|b| **b).count();
        let mut c = false;
        self.group(ui, "fix", "Исправления", &format!("{on} из 3 включено"), |s, ui| {
            c |= st::toggle_row(ui, &mut s.cfg.convert_single_letters, "Исправлять отдельные буквы", "Буква, которая сама по себе слово только в другом языке, переводится: d → в, z → я, ф → a, ш → i. Настоящие a, I, в, и, я остаются. Остальные (q/й) — в язык приложения.", accent);
            c |= st::toggle_row(ui, &mut s.cfg.fix_missing_spaces, "Расставлять пропущенные пробелы", "После предлогов и «не»: снейросеткой → с нейросеткой, вофисе → в офисе, незнаю → не знаю. Только когда слова нет в словарях. Отменяется как исправление.", accent);
            c |= st::toggle_row(ui, &mut s.cfg.auto_learn, "Автозапоминание раскладки", "Запоминает раскладку приложения, только если её ещё нет. Назначенную не меняет.", accent);
        });

        let typo_summary = if self.cfg.fix_typos {
            format!("{}{}", level_name(self.cfg.typo_level), if self.cfg.use_context { " · с контекстом" } else { "" })
        } else {
            "выключено".to_string()
        };
        self.group(ui, "typo", "Опечатки", &typo_summary, |s, ui| {
            c |= st::toggle_row(ui, &mut s.cfg.fix_typos, "Исправлять опечатки", "поечаток → опечаток", accent);
            ui.add_enabled_ui(s.cfg.fix_typos, |ui| {
                st::block(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.label("Смелость");
                    c |= st::segmented(
                        ui,
                        &mut s.cfg.typo_level,
                        &[
                            (TypoLevel::Cautious, level_name(TypoLevel::Cautious)),
                            (TypoLevel::Normal, level_name(TypoLevel::Normal)),
                            (TypoLevel::Aggressive, level_name(TypoLevel::Aggressive)),
                        ],
                        accent,
                    );
                    st::hint(ui, match s.cfg.typo_level {
                        TypoLevel::Cautious => "Как в 0.3.71: одна ошибка — в словах от 4 букв, две — от 8 букв, только когда вариант один и частый.",
                        TypoLevel::Normal => "Одна ошибка — от 3 букв, две — от 5, три — от 9; раскладка вместе с опечаткой (xnjj → что). Если два варианта близки — не трогает.",
                        TypoLevel::Aggressive => "Как «Обычно», но больше ошибок на слово и смелее при близких вариантах. Чаще ошибается — отменяйте Backspace.",
                    });
                });
                c |= st::toggle_row(ui, &mut s.cfg.use_context, "Учитывать соседние слова", "Смотрит на одно-два предыдущих слова предложения (частоты пар и троек слов): выбирает между близкими вариантами (я бы хотл → хотел) и не трогает слово, которое к ним подходит (Нельсон Мандела). Сбрасывается при щелчке, Enter, смене окна.", accent);
            });
        });

        let undo_summary = if self.cfg.undo_on_backspace { "Backspace отменяет" } else { "только сочетание" };
        self.group(ui, "undo", "Отмена исправлений", undo_summary, |s, ui| {
            c |= st::toggle_row(ui, &mut s.cfg.undo_on_backspace, "Backspace сразу после исправления отменяет его", "Только сразу после исправления, пока не нажато ничего другого. Слово возвращается как было; отменили одно и то же дважды — оно попадает в «Не исправлять».", accent);
            st::block(ui, |ui| {
                st::hint(ui, "Раскладка с опечаткой: первый Backspace убирает только исправление опечатки, второй возвращает набранное. Сочетание отмены (в «Горячих клавишах») — сразу всё; нажатое ещё раз — отменяет предыдущее исправленное слово.");
            });
        });

        let hk_summary = self.cfg.hotkey.to_string();
        self.group(ui, "keys", "Горячие клавиши", &hk_summary, |s, ui| {
            st::block(ui, |ui| st::hint(ui, "Удобно начинать с Ctrl+Shift. Букву без Ctrl, Alt или Win назначить нельзя."));
            s.hotkey_picker(
                ui,
                HotkeySlot::Convert,
                "Конвертация слова",
                &[
                    (Hotkey::Pause, "Pause"),
                    (Hotkey::ShiftShift, "Shift+Shift"),
                    (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space"),
                ],
            );
            s.hotkey_picker(
                ui,
                HotkeySlot::Summon,
                "Вызов окна",
                &[
                    (Hotkey::CTRL_SHIFT_V, "Ctrl+Shift+V"),
                    (Hotkey::CTRL_SHIFT_K, "Ctrl+Shift+K"),
                    (Hotkey::CtrlAltSpace, "Ctrl+Alt+Space"),
                ],
            );
            s.hotkey_picker(
                ui,
                HotkeySlot::Undo,
                "Отмена автоисправления",
                &[
                    (Hotkey::CTRL_ALT_Z, if cfg!(target_os = "macos") { "Ctrl+Option+Z" } else { "Ctrl+Alt+Z" }),
                    (Hotkey::CTRL_ALT_BACKSPACE, if cfg!(target_os = "macos") { "Ctrl+Option+Delete" } else { "Ctrl+Alt+Backspace" }),
                    (Hotkey::SHIFT_PAUSE, "Shift+Pause"),
                ],
            );
            st::block(ui, |ui| {
                st::hint(ui, "Отмена возвращает слово как было набрано и больше никогда его не исправляет (список — в разделе «Словарь»).");
                if s.cfg.hotkey == s.cfg.summon_hotkey {
                    st::warn(ui, "Сочетания совпадают — сработает только конвертация слова.");
                }
                if s.cfg.undo_hotkey == s.cfg.hotkey || s.cfg.undo_hotkey == s.cfg.summon_hotkey {
                    st::warn(ui, "Отмена совпадает с другим сочетанием — сработает только отмена.");
                }
            });
        });

        #[cfg(windows)]
        {
            let admin_summary = if self.cfg.run_as_admin { "администратором" } else { "обычный" };
            self.group(ui, "admin", "Запуск", admin_summary, |s, ui| s.admin_section(ui));
        }

        let look = if self.cfg.light_theme { "светлая тема" } else { "тёмная тема" };
        self.group(ui, "look", "Оформление", look, |s, ui| {
            c |= st::toggle_row(ui, &mut s.cfg.light_theme, "Светлая тема", "Можно переключить в любой момент.", accent);
            st::block(ui, |ui| {
                ui.label("Значок");
                st::hint(ui, "Он появится в окне, в трее и в переключателе окон.");
                c |= s.logo_picker(ui);
            });
        });

        self.group(ui, "about", "О программе", concat!("ver. ", env!("CARGO_PKG_VERSION")), |s, ui| {
            st::block(ui, |ui| {
                if st::secondary(ui, "Показать приветственный экран").clicked() {
                    // Shown now; «Не показывать снова» keeps its saved state until «Далее».
                    s.welcome_dont_show = s.cfg.hide_welcome;
                    s.welcome_open = true;
                }
                st::hint(ui, &format!("Конфиг: {}", s.path.display()));
                st::hint(ui, WINDOW_TITLE);
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
        // Finished helper (the UAC prompt runs off the UI thread).
        let done = self.admin_job.as_ref().and_then(|(on, r)| r.lock().ok().and_then(|mut g| g.take()).map(|r| (*on, r)));
        if let Some((on, result)) = done {
            self.admin_job = None;
            self.daemon_admin = (None, true, None);
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
        let toggled = ui
            .add_enabled_ui(!busy, |ui| {
                st::toggle_row(
                    ui,
                    &mut want,
                    "Запуск администратором",
                    "Нужно, чтобы раскладка переключалась и в программах, запущенных от имени администратора (MobaXterm, PowerShell и т. п.): Windows не даёт обычной программе менять в них раскладку. Typed будет запускаться при входе через Планировщик заданий, без окна с вопросом. Windows спросит разрешение только при включении и выключении.",
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
            self.status = "Windows спросит разрешение (один раз)…".into();
        }
        if self.cfg.run_as_admin && !busy {
            let stale = self.daemon_admin.2.map_or(true, |t| t.elapsed() > Duration::from_secs(5));
            if stale {
                self.daemon_admin = (win_admin::other_instance_elevated(), win_admin::task_points_here(), Some(Instant::now()));
            }
            if !self.daemon_admin.1 {
                // No task, or it still starts the pre-0.3.68 layout-keeper.exe.
                let mut refresh = false;
                st::block(ui, |ui| {
                    st::warn(ui, "Задача запуска администратором устарела или удалена.");
                    refresh = st::primary(ui, "Обновить задачу", accent).clicked();
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
                    self.status = "Windows спросит разрешение (один раз)…".into();
                }
            } else if self.daemon_admin.0 == Some(false) {
                let mut restart = false;
                st::block(ui, |ui| {
                    st::warn(ui, "Typed сейчас работает без прав администратора — в окнах администратора раскладка не переключится.");
                    restart = st::primary(ui, "Перезапустить с правами администратора", accent).clicked();
                });
                if restart {
                    crate::platform::stop_daemon();
                    match win_admin::run_task() {
                        Ok(()) => self.status = "Перезапускаю Typed с правами администратора…".into(),
                        Err(e) => self.status = format!("Не удалось запустить задачу «{}»: {e}. Выключите и снова включите запуск администратором.", win_admin::TASK_NAME),
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

    fn go(&mut self, tab: Tab) {
        if self.tab != tab {
            // A message belongs to the section that produced it.
            self.status.clear();
            self.tab = tab;
        }
    }

    /// Short live status shown on a square and under the section title.
    fn section_status(&self, tab: Tab) -> String {
        match tab {
            Tab::Home => String::new(),
            Tab::Layouts => count(self.cfg.apps.len(), ["программа", "программы", "программ"]),
            Tab::Exceptions => match self.cfg.exception_apps.len() {
                0 => "нет".into(),
                n => count(n, ["исключение", "исключения", "исключений"]),
            },
            Tab::Words => count(self.cfg.custom_words.len(), ["слово", "слова", "слов"]),
            Tab::Vault => self.vault.state_text().into(),
            Tab::Settings => {
                if !self.cfg.autocorrect {
                    "автоисправление выкл.".into()
                } else if !self.cfg.fix_typos {
                    "опечатки выкл.".into()
                } else {
                    format!("смелость: {}", level_name(self.cfg.typo_level).to_lowercase())
                }
            }
            Tab::Donate => "Boosty, USDT, GRAM".into(),
        }
    }

    /// Home: five colour squares, one per section, a little uneven like paper cut-outs.
    fn home(&mut self, ui: &mut egui::Ui) {
        let t = st::cur(ui);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(APP_NAME).size(26.0).color(t.ink));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(concat!("ver. ", env!("CARGO_PKG_VERSION"))).small().color(t.muted));
            });
        });
        ui.add_space(10.0);
        let full = ui.available_width();
        let g = 10.0;
        let unit = ((full - 2.0 * g) / 3.0).max(60.0);
        // Row of three, then two shifted right: sides and offsets are a little uneven on
        // purpose, like the reference cut-outs. Everything scales with the window width.
        let (s0, s1, s2) = (unit * 1.06, unit * 0.94, unit);
        let y2 = s0 + g + 6.0;
        let (s3, s4) = (unit * 1.08, unit);
        let x3 = unit * 0.42;
        let layout: [(Tab, f32, f32, f32); 5] = [
            (Tab::Layouts, 0.0, 0.0, s0),
            (Tab::Exceptions, s0 + g, 14.0, s1),
            (Tab::Words, s0 + s1 + 2.0 * g, 4.0, s2),
            (Tab::Vault, x3, y2, s3),
            (Tab::Settings, x3 + s3 + g, y2 + 10.0, s4),
        ];
        let origin = ui.cursor().min;
        let mut bottom = origin.y;
        let mut clicked = None;
        for (i, &(tab, x, y, s)) in layout.iter().enumerate() {
            let (x, y) = (origin.x + x, origin.y + y);
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(s, s));
            bottom = bottom.max(rect.bottom());
            let resp = ui.interact(rect, ui.id().with(("square", i)), egui::Sense::click());
            let lift = if resp.hovered() { -3.0 } else { 0.0 };
            let r = rect.translate(egui::vec2(0.0, lift));
            let color = tab.color();
            let p = ui.painter();
            p.rect_filled(r, 0.0, if resp.is_pointer_button_down_on() { st::shade(color, -0.1) } else { color });
            let pad = if s < 120.0 { 10.0 } else { 12.0 };
            let status = self.section_status(tab);
            let sg = st::line_galley(ui, &status, st::font(12.0), st::INK_ON_COLOR.gamma_multiply(0.75), s - pad * 2.0);
            p.galley(egui::pos2(r.left() + pad, r.top() + pad), sg, st::INK_ON_COLOR);
            // The title shrinks a little rather than being cut on a narrow window.
            let natural = st::text_w(ui, tab.title(), st::font(16.0));
            let size = if natural > s - pad * 2.0 { (16.0 * (s - pad * 2.0) / natural).max(12.0) } else { 16.0 };
            let lg = st::line_galley(ui, tab.title(), st::font(size), st::INK_ON_COLOR, s - pad * 2.0);
            p.galley(egui::pos2(r.left() + pad, r.bottom() - pad - lg.size().y), lg, st::INK_ON_COLOR);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                clicked = Some(tab);
            }
        }
        ui.allocate_rect(egui::Rect::from_min_max(origin, egui::pos2(origin.x + full, bottom)), egui::Sense::hover());
        // The support link sits at the bottom edge of the window (or right below the
        // squares when the window is short).
        let room = ui.clip_rect().bottom() - st::PAD - 20.0 - ui.cursor().top();
        ui.add_space(room.max(18.0));
        // Donations: a small square, not a sixth section.
        let label = "Поддержать Typed";
        let w = 14.0 + 8.0 + st::text_w(ui, label, st::font(13.0));
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::click());
        let p = ui.painter();
        let sq = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 7.0), egui::vec2(14.0, 14.0));
        p.rect_filled(sq, 0.0, if resp.hovered() { st::shade(st::SAND, -0.15) } else { st::SAND });
        p.text(egui::pos2(sq.right() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, label, st::font(13.0), if resp.hovered() { t.ink } else { t.muted });
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            clicked = Some(Tab::Donate);
        }
        if let Some(tab) = clicked {
            self.go(tab);
        }
    }

    /// Section header: a full-width band in the section colour, a back square, the
    /// title with its status, and the five squares as a small switcher on the right.
    fn section_header(&mut self, ui: &mut egui::Ui) {
        let tab = self.tab;
        let color = tab.color();
        let mut target = None;
        ui.horizontal(|ui| {
            ui.set_min_height(40.0);
            if st::back_square(ui, color).clicked() {
                target = Some(Tab::Home);
            }
            ui.add_space(4.0);
            let nav_w = 5.0 * 14.0 + 4.0 * 5.0;
            let title_w = (ui.available_width() - nav_w - 12.0).max(60.0);
            ui.vertical(|ui| {
                ui.set_width(title_w);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add(egui::Label::new(egui::RichText::new(tab.title()).size(20.0).color(st::INK_ON_COLOR)).truncate());
                let s = self.section_status(tab);
                ui.add(egui::Label::new(egui::RichText::new(s).size(12.0).color(st::INK_ON_COLOR.gamma_multiply(0.75))).truncate());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                for &other in SECTIONS.iter().rev() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                    let p = ui.painter();
                    let fill = if other == tab { st::INK_ON_COLOR } else { other.color() };
                    p.rect_filled(rect, 0.0, fill);
                    if other != tab {
                        p.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, st::shade(color, -0.2)));
                    }
                    let resp = resp.on_hover_text(other.title());
                    if resp.clicked() {
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
        const TILE: f32 = 56.0;
        let spacing = 12.0;
        let col = |ui: &egui::Ui, style| TILE.max(st::text_w(ui, crate::logos::label(style), st::font(13.0)));
        let total: f32 = LogoStyle::ALL.iter().map(|&s| col(ui, s)).sum::<f32>() + spacing * (LogoStyle::ALL.len().saturating_sub(1) as f32);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            ui.add_space(((ui.available_width() - total) * 0.5).max(0.0));
            for style in LogoStyle::ALL {
                let col_w = col(ui, style);
                ui.allocate_ui_with_layout(egui::vec2(col_w, 84.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let tex = self.logo_texture(ui.ctx(), style);
                    let img = egui::Image::new(&tex)
                        .fit_to_exact_size(egui::vec2(TILE, TILE))
                        .maintain_aspect_ratio(false)
                        .rounding(12.0)
                        .sense(egui::Sense::click());
                    let response = ui.add(img);
                    let selected = self.cfg.logo_style == style;
                    if selected {
                        // Flat marker: a bar under the chosen icon.
                        let bar = egui::Rect::from_min_size(egui::pos2(response.rect.left(), response.rect.bottom() + 2.0), egui::vec2(TILE, 3.0));
                        ui.painter().rect_filled(bar, 0.0, st::cur(ui).ink);
                    }
                    if response.clicked() && !selected {
                        self.cfg.logo_style = style;
                        changed = true;
                        self.apply_logo(ui.ctx());
                    }
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(crate::logos::label(style)).size(13.0));
                });
            }
        });
        changed
    }

    fn welcome_panel(&mut self, ui: &mut egui::Ui) {
        let heading = egui::Color32::from_rgb(18, 42, 48);
        let body = egui::Color32::from_rgb(36, 58, 64);
        // The welcome page is always light: the faded logo is drawn on white.
        ui.visuals_mut().override_text_color = Some(body);
        ui.visuals_mut().dark_mode = false;
        ui.vertical_centered(|ui| {
            let tex = self.faded_logo_texture(ui.ctx(), self.cfg.logo_style);
            let size = tex.size_vec2();
            if size.x > 0.0 && size.y > 0.0 {
                let logo_w = ui.available_width().min(300.0);
                let logo_h = logo_w * (size.y / size.x);
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(logo_w, logo_h)));
            }
            // The five section colours as a small strip.
            ui.add_space(4.0);
            let (strip, _) = ui.allocate_exact_size(egui::vec2(5.0 * 14.0 + 4.0 * 6.0, 14.0), egui::Sense::hover());
            for (i, tab) in SECTIONS.iter().enumerate() {
                let x = strip.left() + i as f32 * 20.0;
                ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, strip.top()), egui::vec2(14.0, 14.0)), 0.0, tab.color());
            }
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Typed").color(heading).size(28.0));
            ui.label(egui::RichText::new(concat!("ver. ", env!("CARGO_PKG_VERSION"))).color(body).small());
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(egui::RichText::new(
                    "Typed запоминает, какая раскладка нужна в каждой программе, и сам исправляет слова, набранные не в той раскладке: «зуфсу» становится «peace».",
                ).color(body))
                .wrap(),
            );
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            for line in [
                "Память раскладки для каждого приложения",
                "Автоисправление при пробеле, знаке препинания и Enter",
                "Свой словарь",
                "Сейф паролей и карт — только на этом компьютере, открывается мастер-паролем",
                "Ctrl+Shift+V открывает окно сразу в разделе «Сейф»",
                "Окно сворачивается в трей",
            ] {
                ui.add(egui::Label::new(egui::RichText::new(line).color(body)).wrap());
            }
            ui.spacing_mut().item_spacing.y = st::GAP;
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Выберите значок окна и трея:").color(body));
            if self.logo_picker(ui) {
                self.save();
            }
            ui.add_space(10.0);
        });
        let next_w = st::text_w(ui, "Далее", st::font(14.0)) + 24.0;
        let chk_w = 26.0 + st::text_w(ui, "Не показывать снова", st::font(14.0));
        let footer_w = next_w + 20.0 + chk_w;
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - footer_w) * 0.5).max(0.0));
            if st::primary(ui, "Далее", st::CYAN).clicked() {
                self.welcome_open = false;
                self.cfg.welcome_gen = layout_keeper_core::config::CURRENT_WELCOME_GEN;
                self.cfg.hide_welcome = self.welcome_dont_show;
                self.save();
            }
            ui.add_space(12.0);
            st::check(ui, &mut self.welcome_dont_show, "Не показывать снова", st::CYAN);
        });
        ui.add_space(8.0);
    }

    /// One frame of the window content (everything below the OS title bar).
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
                .frame(egui::Frame::none().fill(color).inner_margin(egui::Margin::symmetric(12.0, 12.0)))
                .show_separator_line(false)
                .show(ctx, |ui| self.section_header(ui));
        }
        if !self.status.is_empty() {
            egui::TopBottomPanel::bottom("status")
                .frame(egui::Frame::none().fill(t.surface).inner_margin(egui::Margin::symmetric(st::PAD, 8.0)))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (sq, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().rect_filled(sq, 0.0, self.tab.color());
                        ui.add(egui::Label::new(egui::RichText::new(&self.status).small().color(t.ink)).wrap());
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
                let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::none().inner_margin(egui::Margin::same(st::PAD)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        if !platform::accessibility_trusted() {
                            st::block(ui, |ui| {
                                st::warn(ui, "Нет разрешения «Универсальный доступ» — без него не работают автозамена и горячие клавиши.");
                                if st::primary(ui, "Открыть настройки доступа", self.tab.color()).clicked() {
                                    platform::open_accessibility_settings();
                                }
                            });
                            ui.add_space(4.0);
                        }
                        match self.tab {
                            Tab::Home => self.home(ui),
                            Tab::Vault => {
                                if self.vault.show(ui, focus.app.clone(), &mut self.cfg.vault_idle_minutes) {
                                    self.save();
                                }
                            }
                            Tab::Words => self.words_tab(ui),
                            Tab::Exceptions => self.exceptions_tab(ui, focus),
                            Tab::Layouts => self.layouts_tab(ui, focus),
                            Tab::Settings => self.settings_tab(ui),
                            Tab::Donate => self.donate_tab(ui),
                        }
                    });
                });
                self.content_overflow = (scroll.content_size.x - scroll.inner_rect.width()).max(0.0);
            });
    }
}

#[derive(Clone, Copy)]
enum AppAction {
    Remove,
    Hide(bool),
    Set(Lang),
}

const SECTIONS: [Tab; 5] = [Tab::Layouts, Tab::Exceptions, Tab::Words, Tab::Vault, Tab::Settings];

impl Tab {
    fn color(self) -> egui::Color32 {
        match self {
            Tab::Home => st::CYAN,
            Tab::Layouts => st::YELLOW,
            Tab::Exceptions => st::ORANGE,
            Tab::Words => st::RED,
            Tab::Vault => st::LIME,
            Tab::Settings => st::CYAN,
            Tab::Donate => st::SAND,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Home => APP_NAME,
            Tab::Layouts => "Раскладки",
            Tab::Exceptions => "Исключения",
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

/// "3 программы": Russian plural forms [1, 2–4, 5+].
fn count(n: usize, forms: [&str; 3]) -> String {
    let (d10, d100) = (n % 10, n % 100);
    let f = if d10 == 1 && d100 != 11 {
        forms[0]
    } else if (2..=4).contains(&d10) && !(12..=14).contains(&d100) {
        forms[1]
    } else {
        forms[2]
    };
    format!("{n} {f}")
}

fn toggle_path(set: &mut HashSet<String>, id: &str) {
    if !set.remove(id) {
        set.insert(id.to_string());
    }
}

/// Icon + short app name (cut to fit, full id on hover) + "…" revealing the full path
/// on the next line.
fn app_label(ui: &mut egui::Ui, icons: &mut IconCache, expanded: &mut HashSet<String>, id: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            icons.show(ui, id);
            let title = platform::app_display_name(id).unwrap_or_else(|| basename(id).to_string());
            if has_path(id) {
                let open = expanded.contains(id);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(if open { "▾" } else { "…" })
                        .on_hover_text(if open { "Скрыть путь" } else { "Показать полный путь" })
                        .clicked()
                    {
                        toggle_path(expanded, id);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&title)).truncate()).on_hover_text(id);
                    });
                });
            } else {
                ui.add(egui::Label::new(egui::RichText::new(&title)).truncate()).on_hover_text(id);
            }
        });
        if expanded.contains(id) {
            let c = st::cur(ui).muted;
            ui.add(egui::Label::new(egui::RichText::new(id).monospace().small().color(c)).wrap().selectable(true));
        }
    });
}

/// Wrapped chips; returns the index whose trash was clicked.
fn chips(ui: &mut egui::Ui, words: &[String]) -> Option<usize> {
    if words.is_empty() {
        st::hint(ui, "Пока пусто.");
        return None;
    }
    let mut hit = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
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
        manual_id: String::new(),
        manual_lang: Lang::Ru,
        exception_draft: String::new(),
        tab: if open_vault { Tab::Vault } else { Tab::Home },
        vault: crate::vault_ui::VaultUi::new(),
        capturing_hotkey: None,
        word_draft: String::new(),
        expanded_paths: HashSet::new(),
        show_system_apps: false,
        icons: IconCache::default(),
        open_groups: HashSet::new(),
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
            .with_inner_size([440.0, 600.0])
            .with_min_inner_size([380.0, 440.0])
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
