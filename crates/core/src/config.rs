
use crate::engine::EngineOptions;
use crate::layout::Lang;
use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyCode {
    Pause,
    Space,
    Tab,
    Enter,
    Escape,
    Backspace,
    Letter(char),
    Digit(u8),
    F(u8),
    BothShifts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: HotkeyCode,
}

#[allow(non_upper_case_globals)]
impl Hotkey {
    pub const fn new(ctrl: bool, alt: bool, shift: bool, meta: bool, key: HotkeyCode) -> Self {
        Self { ctrl, alt, shift, meta, key }
    }

    pub const Pause: Self = Self::new(false, false, false, false, HotkeyCode::Pause);
    pub const ShiftShift: Self = Self::new(false, false, false, false, HotkeyCode::BothShifts);
    pub const CtrlAltSpace: Self = Self::new(true, true, false, false, HotkeyCode::Space);
    pub const CTRL_SHIFT_K: Self = Self::new(true, false, true, false, HotkeyCode::Letter('K'));
    pub const CTRL_SHIFT_V: Self = Self::new(true, false, true, false, HotkeyCode::Letter('V'));
    pub const CTRL_ALT_Z: Self = Self::new(true, true, false, false, HotkeyCode::Letter('Z'));
    pub const CTRL_ALT_BACKSPACE: Self = Self::new(true, true, false, false, HotkeyCode::Backspace);
    pub const SHIFT_PAUSE: Self = Self::new(false, false, true, false, HotkeyCode::Pause);
    pub const BACKSPACE: Self = Self::new(false, false, false, false, HotkeyCode::Backspace);

    pub fn is_plain_backspace(self) -> bool {
        self == Self::BACKSPACE
    }

    pub fn is_both_shifts(self) -> bool {
        matches!(self.key, HotkeyCode::BothShifts)
    }

    pub fn matches(self, key: HotkeyCode, ctrl: bool, alt: bool, shift: bool, meta: bool) -> bool {
        !self.is_both_shifts()
            && self.key == key
            && self.ctrl == ctrl
            && self.alt == alt
            && self.shift == shift
            && self.meta == meta
    }

    pub fn matches_relaxed_shift(self, key: HotkeyCode, ctrl: bool, alt: bool, shift: bool, meta: bool) -> bool {
        self.matches(key, ctrl, alt, if self.shift { shift } else { false }, meta)
    }

    pub fn is_safe(self) -> bool {
        if self.is_both_shifts() || matches!(self.key, HotkeyCode::Pause | HotkeyCode::F(_) | HotkeyCode::Backspace) {
            return true;
        }
        self.ctrl || self.alt || self.meta
    }
}

impl Default for Hotkey {
    fn default() -> Self {
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

pub fn default_exceptions() -> Vec<String> {
    [
        "keepass", "1password", "bitwarden", "lastpass", "dashlane", "enpass", "pass-", "seahorse",
        "com.apple.keychainaccess",
        "terminal", "konsole", "xterm", "alacritty", "kitty", "wezterm", "foot", "tilix", "terminator",
        "urxvt", "iterm", "warp", "ghostty", "putty", "mobaxterm", "cmd.exe", "powershell", "pwsh", "conhost",
        "windowsterminal", "mintty",
        "mstsc", "remmina", "vncviewer", "virtualbox", "vmware", "parallels",
        "steam_app_",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogoStyle {
    Beach,
    Artdeco,
    Ukiyoe,
    Poster,
    Porthole,
}

impl Default for LogoStyle {
    fn default() -> Self {
        Self::Beach
    }
}

impl LogoStyle {
    pub const ALL: [LogoStyle; 5] = [
        LogoStyle::Beach,
        LogoStyle::Artdeco,
        LogoStyle::Ukiyoe,
        LogoStyle::Poster,
        LogoStyle::Porthole,
    ];
}

impl<'de> Deserialize<'de> for LogoStyle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_ascii_lowercase().as_str() {
            "beach" | "water" => LogoStyle::Beach,
            "artdeco" | "art_deco" => LogoStyle::Artdeco,
            "ukiyoe" => LogoStyle::Ukiyoe,
            "poster" => LogoStyle::Poster,
            "porthole" => LogoStyle::Porthole,
            _ => LogoStyle::Beach,
        })
    }
}

pub const CURRENT_WELCOME_GEN: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub apps: BTreeMap<String, Lang>,
    pub auto_learn: bool,
    pub autocorrect: bool,
    #[serde(default = "default_convert_single_letters")]
    pub convert_single_letters: bool,
    #[serde(default = "default_true")]
    pub fix_typos: bool,
    pub exception_apps: Vec<String>,
    pub hotkey: Hotkey,
    #[serde(default = "default_summon_hotkey")]
    pub summon_hotkey: Hotkey,
    pub skip_urls_emails: bool,
    pub skip_tokens_with_digits: bool,
    pub min_word_len: usize,
    pub vault_idle_minutes: u32,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub light_theme: bool,
    #[serde(default, alias = "logo_style")]
    pub logo: LogoStyle,
    #[serde(default)]
    pub hide_welcome: bool,
    #[serde(default)]
    pub welcome_gen: u32,
    #[serde(default)]
    pub app_launches: BTreeMap<String, u32>,
    #[serde(default)]
    pub hidden_apps: Vec<String>,
    #[serde(default = "default_undo_hotkey")]
    pub undo_hotkey: Hotkey,
    #[serde(default = "default_true")]
    pub undo_on_backspace: bool,
    #[serde(default = "default_undo_key")]
    pub undo_key: Hotkey,
    #[serde(default)]
    pub never_correct: Vec<String>,
    #[serde(default)]
    pub typo_level: crate::typo::Level,
    #[serde(default)]
    pub never_typo_fix: Vec<String>,
    #[serde(default)]
    pub undone_once: Vec<String>,
    #[serde(default = "default_true")]
    pub use_context: bool,
    #[serde(default = "default_true")]
    pub fix_missing_spaces: bool,
    #[serde(default)]
    pub run_as_admin: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            apps: BTreeMap::new(),
            auto_learn: true,
            autocorrect: true,
            convert_single_letters: default_convert_single_letters(),
            fix_typos: true,
            exception_apps: default_exceptions(),
            hotkey: Hotkey::default(),
            summon_hotkey: default_summon_hotkey(),
            skip_urls_emails: true,
            skip_tokens_with_digits: true,
            min_word_len: 2,
            vault_idle_minutes: 10,
            custom_words: Vec::new(),
            light_theme: false,
            logo: LogoStyle::default(),
            hide_welcome: false,
            welcome_gen: 0,
            app_launches: BTreeMap::new(),
            hidden_apps: Vec::new(),
            undo_hotkey: default_undo_hotkey(),
            undo_on_backspace: true,
            undo_key: default_undo_key(),
            never_correct: Vec::new(),
            typo_level: crate::typo::Level::default(),
            never_typo_fix: Vec::new(),
            undone_once: Vec::new(),
            use_context: true,
            fix_missing_spaces: true,
            run_as_admin: false,
        }
    }
}

fn default_convert_single_letters() -> bool {
    true
}

fn default_undo_hotkey() -> Hotkey {
    Hotkey::BACKSPACE
}

fn default_undo_key() -> Hotkey {
    Hotkey::BACKSPACE
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
        let mut c: Config = serde_json::from_str(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        c.dedupe_app_lists();
        Ok(c)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("config serializes")
    }

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
        let exe = exe_name(app_id);
        self.exception_apps.iter().any(|e| {
            if e.is_empty() {
                return false;
            }
            let el = e.to_lowercase();
            a.contains(&el) || (!exe.is_empty() && exe_name(e) == exe)
        })
    }

    pub fn layout_for(&self, app_id: &str) -> Option<Lang> {
        let id = normalize_app_id(app_id);
        if let Some(l) = self.apps.get(&id) {
            return Some(*l);
        }
        let exe = exe_name(&id);
        if exe.is_empty() {
            return None;
        }
        self.apps.iter().find(|(k, _)| exe_name(k) == exe).map(|(_, l)| *l)
    }

    pub fn set_layout(&mut self, app_id: &str, lang: Lang) -> bool {
        let key = storage_app_id(app_id);
        let exe = exe_name(&key);
        let mut changed = false;
        if !exe.is_empty() {
            let drop: Vec<String> = self
                .apps
                .keys()
                .filter(|k| *k != &key && exe_name(k) == exe)
                .cloned()
                .collect();
            for k in drop {
                self.apps.remove(&k);
                changed = true;
            }
        }
        changed | (self.apps.insert(key, lang) != Some(lang))
    }

    pub fn prefer_app_path(&mut self, app_id: &str) -> bool {
        let key = storage_app_id(app_id);
        if !is_path_id(&key) {
            return false;
        }
        let exe = exe_name(&key);
        if exe.is_empty() {
            return false;
        }
        let mut changed = false;
        let app_keys: Vec<String> = self
            .apps
            .keys()
            .filter(|k| exe_name(k) == exe && *k != &key)
            .cloned()
            .collect();
        for k in app_keys {
            if let Some(lang) = self.apps.remove(&k) {
                self.apps.insert(key.clone(), lang);
                changed = true;
            }
        }
        if self.apps.contains_key(&exe) && exe != key {
            if let Some(lang) = self.apps.remove(&exe) {
                self.apps.insert(key.clone(), lang);
                changed = true;
            }
        }
        let exc_keys: Vec<String> = self
            .exception_apps
            .iter()
            .filter(|e| exe_name(e) == exe && *e != &key)
            .cloned()
            .collect();
        if !exc_keys.is_empty() {
            self.exception_apps.retain(|e| exe_name(e) != exe);
            self.exception_apps.push(key);
            changed = true;
        }
        changed
    }

    pub fn is_user_hidden(&self, app_id: &str) -> bool {
        let id = normalize_app_id(app_id);
        self.hidden_apps.iter().any(|h| *h == id)
    }

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
        let exe = exe_name(&id);
        let keys: Vec<String> = self
            .apps
            .keys()
            .filter(|k| *k == &id || (!exe.is_empty() && exe_name(k) == exe))
            .cloned()
            .collect();
        let mut changed = false;
        for k in &keys {
            self.app_launches.remove(k);
            self.hidden_apps.retain(|h| h != k && (exe.is_empty() || exe_name(h) != exe));
            changed |= self.apps.remove(k).is_some();
        }
        changed
    }

    pub fn add_exception(&mut self, app_id: &str) -> bool {
        let key = storage_app_id(app_id);
        if key.is_empty() {
            return false;
        }
        let exe = exe_name(&key);
        let changed = self.remove_app(&key);
        if self.exception_apps.iter().any(|e| {
            e.eq_ignore_ascii_case(&key) || (!exe.is_empty() && exe_name(e) == exe)
        }) {
            return changed;
        }
        self.exception_apps.push(key);
        true
    }

    pub fn remove_exception(&mut self, app_id: &str) -> bool {
        let id = normalize_app_id(app_id);
        let exe = exe_name(&id);
        let before = self.exception_apps.len();
        self.exception_apps.retain(|e| {
            e.to_lowercase() != id && (exe.is_empty() || exe_name(e) != exe)
        });
        self.exception_apps.len() != before
    }

    pub fn dedupe_app_lists(&mut self) {
        let mut apps: BTreeMap<String, Lang> = BTreeMap::new();
        for (k, lang) in std::mem::take(&mut self.apps) {
            let key = storage_app_id(&k);
            if key.is_empty() {
                continue;
            }
            let exe = exe_name(&key);
            if !exe.is_empty() {
                if let Some(existing) = apps.keys().find(|e| exe_name(e) == exe).cloned() {
                    if is_path_id(&key) && !is_path_id(&existing) {
                        apps.remove(&existing);
                        apps.insert(key, lang);
                    } else {
                        apps.insert(existing, lang);
                    }
                    continue;
                }
            }
            apps.insert(key, lang);
        }
        let mut seen = std::collections::HashSet::new();
        let mut exceptions: Vec<String> = Vec::new();
        for e in std::mem::take(&mut self.exception_apps) {
            if e.trim().is_empty() {
                continue;
            }
            let key = storage_app_id(&e);
            let exe = exe_name(&key);
            let dedupe_key = if exe.is_empty() { key.clone() } else { exe };
            if let Some(i) = exceptions.iter().position(|old| {
                let oe = exe_name(old);
                (!oe.is_empty() && oe == exe_name(&key)) || old == &key
            }) {
                if is_path_id(&key) && !is_path_id(&exceptions[i]) {
                    exceptions[i] = key;
                }
                continue;
            }
            if !seen.insert(dedupe_key) {
                continue;
            }
            exceptions.push(key);
        }
        apps.retain(|k, _| {
            let exe = exe_name(k);
            !exceptions.iter().any(|e| {
                (!exe.is_empty() && exe_name(e) == exe) || e.eq_ignore_ascii_case(k)
            })
        });
        self.apps = apps;
        self.exception_apps = exceptions;
    }

    pub fn launch_count(&self, app_id: &str) -> u32 {
        self.app_launches.get(&normalize_app_id(app_id)).copied().unwrap_or(0)
    }

    pub fn record_launch(&mut self, app_id: &str) -> u32 {
        let id = normalize_app_id(app_id);
        let entry = self.app_launches.entry(id).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    pub fn engine_options(&self) -> EngineOptions {
        EngineOptions {
            autocorrect: self.autocorrect,
            convert_single_letters: true,
            preferred_lang: None,
            skip_urls_emails: self.skip_urls_emails,
            skip_tokens_with_digits: self.skip_tokens_with_digits,
            min_word_len: self.min_word_len,
            user_words: crate::userdict::UserWords::from_words(&self.custom_words),
            fix_typos: true,
            never_correct: self.never_correct.iter().map(|w| crate::engine::never_key(w)).filter(|w| !w.is_empty()).collect(),
            undo_on_backspace: self.undo_on_backspace,
            fix_missing_spaces: true,
            typo_level: self.typo_level,
            never_typo: self.never_typo_fix.iter().map(|w| crate::engine::never_key(w)).filter(|w| !w.is_empty()).collect(),
            undone_once: self.undone_once.iter().cloned().collect(),
            use_context: true,
        }
    }

    pub fn learns_layouts(&self) -> bool {
        true
    }

    pub fn add_never_typo(&mut self, word: &str) -> bool {
        let key = crate::engine::never_key(word);
        if key.is_empty() || self.never_typo_fix.iter().any(|w| crate::engine::never_key(w) == key) {
            return false;
        }
        self.never_typo_fix.push(key);
        true
    }

    pub fn apply_learn(&mut self, l: &crate::engine::Learned) -> bool {
        use crate::engine::Learned;
        match l {
            Learned::NeverCorrect(w) => {
                let key = crate::engine::never_key(w);
                self.undone_once.retain(|u| u != &key);
                self.add_never_correct(w)
            }
            Learned::NeverTypo(w) => {
                let key = format!("typo:{}", crate::engine::never_key(w));
                self.undone_once.retain(|u| u != &key);
                self.add_never_typo(w)
            }
            Learned::UndoneOnce(key) => {
                if self.undone_once.contains(key) {
                    return false;
                }
                self.undone_once.push(key.clone());
                if self.undone_once.len() > UNDONE_ONCE_MAX {
                    self.undone_once.remove(0);
                }
                true
            }
        }
    }

    pub fn add_never_correct(&mut self, word: &str) -> bool {
        let key = crate::engine::never_key(word);
        if key.is_empty() || self.never_correct.iter().any(|w| crate::engine::never_key(w) == key) {
            return false;
        }
        self.never_correct.push(key);
        true
    }
}

pub const UNDONE_ONCE_MAX: usize = 200;

pub fn normalize_app_id(id: &str) -> String {
    id.trim().to_lowercase()
}

pub fn exe_name(id: &str) -> String {
    let t = id.trim().trim_end_matches(['/', '\\']);
    t.rsplit(['/', '\\']).next().unwrap_or(t).to_lowercase()
}

fn is_path_id(id: &str) -> bool {
    id.contains('/') || id.contains('\\')
}

/// Basename of a path, otherwise the normalised id (bundle id, WM_CLASS, bare exe).
/// Used for matching; storage uses [`storage_app_id`] so icon extraction can keep the path.
pub fn canonical_app_id(id: &str) -> String {
    let n = normalize_app_id(id);
    if is_path_id(&n) {
        exe_name(&n)
    } else {
        n
    }
}

/// Id stored in `apps` / `exception_apps`: keep a full path when we have one,
/// otherwise the normalised bare id.
pub fn storage_app_id(id: &str) -> String {
    let n = normalize_app_id(id);
    n.trim_end_matches(['/', '\\']).to_string()
}

pub fn is_path_app_id(id: &str) -> bool {
    is_path_id(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typo_level_and_undo_learning_0_3_72() {
        use crate::engine::Learned;
        use crate::typo::Level;
        // Old configs get the new default level.
        let c = Config::from_json("{}").unwrap();
        assert_eq!(c.typo_level, Level::Normal);
        assert!(c.never_typo_fix.is_empty() && c.undone_once.is_empty());
        let c = Config::from_json(r#"{"typo_level":"cautious"}"#).unwrap();
        assert_eq!(c.typo_level, Level::Cautious);
        assert_eq!(c.engine_options().typo_level, Level::Cautious);
        // Learning: once noted, the second time permanent (and the note goes away).
        let mut c = Config::default();
        assert!(c.apply_learn(&Learned::UndoneOnce("зуфсу".into())));
        assert!(!c.apply_learn(&Learned::UndoneOnce("зуфсу".into())));
        assert!(c.apply_learn(&Learned::NeverCorrect("Зуфсу".into())));
        assert_eq!((c.never_correct.clone(), c.undone_once.clone()), (vec!["зуфсу".to_string()], vec![]));
        assert!(c.apply_learn(&Learned::UndoneOnce("typo:чтоо".into())));
        assert!(c.apply_learn(&Learned::NeverTypo("чтоо".into())));
        assert_eq!(c.never_typo_fix, vec!["чтоо".to_string()]);
        assert!(c.undone_once.is_empty());
        assert!(c.engine_options().never_typo.contains("чтоо"));
        // Bounded list of single undos.
        for i in 0..(UNDONE_ONCE_MAX + 5) {
            c.apply_learn(&Learned::UndoneOnce(format!("w{i}")));
        }
        assert_eq!(c.undone_once.len(), UNDONE_ONCE_MAX);
    }

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
        assert!(c.auto_learn && c.autocorrect && c.fix_typos);
        assert!(c.use_context && c.undo_on_backspace && c.convert_single_letters && c.fix_missing_spaces);
        assert!(!c.light_theme && !c.run_as_admin && !c.hide_welcome);
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
        assert_eq!(old.logo, LogoStyle::Beach);
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
        assert!(!old.run_as_admin);
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
        assert!(!disabled.convert_single_letters);
        assert!(disabled.engine_options().convert_single_letters);
        assert_eq!(Config::from_json(&disabled.to_json()).unwrap(), disabled);
    }

    #[test]
    fn missing_spaces_setting_defaults_on_and_roundtrips() {
        let old = Config::from_json(r#"{"autocorrect":true,"apps":{"chat":"ru"}}"#).unwrap();
        assert!(old.fix_missing_spaces);
        assert!(old.engine_options().fix_missing_spaces);
        assert!(Config::default().fix_missing_spaces);
        let off = Config::from_json(r#"{"fix_missing_spaces":false}"#).unwrap();
        assert!(!off.fix_missing_spaces);
        assert!(off.engine_options().fix_missing_spaces);
        assert_eq!(Config::from_json(&off.to_json()).unwrap(), off);
    }

    #[test]
    fn feature_toggles_default_on_but_saved_false_is_kept() {
        let fresh = Config::from_json("{}").unwrap();
        assert!(fresh.fix_typos && fresh.auto_learn && fresh.use_context);
        assert!(fresh.autocorrect && fresh.convert_single_letters && fresh.fix_missing_spaces && fresh.undo_on_backspace);
        let saved = Config::from_json(r#"{"fix_typos":false,"auto_learn":false,"use_context":false,"convert_single_letters":false,"fix_missing_spaces":false,"undo_on_backspace":false}"#).unwrap();
        assert!(!saved.fix_typos && !saved.auto_learn && !saved.use_context);
        assert!(!saved.convert_single_letters && !saved.fix_missing_spaces && !saved.undo_on_backspace);
        let o = saved.engine_options();
        assert!(o.fix_typos && o.use_context && o.convert_single_letters && o.fix_missing_spaces);
        assert!(!o.undo_on_backspace);
        assert!(saved.learns_layouts());
        assert_eq!(Config::from_json(&saved.to_json()).unwrap(), saved);
    }

    #[test]
    fn undo_key_defaults_to_backspace_and_old_json_loads() {
        let fresh = Config::from_json("{}").unwrap();
        assert!(fresh.undo_on_backspace);
        assert_eq!(fresh.undo_key, Hotkey::BACKSPACE);
        assert_eq!(fresh.undo_hotkey, Hotkey::BACKSPACE);
        assert_eq!(fresh.engine_options().undo_on_backspace, true);
        let old = Config::from_json(r#"{"undo_hotkey":"Ctrl+Alt+Z"}"#).unwrap();
        assert_eq!(old.undo_hotkey, Hotkey::CTRL_ALT_Z);
        assert_eq!(old.undo_key, Hotkey::BACKSPACE);
        assert!(old.undo_on_backspace);
        let off = Config::from_json(r#"{"undo_on_backspace":false,"undo_key":"Ctrl+Alt+Z"}"#).unwrap();
        assert!(!off.undo_on_backspace && !off.engine_options().undo_on_backspace);
        assert_eq!(off.undo_key, Hotkey::CTRL_ALT_Z);
        assert_eq!("Backspace".parse::<Hotkey>().unwrap(), Hotkey::BACKSPACE);
        assert!(Hotkey::BACKSPACE.is_safe() && Hotkey::BACKSPACE.is_plain_backspace());
        assert_eq!(Config::from_json(&off.to_json()).unwrap(), off);
    }

    #[test]
    fn logo_defaults_to_beach_and_old_json_loads() {
        assert_eq!(Config::from_json("{}").unwrap().logo, LogoStyle::Beach);
        let named = Config::from_json(r#"{"logo":"ukiyoe"}"#).unwrap();
        assert_eq!(named.logo, LogoStyle::Ukiyoe);
        let alias = Config::from_json(r#"{"logo_style":"water"}"#).unwrap();
        assert_eq!(alias.logo, LogoStyle::Beach);
        let old = Config::from_json(r#"{"logo_style":"suprematism"}"#).unwrap();
        assert_eq!(old.logo, LogoStyle::Beach);
        assert_eq!(Config::from_json(&named.to_json()).unwrap(), named);
    }

    #[test]
    fn dedupes_apps_and_exceptions_by_exe_name() {
        let c = Config::from_json(r#"{
            "apps": {
                "c:\\users\\a\\layout-keeper.exe": "en",
                "LAYOUT-KEEPER.EXE": "ru",
                "c:\\program files\\google\\chrome\\application\\chrome.exe": "en"
            },
            "exception_apps": [
                "c:\\temp\\innosetup-6.7.1.tmp",
                "InnoSetup-6.7.1.tmp",
                "c:\\app\\layout-keeper.exe",
                "powershell"
            ]
        }"#).unwrap();
        assert_eq!(c.apps.len(), 1);
        assert!(c.apps.keys().any(|k| k.contains("google") && k.ends_with("chrome.exe")));
        assert_eq!(c.layout_for(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), Some(Lang::En));
        assert_eq!(c.layout_for("CHROME.EXE"), Some(Lang::En));
        assert!(c.layout_for("layout-keeper.exe").is_none());
        assert!(c.is_exception(r"C:\Program Files\Typed\layout-keeper.exe"));
        assert_eq!(c.exception_apps.iter().filter(|e| exe_name(e) == "innosetup-6.7.1.tmp").count(), 1);
        assert_eq!(c.exception_apps.iter().filter(|e| exe_name(e) == "layout-keeper.exe").count(), 1);
        assert!(c.exception_apps.iter().any(|e| e == "powershell"));
        let mut c = c;
        assert!(!c.add_exception("layout-keeper.exe"));
        assert!(c.add_exception(r"C:\Temp\Foo.EXE"));
        assert!(c.exception_apps.iter().any(|e| exe_name(e) == "foo.exe" && e.contains("temp")));
        let mut c = Config::default();
        assert!(c.set_layout("chrome.exe", Lang::En));
        assert!(c.prefer_app_path(r"C:\Program Files\Google\Chrome\Application\chrome.exe"));
        assert!(c.apps.keys().any(|k| k.ends_with("chrome.exe") && k.contains("google")));
        assert_eq!(c.layout_for("chrome.exe"), Some(Lang::En));
        assert_eq!(c.apps.len(), 1);
    }
}
