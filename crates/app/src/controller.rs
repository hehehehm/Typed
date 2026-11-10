//! Platform-independent glue between OS events and the core engine:
//! per-app layout memory, auto-learning, exceptions, autocorrect and the manual hotkey.
//!
//! Platform adapters call [`Controller::tick`] periodically with the focused app and the
//! active layout, and [`Controller::on_input`] for each key; they execute the returned
//! [`Command`]s. Nothing here touches the OS, so it is unit-tested on any machine.

use crate::app_id::is_hidden_app;
use layout_keeper_core::{Config, Engine, Hotkey, Key, Lang, Replacement};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key { key: Key, modifiers: bool },
    Hotkey,
    /// Undo the last autocorrection and never correct that word again.
    Undo,
    /// Open or focus the settings / vault window. Works even in exception apps.
    Summon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Make `Lang` the active layout for the focused app.
    SwitchLayout(Lang),
    /// Erase `delete` chars, type `insert`, activate `target`, then report success.
    /// Fail safe: if erasing or
    /// typing cannot be done completely, the adapter must not start it at all.
    Replace(Replacement),
    /// Bring the settings / vault window to the front (or start it).
    SummonGui,
    /// Hotkey with no known word: convert the selection (or the word left of the caret)
    /// via copy / paste. The adapter reports the new layout with `replace_succeeded`.
    ConvertSelection,
}

pub struct Controller {
    pub config: Config,
    path: Option<PathBuf>,
    engine: Engine,
    app: Option<String>,
    layout: Option<Lang>,
    /// Layout we asked for ourselves — its change notification must not be "learned".
    expected: Option<(Lang, Instant)>,
    /// An adapter has not yet acknowledged the queued word replacement.
    replacement_pending: bool,
    config_mtime: Option<SystemTime>,
    last_reload_check: Instant,
    launches_dirty: bool,
    last_launch_save: Instant,
}

const EXPECT_WINDOW: Duration = Duration::from_secs(5);

impl Controller {
    pub fn new(config: Config, path: Option<PathBuf>) -> Self {
        let engine = Engine::new(config.engine_options());
        let config_mtime = path.as_ref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok());
        Controller {
            config,
            path,
            engine,
            app: None,
            layout: None,
            expected: None,
            replacement_pending: false,
            config_mtime,
            last_reload_check: Instant::now(),
            launches_dirty: false,
            last_launch_save: Instant::now(),
        }
    }

    pub fn hotkey(&self) -> Hotkey {
        self.config.hotkey
    }

    pub fn summon_hotkey(&self) -> Hotkey {
        self.config.summon_hotkey
    }

    pub fn undo_hotkey(&self) -> Hotkey {
        self.config.undo_hotkey
    }

    /// Save words the engine learned from an undo into `never_correct`.
    fn save_learned(&mut self) {
        let words = self.engine.take_learned();
        let mut changed = false;
        for w in words {
            changed |= self.config.add_never_correct(&w);
        }
        if changed {
            self.save();
        }
    }

    pub fn current_layout(&self) -> Option<Lang> {
        self.layout
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn current_word_len(&self) -> usize {
        self.engine.current_len()
    }

    fn active_is_exception(&self) -> bool {
        // Typed's own window (vault master password, dictionary field) is never autocorrected.
        self.app.as_deref().is_some_and(|a| is_own_app(a) || self.config.is_exception(a))
    }

    fn learn(&mut self, app: &str, lang: Lang) {
        // Exceptions disable autocorrect only; layout memory still applies.
        if !self.config.auto_learn {
            return;
        }
        // Installers, temp unpackers and OS shell hosts should not accumulate.
        if is_hidden_app(app) {
            return;
        }
        // Sticky defaults: an assigned layout is never overwritten by a keyboard switch.
        // Only `set-app` / the GUI change an existing assignment.
        if self.config.layout_for(app).is_some() {
            return;
        }
        if self.config.set_layout(app, lang) {
            log::info!("learned layout {lang} for app {app}");
            self.save();
        }
    }

    fn save(&mut self) {
        if let Some(p) = &self.path {
            match self.config.save(p) {
                Ok(()) => {
                    self.config_mtime = std::fs::metadata(p).ok().and_then(|m| m.modified().ok());
                    self.launches_dirty = false;
                    self.last_launch_save = Instant::now();
                }
                Err(e) => log::warn!("cannot save config {}: {e}", p.display()),
            }
        }
    }

    fn flush_launches(&mut self) {
        if self.launches_dirty && self.last_launch_save.elapsed() >= Duration::from_secs(8) {
            self.save();
        }
    }

    /// Pick up edits made by `layout-keeper set-app` or by hand while running.
    fn maybe_reload(&mut self) {
        if self.last_reload_check.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last_reload_check = Instant::now();
        let Some(p) = &self.path else { return };
        let mtime = std::fs::metadata(p).ok().and_then(|m| m.modified().ok());
        if mtime.is_some() && mtime != self.config_mtime {
            match std::fs::read_to_string(p).and_then(|s| Config::from_json(&s)) {
                Ok(mut c) => {
                    log::info!("config reloaded");
                    if self.launches_dirty {
                        for (k, v) in &self.config.app_launches {
                            let entry = c.app_launches.entry(k.clone()).or_insert(0);
                            *entry = (*entry).max(*v);
                        }
                    }
                    self.engine.options = c.engine_options();
                    self.config = c;
                    self.refresh_preferred_lang();
                }
                Err(e) => log::warn!("config changed but is invalid, keeping old one: {e}"),
            }
            self.config_mtime = mtime;
        }
    }

    fn refresh_preferred_lang(&mut self) {
        self.engine.options.preferred_lang = self.app.as_deref().and_then(|a| self.config.layout_for(a));
    }

    /// Periodic state report from the adapter.
    pub fn tick(&mut self, app: Option<&str>, layout: Option<Lang>) -> Vec<Command> {
        self.maybe_reload();
        if let Some((_, t)) = self.expected {
            if t.elapsed() > EXPECT_WINDOW {
                self.expected = None;
            }
        }
        let mut out = Vec::new();
        let app_changed = app != self.app.as_deref();
        if app_changed {
            // Leaving an app we had no memory for: remember the layout the user left it on.
            if let (Some(prev), Some(l)) = (self.app.clone(), self.layout) {
                if self.config.layout_for(&prev).is_none() {
                    self.learn(&prev, l);
                }
            }
            self.engine.reset();
            self.replacement_pending = false;
            self.app = app.map(str::to_string);
            self.refresh_preferred_lang();
            self.layout = layout;
            self.expected = None;
            if let Some(a) = app {
                if self.config.layout_for(a).is_some() {
                    self.config.record_launch(a);
                    self.launches_dirty = true;
                }
                if let Some(want) = self.config.layout_for(a) {
                    // Activation / per-window IME may overwrite even an already matching HKL.
                    log::debug!("focus: applying remembered layout {want}");
                    self.expected = Some((want, Instant::now()));
                    out.push(Command::SwitchLayout(want));
                }
            }
            return out;
        }
        if layout != self.layout {
            self.layout = layout;
            // During the grace period all changes may be activation races or our own
            // requests; never learn them as manual input.
            if self.expected.is_none() && !self.replacement_pending {
                if let (Some(l), Some(a)) = (layout, self.app.clone()) {
                    self.learn(&a, l);
                }
            }
        }
        // Keep applying for the FULL grace period: a matching HKL can be transient
        // after AttachThreadInput / ActivateKeyboardLayout, not an acknowledgement.
        if !self.replacement_pending {
            if let Some((want, _)) = self.expected {
                out.push(Command::SwitchLayout(want));
            }
        }
        self.refresh_preferred_lang();
        self.flush_launches();
        out
    }

    pub fn on_input(&mut self, input: Input) -> Vec<Command> {
        self.refresh_preferred_lang();
        if matches!(input, Input::Summon) {
            return vec![Command::SummonGui];
        }
        // Exception list: never autocorrect or hotkey-convert in these apps.
        if self.active_is_exception() {
            self.engine.reset();
            return vec![];
        }
        let rep = match input {
            Input::Key { key, modifiers } => self.engine.feed(key, modifiers),
            Input::Hotkey => match self.engine.manual_convert(self.layout.unwrap_or(Lang::En)) {
                Some(r) => Some(r),
                None => {
                    // Clicked or moved the caret since typing: work on the selection.
                    self.engine.reset();
                    return vec![Command::ConvertSelection];
                }
            },
            Input::Undo => self.engine.undo_last(self.layout.unwrap_or(Lang::En)),
            Input::Summon => return vec![Command::SummonGui],
        };
        self.save_learned();
        match rep {
            Some(r) => {
                self.replacement_pending = true;
                vec![Command::Replace(r)]
            }
            None => vec![],
        }
    }

    /// Start the layout grace period only after the adapter actually replaced the word.
    pub fn replace_succeeded(&mut self, lang: Lang) {
        self.replacement_pending = false;
        self.layout = Some(lang);
        self.expected = Some((lang, Instant::now()));
    }

    /// Adapter reports that a Replace could not be performed; forget the word state.
    pub fn replace_failed(&mut self) {
        self.replacement_pending = false;
        self.engine.reset();
        self.expected = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctl() -> Controller {
        let mut cfg = Config::default();
        cfg.auto_learn = true;
        Controller::new(cfg, None)
    }

    fn typ(c: &mut Controller, s: &str) -> Vec<Command> {
        s.chars().flat_map(|ch| c.on_input(Input::Key { key: Key::Char(ch), modifiers: false })).collect()
    }

    #[test]
    fn applies_remembered_layout_on_focus() {
        let mut c = ctl();
        c.config.set_layout("telegram", Lang::Ru);
        assert_eq!(c.tick(Some("telegram"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::Ru)]);
        // Our own switch is not "learned" as a manual change.
        assert_eq!(c.tick(Some("telegram"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::Ru)]);
        assert_eq!(c.config.layout_for("telegram"), Some(Lang::Ru));
        c.expected = Some((Lang::Ru, Instant::now() - Duration::from_secs(6)));
        assert!(c.tick(Some("telegram"), Some(Lang::Ru)).is_empty());
    }

    #[test]
    fn learns_manual_switch_and_on_leave() {
        let mut c = ctl();
        c.tick(Some("code"), Some(Lang::En));
        c.tick(Some("code"), Some(Lang::Ru));
        assert_eq!(c.config.layout_for("code"), Some(Lang::Ru));
        // Already assigned: a further manual switch does not overwrite it.
        c.tick(Some("code"), Some(Lang::En));
        assert_eq!(c.config.layout_for("code"), Some(Lang::Ru));
        c.tick(Some("firefox"), Some(Lang::En));
        c.tick(Some("slack"), Some(Lang::En));
        assert_eq!(c.config.layout_for("firefox"), Some(Lang::En));
        // Back to code: switch to Ru.
        assert_eq!(c.tick(Some("code"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::Ru)]);
    }

    #[test]
    fn no_learning_when_disabled() {
        let mut c = ctl();
        c.config.auto_learn = false;
        c.tick(Some("code"), Some(Lang::En));
        c.tick(Some("code"), Some(Lang::Ru));
        assert_eq!(c.config.layout_for("code"), None);
    }

    #[test]
    fn exception_skips_autocorrect_but_remembers_layout() {
        let mut c = ctl();
        c.tick(Some("org.keepassxc.KeePassXC"), Some(Lang::En));
        c.tick(Some("org.keepassxc.KeePassXC"), Some(Lang::Ru));
        assert_eq!(c.config.layout_for("org.keepassxc.keepassxc"), Some(Lang::Ru));
        assert!(typ(&mut c, "зуфсу ").is_empty());
        assert!(c.on_input(Input::Hotkey).is_empty());
        c.tick(Some("firefox"), Some(Lang::En));
        assert_eq!(
            c.tick(Some("org.keepassxc.KeePassXC"), Some(Lang::En)),
            vec![Command::SwitchLayout(Lang::Ru)]
        );
    }

    #[test]
    fn retries_layout_switch_until_os_applies() {
        let mut c = ctl();
        c.config.set_layout("mobaxterm", Lang::En);
        assert_eq!(c.tick(Some("mobaxterm"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.tick(Some("mobaxterm"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.tick(Some("mobaxterm"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.tick(Some("mobaxterm"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        c.expected = Some((Lang::En, Instant::now() - Duration::from_secs(6)));
        assert!(c.tick(Some("mobaxterm"), Some(Lang::En)).is_empty());
        assert!(c.tick(Some("mobaxterm"), Some(Lang::Ru)).is_empty());
        assert_eq!(c.config.layout_for("mobaxterm"), Some(Lang::En));
    }

    #[test]
    fn matching_layout_on_focus_still_starts_grace_period() {
        let mut c = ctl();
        c.config.set_layout("telegram", Lang::Ru);
        assert_eq!(c.tick(Some("telegram"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::Ru)]);
        assert!(c.expected.is_some());
        // Windows overwrites an initially matching layout after activation.
        assert_eq!(c.tick(Some("telegram"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::Ru)]);
        assert_eq!(c.config.layout_for("telegram"), Some(Lang::Ru));
    }

    #[test]
    fn minimizing_telegram_to_mobaxterm_changes_expected_layout() {
        let mut c = ctl();
        let moba = r"c:\program files (x86)\mobatek\mobaxterm\mobaxterm.exe";
        c.config.set_layout("telegram", Lang::Ru);
        c.config.set_layout(moba, Lang::En);
        assert_eq!(c.tick(Some("telegram"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::Ru)]);
        // Adapter reports the visible app, not the stale minimized Telegram HWND.
        assert_eq!(c.tick(Some(moba), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.tick(Some(moba), Some(Lang::En)), vec![Command::SwitchLayout(Lang::En)]);
        assert!(c.config.is_exception(moba));
        assert!(typ(&mut c, "зуфсу ").is_empty());
        assert!(c.on_input(Input::Hotkey).is_empty());
        assert_eq!(c.tick(Some(moba), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        // Leaving also cancels the old app's grace period.
        assert!(c.tick(Some("unassigned"), Some(Lang::Ru)).is_empty());
        assert!(c.expected.is_none());
    }

    #[test]
    fn grace_period_layout_changes_are_not_learned() {
        let mut c = ctl();
        c.tick(Some("unassigned"), Some(Lang::Ru));
        c.expected = Some((Lang::En, Instant::now()));
        assert_eq!(c.tick(Some("unassigned"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.tick(Some("unassigned"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.config.layout_for("unassigned"), None);
    }

    #[test]
    fn assigned_layout_is_sticky() {
        for learn in [true, false] {
            let mut c = ctl();
            c.config.auto_learn = learn;
            c.config.set_layout("telegram", Lang::Ru);
            assert_eq!(c.tick(Some("telegram"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::Ru)]);
            c.tick(Some("telegram"), Some(Lang::Ru));
            c.expected = Some((Lang::Ru, Instant::now() - Duration::from_secs(6)));
            // User temporarily switches to En inside the app: not forced back, not saved.
            assert!(c.tick(Some("telegram"), Some(Lang::En)).is_empty());
            assert_eq!(c.config.layout_for("telegram"), Some(Lang::Ru));
            // Leaves on En: still not saved.
            c.tick(Some("firefox"), Some(Lang::En));
            assert_eq!(c.config.layout_for("telegram"), Some(Lang::Ru));
            // Focus returns: default re-applied.
            assert_eq!(c.tick(Some("telegram"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::Ru)]);
        }
    }

    #[test]
    fn default_config_does_not_learn() {
        let mut c = Controller::new(Config::default(), None);
        c.tick(Some("code"), Some(Lang::En));
        c.tick(Some("code"), Some(Lang::Ru));
        c.tick(Some("x"), Some(Lang::En));
        assert_eq!(c.config.layout_for("code"), None);
    }

    #[test]
    fn undo_hotkey_saves_never_correct_word() {
        let dir = std::env::temp_dir().join(format!("typed-undo-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.json");
        let mut c = Controller::new(Config::default(), Some(path.clone()));
        c.tick(Some("firefox"), Some(Lang::Ru));
        assert_eq!(typ(&mut c, "зуфсу ")[0], Command::Replace(Replacement { delete: 6, insert: "peace ".into(), target: Lang::En }));
        c.replace_succeeded(Lang::En);
        let cmds = c.on_input(Input::Undo);
        assert_eq!(cmds, vec![Command::Replace(Replacement { delete: 6, insert: "зуфсу ".into(), target: Lang::Ru })]);
        c.replace_succeeded(Lang::Ru);
        assert_eq!(c.config.never_correct, vec!["зуфсу".to_string()]);
        let saved = Config::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.never_correct, vec!["зуфсу".to_string()]);
        assert!(typ(&mut c, "зуфсу ").is_empty());
        // Nothing to undo: no selection fallback, nothing happens.
        assert!(c.on_input(Input::Undo).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backspace_undo_and_defaults() {
        let cfg = Config::from_json("{}").unwrap();
        assert!(cfg.undo_on_backspace);
        assert_eq!(cfg.undo_hotkey, Hotkey::CTRL_ALT_Z);
        assert!(cfg.never_correct.is_empty());
        let mut c = ctl();
        c.tick(Some("firefox"), Some(Lang::En));
        typ(&mut c, "ghbdtn ");
        c.replace_succeeded(Lang::Ru);
        let cmds = c.on_input(Input::Key { key: Key::Backspace, modifiers: false });
        assert_eq!(cmds, vec![Command::Replace(Replacement { delete: 7, insert: "ghbdtn ".into(), target: Lang::En })]);
        c.replace_succeeded(Lang::En);
        typ(&mut c, "hello ");
        assert_eq!(c.config.never_correct, vec!["ghbdtn".to_string()]);
    }

    #[test]
    fn autocorrect_flow() {
        let mut c = ctl();
        c.tick(Some("firefox"), Some(Lang::Ru));
        let cmds = typ(&mut c, "зуфсу ");
        assert_eq!(cmds, vec![Command::Replace(Replacement { delete: 6, insert: "peace ".into(), target: Lang::En })]);
        c.replace_succeeded(Lang::En);
        // The resulting layout change is ours, not learned.
        c.tick(Some("firefox"), Some(Lang::En));
        assert_eq!(c.config.layout_for("firefox"), None);
        // Hotkey undoes.
        let cmds = c.on_input(Input::Hotkey);
        assert_eq!(cmds, vec![Command::Replace(Replacement { delete: 6, insert: "зуфсу ".into(), target: Lang::Ru })]);
        c.replace_succeeded(Lang::Ru);
        c.tick(Some("firefox"), Some(Lang::Ru));
        assert_eq!(c.config.layout_for("firefox"), None);
    }

    #[test]
    fn holding_shift_does_not_flip_layout_before_replacement_succeeds() {
        // Holding Shift may leave Replace queued in an adapter: no early switch or learning.
        let mut c = ctl();
        c.tick(Some("firefox"), Some(Lang::Ru));
        assert!(matches!(typ(&mut c, "зуфсу ").as_slice(), [Command::Replace(_)]));
        assert!(c.expected.is_none());
        assert!(c.tick(Some("firefox"), Some(Lang::Ru)).is_empty());
        assert!(c.tick(Some("firefox"), Some(Lang::En)).is_empty());
        assert_eq!(c.config.layout_for("firefox"), None);
        c.replace_succeeded(Lang::En);
        assert_eq!(c.current_layout(), Some(Lang::En));
        assert_eq!(c.tick(Some("firefox"), Some(Lang::Ru)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.tick(Some("firefox"), Some(Lang::En)), vec![Command::SwitchLayout(Lang::En)]);
        assert_eq!(c.config.layout_for("firefox"), None);
        c.expected = Some((Lang::En, Instant::now() - Duration::from_secs(6)));
        assert!(c.tick(Some("firefox"), Some(Lang::En)).is_empty());
    }

    #[test]
    fn hotkey_converts_in_progress_word_without_space() {
        let mut c = ctl();
        c.tick(Some("firefox"), Some(Lang::Ru));
        typ(&mut c, "зуфсу");
        let cmds = c.on_input(Input::Hotkey);
        assert_eq!(cmds, vec![Command::Replace(Replacement { delete: 5, insert: "peace".into(), target: Lang::En })]);
        assert!(c.expected.is_none());
        c.replace_succeeded(Lang::En);
        assert_eq!(c.current_layout(), Some(Lang::En));
    }

    #[test]
    fn failed_replacement_never_requests_its_target_layout() {
        let mut c = ctl();
        c.tick(Some("firefox"), Some(Lang::Ru));
        assert!(matches!(typ(&mut c, "зуфсу ").as_slice(), [Command::Replace(_)]));
        c.replace_failed();
        assert!(!c.replacement_pending);
        assert!(c.expected.is_none());
        assert!(c.tick(Some("firefox"), Some(Lang::Ru)).is_empty());
        assert_eq!(c.config.layout_for("firefox"), None);
    }

    #[test]
    fn focus_change_clears_word() {
        let mut c = ctl();
        c.tick(Some("a"), Some(Lang::En));
        typ(&mut c, "ghbd");
        c.tick(Some("b"), Some(Lang::En));
        assert!(typ(&mut c, "tn ").iter().all(|cmd| !matches!(cmd, Command::Replace(r) if r.insert.starts_with("привет"))));
    }

    #[test]
    fn hotkey_without_known_word_converts_selection() {
        let mut c = ctl();
        c.tick(Some("firefox"), Some(Lang::En));
        assert_eq!(c.on_input(Input::Hotkey), vec![Command::ConvertSelection]);
        // After a click the typed word is unknown too.
        typ(&mut c, "hello ");
        c.on_input(Input::Key { key: Key::Reset, modifiers: false });
        assert_eq!(c.on_input(Input::Hotkey), vec![Command::ConvertSelection]);
        // Right after typing, the hotkey still flips the last word.
        typ(&mut c, "ghbdtn");
        assert!(matches!(c.on_input(Input::Hotkey).as_slice(), [Command::Replace(_)]));
    }

    #[test]
    fn own_window_is_never_autocorrected() {
        let mut c = ctl();
        c.tick(Some(r"c:\users\x\appdata\local\typed\layout-keeper.exe"), Some(Lang::Ru));
        assert!(typ(&mut c, "ghbdtn ").is_empty());
    }

    #[test]
    fn summon_works_on_exception_app() {
        let mut c = ctl();
        c.tick(Some("org.keepassxc.KeePassXC"), Some(Lang::En));
        assert_eq!(c.on_input(Input::Summon), vec![Command::SummonGui]);
        assert!(c.on_input(Input::Hotkey).is_empty());
    }
    #[test]
    fn single_letters_follow_current_app_assignment() {
        let mut c = Controller::new(Config::default(), None);
        c.config.set_layout("chat", Lang::Ru);
        c.tick(Some("chat"), Some(Lang::En));
        assert_eq!(c.engine.options.preferred_lang, Some(Lang::Ru));
        assert!(typ(&mut c, "dashboard ").is_empty());
        assert_eq!(typ(&mut c, "d "), vec![Command::Replace(Replacement {
            delete: 2, insert: "в ".into(), target: Lang::Ru,
        })]);
        // Assignment edits are used on every input, not the last OS-reported layout.
        c.config.set_layout("chat", Lang::En);
        assert!(typ(&mut c, "q ").is_empty());
        assert_eq!(typ(&mut c, "й ")[0], Command::Replace(Replacement {
            delete: 2, insert: "q ".into(), target: Lang::En,
        }));
        c.tick(Some("unassigned"), Some(Lang::En));
        assert_eq!(c.engine.options.preferred_lang, None);
        assert!(typ(&mut c, "q ").is_empty());
        c.tick(None, None);
        assert_eq!(c.engine.options.preferred_lang, None);
    }

    #[test]
    fn reload_reapplies_current_app_language() {
        let dir = std::env::temp_dir().join(format!("lk-reload-{}", std::process::id()));
        let path = dir.join("config.json");
        let mut cfg = Config::default();
        cfg.set_layout("chat", Lang::Ru);
        cfg.save(&path).unwrap();
        let mut c = Controller::new(cfg.clone(), Some(path.clone()));
        c.tick(Some("chat"), Some(Lang::En));
        // Force a reload check without sleeping or depending on filesystem time precision.
        cfg.set_layout("chat", Lang::En);
        cfg.save(&path).unwrap();
        c.config_mtime = None;
        c.last_reload_check = Instant::now() - Duration::from_secs(2);
        c.tick(Some("chat"), Some(Lang::En));
        assert_eq!(c.engine.options.preferred_lang, Some(Lang::En));
        assert_eq!(typ(&mut c, "й ")[0], Command::Replace(Replacement {
            delete: 2, insert: "q ".into(), target: Lang::En,
        }));
        cfg.convert_single_letters = false;
        cfg.save(&path).unwrap();
        c.config_mtime = None;
        c.last_reload_check = Instant::now() - Duration::from_secs(2);
        c.tick(Some("chat"), Some(Lang::En));
        assert!(!c.engine.options.convert_single_letters);
        assert!(typ(&mut c, "в ").is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn does_not_auto_learn_hidden_or_installer_apps() {
        let mut c = ctl();
        c.tick(Some("explorer.exe"), Some(Lang::En));
        c.tick(Some("explorer.exe"), Some(Lang::Ru));
        assert_eq!(c.config.layout_for("explorer.exe"), None);
        let tmp = r"C:\Users\x\AppData\Local\Temp\typed-setup-0.3.15.tmp";
        c.tick(Some(tmp), Some(Lang::En));
        c.tick(Some("firefox"), Some(Lang::En));
        assert_eq!(c.config.layout_for(tmp), None);
        c.tick(Some("slack"), Some(Lang::En));
        assert_eq!(c.config.layout_for("firefox"), Some(Lang::En));
        assert_eq!(c.config.layout_for(tmp), None);
    }

    #[test]
    fn records_launches_for_remembered_apps() {
        let mut c = ctl();
        c.config.auto_learn = false;
        c.config.set_layout("telegram", Lang::Ru);
        c.config.set_layout("firefox", Lang::En);
        assert_eq!(c.config.launch_count("telegram"), 0);
        c.tick(Some("telegram"), Some(Lang::Ru));
        assert_eq!(c.config.launch_count("telegram"), 1);
        c.tick(Some("firefox"), Some(Lang::En));
        c.tick(Some("telegram"), Some(Lang::Ru));
        assert_eq!(c.config.launch_count("telegram"), 2);
        assert_eq!(c.config.launch_count("firefox"), 1);
    }

}

/// Our own GUI: `layout-keeper.exe` on Windows, `layout-keeper-bin` / `app.layoutkeeper` on macOS.
fn is_own_app(app: &str) -> bool {
    let a = app.to_ascii_lowercase();
    a.contains("layout-keeper.exe") || a.contains("layout-keeper-bin") || a.contains("app.layoutkeeper")
        || a.ends_with("/layout-keeper") || a.ends_with("\\layout-keeper")
}
