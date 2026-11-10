//! "Сейф" tab of the settings window: encrypted logins/cards and user-gesture autofill.
//!
//! * The vault is decrypted only in this process's memory after the master password is entered;
//!   it is locked (key + entries zeroized on drop) on window close, by the "Заблокировать"
//!   button, and after `vault_idle_minutes` without input in this window.
//! * Filling is done by *this* GUI process: it minimizes itself, waits for the previous app to
//!   regain focus, checks that the focused app is not Typed (and is the app shown as
//!   target), then types via the platform injection API (SendInput / XTest / CGEvent).
//!   No IPC with the daemon, no clipboard, no network, nothing secret is logged.

use crate::platform;
use eframe::egui;
use layout_keeper_core::vault::{self, Card, Login, Vault};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

/// Warning copy: deep amber on light, warm gold on dark — readable on both.
fn warn_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(232, 196, 72)
    } else {
        egui::Color32::from_rgb(148, 90, 0)
    }
}

enum Edit {
    None,
    Login(Option<usize>, Login),
    Card(Option<usize>, Card),
}

pub struct VaultUi {
    path: PathBuf,
    vault: Option<Vault>,
    pw1: String,
    pw2: String,
    msg: String,
    edit: Edit,
    reveal: Option<usize>,
    confirm_delete: Option<(bool, usize)>,
    last_activity: Instant,
    fill_result: Arc<Mutex<Option<String>>>,
}

fn wipe(s: &mut String) {
    s.zeroize();
}

impl VaultUi {
    pub fn new() -> Self {
        VaultUi {
            path: vault::default_path(),
            vault: None,
            pw1: String::new(),
            pw2: String::new(),
            msg: String::new(),
            edit: Edit::None,
            reveal: None,
            confirm_delete: None,
            last_activity: Instant::now(),
            fill_result: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.vault.is_some()
    }

    /// Drop the decrypted vault and any secret edit buffers (all zeroize on drop).
    pub fn lock(&mut self) {
        self.vault = None;
        self.edit = Edit::None;
        self.reveal = None;
        self.confirm_delete = None;
        wipe(&mut self.pw1);
        wipe(&mut self.pw2);
    }

    /// Called every frame (even when the tab is hidden) to enforce the idle timeout.
    pub fn tick(&mut self, ctx: &egui::Context, idle_minutes: u32) {
        let active = ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving());
        if active {
            self.last_activity = Instant::now();
        }
        let limit = Duration::from_secs(60 * idle_minutes.clamp(1, 120) as u64);
        if self.vault.is_some() && self.last_activity.elapsed() > limit {
            self.lock();
            self.msg = "Сейф заблокирован после бездействия.".into();
        }
        if let Some(r) = self.fill_result.lock().ok().and_then(|mut g| g.take()) {
            self.msg = r;
        }
    }

    fn save(&mut self) {
        if let Some(v) = &self.vault {
            self.msg = match v.save(&self.path) {
                Ok(()) => "Сейф сохранён (зашифрован).".into(),
                Err(e) => format!("Ошибка сохранения сейфа: {e}"),
            };
        }
    }

    /// Hide to the tray, then type into whatever app regains focus — only if it is not
    /// Typed and matches the target app the user saw in the window.
    fn fill(&mut self, ctx: &egui::Context, target: Option<String>, text: Zeroizing<String>) {
        if text.is_empty() {
            self.msg = "Поле пустое — нечего подставлять.".into();
            return;
        }
        crate::gui::hide_window(ctx);
        self.msg = "Подставляю…".into();
        let out = self.fill_result.clone();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(700));
            let (app, _) = platform::probe();
            let res = match app {
                None => "Не удалось определить активное окно — ничего не набрано.".to_string(),
                Some(a) if crate::gui::is_self(&a) => {
                    "Активно окно Typed — ничего не набрано. Переключитесь в нужное поле.".into()
                }
                Some(a) if target.as_deref().is_some_and(|t| t != a) => {
                    "Фокус не в том приложении — ничего не набрано.".into()
                }
                Some(_) => match platform::type_text(&text) {
                    Ok(()) => "Подставлено в активное окно.".into(),
                    Err(e) => format!("Не удалось подставить: {e}"),
                },
            };
            drop(text);
            if let Ok(mut g) = out.lock() {
                *g = Some(res);
            }
            ctx2.request_repaint();
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, focus_app: Option<String>, idle_minutes: &mut u32) -> bool {
        let mut cfg_changed = false;
        ui.heading("Сейф: пароли и карты");
        ui.add(egui::Label::new(egui::RichText::new(format!("Файл: {}", self.path.display())).small()).wrap());
        ui.small("На диске только шифротекст.");
        if !self.msg.is_empty() {
            ui.label(&self.msg);
        }
        ui.separator();

        if self.vault.is_none() {
            self.show_locked(ui);
        } else {
            ui.horizontal(|ui| {
                if ui.button("🔒 Заблокировать").clicked() {
                    self.lock();
                    self.msg = "Сейф заблокирован.".into();
                }
                ui.label("Автоблокировка через, мин:");
                cfg_changed |= ui.add(egui::DragValue::new(idle_minutes).range(1..=120)).changed();
            });
            if self.vault.is_some() {
                self.show_unlocked(ui, focus_app);
            }
        }
        cfg_changed
    }

    fn show_locked(&mut self, ui: &mut egui::Ui) {
        let exists = self.path.exists();
        if exists {
            ui.label("Введите мастер-пароль, чтобы открыть сейф.");
        } else {
            ui.label("Сейф ещё не создан. Придумайте мастер-пароль и введите его дважды.");
            ui.colored_label(warn_color(ui), "Мастер-пароль нельзя восстановить. Если забудете — данные не вернуть.");
        }
        let mut go = false;
        ui.horizontal(|ui| {
            ui.label("Мастер-пароль:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                go = ui.button(if exists { "Открыть" } else { "Создать сейф" }).clicked();
                ui.add(egui::TextEdit::singleline(&mut self.pw1).password(true).desired_width(ui.available_width().max(60.0)));
            });
        });
        if !exists {
            ui.horizontal(|ui| {
                ui.label("Ещё раз:");
                ui.add(egui::TextEdit::singleline(&mut self.pw2).password(true).desired_width(ui.available_width().max(60.0)));
            });
        }
        let go = go || ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !go || self.pw1.is_empty() {
            return;
        }
        if exists {
            match Vault::open(&self.path, &self.pw1) {
                Ok(v) => {
                    self.vault = Some(v);
                    self.msg = "Сейф открыт.".into();
                }
                Err(e) => self.msg = format!("Не удалось открыть: {e}"),
            }
        } else if self.pw1 != self.pw2 {
            self.msg = "Пароли не совпадают.".into();
            wipe(&mut self.pw2);
            return;
        } else if self.pw1.chars().count() < 8 {
            self.msg = "Мастер-пароль должен быть не короче 8 символов.".into();
            return;
        } else {
            match Vault::create(&self.pw1) {
                Ok(v) => {
                    self.vault = Some(v);
                    self.save();
                }
                Err(e) => self.msg = format!("Ошибка: {e}"),
            }
        }
        wipe(&mut self.pw1);
        wipe(&mut self.pw2);
        self.last_activity = Instant::now();
    }

    fn show_unlocked(&mut self, ui: &mut egui::Ui, focus_app: Option<String>) {
        let ctx = ui.ctx().clone();
        ui.label("Щёлкните в поле другой программы, вернитесь и нажмите кнопку. Наберётся в текущее приложение.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Текущее:");
            ui.add(egui::Label::new(egui::RichText::new(focus_app.as_deref().unwrap_or("(не определено)")).monospace()).wrap());
        });

        if !matches!(self.edit, Edit::None) {
            self.show_editor(ui);
            return;
        }

        // ---- logins
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Логины");
            if ui.button("+ Добавить логин").clicked() {
                let mut l = Login::default();
                if let Some(a) = &focus_app {
                    l.app_match = a.clone();
                }
                self.edit = Edit::Login(None, l);
            }
        });
        let v = self.vault.as_ref().expect("unlocked");
        let mut order: Vec<usize> = (0..v.data.logins.len()).collect();
        let is_match = |i: usize| focus_app.as_deref().is_some_and(|a| v.data.logins[i].matches_app(a));
        order.sort_by_key(|&i| !is_match(i));
        let matches: Vec<bool> = order.iter().map(|&i| is_match(i)).collect();
        if order.is_empty() {
            ui.label("(пока пусто)");
        }
        let mut action: Option<(usize, &'static str)> = None;
        for (&i, &m) in order.iter().zip(&matches) {
            let l = &self.vault.as_ref().unwrap().data.logins[i];
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if m {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, "★ подходит");
                    }
                    ui.strong(&l.label);
                    ui.label(format!("— {}", l.username));
                    if self.reveal == Some(i) {
                        ui.monospace(&l.password);
                        if ui.button("скрыть").clicked() {
                            action = Some((i, "hide"));
                        }
                    } else {
                        ui.monospace("••••••••");
                        if ui.button("показать").clicked() {
                            action = Some((i, "show"));
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (k, name) in [("both", "Подставить логин"), ("pass", "Только пароль"), ("user", "Только имя")] {
                        if ui.button(name).clicked() {
                            action = Some((i, k));
                        }
                    }
                    if ui.button("Изменить").clicked() {
                        action = Some((i, "edit"));
                    }
                    if crate::icons::delete_icon_button(ui).clicked() {
                        action = Some((i, "del"));
                    }
                });
            });
        }
        if let Some((i, a)) = action {
            let l = &self.vault.as_ref().unwrap().data.logins[i];
            match a {
                "show" => self.reveal = Some(i),
                "hide" => self.reveal = None,
                "both" => {
                    let t = Zeroizing::new(format!("{}\t{}", l.username, l.password));
                    self.fill(&ctx, focus_app.clone(), t)
                }
                "pass" => {
                    let t = Zeroizing::new(l.password.clone());
                    self.fill(&ctx, focus_app.clone(), t)
                }
                "user" => {
                    let t = Zeroizing::new(l.username.clone());
                    self.fill(&ctx, focus_app.clone(), t)
                }
                "edit" => self.edit = Edit::Login(Some(i), l.clone()),
                "del" => self.confirm_delete = Some((true, i)),
                _ => {}
            }
        }

        // ---- cards
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Карты");
            if ui.button("+ Добавить карту").clicked() {
                self.edit = Edit::Card(None, Card::default());
            }
        });
        let n = self.vault.as_ref().unwrap().data.cards.len();
        if n == 0 {
            ui.label("(пока пусто)");
        }
        let mut caction: Option<(usize, &'static str)> = None;
        for i in 0..n {
            let c = &self.vault.as_ref().unwrap().data.cards[i];
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(&c.label);
                    ui.monospace(c.masked());
                    ui.label(&c.expiry);
                });
                ui.horizontal_wrapped(|ui| {
                    for (k, name) in [("num", "Номер"), ("exp", "Срок"), ("name", "Имя")] {
                        if ui.button(name).clicked() {
                            caction = Some((i, k));
                        }
                    }
                    if !c.cvc.is_empty() && ui.button("CVC").clicked() {
                        caction = Some((i, "cvc"));
                    }
                    if ui.button("Изменить").clicked() {
                        caction = Some((i, "edit"));
                    }
                    if crate::icons::delete_icon_button(ui).clicked() {
                        caction = Some((i, "del"));
                    }
                });
            });
        }
        if let Some((i, a)) = caction {
            let c = &self.vault.as_ref().unwrap().data.cards[i];
            let text = match a {
                "num" => Some(c.number.chars().filter(|ch| ch.is_ascii_digit()).collect::<String>()),
                "exp" => Some(c.expiry.clone()),
                "name" => Some(c.holder.clone()),
                "cvc" => Some(c.cvc.clone()),
                "edit" => {
                    self.edit = Edit::Card(Some(i), c.clone());
                    None
                }
                "del" => {
                    self.confirm_delete = Some((false, i));
                    None
                }
                _ => None,
            };
            if let Some(t) = text {
                self.fill(&ctx, focus_app.clone(), Zeroizing::new(t));
            }
        }

        if let Some((is_login, i)) = self.confirm_delete {
            ui.separator();
            ui.colored_label(warn_color(ui), "Удалить запись безвозвратно?");
            ui.horizontal(|ui| {
                if ui.button("Да, удалить").clicked() {
                    let v = self.vault.as_mut().unwrap();
                    if is_login && i < v.data.logins.len() {
                        v.data.logins.remove(i);
                    } else if !is_login && i < v.data.cards.len() {
                        v.data.cards.remove(i);
                    }
                    self.confirm_delete = None;
                    self.reveal = None;
                    self.save();
                }
                if ui.button("Отмена").clicked() {
                    self.confirm_delete = None;
                }
            });
        }
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let mut done: Option<bool> = None; // Some(true)=save, Some(false)=cancel
        let row = |ui: &mut egui::Ui, label: &str, s: &mut String, secret: bool| {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(s).password(secret).desired_width(ui.available_width().max(80.0)));
            });
        };
        match &mut self.edit {
            Edit::Login(_, l) => {
                ui.heading("Логин");
                row(ui, "Название:", &mut l.label, false);
                row(ui, "Имя:", &mut l.username, false);
                row(ui, "Пароль:", &mut l.password, true);
                row(ui, "Приложение / адрес:", &mut l.app_match, false);
                ui.small("Если путь или адрес содержит эту строку, логин подсветится.");
                ui.label("Заметки:");
                ui.add(egui::TextEdit::multiline(&mut l.notes).desired_rows(3));
            }
            Edit::Card(_, c) => {
                ui.heading("Карта");
                row(ui, "Название:", &mut c.label, false);
                row(ui, "Номер:", &mut c.number, true);
                row(ui, "Срок (ММ/ГГ):", &mut c.expiry, false);
                row(ui, "Имя на карте:", &mut c.holder, false);
                row(ui, "CVC (необязательно):", &mut c.cvc, true);
                ui.colored_label(warn_color(ui), "CVC лучше не сохранять, а вводить вручную.");
            }
            Edit::None => {}
        }
        ui.horizontal(|ui| {
            if ui.button("Сохранить").clicked() {
                done = Some(true);
            }
            if ui.button("Отмена").clicked() {
                done = Some(false);
            }
        });
        let Some(save) = done else { return };
        let edit = std::mem::replace(&mut self.edit, Edit::None);
        if !save {
            return; // `edit` drops here and zeroizes
        }
        let v = self.vault.as_mut().unwrap();
        match edit {
            Edit::Login(idx, l) => {
                if l.label.trim().is_empty() && l.username.trim().is_empty() {
                    self.msg = "Укажите название или имя пользователя.".into();
                    self.edit = Edit::Login(idx, l);
                    return;
                }
                match idx {
                    Some(i) if i < v.data.logins.len() => v.data.logins[i] = l,
                    _ => v.data.logins.push(l),
                }
            }
            Edit::Card(idx, c) => {
                let digits = c.number.chars().filter(|ch| ch.is_ascii_digit()).count();
                let exp_ok = {
                    let b = c.expiry.as_bytes();
                    b.len() == 5 && b[2] == b'/' && c.expiry[..2].parse::<u8>().is_ok_and(|m| (1..=12).contains(&m)) && c.expiry[3..].parse::<u8>().is_ok()
                };
                if !(12..=19).contains(&digits) || !exp_ok {
                    self.msg = "Проверьте номер (12–19 цифр) и срок в формате ММ/ГГ.".into();
                    self.edit = Edit::Card(idx, c);
                    return;
                }
                match idx {
                    Some(i) if i < v.data.cards.len() => v.data.cards[i] = c,
                    _ => v.data.cards.push(c),
                }
            }
            Edit::None => {}
        }
        self.reveal = None;
        self.save();
    }
}
