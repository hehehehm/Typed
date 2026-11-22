
use crate::platform;
use crate::ui_style as st;
use eframe::egui;
use layout_keeper_core::vault::{self, Card, Login, Vault};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

const ACCENT: egui::Color32 = st::SEAFOAM;

fn heading_row(ui: &mut egui::Ui, caption: &str, count: usize, tip: &str, clicked: &mut bool) {
    let t = st::cur(ui);
    ui.add_space(st::PAD);
    st::row(
        ui,
        egui::Color32::TRANSPARENT,
        |ui| {
            ui.add_space(st::INSET);
            ui.label(egui::RichText::new(caption).size(st::FONT_SM).color(t.muted));
            ui.label(egui::RichText::new(count.to_string()).size(st::FONT_SM).color(t.muted));
        },
        |ui| *clicked = st::plus_square(ui, ACCENT, tip).clicked(),
    );
    ui.add_space(st::GAP);
}

fn entry_rule(ui: &mut egui::Ui) {
    st::sep(ui);
}

fn entry<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(0.0, st::PAD))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
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

    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf, vault: Option<Vault>) -> Self {
        let mut v = Self::new();
        v.path = path;
        v.vault = vault;
        v
    }

    pub fn entry_count(&self) -> Option<usize> {
        self.vault.as_ref().map(|v| v.data.logins.len() + v.data.cards.len())
    }

    pub fn state_text(&self) -> &'static str {
        if self.vault.is_some() {
            "открыт"
        } else if self.path.exists() {
            "заперт"
        } else {
            "не создан"
        }
    }

    pub fn lock(&mut self) {
        self.vault = None;
        self.edit = Edit::None;
        self.reveal = None;
        self.confirm_delete = None;
        wipe(&mut self.pw1);
        wipe(&mut self.pw2);
    }

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
                Ok(()) => String::new(),
                Err(e) => format!("Ошибка: {e}"),
            };
        }
    }

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
        let t = st::cur(ui);
        if !self.msg.is_empty() {
            st::block(ui, |ui| {
                ui.horizontal(|ui| {
                    let (sq, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(sq.center(), 4.0, ACCENT);
                    ui.add(egui::Label::new(egui::RichText::new(&self.msg).color(t.ink)).truncate());
                });
            });
        }

        if self.vault.is_none() {
            self.show_locked(ui);
        } else {
            st::group(ui, |ui| {
                st::line_row(
                    ui,
                    |ui| {
                        let r = st::secondary(ui, "Заблокировать");
                        let tip = format!("Файл: {}", self.path.display());
                        if r.on_hover_text(tip).clicked() {
                            self.lock();
                            self.msg.clear();
                        }
                    },
                    |ui| {
                        ui.scope(|ui| {
                            let v = &mut ui.visuals_mut().widgets;
                            v.inactive.weak_bg_fill = t.field;
                            v.inactive.bg_stroke = egui::Stroke::new(1.0, t.line);
                            v.hovered.bg_stroke = egui::Stroke::new(1.0, t.muted);
                            v.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
                            cfg_changed |= ui
                                .add_sized([st::ROW, st::CTRL_H], egui::DragValue::new(idle_minutes).range(1..=120))
                                .on_hover_text("Автоблокировка, мин")
                                .changed();
                        });
                        ui.label(egui::RichText::new("мин").color(t.muted)).on_hover_text("Автоблокировка, мин");
                    },
                );
            });
            if self.vault.is_some() {
                self.show_unlocked(ui, focus_app);
            }
        }
        cfg_changed
    }

    fn show_locked(&mut self, ui: &mut egui::Ui) {
        let exists = self.path.exists();
        let mut go = false;
        st::group(ui, |ui| {
            egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, st::GAP)).show(ui, |ui| {
                let w = ui.available_width();
                if exists {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = st::PAD;
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = st::PAD;
                            go = st::primary(ui, "Открыть", ACCENT)
                                .on_hover_text(format!("Файл: {}", self.path.display()))
                                .clicked();
                            st::password_bg(ui, &mut self.pw1, "пароль", ui.available_width(), ACCENT);
                        });
                    });
                } else {
                    st::password_bg(ui, &mut self.pw1, "новый пароль (8+)", w, ACCENT);
                    st::password_bg(ui, &mut self.pw2, "повтор", w, ACCENT);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            go = st::primary(ui, "Создать", ACCENT)
                                .on_hover_text("Мастер-пароль нельзя восстановить")
                                .clicked();
                        });
                    });
                }
            });
        });
        let go = go || ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !go || self.pw1.is_empty() {
            return;
        }
        if exists {
            match Vault::open(&self.path, &self.pw1) {
                Ok(v) => {
                    self.vault = Some(v);
                    self.msg.clear();
                }
                Err(e) => self.msg = format!("Ошибка: {e}"),
            }
        } else if self.pw1 != self.pw2 {
            self.msg = "Не совпадают".into();
            wipe(&mut self.pw2);
            return;
        } else if self.pw1.chars().count() < 8 {
            self.msg = "Короче 8 символов".into();
            return;
        } else {
            match Vault::create(&self.pw1) {
                Ok(v) => {
                    self.vault = Some(v);
                    if let Err(e) = self.vault.as_ref().unwrap().save(&self.path) {
                        self.msg = format!("Ошибка: {e}");
                    } else {
                        self.msg.clear();
                    }
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
        let t = st::cur(ui);

        if !matches!(self.edit, Edit::None) {
            self.show_editor(ui);
            return;
        }

        let mut add_login = false;
        let n_logins = self.vault.as_ref().map_or(0, |v| v.data.logins.len());
        heading_row(ui, "Логины", n_logins, "Добавить логин", &mut add_login);
        if add_login {
            let mut l = Login::default();
            if let Some(a) = &focus_app {
                l.app_match = a.clone();
            }
            self.edit = Edit::Login(None, l);
        }
        let v = self.vault.as_ref().expect("unlocked");
        let mut order: Vec<usize> = (0..v.data.logins.len()).collect();
        let is_match = |i: usize| focus_app.as_deref().is_some_and(|a| v.data.logins[i].matches_app(a));
        order.sort_by_key(|&i| !is_match(i));
        let matches: Vec<bool> = order.iter().map(|&i| is_match(i)).collect();
        let mut action: Option<(usize, &'static str)> = None;
        if !order.is_empty() {
            st::group(ui, |ui| {
                for (k, (&i, &m)) in order.iter().zip(&matches).enumerate() {
                    if k > 0 {
                        entry_rule(ui);
                    }
                    let l = &self.vault.as_ref().unwrap().data.logins[i];
                    let revealed = self.reveal == Some(i);
                    entry(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = st::GAP;
                        st::row(
                            ui,
                            egui::Color32::TRANSPARENT,
                            |ui| {
                                if m {
                                    let (r, resp) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                                    ui.painter().circle_filled(r.center(), 4.0, ACCENT);
                                    resp.on_hover_text("Подходит к текущему приложению");
                                }
                                ui.add(egui::Label::new(egui::RichText::new(&l.label).color(t.ink)).truncate()).on_hover_text(&l.label);
                                ui.add(egui::Label::new(egui::RichText::new(&l.username).color(t.muted)).truncate()).on_hover_text(&l.username);
                                if revealed {
                                    ui.add(egui::Label::new(egui::RichText::new(&l.password).monospace().color(t.ink)).truncate());
                                }
                            },
                            |ui| {
                                if crate::icons::delete_icon_button(ui).clicked() {
                                    action = Some((i, "del"));
                                }
                                if crate::icons::edit_icon_button(ui).clicked() {
                                    action = Some((i, "edit"));
                                }
                                if crate::icons::eye_icon_button(ui, revealed).clicked() {
                                    action = Some((i, if revealed { "hide" } else { "show" }));
                                }
                            },
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(st::PAD, st::GAP);
                            if st::primary(ui, "Логин", ACCENT).on_hover_text("Подставить имя и пароль").clicked() {
                                action = Some((i, "both"));
                            }
                            if st::secondary(ui, "Пароль").on_hover_text("Только пароль").clicked() {
                                action = Some((i, "pass"));
                            }
                            if st::secondary(ui, "Имя").on_hover_text("Только имя").clicked() {
                                action = Some((i, "user"));
                            }
                        });
                    });
                }
            });
        }
        if let Some((i, a)) = action {
            let l = &self.vault.as_ref().unwrap().data.logins[i];
            match a {
                "show" => self.reveal = Some(i),
                "hide" => self.reveal = None,
                "both" => {
                    let t = Zeroizing::new(format!("{}	{}", l.username, l.password));
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

        let n = self.vault.as_ref().unwrap().data.cards.len();
        let mut add_card = false;
        heading_row(ui, "Карты", n, "Добавить карту", &mut add_card);
        if add_card {
            self.edit = Edit::Card(None, Card::default());
        }
        let mut caction: Option<(usize, &'static str)> = None;
        if n > 0 {
            st::group(ui, |ui| {
                for i in 0..n {
                    if i > 0 {
                        entry_rule(ui);
                    }
                    let c = &self.vault.as_ref().unwrap().data.cards[i];
                    entry(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = st::GAP;
                        st::row(
                            ui,
                            egui::Color32::TRANSPARENT,
                            |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(&c.label).color(t.ink)).truncate()).on_hover_text(&c.label);
                                ui.add(egui::Label::new(egui::RichText::new(format!("{}  {}", c.masked(), c.expiry)).monospace().color(t.muted)).truncate());
                            },
                            |ui| {
                                if crate::icons::delete_icon_button(ui).clicked() {
                                    caction = Some((i, "del"));
                                }
                                if crate::icons::edit_icon_button(ui).clicked() {
                                    caction = Some((i, "edit"));
                                }
                            },
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(st::PAD, st::GAP);
                            if st::primary(ui, "Номер", ACCENT).clicked() {
                                caction = Some((i, "num"));
                            }
                            for (k, name) in [("exp", "Срок"), ("name", "Имя")] {
                                if st::secondary(ui, name).clicked() {
                                    caction = Some((i, k));
                                }
                            }
                            if !c.cvc.is_empty() && st::secondary(ui, "CVC").clicked() {
                                caction = Some((i, "cvc"));
                            }
                        });
                    });
                }
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
            let mut yes = false;
            let mut no = false;
            st::block(ui, |ui| {
                st::row(
                    ui,
                    egui::Color32::TRANSPARENT,
                    |ui| {
                        st::warn(ui, "Удалить?");
                    },
                    |ui| {
                        no = st::secondary(ui, "Нет").clicked();
                        yes = st::tile(ui, "Да", st::CORAL, st::INK_ON_COLOR).clicked();
                    },
                );
            });
            if yes {
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
            if no {
                self.confirm_delete = None;
            }
        }
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let mut done: Option<bool> = None;
        let row = |ui: &mut egui::Ui, label: &str, tip: &str, s: &mut String, secret: bool| {
            let c = st::cur(ui).muted;
            let l = ui.label(egui::RichText::new(label).small().color(c));
            if !tip.is_empty() {
                l.on_hover_text(tip);
            }
            let w = ui.available_width();
            if secret {
                st::password_bg(ui, s, "", w, ACCENT);
            } else {
                st::field_bg(ui, s, "", w, ACCENT);
            }
        };
        st::group(ui, |ui| entry(ui, |ui| {
            ui.spacing_mut().item_spacing.y = st::GAP;
            match &mut self.edit {
                Edit::Login(_, l) => {
                    row(ui, "Название", "", &mut l.label, false);
                    row(ui, "Логин", "", &mut l.username, false);
                    row(ui, "Пароль", "", &mut l.password, true);
                    row(ui, "Приложение", "Фрагмент пути или адреса для подсветки", &mut l.app_match, false);
                    let c = st::cur(ui).muted;
                    ui.label(egui::RichText::new("Заметки").small().color(c));
                    ui.add(egui::TextEdit::multiline(&mut l.notes).desired_rows(2).desired_width(f32::INFINITY));
                }
                Edit::Card(_, c) => {
                    row(ui, "Название", "", &mut c.label, false);
                    row(ui, "Номер", "", &mut c.number, true);
                    row(ui, "ММ/ГГ", "", &mut c.expiry, false);
                    row(ui, "Владелец", "", &mut c.holder, false);
                    row(ui, "CVC", "Необязательно — лучше вводить вручную", &mut c.cvc, true);
                }
                Edit::None => {}
            }
            ui.add_space(st::GAP);
            ui.horizontal(|ui| {
                if st::primary(ui, "Сохранить", ACCENT).clicked() {
                    done = Some(true);
                }
                if st::secondary(ui, "Отмена").clicked() {
                    done = Some(false);
                }
            });
        }));
        let Some(save) = done else { return };
        let edit = std::mem::replace(&mut self.edit, Edit::None);
        if !save {
            return;
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
