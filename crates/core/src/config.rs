//! Local JSON configuration: remembered per-app layouts, exceptions, hotkey, autocorrect switches.
//!
//! Stored at `<user config dir>/typed/config.json`
//! (Linux `~/.config`, macOS `~/Library/Application Support`, Windows `%APPDATA%`).
//! The file contains app identifiers and layout codes only — never typed text.

use crate::engine::EngineOptions;
use crate::layout::Lang;
use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// A key that can take part in a user-chosen hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyCode {
    Pause,
    Space,
    Tab,
    Enter,
    Escape,
    Backspace,
    /// `A`–`Z`.
    Letter(char),
    /// `0`–`9`.
    Digit(u8),
    /// `F1`–`F12`.
    F(u8),
    /// Press both Shift keys together (Punto Switcher style).
    BothShifts,
}

/// User-chosen key combination, stored as `"Ctrl+Shift+K"`.
/// Old values `Pause`, `CtrlAltSpace` and `ShiftShift` still parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: HotkeyCode,
}

#[allow(non_upper_case_globals)] // Pause / ShiftShift / CtrlAltSpace keep the old enum names.
impl Hotkey {
    pub const fn new(ctrl: bool, alt: bool, shift: bool, meta: bool, key: HotkeyCode) -> Self {
        Self { ctrl, alt, shift, meta, key }
    }

    pub const Pause: Self = Self::new(false, false, false, false, HotkeyCode::Pause);
    pub const ShiftShift: Self = Self::new(false, false, false, false, HotkeyCode::BothShifts);
    pub const CtrlAltSpace: Self = Self::new(true, true, false, false, HotkeyCode::Space);
    pub const CTRL_SHIFT_K: Self = Self::new(true, false, true, false, HotkeyCode::Letter('K'));
    pub const CTRL_SHIFT_V: Self = Self::new(true, false, true, false, HotkeyCode::Letter('V'));
    /// Default "undo the last autocorrection" (Ctrl+Option+Z on macOS). Ctrl+Shift+Z is
    /// Redo in many apps, so it is not used.
    pub const CTRL_ALT_Z: Self = Self::new(true, true, false, false, HotkeyCode::Letter('Z'));
    pub const CTRL_ALT_BACKSPACE: Self = Self::new(true, true, false, false, HotkeyCode::Backspace);
    pub const SHIFT_PAUSE: Self = Self::new(false, false, true, false, HotkeyCode::Pause);

    pub fn is_both_shifts(self) -> bool {
        matches!(self.key, HotkeyCode::BothShifts)
    }

    /// Does this combination match a physical key plus the current modifiers?
    pub fn matches(self, key: HotkeyCode, ctrl: bool, alt: bool, shift: bool, meta: bool) -> bool {
        !self.is_both_shifts()
            && self.key == key
            && self.ctrl == ctrl
            && self.alt == alt
            && self.shift == shift
            && self.meta == meta
    }

    /// Like [`matches`], but extra Shift does not block a combo that does not include Shift
    /// (so Pause still converts a word while the user is holding Shift to capitalize).
    pub fn matches_relaxed_shift(self, key: HotkeyCode, ctrl: bool, alt: bool, shift: bool, meta: bool) -> bool {
        self.matches(key, ctrl, alt, if self.shift { shift } else { false }, meta)
    }

    /// Letters, digits and ordinary keys need Ctrl, Alt or Win/Cmd so they are
    /// not stolen from normal typing. Pause, function keys and Shift+Shift are fine alone.
    pub fn is_safe(self) -> bool {
        if self.is_both_shifts() || matches!(self.key, HotkeyCode::Pause | HotkeyCode::F(_)) {
            return true;
        }
        self.ctrl || self.alt || self.meta
    }
}

impl Default for Hotkey {
    fn default() -> Self {
        // Macs have no Pause key and Ctrl+Option+Space is a system input-source shortcut.
        if cfg!(target_os = "macos") {
            Hotkey::ShiftShift
        } else {
            Hotkey::Pause
        }
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_both_shifts() {
            return f.write_str("Shift+Shift");
        }
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        if self.meta {
            out.push_str("Win+");
        }
        match self.key {
            HotkeyCode::Pause => out.push_str("Pause"),
            HotkeyCode::Space => out.push_str("Space"),
            HotkeyCode::Tab => out.push_str("Tab"),
            HotkeyCode::Enter => out.push_str("Enter"),
            HotkeyCode::Escape => out.push_str("Esc"),
            HotkeyCode::Backspace => out.push_str("Backspace"),
            HotkeyCode::Letter(c) => out.push(c.to_ascii_uppercase()),
            HotkeyCode::Digit(d) => out.push(char::from(b'0' + d)),
            HotkeyCode::F(n) => out.push_str(&format!("F{n}")),
            HotkeyCode::BothShifts => out.push_str("Shift"),
        }
        f.write_str(&out)
    }
}

impl FromStr for Hotkey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let compact: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
            .collect::<String>()
            .to_ascii_lowercase();
        match compact.as_str() {
            "pause" | "break" | "pausebreak" => return Ok(Self::Pause),
            "shiftshift" | "shift+shift" => return Ok(Self::ShiftShift),
            "ctrlaltspace" | "ctrl+alt+space" | "ctrl+option+space" => return Ok(Self::CtrlAltSpace),
            _ => {}
        }

        let tokens: Vec<&str> = s
            .split(|c: char| c == '+' || c == '-')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            return Err(format!("unknown hotkey '{s}'"));
        }

        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut meta = false;
        let mut shift_count = 0usize;
        let mut key = None;
        for tok in &tokens {
            let t = tok.to_ascii_lowercase();
            match t.as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" | "option" => alt = true,
                "shift" => {
                    shift = true;
                    shift_count += 1;
                }
                "win" | "super" | "meta" | "cmd" | "command" => meta = true,
                "pause" | "break" => key = Some(HotkeyCode::Pause),
                "space" => key = Some(HotkeyCode::Space),
                "tab" => key = Some(HotkeyCode::Tab),
                "enter" | "return" => key = Some(HotkeyCode::Enter),
                "esc" | "escape" => key = Some(HotkeyCode::Escape),
                "backspace" | "bksp" => key = Some(HotkeyCode::Backspace),
                t if t.len() == 1 && t.as_bytes()[0].is_ascii_alphabetic() => {
                    key = Some(HotkeyCode::Letter((t.as_bytes()[0] as char).to_ascii_uppercase()));
                }
                t if t.len() == 1 && t.as_bytes()[0].is_ascii_digit() => {
                    key = Some(HotkeyCode::Digit(t.as_bytes()[0] - b'0'));
                }
                t if t.starts_with('f') => {
                    let n: u8 = t[1..].parse().map_err(|_| format!("unknown hotkey '{s}'"))?;
                    if !(1..=12).contains(&n) {
                        return Err(format!("unknown hotkey '{s}'"));
                    }
                    key = Some(HotkeyCode::F(n));
                }
                _ => return Err(format!("unknown hotkey '{s}'")),
            }
        }
        if shift_count >= 2 && key.is_none() {
            return Ok(Self::ShiftShift);
        }
        let Some(key) = key else {
            return Err(format!("unknown hotkey '{s}'"));
        };
        let hk = Self { ctrl, alt, shift, meta, key };
        if !hk.is_safe() {
            return Err(format!(
                "hotkey '{s}' would steal ordinary typing; add Ctrl, Alt or Win"
            ));
        }
        Ok(hk)
    }
}

impl Serialize for Hotkey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(de::Error::custom)
    }
}

/// Default apps where LayoutKeeper never touches input (case-insensitive substring of app id).
pub fn default_exceptions() -> Vec<String> {
    [
        // Password managers
        "keepass", "1password", "bitwarden", "lastpass", "dashlane", "enpass", "pass-", "seahorse",
        "com.apple.keychainaccess",
        // Terminals / consoles (commands are not words)
        "terminal", "konsole", "xterm", "alacritty", "kitty", "wezterm", "foot", "tilix", "terminator",
        "urxvt", "iterm", "warp", "ghostty", "putty", "mobaxterm", "cmd.exe", "powershell", "pwsh", "conhost",
        "windowsterminal", "mintty",
        // Remote desktops / VMs (the remote side has its own layout)
        "mstsc", "remmina", "vncviewer", "virtualbox", "vmware", "parallels",
        // Games commonly use WASD; layout switching there is harmful
        "steam_app_",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}


/// User-facing app mark shown in the window, tray and shortcuts picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogoStyle {
    Water,
    Suprematism,
    Geometry,
}

impl Default for LogoStyle {
    fn default() -> Self {
        Self::Water
    }
}

impl LogoStyle {
    pub const ALL: [LogoStyle; 3] = [LogoStyle::Water, LogoStyle::Suprematism, LogoStyle::Geometry];
}

/// Bump when the welcome screen itself changes and should be shown once more.
pub const CURRENT_WELCOME_GEN: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Remembered layout per application id
    /// (Windows: lowercase exe path; macOS: bundle id; Linux: WM_CLASS or process name).
    pub apps: BTreeMap<String, Lang>,
    /// Fill in a default for apps that have none yet, from the layout the user picks there.
    /// Never overwrites an existing assignment (only `set-app` / the GUI change those).
    pub auto_learn: bool,
    /// Automatically fix words typed in the wrong layout.
    pub autocorrect: bool,
    /// Convert single letters toward the focused app's assigned language.
    #[serde(default = "default_convert_single_letters")]
    pub convert_single_letters: bool,
    /// Fix one-letter typos (поечаток → опечаток). Off by default.
    #[serde(default)]
    pub fix_typos: bool,
    /// Never autocorrect or hotkey-convert in these apps (substring match, case-insensitive).
    /// Layout memory still applies: assigned defaults are saved and restored on focus.
    pub exception_apps: Vec<String>,
    /// Manual convert hotkey.
    pub hotkey: Hotkey,
    /// Global shortcut that brings the settings / vault window to the front.
    #[serde(default = "default_summon_hotkey")]
    pub summon_hotkey: Hotkey,
    pub skip_urls_emails: bool,
    pub skip_tokens_with_digits: bool,
    /// Minimum letters in a word for autocorrect.
    pub min_word_len: usize,
    /// Lock the vault window after this many idle minutes (1..=120). Not a secret.
    pub vault_idle_minutes: u32,
    /// User words / abbreviations for autocorrect (single tokens, Latin or Cyrillic).
    /// They override built-in words: with `exe` here, `учу` typed in RU becomes `exe`.
    #[serde(default)]
    pub custom_words: Vec<String>,
    /// Light (white) settings window. Default is the dark style.
    #[serde(default)]
    pub light_theme: bool,
    /// Window, tray and Alt-Tab mark. Default is the original water painting.
    #[serde(default)]
    pub logo_style: LogoStyle,
    /// Skip the first-launch welcome screen.
    #[serde(default)]
    pub hide_welcome: bool,
    /// Which welcome revision the user has already been shown. Older configs are 0,
    /// so a redesigned welcome can appear once even if hide_welcome is already set.
    #[serde(default)]
    pub welcome_gen: u32,
    /// How many times each remembered app became focused. Used to sort the list.
    #[serde(default)]
    pub app_launches: BTreeMap<String, u32>,
    /// Apps the user moved into the collapsed «hidden and system» group of the list
    /// (normalised ids). Visual only: their layout is still applied on focus.
    #[serde(default)]
    pub hidden_apps: Vec<String>,
    /// Undo the last autocorrection: the typed word comes back and joins `never_correct`.
    #[serde(default = "default_undo_hotkey")]
    pub undo_hotkey: Hotkey,
    /// Backspace right after an autocorrection (before any other key) undoes it.
    #[serde(default = "default_true")]
    pub undo_on_backspace: bool,
    /// Words the user undid: never autocorrected again (normalised, lowercase).
    #[serde(default)]
    pub never_correct: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            apps: BTreeMap::new(),
            auto_learn: false,
            autocorrect: true,
            convert_single_letters: default_convert_single_letters(),
            fix_typos: false,
            exception_apps: default_exceptions(),
            hotkey: Hotkey::default(),
            summon_hotkey: default_summon_hotkey(),
            skip_urls_emails: true,
            skip_tokens_with_digits: true,
            min_word_len: 2,
            vault_idle_minutes: 10,
            custom_words: Vec::new(),
            light_theme: false,
            logo_style: LogoStyle::default(),
            hide_welcome: false,
            welcome_gen: 0,
            app_launches: BTreeMap::new(),
            hidden_apps: Vec::new(),
            undo_hotkey: default_undo_hotkey(),
            undo_on_backspace: true,
            never_correct: Vec::new(),
        }
    }
}

fn default_convert_single_letters() -> bool {
    true
}

fn default_undo_hotkey() -> Hotkey {
    Hotkey::CTRL_ALT_Z
}

fn default_true() -> bool {
    true
}

fn default_summon_hotkey() -> Hotkey {
    Hotkey::CTRL_SHIFT_V
}

const LEGACY_MIGRATE_FILES: &[&str] = &["config.json", "vault.bin", "typed.chk"];

fn migrate_legacy_dir(old_dir: &Path, new_dir: &Path) {
    if !old_dir.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(new_dir);
    for name in LEGACY_MIGRATE_FILES {
        let src = old_dir.join(name);
        let dst = new_dir.join(name);
        if src.exists() && !dst.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
    }
}

impl Config {
    /// `<config dir>/typed/config.json`, or `./typed.json` as a last resort.
    /// `LAYOUT_KEEPER_CONFIG` overrides the path.
    /// On first run, copies `config.json`, `vault.bin` and `typed.chk` from the
    /// legacy `<config dir>/layout-keeper/` folder if the new files are missing.
    pub fn default_path() -> PathBuf {
        if let Some(p) = std::env::var_os("LAYOUT_KEEPER_CONFIG") {
            return PathBuf::from(p);
        }
        match dirs::config_dir() {
            Some(base) => {
                let new_dir = base.join("typed");
                migrate_legacy_dir(&base.join("layout-keeper"), &new_dir);
                new_dir.join("config.json")
            }
            None => {
                let new_path = PathBuf::from("typed.json");
                let old_path = PathBuf::from("layout-keeper.json");
                if !new_path.exists() && old_path.exists() {
                    let _ = std::fs::copy(&old_path, &new_path);
                }
                new_path
            }
        }
    }

    /// Load from `path`; a missing file yields defaults (and is created).
    pub fn load_or_create(path: &Path) -> io::Result<Config> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::from_json(&s),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let c = Config::default();
                c.save(path)?;
                Ok(c)
            }
            Err(e) => Err(e),
        }
    }

    pub fn from_json(s: &str) -> io::Result<Config> {
        serde_json::from_str(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("config serializes")
    }

    /// Atomic save: write a temp file next to it, then rename.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)?;
            }
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_json())?;
        std::fs::rename(&tmp, path)
    }

    pub fn is_exception(&self, app_id: &str) -> bool {
        let a = app_id.to_lowercase();
        self.exception_apps.iter().any(|e| !e.is_empty() && a.contains(&e.to_lowercase()))
    }

    pub fn layout_for(&self, app_id: &str) -> Option<Lang> {
        self.apps.get(&normalize_app_id(app_id)).copied()
    }

    /// Remember `lang` for the app. Returns true if something changed.
    pub fn set_layout(&mut self, app_id: &str, lang: Lang) -> bool {
        self.apps.insert(normalize_app_id(app_id), lang) != Some(lang)
    }

    /// The user hid this app in the settings list.
    pub fn is_user_hidden(&self, app_id: &str) -> bool {
        let id = normalize_app_id(app_id);
        self.hidden_apps.iter().any(|h| *h == id)
    }

    /// Hide (or show again) the app in the settings list. Returns true if something changed.
    pub fn set_user_hidden(&mut self, app_id: &str, hidden: bool) -> bool {
        let id = normalize_app_id(app_id);
        let had = self.hidden_apps.iter().any(|h| *h == id);
        if hidden && !had {
            self.hidden_apps.push(id);
            true
        } else if !hidden && had {
            self.hidden_apps.retain(|h| *h != id);
            true
        } else {
            false
        }
    }

    pub fn remove_app(&mut self, app_id: &str) -> bool {
        let id = normalize_app_id(app_id);
        self.app_launches.remove(&id);
        self.hidden_apps.retain(|h| *h != id);
        self.apps.remove(&id).is_some()
    }

    pub fn launch_count(&self, app_id: &str) -> u32 {
        self.app_launches.get(&normalize_app_id(app_id)).copied().unwrap_or(0)
    }

    /// Count one focus of a remembered app. Returns the new total.
    pub fn record_launch(&mut self, app_id: &str) -> u32 {
        let id = normalize_app_id(app_id);
        let entry = self.app_launches.entry(id).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    pub fn engine_options(&self) -> EngineOptions {
        EngineOptions {
            autocorrect: self.autocorrect,
            convert_single_letters: self.convert_single_letters,
            preferred_lang: None,
            skip_urls_emails: self.skip_urls_emails,
            skip_tokens_with_digits: self.skip_tokens_with_digits,
            min_word_len: self.min_word_len,
            user_words: crate::userdict::UserWords::from_words(&self.custom_words),
            fix_typos: self.fix_typos,
            never_correct: self.never_correct.iter().map(|w| crate::engine::never_key(w)).filter(|w| !w.is_empty()).collect(),
            undo_on_backspace: self.undo_on_backspace,
        }
    }

    /// Add a never-correct word (normalised); `false` if it was already there or empty.
    pub fn add_never_correct(&mut self, word: &str) -> bool {
        let key = crate::engine::never_key(word);
        if key.is_empty() || self.never_correct.iter().any(|w| crate::engine::never_key(w) == key) {
            return false;
        }
        self.never_correct.push(key);
        true
    }
}

/// App ids are compared case-insensitively (Windows paths, WM_CLASS variations).
pub fn normalize_app_id(id: &str) -> String {
    id.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_words_default_and_old_json() {
        assert!(Config::default().custom_words.is_empty());
        let c: Config = serde_json::from_str(r#"{"autocorrect": true}"#).unwrap();
        assert!(c.custom_words.is_empty());
        let c: Config = serde_json::from_str(r#"{"custom_words": ["exe", "имхо"]}"#).unwrap();
        assert_eq!(c.custom_words, vec!["exe", "имхо"]);
        assert_eq!(c.engine_options().user_words.len(), 2);
    }

    #[test]
    fn terminals_are_exceptions_including_mobaxterm() {
        let c = Config::default();
        // Autocorrect stays off in terminals; layout memory still applies.
        assert!(c.is_exception(r"C:\Program Files\Mobatek\MobaXterm\MobaXterm.exe"));
        assert!(c.is_exception(r"c:\program files (x86)\mobatek\mobaxterm\mobaxterm.exe"));
        assert!(c.is_exception(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"));
        for a in ["gnome-terminal-server", "keepassxc", "xterm", "/usr/bin/xterm", "putty"] {
            assert!(c.is_exception(a), "{a}");
        }
        assert!(!c.is_exception("firefox"));
    }

    #[test]
    fn defaults() {
        let c = Config::default();
        assert!(!c.auto_learn && c.autocorrect);
        assert!(c.is_exception("org.keepassxc.KeePassXC"));
        assert!(c.is_exception("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"));
        assert!(c.is_exception("gnome-terminal-server"));
        assert!(!c.is_exception("firefox"));
    }

    #[test]
    fn roundtrip_and_partial_json() {
        let mut c = Config::default();
        assert!(c.set_layout("Firefox", Lang::En));
        assert!(!c.set_layout("firefox", Lang::En));
        c.set_layout("org.telegram.desktop", Lang::Ru);
        let back = Config::from_json(&c.to_json()).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.layout_for("FIREFOX"), Some(Lang::En));
        // Unknown/missing fields fall back to defaults.
        let p = Config::from_json(r#"{"apps":{"code":"en"},"hotkey":"ShiftShift"}"#).unwrap();
        assert_eq!(p.layout_for("code"), Some(Lang::En));
        assert_eq!(p.hotkey, Hotkey::ShiftShift);
        assert_eq!(p.summon_hotkey, Hotkey::CTRL_SHIFT_V);
        assert!(p.autocorrect);
    }

    #[test]
    fn save_load_file() {
        let dir = std::env::temp_dir().join(format!("lk-test-{}", std::process::id()));
        let path = dir.join("config.json");
        let _ = std::fs::remove_dir_all(&dir);
        let mut c = Config::load_or_create(&path).unwrap();
        assert!(path.exists());
        c.set_layout("slack", Lang::Ru);
        c.save(&path).unwrap();
        assert_eq!(Config::load_or_create(&path).unwrap().layout_for("slack"), Some(Lang::Ru));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migrates_legacy_config_dir_without_overwriting() {
        let root = std::env::temp_dir().join(format!("typed-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let old = root.join("layout-keeper");
        let new = root.join("typed");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("config.json"), r#"{"autocorrect":false}"#).unwrap();
        std::fs::write(old.join("vault.bin"), b"secret").unwrap();
        super::migrate_legacy_dir(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("config.json")).unwrap(), r#"{"autocorrect":false}"#);
        assert_eq!(std::fs::read(new.join("vault.bin")).unwrap(), b"secret");
        std::fs::write(new.join("config.json"), r#"{"autocorrect":true}"#).unwrap();
        super::migrate_legacy_dir(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("config.json")).unwrap(), r#"{"autocorrect":true}"#);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn hotkey_parse() {
        assert_eq!("pause".parse::<Hotkey>().unwrap(), Hotkey::Pause);
        assert_eq!("Ctrl+Alt+Space".parse::<Hotkey>().unwrap(), Hotkey::CtrlAltSpace);
        assert_eq!("shift-shift".parse::<Hotkey>().unwrap(), Hotkey::ShiftShift);
        assert_eq!("Ctrl+Shift+K".parse::<Hotkey>().unwrap(), Hotkey::CTRL_SHIFT_K);
        assert_eq!(Hotkey::CTRL_SHIFT_K.to_string(), "Ctrl+Shift+K");
        assert_eq!("Ctrl+Shift+V".parse::<Hotkey>().unwrap(), Hotkey::CTRL_SHIFT_V);
        assert_eq!(Hotkey::CTRL_SHIFT_V.to_string(), "Ctrl+Shift+V");
        assert!(Hotkey::Pause.matches_relaxed_shift(HotkeyCode::Pause, false, false, true, false));
        assert!(!Hotkey::CTRL_SHIFT_V.matches_relaxed_shift(HotkeyCode::Letter('V'), true, false, false, false));
        assert!(Hotkey::CTRL_SHIFT_V.matches_relaxed_shift(HotkeyCode::Letter('V'), true, false, true, false));
        assert!("K".parse::<Hotkey>().is_err());
        assert!("f13".parse::<Hotkey>().is_err());
    }
    #[test]
    fn theme_and_launches_default_from_old_json() {
        let old = Config::from_json(r#"{"autocorrect":true,"apps":{"chat":"ru"}}"#).unwrap();
        assert!(!old.light_theme);
        assert_eq!(old.logo_style, LogoStyle::Water);
        assert!(!old.hide_welcome);
        assert_eq!(old.welcome_gen, 0);
        assert!(old.app_launches.is_empty());
        assert_eq!(old.launch_count("chat"), 0);
        let mut c = Config::default();
        c.set_layout("Firefox", Lang::En);
        assert_eq!(c.launch_count("firefox"), 0);
        assert_eq!(c.record_launch("FIREFOX"), 1);
        assert_eq!(c.record_launch("firefox"), 2);
        assert_eq!(c.launch_count("firefox"), 2);
        assert!(c.remove_app("firefox"));
        assert_eq!(c.launch_count("firefox"), 0);
        let light = Config::from_json(r#"{"light_theme":true}"#).unwrap();
        assert!(light.light_theme);
        assert_eq!(Config::from_json(&light.to_json()).unwrap(), light);
    }

    #[test]
    fn hidden_apps_default_and_roundtrip() {
        let old = Config::from_json(r#"{"autocorrect":true,"apps":{"chat":"ru"}}"#).unwrap();
        assert!(old.hidden_apps.is_empty());
        let mut c = old.clone();
        assert!(c.set_user_hidden("CHAT", true));
        assert!(!c.set_user_hidden("chat", true));
        assert!(c.is_user_hidden("Chat"));
        // Hiding is visual only: the layout stays assigned.
        assert_eq!(c.layout_for("chat"), Some(Lang::Ru));
        let back = Config::from_json(&c.to_json()).unwrap();
        assert!(back.is_user_hidden("chat"));
        assert!(c.set_user_hidden("chat", false));
        assert!(!c.is_user_hidden("chat"));
        c.set_user_hidden("chat", true);
        assert!(c.remove_app("chat"));
        assert!(!c.is_user_hidden("chat"));
    }

    #[test]
    fn single_letter_setting_defaults_and_roundtrips() {
        let old = Config::from_json(r#"{"autocorrect":true,"apps":{"chat":"ru"}}"#).unwrap();
        assert!(old.convert_single_letters);
        assert!(old.engine_options().convert_single_letters);
        assert_eq!(old.engine_options().preferred_lang, None);
        let disabled = Config::from_json(r#"{"convert_single_letters":false}"#).unwrap();
        assert!(!disabled.engine_options().convert_single_letters);
        assert_eq!(Config::from_json(&disabled.to_json()).unwrap(), disabled);
    }

}
