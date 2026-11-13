//! System tray / menu-bar icon.
//!
//! * Windows (since 0.3.69): the icon lives in the background daemon (`typed.exe` with no
//!   arguments). Its commands open the settings window — a separate `typed.exe gui`
//!   process that exits whenever it is closed or minimized — or quit both processes.
//! * macOS: the key tap runs inside Typed.app, so the menu-bar icon stays in that process;
//!   closing or minimizing hides the window and the icon brings it back.

use eframe::egui;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TrayCmd {
    Show,
    ShowVault,
    Quit,
}

pub struct TrayHandle {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
    rx: Option<Receiver<TrayCmd>>,
}

#[allow(dead_code)]
impl TrayHandle {
    pub fn try_recv(&self) -> Option<TrayCmd> {
        self.rx.as_ref()?.try_recv().ok()
    }

    pub fn set_repaint(&self, ctx: egui::Context) {
        if let Ok(mut slot) = REPAINT.lock() {
            *slot = Some(ctx);
        }
    }

    pub fn is_active(&self) -> bool {
        self.hwnd != 0
    }

    pub fn set_logo(&self, index: u8) {
        #[cfg(windows)]
        if self.hwnd != 0 {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                    self.hwnd as windows_sys::Win32::Foundation::HWND,
                    win::WM_SET_LOGO,
                    index as windows_sys::Win32::Foundation::WPARAM,
                    0,
                );
            }
        }
        #[cfg(target_os = "macos")]
        if self.hwnd != 0 {
            mac::set_logo(self.hwnd as *mut std::ffi::c_void, index);
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = index;
    }
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.hwnd != 0 {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                    self.hwnd as windows_sys::Win32::Foundation::HWND,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    0,
                    0,
                );
            }
        }
        #[cfg(target_os = "macos")]
        if self.hwnd != 0 {
            mac::remove(self.hwnd as *mut std::ffi::c_void);
            self.hwnd = 0;
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Ok(mut slot) = REPAINT.lock() {
            *slot = None;
        }
    }
}

static REPAINT: Mutex<Option<egui::Context>> = Mutex::new(None);

/// Settings-window tray (macOS menu bar; elsewhere there is none in the GUI process).
#[allow(dead_code)]
pub fn start(logo_index: u8) -> Option<TrayHandle> {
    #[cfg(target_os = "macos")]
    {
        return mac::start(logo_index);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = logo_index;
        None
    }
}

/// Windows daemon tray: `handler` runs on the tray thread for every menu / click command.
#[cfg(windows)]
pub fn start_daemon_tray(logo_index: u8, handler: fn(TrayCmd)) -> Option<TrayHandle> {
    win::start(logo_index, handler)
}

#[allow(dead_code)]
pub fn wake_gui() {
    if let Ok(slot) = REPAINT.lock() {
        if let Some(ctx) = slot.as_ref() {
            ctx.request_repaint();
        }
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{ChangeWindowMessageFilterEx, KillTimer, SetTimer, MSGFLT_ALLOW, WM_TIMER};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
        GetWindowLongPtrW, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
        SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, TrackPopupMenu, TranslateMessage, ICONINFO,
        WNDCLASSW, GWLP_USERDATA, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL,
        WM_RBUTTONUP, WM_APP, WS_EX_TOOLWINDOW, WS_POPUP,
    };

    const TPM_RIGHTBUTTON: u32 = 0x0002;
    const TPM_RETURNCMD: u32 = 0x0100;
    const MF_STRING: u32 = 0x0000;
    const MF_SEPARATOR: u32 = 0x0800;
    const ID_SHOW: usize = 1;
    const ID_VAULT: usize = 2;
    const ID_QUIT: usize = 3;
    const CALLBACK: u32 = WM_APP + 0x20;
    pub const WM_SET_LOGO: u32 = WM_APP + 0x21;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const SW_HIDE: i32 = 0;
    const HWND_TOPMOST: HWND = -1isize as HWND;
    /// The daemon re-reads the logo choice from config.json this often.
    const LOGO_TIMER: usize = 1;
    const LOGO_POLL_MS: u32 = 2000;

    struct TrayState {
        handler: fn(TrayCmd),
        hicon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
        logo: u8,
        config_mtime: Option<std::time::SystemTime>,
    }

    /// Logo chosen in the settings window (config.json), if the file changed since `seen`.
    fn config_logo(seen: &mut Option<std::time::SystemTime>) -> Option<u8> {
        let path = layout_keeper_core::Config::default_path();
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if mtime.is_none() || mtime == *seen {
            return None;
        }
        *seen = mtime;
        let cfg = std::fs::read_to_string(&path).ok().and_then(|s| layout_keeper_core::Config::from_json(&s).ok())?;
        Some(crate::logos::index(cfg.logo_style))
    }

    /// Remove the icon and free the state; safe to call twice.
    fn cleanup(hwnd: HWND) {
        unsafe {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
            if !raw.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let state = Box::from_raw(raw);
                let data = nid(hwnd, state.hicon);
                Shell_NotifyIconW(NIM_DELETE, &data);
                DestroyIcon(state.hicon);
                KillTimer(hwnd, LOGO_TIMER);
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn hicon_from_rgba(width: i32, height: i32, rgba: &[u8]) -> Option<windows_sys::Win32::UI::WindowsAndMessaging::HICON> {
        unsafe {
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let hdc = GetDC(std::ptr::null_mut());
            let color = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            ReleaseDC(std::ptr::null_mut(), hdc);
            if color.is_null() || bits.is_null() {
                return None;
            }
            let n = (width * height) as usize;
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, n * 4);
            for i in 0..n {
                dst[i * 4] = rgba[i * 4 + 2];
                dst[i * 4 + 1] = rgba[i * 4 + 1];
                dst[i * 4 + 2] = rgba[i * 4];
                dst[i * 4 + 3] = rgba[i * 4 + 3];
            }
            let mask = CreateBitmap(width, height, 1, 1, std::ptr::null());
            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&info);
            DeleteObject(mask);
            DeleteObject(color);
            if icon.is_null() {
                None
            } else {
                Some(icon)
            }
        }
    }

    fn nid(hwnd: HWND, icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON) -> NOTIFYICONDATAW {
        unsafe {
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            data.uCallbackMessage = CALLBACK;
            data.hIcon = icon;
            let tip = wide("Typed");
            for (i, ch) in tip.iter().take(data.szTip.len() - 1).enumerate() {
                data.szTip[i] = *ch;
            }
            data
        }
    }

    fn send_cmd(hwnd: HWND, cmd: TrayCmd) {
        let handler = unsafe {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
            if raw.is_null() {
                return;
            }
            (*raw).handler
        };
        handler(cmd);
    }

    fn hicon_from_png(bytes: &[u8]) -> Option<windows_sys::Win32::UI::WindowsAndMessaging::HICON> {
        let icon_data = eframe::icon_data::from_png_bytes(bytes).ok()?;
        hicon_from_rgba(icon_data.width as i32, icon_data.height as i32, &icon_data.rgba)
    }

    fn apply_icon(hwnd: HWND, index: u8) {
        unsafe {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
            if raw.is_null() {
                return;
            }
            let Some(new_icon) = hicon_from_png(crate::logos::icon_png(crate::logos::from_index(index))) else {
                return;
            };
            let old = (*raw).hicon;
            (*raw).hicon = new_icon;
            (*raw).logo = index;
            let data = nid(hwnd, new_icon);
            Shell_NotifyIconW(NIM_MODIFY, &data);
            DestroyIcon(old);
        }
    }

    fn popup_menu(hwnd: HWND) {
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }
            let open = wide("Открыть");
            let vault = wide("Сейф");
            let quit = wide("Выход");
            AppendMenuW(menu, MF_STRING, ID_SHOW, open.as_ptr());
            AppendMenuW(menu, MF_STRING, ID_VAULT, vault.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, ID_QUIT, quit.as_ptr());
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            SetWindowPos(hwnd, HWND_TOPMOST, pt.x, pt.y, 1, 1, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            SetForegroundWindow(hwnd);
            let picked = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, pt.x, pt.y, 0, hwnd, std::ptr::null());
            PostMessageW(hwnd, WM_NULL, 0, 0);
            ShowWindow(hwnd, SW_HIDE);
            DestroyMenu(menu);
            match picked {
                x if x == ID_SHOW as i32 => send_cmd(hwnd, TrayCmd::Show),
                x if x == ID_VAULT as i32 => send_cmd(hwnd, TrayCmd::ShowVault),
                x if x == ID_QUIT as i32 => send_cmd(hwnd, TrayCmd::Quit),
                _ => {}
            }
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        static TASKBAR_CREATED: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        let taskbar_created = *TASKBAR_CREATED.get_or_init(|| {
            let name = wide("TaskbarCreated");
            unsafe { RegisterWindowMessageW(name.as_ptr()) }
        });
        if msg == taskbar_created {
            unsafe {
                let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
                if !raw.is_null() {
                    let data = nid(hwnd, (*raw).hicon);
                    Shell_NotifyIconW(NIM_ADD, &data);
                }
            }
            return 0;
        }
        match msg {
            CALLBACK => {
                let event = lparam as u32;
                if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                    send_cmd(hwnd, TrayCmd::Show);
                } else if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    popup_menu(hwnd);
                }
                0
            }
            x if x == WM_SET_LOGO => {
                apply_icon(hwnd, wparam as u8);
                0
            }
            WM_TIMER if wparam == LOGO_TIMER => {
                let changed = unsafe {
                    let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
                    if raw.is_null() {
                        None
                    } else {
                        config_logo(&mut (*raw).config_mtime).filter(|&l| l != (*raw).logo)
                    }
                };
                if let Some(l) = changed {
                    apply_icon(hwnd, l);
                }
                0
            }
            WM_CLOSE => {
                cleanup(hwnd);
                unsafe { DestroyWindow(hwnd) };
                0
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    fn setup(handler: fn(TrayCmd), logo_index: u8) -> Result<HWND, String> {
        let hicon = hicon_from_png(crate::logos::icon_png(crate::logos::from_index(logo_index)))
            .ok_or_else(|| "CreateIconIndirect failed".to_string())?;

        let class = wide("TypedTrayWnd");
        unsafe {
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(std::ptr::null()),
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc);
            let title = wide("TypedTrayHost");
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            if hwnd.is_null() {
                DestroyIcon(hicon);
                return Err("CreateWindowExW(TypedTrayWnd) failed".into());
            }
            let mut config_mtime = None;
            let _ = config_logo(&mut config_mtime);
            let state = Box::new(TrayState { handler, hicon, logo: logo_index, config_mtime });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            // A daemon running as administrator (the «Typed» task) must still receive
            // Explorer's tray clicks and its restart broadcast (UIPI filters them otherwise).
            let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
            for m in [CALLBACK, taskbar_created, WM_CONTEXTMENU] {
                ChangeWindowMessageFilterEx(hwnd, m, MSGFLT_ALLOW, std::ptr::null_mut());
            }
            SetTimer(hwnd, LOGO_TIMER, LOGO_POLL_MS, None);
            let data = nid(hwnd, hicon);
            let mut added = false;
            for _ in 0..8 {
                if Shell_NotifyIconW(NIM_ADD, &data) != 0 {
                    added = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            if !added {
                let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
                if !raw.is_null() {
                    let state = Box::from_raw(raw);
                    DestroyIcon(state.hicon);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                DestroyWindow(hwnd);
                return Err("Shell_NotifyIconW(NIM_ADD) failed".into());
            }
            Ok(hwnd)
        }
    }

    fn message_loop() {
        unsafe {
            let mut msg = std::mem::zeroed();
            loop {
                let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if r == 0 || r == -1 {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub fn start(logo_index: u8, handler: fn(TrayCmd)) -> Option<TrayHandle> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();
        let thread = thread::Builder::new()
            .name("typed-tray".into())
            .spawn(move || match setup(handler, logo_index) {
                Ok(hwnd) => {
                    let _ = ready_tx.send(Ok(hwnd as isize));
                    message_loop();
                    // A WM_QUIT posted to every thread (`stop_daemon` from another
                    // process) ends the loop before WM_CLOSE: remove the icon anyway,
                    // or a dead icon stays in the tray until the mouse passes over it.
                    cleanup(hwnd);
                    unsafe { DestroyWindow(hwnd) };
                }
                Err(e) => {
                    log::warn!("system tray: {e}");
                    let _ = ready_tx.send(Err(e));
                }
            })
            .ok()?;
        match ready_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(hwnd)) => Some(TrayHandle {
                hwnd,
                thread: Some(thread),
                rx: None,
            }),
            Ok(Err(e)) => {
                log::warn!("system tray unavailable: {e}");
                let _ = thread.join();
                None
            }
            Err(_) => {
                log::warn!("system tray did not start in time");
                None
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::mpsc::Sender;
    use std::sync::Mutex;

    type Id = *mut c_void;
    type Sel = *const c_void;

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
        fn objc_registerClassPair(cls: Id);
        fn class_addMethod(cls: Id, name: Sel, imp: *mut c_void, types: *const c_char) -> u8;
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(p: *mut c_void);
    }

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    static TX: Mutex<Option<Sender<TrayCmd>>> = Mutex::new(None);
    static TARGET: Mutex<Option<usize>> = Mutex::new(None);

    fn sel(name: &CStr) -> Sel {
        unsafe { sel_registerName(name.as_ptr()) }
    }

    unsafe fn msg0(obj: Id, s: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(obj, s)
    }

    unsafe fn msg1(obj: Id, s: Sel, a: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(obj, s, a)
    }

    unsafe fn msg_f64(obj: Id, s: Sel, v: f64) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, f64) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(obj, s, v)
    }

    fn nsstring(s: &str) -> Id {
        let c = CString::new(s).unwrap_or_default();
        unsafe {
            let cls = objc_getClass(c"NSString".as_ptr());
            let alloc = msg0(cls, sel(c"alloc"));
            let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            f(alloc, sel(c"initWithUTF8String:"), c.as_ptr())
        }
    }

    fn send(cmd: TrayCmd) {
        if let Ok(g) = TX.lock() {
            if let Some(tx) = g.as_ref() {
                let _ = tx.send(cmd);
            }
        }
        super::wake_gui();
    }

    unsafe extern "C" fn on_show(_this: Id, _cmd: Sel, _sender: Id) {
        send(TrayCmd::Show);
    }
    unsafe extern "C" fn on_vault(_this: Id, _cmd: Sel, _sender: Id) {
        send(TrayCmd::ShowVault);
    }
    unsafe extern "C" fn on_quit(_this: Id, _cmd: Sel, _sender: Id) {
        send(TrayCmd::Quit);
    }

    fn nsimage_from_png(bytes: &[u8], point_size: f64) -> Id {
        unsafe {
            let data_cls = objc_getClass(c"NSData".as_ptr());
            let f: unsafe extern "C" fn(Id, Sel, *const u8, usize) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let data = f(
                data_cls,
                sel(c"dataWithBytes:length:"),
                bytes.as_ptr(),
                bytes.len(),
            );
            if data.is_null() {
                return std::ptr::null_mut();
            }
            let img_cls = objc_getClass(c"NSImage".as_ptr());
            let img = msg0(img_cls, sel(c"alloc"));
            let img = msg1(img, sel(c"initWithData:"), data);
            if img.is_null() {
                return std::ptr::null_mut();
            }
            let set_size: unsafe extern "C" fn(Id, Sel, f64, f64) =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            set_size(img, sel(c"setSize:"), point_size, point_size);
            let set_tmpl: unsafe extern "C" fn(Id, Sel, bool) =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            set_tmpl(img, sel(c"setTemplate:"), false);
            img
        }
    }

    fn ensure_target_class() -> Id {
        unsafe {
            let existing = objc_getClass(c"TypedTrayTarget".as_ptr());
            if !existing.is_null() {
                return existing;
            }
            let super_cls = objc_getClass(c"NSObject".as_ptr());
            let cls = objc_allocateClassPair(super_cls, c"TypedTrayTarget".as_ptr(), 0);
            if cls.is_null() {
                return std::ptr::null_mut();
            }
            let types = c"v@:@".as_ptr();
            class_addMethod(cls, sel(c"onShow:"), on_show as *mut c_void, types);
            class_addMethod(cls, sel(c"onVault:"), on_vault as *mut c_void, types);
            class_addMethod(cls, sel(c"onQuit:"), on_quit as *mut c_void, types);
            objc_registerClassPair(cls);
            cls
        }
    }

    fn add_item(menu: Id, title: &str, action: &CStr, target: Id) {
        unsafe {
            let item_cls = objc_getClass(c"NSMenuItem".as_ptr());
            let item = msg0(item_cls, sel(c"alloc"));
            let title_s = nsstring(title);
            let key = nsstring("");
            let init: unsafe extern "C" fn(Id, Sel, Id, Sel, Id) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let item = init(
                item,
                sel(c"initWithTitle:action:keyEquivalent:"),
                title_s,
                sel(action),
                key,
            );
            msg1(item, sel(c"setTarget:"), target);
            msg1(menu, sel(c"addItem:"), item);
        }
    }

    pub fn start(logo_index: u8) -> Option<TrayHandle> {
        let (tx, rx) = std::sync::mpsc::channel();
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let result = (|| {
                let bar_cls = objc_getClass(c"NSStatusBar".as_ptr());
                if bar_cls.is_null() {
                    return None;
                }
                let bar = msg0(bar_cls, sel(c"systemStatusBar"));
                let item = msg_f64(bar, sel(c"statusItemWithLength:"), -2.0);
                if item.is_null() {
                    return None;
                }
                let _ = msg0(item, sel(c"retain"));
                let button = msg0(item, sel(c"button"));
                let img = nsimage_from_png(
                    crate::logos::icon_png(crate::logos::from_index(logo_index)),
                    18.0,
                );
                if !button.is_null() && !img.is_null() {
                    msg1(button, sel(c"setImage:"), img);
                    msg1(button, sel(c"setToolTip:"), nsstring("Typed"));
                }
                let cls = ensure_target_class();
                if cls.is_null() {
                    return None;
                }
                let target = msg0(msg0(cls, sel(c"alloc")), sel(c"init"));
                if target.is_null() {
                    return None;
                }
                let _ = msg0(target, sel(c"retain"));
                *TARGET.lock().ok()? = Some(target as usize);
                *TX.lock().ok()? = Some(tx);

                let menu_cls = objc_getClass(c"NSMenu".as_ptr());
                let menu = msg0(msg0(menu_cls, sel(c"alloc")), sel(c"init"));
                add_item(menu, "Открыть", c"onShow:", target);
                add_item(menu, "Сейф", c"onVault:", target);
                let sep = msg0(objc_getClass(c"NSMenuItem".as_ptr()), sel(c"separatorItem"));
                msg1(menu, sel(c"addItem:"), sep);
                add_item(menu, "Выход", c"onQuit:", target);
                msg1(item, sel(c"setMenu:"), menu);

                Some(TrayHandle {
                    hwnd: item as isize,
                    thread: None,
                    rx: Some(rx),
                })
            })();
            objc_autoreleasePoolPop(pool);
            result
        }
    }

    pub fn set_logo(item: *mut c_void, index: u8) {
        if item.is_null() {
            return;
        }
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let button = msg0(item, sel(c"button"));
            let img = nsimage_from_png(
                crate::logos::icon_png(crate::logos::from_index(index)),
                18.0,
            );
            if !button.is_null() && !img.is_null() {
                msg1(button, sel(c"setImage:"), img);
            }
            objc_autoreleasePoolPop(pool);
        }
    }

    pub fn remove(item: *mut c_void) {
        if item.is_null() {
            return;
        }
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let bar_cls = objc_getClass(c"NSStatusBar".as_ptr());
            let bar = msg0(bar_cls, sel(c"systemStatusBar"));
            msg1(bar, sel(c"removeStatusItem:"), item);
            let _ = msg0(item, sel(c"release"));
            if let Ok(mut g) = TARGET.lock() {
                if let Some(t) = g.take() {
                    let _ = msg0(t as Id, sel(c"release"));
                }
            }
            if let Ok(mut g) = TX.lock() {
                *g = None;
            }
            objc_autoreleasePoolPop(pool);
        }
    }
}
