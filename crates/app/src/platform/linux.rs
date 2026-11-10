//! Linux adapter — X11 (full) with an honest Wayland check.
//!
//! * focused app: EWMH `_NET_ACTIVE_WINDOW` → `WM_CLASS` (res_class), fallback `_NET_WM_PID` → `/proc/<pid>/comm`;
//! * layout: XKB group state (`XkbGetState`), group names (`XkbGetNames`) mapped to en/ru,
//!   switching with `XkbLockGroup`;
//! * keys: XRecord extension on a separate connection (read-only observation of key/button events);
//! * replacement: XTest fake key events — N × BackSpace, then the corrected word replayed as
//!   *physical keys* with the target XKB group locked (no keymap hacking needed).
//!
//! All Xlib/X11 libraries are loaded at runtime (`x11-dl`), so the binary builds without X11
//! headers and fails gracefully with a message when no X server is reachable.
//!
//! Wayland: global key observation and synthetic input are deliberately impossible for normal
//! clients. Under a pure Wayland session we refuse to start and explain why (see README).

use crate::controller::{Command, Controller, Input};
use layout_keeper_core::{layout, HotkeyCode, Key, Lang, Replacement};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong};
use std::ptr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};
use x11_dl::xlib::{self, Display, Xlib};
use x11_dl::{xrecord, xtest};

const XKB_USE_CORE_KBD: c_uint = 0x0100;
const XKB_GROUP_NAMES_MASK: c_uint = 1 << 12;
const KEY_PRESS: u8 = 2;
const KEY_RELEASE: u8 = 3;
const BUTTON_PRESS: u8 = 4;

// Keysyms we care about.
const XK_BACKSPACE: c_ulong = 0xff08;
const XK_TAB: c_ulong = 0xff09;
const XK_RETURN: c_ulong = 0xff0d;
const XK_KP_ENTER: c_ulong = 0xff8d;
const XK_PAUSE: c_ulong = 0xff13;
const XK_SHIFT_L: c_ulong = 0xffe1;
const XK_SHIFT_R: c_ulong = 0xffe2;
const XK_CONTROL_L: c_ulong = 0xffe3;
const XK_CONTROL_R: c_ulong = 0xffe4;
const XK_CAPS_LOCK: c_ulong = 0xffe5;
const XK_META_L: c_ulong = 0xffe7;
const XK_SUPER_R: c_ulong = 0xffec;
const XK_ISO_LEVEL3_SHIFT: c_ulong = 0xfe03;
const XK_SPACE: c_ulong = 0x20;
const XK_ESCAPE: c_ulong = 0xff1b;
const XK_F1: c_ulong = 0xffbe;
const XK_F12: c_ulong = 0xffc9;

fn keysym_to_hotkey_code(ks: c_ulong) -> Option<HotkeyCode> {
    match ks {
        XK_PAUSE => Some(HotkeyCode::Pause),
        XK_SPACE => Some(HotkeyCode::Space),
        XK_TAB => Some(HotkeyCode::Tab),
        XK_RETURN | XK_KP_ENTER => Some(HotkeyCode::Enter),
        XK_ESCAPE => Some(HotkeyCode::Escape),
        XK_BACKSPACE => Some(HotkeyCode::Backspace),
        n if (XK_F1..=XK_F12).contains(&n) => Some(HotkeyCode::F((n - XK_F1 + 1) as u8)),
        n if (0x61..=0x7a).contains(&n) => Some(HotkeyCode::Letter((n as u8 as char).to_ascii_uppercase())),
        n if (0x41..=0x5a).contains(&n) => Some(HotkeyCode::Letter(n as u8 as char)),
        n if (0x30..=0x39).contains(&n) => Some(HotkeyCode::Digit((n - 0x30) as u8)),
        _ => None,
    }
}


/// Raw event forwarded from the XRecord thread.
#[derive(Debug, Clone, Copy)]
struct RawEvent {
    kind: u8,
    keycode: u8,
}

#[derive(Default, Debug)]
struct Mods {
    shift_l: bool,
    shift_r: bool,
    ctrl: bool,
    alt: bool,
    sup: bool,
    altgr: bool,
}

impl Mods {
    fn shift(&self) -> bool {
        self.shift_l || self.shift_r
    }
    fn shortcut(&self) -> bool {
        self.ctrl || self.alt || self.sup
    }
    fn any(&self) -> bool {
        self.shift() || self.shortcut() || self.altgr
    }
}

unsafe extern "C" fn ignore_x_errors(_d: *mut Display, e: *mut xlib::XErrorEvent) -> c_int {
    // Windows vanish between "get active window" and "read its properties" all the time.
    // Never let Xlib's default handler kill the daemon for that.
    if !e.is_null() {
        log::debug!("X error ignored: code {}", (*e).error_code);
    }
    0
}

struct X11 {
    xl: Xlib,
    xt: Option<xtest::Xf86vmode>,
    dpy: *mut Display,
    root: c_ulong,
    a_active: c_ulong,
    a_pid: c_ulong,
    /// Lang of each XKB group index (up to 4).
    groups: [Option<Lang>; 4],
    groups_checked: Instant,
}

impl Drop for X11 {
    fn drop(&mut self) {
        unsafe { (self.xl.XCloseDisplay)(self.dpy) };
    }
}

fn lang_from_group_name(name: &str) -> Option<Lang> {
    let n = name.to_lowercase();
    if n.contains("russian") || n.starts_with("ru") || n.contains("рус") {
        Some(Lang::Ru)
    } else if n.contains("english") || n.starts_with("us") || n.starts_with("en") {
        Some(Lang::En)
    } else {
        None
    }
}

impl X11 {
    fn open() -> Result<X11, String> {
        let xl = Xlib::open().map_err(|e| format!("cannot load libX11: {e}"))?;
        unsafe {
            (xl.XSetErrorHandler)(Some(ignore_x_errors));
            let dpy = (xl.XOpenDisplay)(ptr::null());
            if dpy.is_null() {
                return Err("cannot open X display (is DISPLAY set?)".into());
            }
            let root = (xl.XDefaultRootWindow)(dpy);
            let atom = |n: &str| {
                let c = CString::new(n).unwrap();
                (xl.XInternAtom)(dpy, c.as_ptr(), xlib::False)
            };
            let a_active = atom("_NET_ACTIVE_WINDOW");
            let a_pid = atom("_NET_WM_PID");
            let xt = xtest::Xf86vmode::open().ok();
            let mut x = X11 { xl, xt, dpy, root, a_active, a_pid, groups: [None; 4], groups_checked: Instant::now() };
            x.refresh_groups();
            Ok(x)
        }
    }

    fn refresh_groups(&mut self) {
        self.groups_checked = Instant::now();
        unsafe {
            let desc = (self.xl.XkbAllocKeyboard)();
            if desc.is_null() {
                return;
            }
            (*desc).dpy = self.dpy;
            (*desc).device_spec = XKB_USE_CORE_KBD as u16;
            if (self.xl.XkbGetNames)(self.dpy, XKB_GROUP_NAMES_MASK, desc) != 0 {
                (self.xl.XkbFreeKeyboard)(desc, 0, xlib::True);
                return;
            }
            let names = (*desc).names;
            if !names.is_null() {
                for i in 0..4 {
                    let a = (*names).groups[i];
                    self.groups[i] = if a == 0 {
                        None
                    } else {
                        let p = (self.xl.XGetAtomName)(self.dpy, a);
                        if p.is_null() {
                            None
                        } else {
                            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
                            (self.xl.XFree)(p as *mut _);
                            lang_from_group_name(&s)
                        }
                    };
                }
            }
            (self.xl.XkbFreeNames)(desc, XKB_GROUP_NAMES_MASK, xlib::True);
            (self.xl.XkbFreeKeyboard)(desc, 0, xlib::True);
        }
    }

    fn group_state(&self) -> Option<(usize, u8)> {
        unsafe {
            let mut st: xlib::XkbStateRec = std::mem::zeroed();
            if (self.xl.XkbGetState)(self.dpy, XKB_USE_CORE_KBD, &mut st) != 0 {
                return None;
            }
            Some((st.group as usize, st.locked_mods))
        }
    }

    fn current_lang(&mut self) -> Option<Lang> {
        if self.groups_checked.elapsed() > Duration::from_secs(5) {
            self.refresh_groups();
        }
        let (g, _) = self.group_state()?;
        self.groups.get(g).copied().flatten()
    }

    fn caps_lock(&self) -> bool {
        self.group_state().is_some_and(|(_, locked)| locked & 0x02 != 0)
    }

    fn group_for(&self, lang: Lang) -> Option<usize> {
        self.groups.iter().position(|g| *g == Some(lang))
    }

    /// Group used to read "physical US characters" from keycodes.
    fn us_group(&self) -> c_int {
        self.group_for(Lang::En).unwrap_or(0) as c_int
    }

    fn switch_to(&mut self, lang: Lang) -> bool {
        let Some(g) = self.group_for(lang) else {
            log::warn!("no XKB group for layout {lang}; add it, e.g. setxkbmap -layout us,ru");
            return false;
        };
        unsafe {
            (self.xl.XkbLockGroup)(self.dpy, XKB_USE_CORE_KBD, g as c_uint);
            (self.xl.XSync)(self.dpy, xlib::False);
        }
        true
    }

    fn window_prop_card(&self, w: c_ulong, prop: c_ulong) -> Option<c_ulong> {
        unsafe {
            let mut actual_type = 0;
            let mut fmt = 0;
            let mut n = 0;
            let mut after = 0;
            let mut data: *mut c_uchar = ptr::null_mut();
            let r = (self.xl.XGetWindowProperty)(
                self.dpy, w, prop, 0, 1, xlib::False, xlib::AnyPropertyType as c_ulong,
                &mut actual_type, &mut fmt, &mut n, &mut after, &mut data,
            );
            if r != 0 || data.is_null() {
                return None;
            }
            let v = if n > 0 && fmt == 32 { Some(*(data as *const c_ulong)) } else { None };
            (self.xl.XFree)(data as *mut _);
            v
        }
    }

    fn active_window(&self) -> Option<c_ulong> {
        if let Some(w) = self.window_prop_card(self.root, self.a_active).filter(|&w| w != 0) {
            return Some(w);
        }
        // Non-EWMH window managers: fall back to input focus.
        unsafe {
            let mut w = 0;
            let mut revert = 0;
            (self.xl.XGetInputFocus)(self.dpy, &mut w, &mut revert);
            (w > 1).then_some(w)
        }
    }

    fn app_id(&self) -> Option<String> {
        let w = self.active_window()?;
        unsafe {
            let mut hint: xlib::XClassHint = std::mem::zeroed();
            if (self.xl.XGetClassHint)(self.dpy, w, &mut hint) != 0 {
                let class = (!hint.res_class.is_null()).then(|| CStr::from_ptr(hint.res_class).to_string_lossy().into_owned());
                let name = (!hint.res_name.is_null()).then(|| CStr::from_ptr(hint.res_name).to_string_lossy().into_owned());
                if !hint.res_class.is_null() {
                    (self.xl.XFree)(hint.res_class as *mut _);
                }
                if !hint.res_name.is_null() {
                    (self.xl.XFree)(hint.res_name as *mut _);
                }
                if let Some(c) = class.or(name).filter(|s| !s.is_empty()) {
                    return Some(c);
                }
            }
        }
        let pid = self.window_prop_card(w, self.a_pid)?;
        std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
    }

    fn keysym(&self, keycode: u8, group: c_int, level: c_int) -> c_ulong {
        unsafe { (self.xl.XkbKeycodeToKeysym)(self.dpy, keycode, group, level) }
    }

    /// Keycode + shift needed to type US character `c` (in the US group).
    fn key_for_us_char(&self, c: char) -> Option<(u8, bool)> {
        let ks = c as c_ulong;
        let kc = unsafe { (self.xl.XKeysymToKeycode)(self.dpy, ks) };
        if kc == 0 {
            return None;
        }
        let g = self.us_group();
        if self.keysym(kc, g, 0) == ks {
            Some((kc, false))
        } else if self.keysym(kc, g, 1) == ks {
            Some((kc, true))
        } else {
            None
        }
    }

    /// Erase + retype. Returns the number of synthetic key presses (for echo suppression),
    /// or an error without having sent anything.
    fn replace(&mut self, r: &Replacement) -> Result<usize, String> {
        if self.xt.is_none() {
            return Err("XTest extension library (libXtst) not available".into());
        }
        // Plan everything first; abort before touching the text if any char is untypeable.
        let mut plan = Vec::with_capacity(r.insert.chars().count());
        for c in r.insert.chars() {
            let us = layout::us_key_for(c, r.target).ok_or("character not typeable in target layout")?;
            let key = match us {
                ' ' => (unsafe { (self.xl.XKeysymToKeycode)(self.dpy, XK_SPACE) }, false),
                '\n' => (unsafe { (self.xl.XKeysymToKeycode)(self.dpy, XK_RETURN) }, false),
                '\t' => (unsafe { (self.xl.XKeysymToKeycode)(self.dpy, XK_TAB) }, false),
                _ => self.key_for_us_char(us).ok_or("no key for character")?,
            };
            if key.0 == 0 {
                return Err("no keycode".into());
            }
            plan.push(key);
        }
        let bs = unsafe { (self.xl.XKeysymToKeycode)(self.dpy, XK_BACKSPACE) };
        let shift = unsafe { (self.xl.XKeysymToKeycode)(self.dpy, XK_SHIFT_L) };
        if bs == 0 || shift == 0 {
            return Err("no BackSpace/Shift keycode".into());
        }
        if self.current_lang() != Some(r.target) && !self.switch_to(r.target) {
            return Err("cannot switch layout".into());
        }
        let caps = self.caps_lock();
        let xt = self.xt.as_ref().expect("checked above");
        let mut presses = 0;
        unsafe {
            let tap = |kc: u8| {
                (xt.XTestFakeKeyEvent)(self.dpy, kc as c_uint, xlib::True, 0);
                (xt.XTestFakeKeyEvent)(self.dpy, kc as c_uint, xlib::False, 0);
            };
            for _ in 0..r.delete {
                tap(bs);
                presses += 1;
            }
            for (kc, mut need_shift) in plan {
                // Caps Lock inverts the shift level for letters; compensate.
                if caps && self.keysym(kc, self.us_group(), 0) < 0x100 && (self.keysym(kc, self.us_group(), 0) as u8).is_ascii_alphabetic() {
                    need_shift = !need_shift;
                }
                if need_shift {
                    (xt.XTestFakeKeyEvent)(self.dpy, shift as c_uint, xlib::True, 0);
                    presses += 1;
                }
                tap(kc);
                presses += 1;
                if need_shift {
                    (xt.XTestFakeKeyEvent)(self.dpy, shift as c_uint, xlib::False, 0);
                }
            }
            (self.xl.XSync)(self.dpy, xlib::False);
        }
        Ok(presses)
    }
}

// ---------------------------------------------------------------- XRecord thread

unsafe extern "C" fn record_cb(closure: *mut c_char, data: *mut xrecord::XRecordInterceptData) {
    let tx = &*(closure as *const Sender<RawEvent>);
    let d = &*data;
    if d.category == xrecord::XRecordFromServer && d.data_len >= 1 && !d.data.is_null() {
        let kind = *d.data & 0x7f;
        let keycode = *d.data.add(1);
        if matches!(kind, KEY_PRESS | KEY_RELEASE | BUTTON_PRESS) {
            let _ = tx.send(RawEvent { kind, keycode });
        }
    }
    // The library resolves XRecordFreeData through the same handle; it is loaded in the thread.
    if let Ok(xr) = RECORD_LIB.with(|l| l.get().copied().ok_or(())) {
        ((*xr).XRecordFreeData)(data);
    }
}

thread_local! {
    static RECORD_LIB: std::cell::OnceCell<*const xrecord::Xf86vmode> = const { std::cell::OnceCell::new() };
}

fn spawn_record(tx: Sender<RawEvent>) -> Result<(), String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("xrecord".into())
        .spawn(move || unsafe {
            let fail = |m: &str| {
                let _ = ready_tx.send(Err(m.to_string()));
            };
            let Ok(xl) = Xlib::open() else { return fail("libX11") };
            let Ok(xr) = xrecord::Xf86vmode::open() else { return fail("libXtst (XRecord) not installed") };
            let ctrl = (xl.XOpenDisplay)(ptr::null());
            let data = (xl.XOpenDisplay)(ptr::null());
            if ctrl.is_null() || data.is_null() {
                return fail("cannot open display for XRecord");
            }
            let (mut maj, mut min) = (0, 0);
            if (xr.XRecordQueryVersion)(ctrl, &mut maj, &mut min) == 0 {
                return fail("X server has no RECORD extension");
            }
            let range = (xr.XRecordAllocRange)();
            if range.is_null() {
                return fail("XRecordAllocRange failed");
            }
            (*range).device_events.first = KEY_PRESS;
            (*range).device_events.last = BUTTON_PRESS;
            let mut clients = xrecord::XRecordAllClients;
            let mut r = range;
            let ctx = (xr.XRecordCreateContext)(ctrl, 0, &mut clients, 1, &mut r, 1);
            (xl.XFree)(range as *mut _);
            if ctx == 0 {
                return fail("XRecordCreateContext failed");
            }
            (xl.XSync)(ctrl, xlib::False);
            RECORD_LIB.with(|l| {
                let _ = l.set(&xr as *const _);
            });
            let _ = ready_tx.send(Ok(()));
            let boxed = Box::new(tx);
            let closure = &*boxed as *const Sender<RawEvent> as *mut c_char;
            // Blocks for the lifetime of the daemon.
            (xr.XRecordEnableContext)(data, ctx, Some(record_cb), closure);
            log::warn!("XRecord context ended");
            (xr.XRecordFreeContext)(ctrl, ctx);
            drop(boxed);
        })
        .map_err(|e| e.to_string())?;
    ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "XRecord thread did not start".to_string())?
}

// ---------------------------------------------------------------- main loop

struct Daemon {
    x: X11,
    ctl: Controller,
    mods: Mods,
    /// Synthetic key presses still to be seen echoed back by XRecord.
    echo: usize,
    /// Replacement waiting for the user to release modifiers (e.g. Shift after "!").
    pending: Option<Replacement>,
}

impl Daemon {
    fn run_commands(&mut self, cmds: Vec<Command>) {
        for c in cmds {
            match c {
                Command::SwitchLayout(l) => {
                    if self.pending.is_none() { self.x.switch_to(l); }
                }
                Command::Replace(r) => {
                    if self.mods.shortcut() {
                        self.pending = Some(r);
                    } else {
                        self.do_replace(r);
                    }
                }
                Command::SummonGui => {
                    crate::platform::request_open_vault();
                    crate::platform::spawn_settings_gui_vault();
                }
                Command::ConvertSelection => log::debug!("convert selection is not supported on Linux yet"),
            }
        }
    }

    fn do_replace(&mut self, r: Replacement) {
        match self.x.replace(&r) {
            Ok(n) => {
                self.echo += n;
                self.ctl.replace_succeeded(r.target);
                log::debug!("replaced {} chars with {} chars, layout {}", r.delete, r.insert.chars().count(), r.target);
            }
            Err(e) => {
                log::warn!("replacement skipped, original text left intact: {e}");
                self.ctl.replace_failed();
            }
        }
    }

    fn translate(&mut self, keycode: u8) -> Option<Input> {
        let g = self.x.us_group();
        let base = self.x.keysym(keycode, g, 0);
        if let Some(code) = keysym_to_hotkey_code(base) {
            let (c, a, s, m) = (self.mods.ctrl, self.mods.alt, self.mods.shift(), self.mods.sup);
            if self.ctl.undo_hotkey().matches(code, c, a, s, m) {
                return Some(Input::Undo);
            }
            if self.ctl.hotkey().matches_relaxed_shift(code, c, a, s, m) {
                return Some(Input::Hotkey);
            }
            if self.ctl.summon_hotkey().matches(code, c, a, s, m) {
                return Some(Input::Summon);
            }
        }
        let modifiers = self.mods.shortcut();
        let key = match base {
            XK_BACKSPACE => Key::Backspace,
            // XRecord cannot swallow keys, so Enter/Tab have already reached the
            // app: treat them as typed boundaries (same delete-count as space).
            XK_RETURN | XK_KP_ENTER => Key::Char('\n'),
            XK_TAB => Key::Char('\t'),
            XK_SPACE => Key::Char(' '),
            XK_PAUSE => return None,
            0x21..=0x7e => {
                let level = if self.mods.shift() { 1 } else { 0 };
                let ks = self.x.keysym(keycode, g, level);
                let mut us = if (0x20..=0x7e).contains(&ks) { char::from(ks as u8) } else { return Some(Input::Key { key: Key::Reset, modifiers }) };
                if self.x.caps_lock() && us.is_ascii_alphabetic() {
                    us = if us.is_ascii_uppercase() { us.to_ascii_lowercase() } else { us.to_ascii_uppercase() };
                }
                let c = match self.ctl.current_layout() {
                    Some(Lang::Ru) => layout::en_to_ru(us).unwrap_or(us),
                    _ => us,
                };
                if self.mods.altgr {
                    Key::Reset
                } else {
                    Key::Char(c)
                }
            }
            // Function keys, arrows, Home/End, Escape, Delete…: caret may have moved.
            0xff00..=0xffff if !(XK_SHIFT_L..=XK_SUPER_R).contains(&base) && base != XK_CAPS_LOCK => Key::Reset,
            _ => return None,
        };
        Some(Input::Key { key, modifiers })
    }

    fn on_raw(&mut self, ev: RawEvent) {
        if ev.kind == BUTTON_PRESS {
            if self.pending.take().is_some() { self.ctl.replace_failed(); }
            let cmds = self.ctl.on_input(Input::Key { key: Key::Reset, modifiers: false });
            self.run_commands(cmds);
            return;
        }
        let ks = self.x.keysym(ev.keycode, self.x.us_group(), 0);
        let down = ev.kind == KEY_PRESS;
        // Modifier bookkeeping (also for our own synthetic Shift so state stays exact).
        match ks {
            XK_SHIFT_L | XK_SHIFT_R => {
                let other = if ks == XK_SHIFT_L { self.mods.shift_r } else { self.mods.shift_l };
                if ks == XK_SHIFT_L { self.mods.shift_l = down } else { self.mods.shift_r = down }
                // Shift+Shift hotkey: second Shift pressed while the first is held.
                if down && self.echo == 0 && other {
                    if self.ctl.hotkey().is_both_shifts() {
                        let cmds = self.ctl.on_input(Input::Hotkey);
                        self.run_commands(cmds);
                    } else if self.ctl.summon_hotkey().is_both_shifts() {
                        let cmds = self.ctl.on_input(Input::Summon);
                        self.run_commands(cmds);
                    }
                }
            }
            XK_CONTROL_L | XK_CONTROL_R => self.mods.ctrl = down,
            0xffe9 | 0xffea | XK_META_L | 0xffe8 => self.mods.alt = down,
            0xffeb | XK_SUPER_R => self.mods.sup = down,
            XK_ISO_LEVEL3_SHIFT => self.mods.altgr = down,
            _ => {}
        }
        if !down {
            return;
        }
        if self.echo > 0 {
            self.echo -= 1;
            return;
        }
        if (XK_SHIFT_L..=XK_SUPER_R).contains(&ks) || ks == XK_ISO_LEVEL3_SHIFT || ks == XK_CAPS_LOCK {
            return;
        }
        // Typing anything while a replacement waits would invalidate its delete count.
        if self.pending.take().is_some() {
            log::debug!("pending replacement dropped: more input arrived");
            self.ctl.replace_failed();
        }
        if let Some(input) = self.translate(ev.keycode) {
            let mut cmds = self.ctl.on_input(input);
            // XRecord cannot swallow the Backspace that undid an autocorrection: it has
            // already erased one boundary character.
            if matches!(input, Input::Key { key: Key::Backspace, .. }) {
                for c in cmds.iter_mut() {
                    if let Command::Replace(r) = c {
                        r.delete = r.delete.saturating_sub(1);
                    }
                }
            }
            self.run_commands(cmds);
        }
    }

    fn tick(&mut self) {
        let app = self.x.app_id();
        let lang = self.x.current_lang();
        let cmds = self.ctl.tick(app.as_deref(), lang);
        self.run_commands(cmds);
        if !self.mods.any() {
            if let Some(r) = self.pending.take() {
                self.do_replace(r);
            }
        }
    }
}

fn check_session() -> Result<(), String> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    match (wayland, x11) {
        (true, false) => Err("pure Wayland session: global keyboard observation and input injection are not \
             permitted for normal apps. Use an X11 session, or the IBus/Fcitx route described in README (Wayland)."
            .into()),
        (true, true) => {
            log::warn!("Wayland session with XWayland: only X11 (XWayland) apps are visible; native Wayland apps are not");
            Ok(())
        }
        (false, false) => Err("no DISPLAY: not inside a graphical X11 session".into()),
        _ => Ok(()),
    }
}

pub fn run(ctl: Controller) -> Result<(), String> {
    check_session()?;
    let x = X11::open()?;
    if !x.groups.iter().any(|g| *g == Some(Lang::Ru)) || !x.groups.iter().any(|g| *g == Some(Lang::En)) {
        log::warn!("XKB groups detected: {:?}. Configure both layouts, e.g. `setxkbmap -layout us,ru`", x.groups);
    }
    if x.xt.is_none() {
        log::warn!("libXtst not found: autocorrect replacement disabled (install libxtst6)");
    }
    let (tx, rx): (Sender<RawEvent>, Receiver<RawEvent>) = mpsc::channel();
    let keys_ok = match spawn_record(tx) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("key observation unavailable ({e}); running per-app layout memory only");
            false
        }
    };
    log::info!("running on X11 (keys: {})", if keys_ok { "XRecord" } else { "off" });
    let mut d = Daemon { x, ctl, mods: Mods::default(), echo: 0, pending: None };
    let tick_every = Duration::from_millis(120);
    let mut last_tick = Instant::now() - tick_every;
    loop {
        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(ev) => d.on_raw(ev),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if keys_ok {
                    return Err("XRecord thread stopped".into());
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        if last_tick.elapsed() >= tick_every {
            last_tick = Instant::now();
            d.tick();
        }
    }
}

pub fn probe() -> (Option<String>, Option<Lang>) {
    if check_session().is_err() {
        return (None, None);
    }
    match X11::open() {
        Ok(mut x) => (x.app_id(), x.current_lang()),
        Err(_) => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::lang_from_group_name;
    use layout_keeper_core::Lang;

    #[test]
    fn group_names() {
        assert_eq!(lang_from_group_name("English (US)"), Some(Lang::En));
        assert_eq!(lang_from_group_name("Russian"), Some(Lang::Ru));
        assert_eq!(lang_from_group_name("German"), None);
    }
}

/// Type `text` into the focused X11 window (vault autofill) via XTest, as physical keys.
/// All characters must be typeable in one layout (Latin → EN, Cyrillic → RU); `\t` = Tab.
/// Note: unlike Windows/macOS, XTest events are not tagged, so a running daemon sees them;
/// autocorrect still skips tokens with digits/symbols, but the daemon never stores them.
pub fn type_text(text: &str) -> Result<(), String> {
    let has_cyr = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    let target = if has_cyr { Lang::Ru } else { Lang::En };
    let mut x = X11::open()?;
    let before = x.current_lang();
    let r = Replacement { delete: 0, insert: text.to_string(), target };
    let res = x.replace(&r).map(|_| ());
    drop(r);
    if let Some(b) = before {
        if b != target {
            x.switch_to(b);
            unsafe { (x.xl.XSync)(x.dpy, xlib::False) };
        }
    }
    res.map_err(|_| "не удалось набрать текст: символ недоступен в раскладке или нет XTest".into())
}
