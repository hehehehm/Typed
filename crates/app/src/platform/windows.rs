//! Windows adapter.
//!
//! * focused app: GUI focus / visible Z-order fallback → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`
//!   (app id = lowercase full exe path);
//! * layout: `GetKeyboardLayout(thread of foreground window)` (LANGID primary 0x09 = en, 0x19 = ru),
//!   switching with `WM_INPUTLANGCHANGEREQUEST` to an installed HKL from `GetKeyboardLayoutList`;
//! * keys: `WH_KEYBOARD_LL` low-level hook (+ `WH_MOUSE_LL` to forget the word on clicks);
//! * replacement: `SendInput` — N × VK_BACK, then the corrected text as `KEYEVENTF_UNICODE`.
//!
//! The hook callback only updates the in-memory buffer and queues commands; the actual
//! `SendInput` runs from the message loop after the hook has returned, and every callback
//! is wrapped in `catch_unwind` so a bug can never take the user's input down with it.
//! No administrator rights are needed, but elevated windows cannot be hooked/injected from a
//! non-elevated process (UIPI) — run LayoutKeeper elevated only if you really need that.

use crate::controller::{Command, Controller, Input};
use layout_keeper_core::{layout, HotkeyCode, Key, Lang, Replacement};
use std::cell::RefCell;
use std::ptr;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, HANDLE, BOOL, HWND, LPARAM, LRESULT, WPARAM, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
};
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, CreateMutexW, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId,
    OpenMutexW, OpenProcess, QueryFullProcessImageNameW, SetPriorityClass, SetProcessInformation,
    TerminateProcess, WaitForSingleObject, ABOVE_NORMAL_PRIORITY_CLASS, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_POWER_THROTTLING_STATE, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_TERMINATE, ProcessPowerThrottling,
    SYNCHRONIZATION_SYNCHRONIZE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ActivateKeyboardLayout, GetAsyncKeyState, GetFocus, GetKeyState, GetKeyboardLayout, GetKeyboardLayoutList, SendInput, SetActiveWindow, HKL, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_LSHIFT,
    VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LWIN, VK_MENU, VK_OEM_3, VK_RETURN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_TAB,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EnumWindows, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, WINEVENT_OUTOFCONTEXT, FindWindowW, GetAncestor, GetClassNameW, GetForegroundWindow,
    GetGUIThreadInfo, GetMessageW, GetTopWindow, GetWindow, GetWindowLongW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, GA_ROOT, GUITHREADINFO, GWL_EXSTYLE, GW_HWNDFIRST, GW_HWNDNEXT, GW_OWNER,
    WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    AllowSetForegroundWindow, ASFW_ANY, BringWindowToTop, PostMessageW, PostThreadMessageW, SendMessageTimeoutW, SMTO_ABORTIFHUNG, SetForegroundWindow, SetTimer, SetWindowsHookExW, ShowWindowAsync,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, SW_RESTORE, SW_SHOW, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_APP, WM_INPUTLANGCHANGEREQUEST, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_NULL, WM_QUIT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

/// Marker in `dwExtraInfo` of our own synthetic input.
const LK_MAGIC: usize = 0x4C4B_5045; // "LKPE"
const WM_LK_FLUSH: u32 = WM_APP + 1;
/// Selection converted on the worker thread; wParam 1 = EN, 2 = RU.
const WM_LK_SEL_DONE: u32 = WM_APP + 2;

struct WinKeys {
    hook_thread: u32,
}

impl crate::selection::Keys for WinKeys {
    fn modifiers_held(&self) -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN].into_iter().any(key_down)
    }
    fn copy(&self) {
        send_chord(&[VK_LCONTROL], 0x43); // C
    }
    fn paste(&self) {
        send_chord(&[VK_LCONTROL], 0x56); // V
    }
    fn select_word_left(&self) {
        send_chord_ext(&[VK_LCONTROL, VK_LSHIFT], VK_LEFT, true);
    }
    fn done(&self, target: Lang) {
        let w = match target { Lang::En => 1, Lang::Ru => 2 };
        unsafe { PostThreadMessageW(self.hook_thread, WM_LK_SEL_DONE, w, 0); }
    }
}

fn send_chord(mods: &[u16], vk: u16) {
    send_chord_ext(mods, vk, false)
}

/// Press `mods`, tap `vk`, release. Tagged with LK_MAGIC so our hook ignores it.
fn send_chord_ext(mods: &[u16], vk: u16, extended: bool) {
    const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
    let ext = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    let mut v: Vec<INPUT> = mods.iter().map(|&k| key_input(k, 0, 0)).collect();
    v.push(key_input(vk, 0, ext));
    v.push(key_input(vk, 0, ext | KEYEVENTF_KEYUP));
    v.extend(mods.iter().rev().map(|&k| key_input(k, 0, KEYEVENTF_KEYUP)));
    unsafe {
        SendInput(v.len() as u32, v.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

struct State {
    ctl: Controller,
    pending: Vec<Command>,
    thread_id: u32,
    lshift: bool,
    rshift: bool,
    /// Swallow the matching KEYUP after we ate Enter/Tab on KEYDOWN.
    swallow_up: Option<u32>,
    /// Layout used to interpret the word in progress. Frozen on the first letter
    /// from the live foreground HKL so a layout switch after the previous word
    /// cannot mix scripts mid-word (capitalized second word would not convert).
    word_lang: Option<Lang>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn key_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

fn shortcut_mods() -> bool {
    key_down(VK_CONTROL) || key_down(VK_MENU) || key_down(VK_LWIN) || key_down(VK_RWIN)
}

fn live_layout() -> Option<Lang> {
    foreground().1
}

fn lang_of_hkl(h: HKL) -> Option<Lang> {
    match (h as usize) & 0x3ff {
        0x09 => Some(Lang::En),
        0x19 => Some(Lang::Ru),
        _ => None,
    }
}

/// Only use ordinary visible app windows when recovering from a stale/minimized foreground.
unsafe fn usable_window(hwnd: HWND, walking: bool) -> bool {
    if hwnd.is_null() || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
        return false;
    }
    if walking && !GetWindow(hwnd, GW_OWNER).is_null() {
        return false;
    }
    let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if style & WS_EX_TOOLWINDOW != 0 && style & WS_EX_APPWINDOW == 0 {
        return false;
    }
    let mut class = [0u16; 256];
    let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
    !matches!(String::from_utf16_lossy(&class[..len]).as_str(),
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW" |
        "ForegroundStaging" | "NotifyIconOverflowWindow")
}

/// Windows can temporarily keep a minimized app as foreground. Resolve the input
/// focus first, then fall back to the first usable app in top-level Z-order.
fn target_window() -> HWND {
    unsafe {
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let mut hwnd = ptr::null_mut();
        if GetGUIThreadInfo(0, &mut info) != 0 {
            hwnd = if !info.hwndFocus.is_null() { info.hwndFocus } else { info.hwndActive };
        }
        if hwnd.is_null() {
            hwnd = GetForegroundWindow();
        }
        if !hwnd.is_null() {
            hwnd = GetAncestor(hwnd, GA_ROOT);
        }
        if usable_window(hwnd, false) {
            return hwnd;
        }
        let mut next = if hwnd.is_null() {
            GetTopWindow(ptr::null_mut())
        } else {
            GetWindow(hwnd, GW_HWNDFIRST)
        };
        // Windows may destroy/reorder windows during traversal; bound the walk.
        for _ in 0..4096 {
            if next.is_null() {
                break;
            }
            if usable_window(next, true) {
                return next;
            }
            next = GetWindow(next, GW_HWNDNEXT);
        }
        ptr::null_mut()
    }
}

fn foreground() -> (Option<String>, Option<Lang>) {
    foreground_for(target_window())
}

fn foreground_for(hwnd: HWND) -> (Option<String>, Option<Lang>) {
    unsafe {
        if hwnd.is_null() {
            return (None, None);
        }
        let mut pid = 0u32;
        let tid = GetWindowThreadProcessId(hwnd, &mut pid);
        if tid == 0 {
            return (None, None);
        }
        let lang = lang_of_hkl(GetKeyboardLayout(tid));
        let mut app = None;
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !h.is_null() {
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0 {
                app = Some(String::from_utf16_lossy(&buf[..len as usize]).to_lowercase());
            }
            CloseHandle(h);
        }
        (app, lang)
    }
}

fn switch_layout(lang: Lang) -> bool {
    switch_layout_for(target_window(), lang)
}

fn switch_layout_for(hwnd: HWND, lang: Lang) -> bool {
    unsafe {
        let mut list: [HKL; 32] = [ptr::null_mut(); 32];
        let n = GetKeyboardLayoutList(list.len() as i32, list.as_mut_ptr()).max(0) as usize;
        let Some(hkl) = list[..n].iter().copied().find(|&h| lang_of_hkl(h) == Some(lang)) else {
            log::warn!("no installed keyboard layout for {lang} (Settings → Time & language → Language)");
            return false;
        };
        const INPUTLANGCHANGE_SYSCHARSET: WPARAM = 1;
        const KLF_SETFORPROCESS: u32 = 0x0000_0100;
        if hwnd.is_null() || IsIconic(hwnd) != 0 || IsWindowVisible(hwnd) == 0 {
            return false;
        }
        let mut pid = 0u32;
        let their = GetWindowThreadProcessId(hwnd, &mut pid);
        if their == 0 {
            return false;
        }
        // Read BEFORE any attach / activate: those poison later reads.
        if lang_of_hkl(GetKeyboardLayout(their)) == Some(lang) {
            // Still nudge with a message (apps may overwrite after activation), no hotkey.
            post_request(hwnd, hkl);
            return true;
        }
        let ours = GetCurrentThreadId();
        // A. Messages to the root, the focused child and all visible windows of the PID.
        send_request(hwnd, hkl);
        let attached = their != ours && AttachThreadInput(ours, their, 1) != 0;
        if attached {
            let focus = GetFocus();
            if !focus.is_null() && focus != hwnd && GetAncestor(focus, GA_ROOT) == hwnd {
                send_request(focus, hkl);
            }
            // B. Extra attempt; result is NOT trusted.
            if ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS).is_null() {
                ActivateKeyboardLayout(hkl, 0);
            }
            AttachThreadInput(ours, their, 0);
        }
        let mut ctx = EnumCtx { pid, hkl, skip: hwnd };
        EnumWindows(Some(enum_proc), &mut ctx as *mut EnumCtx as LPARAM);
        let _ = INPUTLANGCHANGE_SYSCHARSET;
        // C. Fallback: the user's own Windows language hotkey, at most one toggle per call.
        if lang_of_hkl(GetKeyboardLayout(their)) == Some(lang) {
            return true;
        }
        let Some(combo) = language_hotkey() else { return false };
        if !foreground_is_target(hwnd, pid) {
            return false;
        }
        log::debug!("layout request ignored; simulating Windows language hotkey");
        send_combo(combo);
        true
    }
}

unsafe fn send_request(w: HWND, hkl: HKL) {
    let mut res: usize = 0;
    SendMessageTimeoutW(w, WM_INPUTLANGCHANGEREQUEST, 1, hkl as LPARAM, SMTO_ABORTIFHUNG, 50, &mut res);
    PostMessageW(w, WM_INPUTLANGCHANGEREQUEST, 1, hkl as LPARAM);
}

unsafe fn post_request(w: HWND, hkl: HKL) {
    PostMessageW(w, WM_INPUTLANGCHANGEREQUEST, 1, hkl as LPARAM);
}

struct EnumCtx {
    pid: u32,
    hkl: HKL,
    skip: HWND,
}

unsafe extern "system" fn enum_proc(w: HWND, lp: LPARAM) -> BOOL {
    let ctx = &*(lp as *const EnumCtx);
    if w != ctx.skip && IsWindowVisible(w) != 0 {
        let mut pid = 0u32;
        GetWindowThreadProcessId(w, &mut pid);
        if pid == ctx.pid {
            send_request(w, ctx.hkl);
        }
    }
    1
}

/// Only press the hotkey when the real foreground is our target app (or is
/// iconic / shell chrome while our resolved window is the target), so we never
/// flip explorer's layout when the taskbar swallowed the click.
unsafe fn foreground_is_target(hwnd: HWND, pid: u32) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_null() {
        return false;
    }
    let mut fpid = 0u32;
    GetWindowThreadProcessId(fg, &mut fpid);
    if fpid == pid {
        return true;
    }
    let root = GetAncestor(fg, GA_ROOT);
    !usable_window(if root.is_null() { fg } else { root }, false) && target_window() == hwnd
}

#[derive(Clone, Copy)]
enum Combo {
    AltShift,
    CtrlShift,
    Grave,
}

fn reg_string(name: &str) -> Option<String> {
    unsafe {
        let sub: Vec<u16> = "Keyboard Layout\\Toggle\0".encode_utf16().collect();
        let mut key: HKEY = ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut buf = [0u16; 16];
        let mut len = (buf.len() * 2) as u32;
        let mut ty = 0u32;
        let r = RegQueryValueExW(key, name.as_ptr(), ptr::null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut len);
        RegCloseKey(key);
        if r != 0 {
            return None;
        }
        let chars = (len as usize / 2).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..chars]).trim_end_matches('\0').trim().to_string())
    }
}

/// HKCU\Keyboard Layout\Toggle: "1" Alt+Shift, "2" Ctrl+Shift, "4" grave; "3"/missing = none.
fn language_hotkey() -> Option<Combo> {
    let v = reg_string("Language Hotkey").or_else(|| reg_string("Hotkey"))?;
    match v.as_str() {
        "1" => Some(Combo::AltShift),
        "2" => Some(Combo::CtrlShift),
        "4" => Some(Combo::Grave),
        _ => None,
    }
}

fn send_combo(c: Combo) {
    let keys: &[u16] = match c {
        Combo::AltShift => &[VK_LMENU, VK_LSHIFT],
        Combo::CtrlShift => &[VK_LCONTROL, VK_LSHIFT],
        Combo::Grave => &[VK_OEM_3],
    };
    let mut v: Vec<INPUT> = keys.iter().map(|&k| key_input(k, 0, 0)).collect();
    v.extend(keys.iter().rev().map(|&k| key_input(k, 0, KEYEVENTF_KEYUP)));
    unsafe {
        SendInput(v.len() as u32, v.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

fn key_input(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: LK_MAGIC } },
    }
}

fn replace(r: &Replacement) -> Result<(), String> {
    const MODS: [u16; 8] = [VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN];
    let held = MODS.map(key_down);
    // GetAsyncKeyState changes on synthetic KEYUP too. Preserve physical Shift in
    // the hook bookkeeping, which ignores LK_MAGIC but still sees real releases.
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            st.lshift = held[0];
            st.rshift = held[1];
        }
    });
    let mut inputs = Vec::with_capacity(r.delete * 2 + r.insert.len() * 2 + 10);
    for (vk, down) in MODS.into_iter().zip(held) {
        if down { inputs.push(key_input(vk, 0, KEYEVENTF_KEYUP)); }
    }
    for _ in 0..r.delete {
        inputs.push(key_input(VK_BACK, 0, 0));
        inputs.push(key_input(VK_BACK, 0, KEYEVENTF_KEYUP));
    }
    for c in r.insert.chars() {
        match c {
            '\n' | '\r' | '\t' => {
                let vk = if c == '\t' { VK_TAB } else { VK_RETURN };
                // Preserve Shift so Shift+Enter stays a newline in chats (not a send).
                let shift_was = held[0] || held[1];
                if shift_was {
                    inputs.push(key_input(VK_LSHIFT, 0, 0));
                }
                inputs.push(key_input(vk, 0, 0));
                inputs.push(key_input(vk, 0, KEYEVENTF_KEYUP));
                if shift_was {
                    inputs.push(key_input(VK_LSHIFT, 0, KEYEVENTF_KEYUP));
                }
            }
            _ => {
                let mut units = [0u16; 2];
                for &u in c.encode_utf16(&mut units).iter() {
                    inputs.push(key_input(0, u, KEYEVENTF_UNICODE));
                    inputs.push(key_input(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
                }
            }
        }
    }
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    let complete = sent as usize == inputs.len();
    // Never flip the layout if Windows blocked the replacement (e.g. UIPI).
    if complete { switch_layout(r.target); }
    // Restore only Shift so we do not re-press Ctrl+Shift (Windows language hotkey).
    let physical = STATE.with(|s| s.borrow().as_ref().map(|st| [st.lshift, st.rshift]).unwrap_or([held[0], held[1]]));
    let restore: Vec<INPUT> = [VK_LSHIFT, VK_RSHIFT].into_iter().zip([held[0], held[1]]).zip(physical)
        .filter_map(|((vk, was_down), still_down)| {
            (was_down && still_down).then(|| key_input(vk, 0, 0))
        }).collect();
    if !restore.is_empty() {
        unsafe { SendInput(restore.len() as u32, restore.as_ptr(), std::mem::size_of::<INPUT>() as i32); }
    }
    if !complete {
        // SendInput is blocked as a whole by UIPI (elevated target); a partial send is very unlikely.
        return Err(format!("SendInput sent {sent}/{} events (elevated target window?)", inputs.len()));
    }
    Ok(())
}

/// US-layout character for a virtual key with the given shift state.
fn us_char(vk: u32, shift: bool) -> Option<char> {
    let pick = |a: char, b: char| Some(if shift { b } else { a });
    match vk {
        0x41..=0x5A => {
            let c = char::from(vk as u8 + 32);
            Some(if shift { c.to_ascii_uppercase() } else { c })
        }
        0x30..=0x39 => {
            let i = (vk - 0x30) as usize;
            pick(char::from(b'0' + i as u8), ")!@#$%^&*(".chars().nth(i)?)
        }
        0xBA => pick(';', ':'),
        0xBB => pick('=', '+'),
        0xBC => pick(',', '<'),
        0xBD => pick('-', '_'),
        0xBE => pick('.', '>'),
        0xBF => pick('/', '?'),
        0xC0 => pick('`', '~'),
        0xDB => pick('[', '{'),
        0xDC => pick('\\', '|'),
        0xDD => pick(']', '}'),
        0xDE => pick('\'', '"'),
        0x20 => Some(' '),
        _ => None,
    }
}

fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0x14 | 0x5B | 0x5C | 0xA0..=0xA5)
}

fn vk_to_hotkey_code(vk: u32) -> Option<HotkeyCode> {
    match vk {
        0x13 => Some(HotkeyCode::Pause),
        0x20 => Some(HotkeyCode::Space),
        0x09 => Some(HotkeyCode::Tab),
        0x0D => Some(HotkeyCode::Enter),
        0x1B => Some(HotkeyCode::Escape),
        0x08 => Some(HotkeyCode::Backspace),
        0x30..=0x39 => Some(HotkeyCode::Digit((vk - 0x30) as u8)),
        0x41..=0x5A => Some(HotkeyCode::Letter(char::from(vk as u8))),
        0x70..=0x7B => Some(HotkeyCode::F((vk - 0x6F) as u8)),
        _ => None,
    }
}

fn combo_mods() -> (bool, bool, bool, bool) {
    (
        key_down(VK_CONTROL),
        key_down(VK_MENU),
        key_down(VK_SHIFT),
        key_down(VK_LWIN) || key_down(VK_RWIN),
    )
}

fn match_hotkey(st: &State, vk: u32) -> Option<Input> {
    let code = vk_to_hotkey_code(vk)?;
    let (c, a, s, m) = combo_mods();
    // Undo first: its default Ctrl+Alt+Z must not fall into a relaxed conversion match.
    if st.ctl.undo_hotkey().matches(code, c, a, s, m) {
        Some(Input::Undo)
    } else if st.ctl.hotkey().matches_relaxed_shift(code, c, a, s, m) {
        Some(Input::Hotkey)
    } else if st.ctl.summon_hotkey().matches(code, c, a, s, m) {
        Some(Input::Summon)
    } else {
        None
    }
}

fn enqueue(st: &mut State, input: Input) {
    let cmds = st.ctl.on_input(input);
    if !cmds.is_empty() {
        st.pending.extend(cmds);
        unsafe { PostThreadMessageW(st.thread_id, WM_LK_FLUSH, 0, 0) };
    }
}

/// Returns true to swallow the key (our hotkeys, and Enter/Tab that triggered a replacement).
fn handle_key(st: &mut State, vk: u32, down: bool) -> bool {
    if !down {
        if st.swallow_up == Some(vk) {
            st.swallow_up = None;
            return true;
        }
        if is_modifier(vk) && !st.pending.is_empty() {
            unsafe { PostThreadMessageW(st.thread_id, WM_LK_FLUSH, 0, 0); }
        }
    }
    if vk == VK_LSHIFT as u32 || vk == VK_RSHIFT as u32 || vk == VK_SHIFT as u32 {
        let other = if vk == VK_LSHIFT as u32 { st.rshift } else { st.lshift };
        if vk == VK_LSHIFT as u32 { st.lshift = down } else { st.rshift = down }
        // Shift+Shift: the second Shift goes down while the first is still held.
        if down && other {
            if st.ctl.hotkey().is_both_shifts() {
                enqueue(st, Input::Hotkey);
            } else if st.ctl.summon_hotkey().is_both_shifts() {
                enqueue(st, Input::Summon);
            }
        }
        return false;
    }
    if !down || is_modifier(vk) {
        return false;
    }
    if let Some(input) = match_hotkey(st, vk) {
        enqueue(st, input);
        st.word_lang = None;
        return true;
    }
    let mods = shortcut_mods();
    let input = match vk {
        0x08 => Some(Input::Key { key: Key::Backspace, modifiers: mods }),
        0x0D => Some(Input::Key { key: Key::Boundary('\n'), modifiers: mods }),
        0x09 => Some(Input::Key { key: Key::Boundary('\t'), modifiers: mods }),
        _ => match us_char(vk, key_down(VK_SHIFT)) {
            Some(mut us) => {
                if unsafe { GetKeyState(VK_CAPITAL as i32) } & 1 != 0 && us.is_ascii_alphabetic() {
                    us = if us.is_ascii_uppercase() { us.to_ascii_lowercase() } else { us.to_ascii_uppercase() };
                }
                if us.is_alphabetic() && st.word_lang.is_none() {
                    st.word_lang = live_layout().or_else(|| st.ctl.current_layout());
                }
                let c = match st.word_lang.or_else(live_layout).or_else(|| st.ctl.current_layout()) {
                    Some(Lang::Ru) => layout::en_to_ru(us).unwrap_or(us),
                    _ => us,
                };
                Some(Input::Key { key: Key::Char(c), modifiers: mods })
            }
            // Arrows, Home/End, Escape, Delete, F-keys…: caret may have moved.
            None => {
                st.word_lang = None;
                Some(Input::Key { key: Key::Reset, modifiers: mods })
            }
        },
    };
    let Some(input) = input else { return false };
    enqueue(st, input);
    if st.ctl.current_word_len() == 0 {
        st.word_lang = None;
    }
    // Swallow Enter/Tab when they triggered a replacement so the chat does not
    // send the uncorrected word. replace() injects the key after the new text.
    // Backspace right after an autocorrection undoes it: the restore deletes the
    // boundary itself, so this Backspace must not reach the app.
    if matches!(vk, 0x0D | 0x09 | 0x08) && st.pending.iter().any(|c| matches!(c, Command::Replace(_))) {
        st.swallow_up = Some(vk);
        return true;
    }
    false
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        let injected = kb.flags & LLKHF_INJECTED != 0 || kb.dwExtraInfo == LK_MAGIC;
        let msg = wparam as u32;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if !injected && (down || up) {
            let vk = kb.vkCode;
            let swallow = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if down && !is_modifier(vk) {
                    let has_pending = STATE.with(|s| {
                        s.try_borrow().ok().and_then(|g| g.as_ref().map(|st| !st.pending.is_empty())).unwrap_or(false)
                    });
                    if has_pending {
                        flush();
                    }
                }
                STATE.with(|s| match s.try_borrow_mut() {
                    Ok(mut g) => g.as_mut().map(|st| handle_key(st, vk, down)).unwrap_or(false),
                    Err(_) => false,
                })
            }))
            .unwrap_or(false);
            if swallow {
                return 1;
            }
        }
    }
    CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wparam as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        let _ = std::panic::catch_unwind(|| {
            STATE.with(|s| {
                if let Ok(mut g) = s.try_borrow_mut() {
                    if let Some(st) = g.as_mut() {
                        if st.pending.iter().any(|c| matches!(c, Command::Replace(_))) {
                            st.ctl.replace_failed();
                        }
                        st.pending.clear();
                        st.word_lang = None;
                        st.ctl.on_input(Input::Key { key: Key::Reset, modifiers: false });
                    }
                }
            })
        });
    }
    CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
}

fn flush() {
    let cmds: Vec<Command> = STATE.with(|s| {
        let mut g = s.borrow_mut();
        let Some(st) = g.as_mut() else { return vec![] };
        if st.pending.is_empty() {
            return vec![];
        }
        // Do not wait for Ctrl/Alt/Win: replace() releases them before SendInput,
        // and SummonGui should raise the window even while the chord is still down.
        std::mem::take(&mut st.pending)
    });
    for c in cmds {
        let ok = match &c {
            Command::SwitchLayout(l) => switch_layout(*l),
            Command::Replace(r) => match replace(r) {
                Ok(()) => {
                    STATE.with(|s| {
                        if let Some(st) = s.borrow_mut().as_mut() {
                            st.ctl.replace_succeeded(r.target);
                        }
                    });
                    log::debug!("replaced {} chars with {} chars, layout {}", r.delete, r.insert.chars().count(), r.target);
                    true
                }
                Err(e) => {
                    log::warn!("replacement skipped: {e}");
                    false
                }
            },
            Command::SummonGui => {
                summon_gui();
                true
            }
            Command::ConvertSelection => {
                let hook_thread = STATE.with(|s| s.borrow().as_ref().map(|st| st.thread_id)).unwrap_or(0);
                crate::selection::spawn(WinKeys { hook_thread });
                true
            }
        };
        if !ok {
            STATE.with(|s| {
                if let Some(st) = s.borrow_mut().as_mut() {
                    st.ctl.replace_failed();
                }
            });
        }
    }
}

fn tick() {
    // Keep the observation and language request on exactly the same HWND.
    let hwnd = target_window();
    let (app, lang) = foreground_for(hwnd);
    let cmds = STATE.with(|s| {
        let mut g = s.borrow_mut();
        let Some(st) = g.as_mut() else { return vec![] };
        let mut cmds = st.ctl.tick(app.as_deref(), lang);
        if st.pending.iter().any(|c| matches!(c, Command::Replace(_))) {
            cmds.retain(|c| !matches!(c, Command::SwitchLayout(_)));
        }
        cmds
    });
    for c in cmds {
        if let Command::SwitchLayout(l) = c {
            switch_layout_for(hwnd, l);
        }
    }
}

unsafe extern "system" fn win_event_proc(_h: HWINEVENTHOOK, _e: u32, _w: HWND, _o: i32, _c: i32, _t: u32, _ms: u32) {
    // Out-of-context hooks are delivered on our message loop thread; avoid re-entrancy.
    let busy = STATE.with(|s| s.try_borrow_mut().is_err());
    if !busy {
        tick();
    }
}

fn daemon_mutex_name() -> Vec<u16> {
    "Local\\LayoutKeeper-daemon\0".encode_utf16().collect()
}

/// Own this handle until the event loop exits; even error paths release it.
struct DaemonMutex(HANDLE);
impl Drop for DaemonMutex {
    fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
}

fn acquire_daemon_mutex() -> Result<Option<DaemonMutex>, String> {
    let name = daemon_mutex_name();
    unsafe {
        let handle = CreateMutexW(ptr::null(), 0, name.as_ptr());
        let error = GetLastError();
        if handle.is_null() {
            return Err(format!("CreateMutexW(LayoutKeeper-daemon) failed: {error}"));
        }
        let guard = DaemonMutex(handle);
        if error == ERROR_ALREADY_EXISTS {
            drop(guard);
            return Ok(None);
        }
        Ok(Some(guard))
    }
}

pub fn daemon_is_running() -> Result<bool, String> {
    let name = daemon_mutex_name();
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if !handle.is_null() {
            CloseHandle(handle);
            return Ok(true);
        }
        let error = GetLastError();
        if error == ERROR_FILE_NOT_FOUND { Ok(false) }
        else { Err(format!("OpenMutexW(LayoutKeeper-daemon) failed: {error}")) }
    }
}

pub fn ensure_daemon_running() -> Result<(), String> {
    if !daemon_is_running()? {
        crate::platform::spawn_daemon()?;
    }
    Ok(())
}

fn post_quit_to_process(pid: u32) {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snap == INVALID_HANDLE_VALUE {
            return;
        }
        let mut entry = std::mem::zeroed::<THREADENTRY32>();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snap, &mut entry) != 0 {
            loop {
                if entry.th32OwnerProcessID == pid {
                    PostThreadMessageW(entry.th32ThreadID, WM_QUIT, 0, 0);
                }
                if Thread32Next(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
}

/// Stop the background `layout-keeper.exe` (no-argument) process, leaving this GUI alive.
/// Ask it to unhook via WM_QUIT first; only force-kill if it ignores that.
pub fn stop_daemon() {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return;
        }
        let self_pid = GetCurrentProcessId();
        let mut pids = Vec::new();
        let mut entry = std::mem::zeroed::<PROCESSENTRY32W>();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let name = {
                    let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    String::from_utf16_lossy(&entry.szExeFile[..end])
                };
                if name.eq_ignore_ascii_case("layout-keeper.exe") && entry.th32ProcessID != self_pid {
                    pids.push(entry.th32ProcessID);
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        for pid in pids {
            post_quit_to_process(pid);
            let handle = OpenProcess(PROCESS_TERMINATE | SYNCHRONIZATION_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                continue;
            }
            let waited = WaitForSingleObject(handle, 1500);
            if waited != 0 {
                TerminateProcess(handle, 0);
                let _ = WaitForSingleObject(handle, 300);
            }
            CloseHandle(handle);
        }
    }
}

fn gui_mutex_name() -> Vec<u16> {
    "Local\\LayoutKeeper-gui\0".encode_utf16().collect()
}

/// Held for the lifetime of the settings process so a second launch raises the existing window.
pub struct GuiMutex(HANDLE);
impl Drop for GuiMutex {
    fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
}

pub fn acquire_gui_mutex() -> Result<Option<GuiMutex>, String> {
    let name = gui_mutex_name();
    unsafe {
        let handle = CreateMutexW(ptr::null(), 0, name.as_ptr());
        let error = GetLastError();
        if handle.is_null() {
            return Err(format!("CreateMutexW(LayoutKeeper-gui) failed: {error}"));
        }
        let guard = GuiMutex(handle);
        if error == ERROR_ALREADY_EXISTS {
            drop(guard);
            return Ok(None);
        }
        Ok(Some(guard))
    }
}

pub fn raise_settings_window() -> bool {
    let mut title: Vec<u16> = crate::brand::WINDOW_TITLE.encode_utf16().collect();
    title.push(0);
    unsafe {
        let hwnd = FindWindowW(ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return false;
        }
        force_foreground(hwnd);
        true
    }
}

/// Wake a hidden settings window without blocking the caller (used on tray Quit).
pub fn wake_settings_window_async() -> bool {
    let mut title: Vec<u16> = crate::brand::WINDOW_TITLE.encode_utf16().collect();
    title.push(0);
    unsafe {
        let hwnd = FindWindowW(ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return false;
        }
        ShowWindowAsync(hwnd, SW_SHOW);
        ShowWindowAsync(hwnd, SW_RESTORE);
        PostMessageW(hwnd, WM_NULL, 0, 0);
        true
    }
}

fn keep_daemon_responsive() {
    unsafe {
        let process = GetCurrentProcess();
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: 0,
        };
        // Best effort: older Windows versions may not support this information class.
        SetProcessInformation(process, ProcessPowerThrottling,
            &state as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32);
        SetPriorityClass(process, ABOVE_NORMAL_PRIORITY_CLASS);
    }
}

pub fn run(ctl: Controller) -> Result<(), String> {
    let Some(_daemon_mutex) = acquire_daemon_mutex()? else {
        log::info!("Typed daemon already running; not installing duplicate hooks");
        return Ok(());
    };
    keep_daemon_responsive();
    unsafe {
        let thread_id = GetCurrentThreadId();
        STATE.with(|s| {
            *s.borrow_mut() =
                Some(State { ctl, pending: vec![], thread_id, lshift: false, rshift: false, swallow_up: None, word_lang: None })
        });
        let hmod = GetModuleHandleW(ptr::null());
        let kb: HHOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0);
        if kb.is_null() {
            return Err("SetWindowsHookExW(WH_KEYBOARD_LL) failed".into());
        }
        let ms: HHOOK = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0);
        SetTimer(ptr::null_mut(), 0, 40, None);
        let ev_fg = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, ptr::null_mut(), Some(win_event_proc), 0, 0, WINEVENT_OUTOFCONTEXT);
        let ev_min = SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND, ptr::null_mut(), Some(win_event_proc), 0, 0, WINEVENT_OUTOFCONTEXT);
        log::info!("running on Windows (WH_KEYBOARD_LL)");
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            match msg.message {
                WM_LK_FLUSH => flush(),
                WM_LK_SEL_DONE => {
                    let lang = if msg.wParam == 2 { Lang::Ru } else { Lang::En };
                    if switch_layout(lang) {
                        STATE.with(|s| {
                            if let Some(st) = s.borrow_mut().as_mut() {
                                st.ctl.replace_succeeded(lang);
                            }
                        });
                    }
                }
                WM_TIMER => {
                    flush();
                    tick();
                }
                _ => {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
        for h in [ev_fg, ev_min] {
            if !h.is_null() {
                UnhookWinEvent(h);
            }
        }
        UnhookWindowsHookEx(kb);
        if !ms.is_null() {
            UnhookWindowsHookEx(ms);
        }
    }
    Ok(())
}

pub fn probe() -> (Option<String>, Option<Lang>) {
    foreground()
}

/// Type `text` into the focused window (vault autofill from the GUI process).
/// Uses `KEYEVENTF_UNICODE`, so it is independent of the active layout; `\t` is a real Tab.
/// Events carry `LK_MAGIC`, so a running daemon's hook ignores them (no autocorrect, no buffering).
pub fn type_text(text: &str) -> Result<(), String> {
    let mut inputs = Vec::with_capacity(text.len() * 2);
    for c in text.chars() {
        if c == '\t' {
            inputs.push(key_input(VK_TAB, 0, 0));
            inputs.push(key_input(VK_TAB, 0, KEYEVENTF_KEYUP));
        } else {
            let mut units = [0u16; 2];
            for &u in c.encode_utf16(&mut units).iter() {
                inputs.push(key_input(0, u, KEYEVENTF_UNICODE));
                inputs.push(key_input(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
            }
        }
    }
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    // Overwrite the UTF-16 units we copied into the INPUT buffer.
    for i in inputs.iter_mut() {
        i.Anonymous.ki.wScan = 0;
    }
    if sent as usize != inputs.len() {
        return Err("Windows заблокировал ввод (окно запущено от администратора?)".into());
    }
    Ok(())
}


fn summon_gui() {
    crate::platform::request_open_vault();
    let mut title: Vec<u16> = crate::brand::WINDOW_TITLE.encode_utf16().collect();
    title.push(0);
    unsafe {
        let hwnd = FindWindowW(ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            force_foreground(hwnd);
            return;
        }
    }
    crate::platform::spawn_settings_gui_vault();
}

fn force_foreground(hwnd: HWND) {
    unsafe {
        ShowWindowAsync(hwnd, SW_SHOW);
        ShowWindowAsync(hwnd, SW_RESTORE);
        let fg = GetForegroundWindow();
        let mut _fg_pid = 0u32;
        let fg_tid = GetWindowThreadProcessId(fg, &mut _fg_pid);
        let mut gui_pid = 0u32;
        let gui_tid = GetWindowThreadProcessId(hwnd, &mut gui_pid);
        let this_tid = GetCurrentThreadId();
        AllowSetForegroundWindow(if gui_pid != 0 { gui_pid } else { ASFW_ANY });
        let attached_fg = fg_tid != 0 && fg_tid != this_tid && AttachThreadInput(this_tid, fg_tid, 1) != 0;
        let attached_gui = gui_tid != 0 && gui_tid != this_tid && gui_tid != fg_tid && AttachThreadInput(this_tid, gui_tid, 1) != 0;
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        SetActiveWindow(hwnd);
        if attached_gui {
            AttachThreadInput(this_tid, gui_tid, 0);
        }
        if attached_fg {
            AttachThreadInput(this_tid, fg_tid, 0);
        }
        if GetForegroundWindow() != hwnd {
            AllowSetForegroundWindow(ASFW_ANY);
            SetForegroundWindow(hwnd);
        }
    }
}
