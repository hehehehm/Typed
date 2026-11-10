//! macOS adapter.
//!
//! * focused app: `NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier` (via the ObjC runtime);
//! * layout: Text Input Sources (Carbon `TIS*`): the current keyboard input source's first language,
//!   switching with `TISSelectInputSource` to an installed, selectable source whose language is
//!   `en` / `ru` (no hard-coded source ids — "ABC", "U.S.", "Russian – PC" … all work);
//! * keys: `CGEventTap` that can swallow Enter/Tab when a replacement is queued (needs **Input Monitoring** / Accessibility);
//! * replacement: synthetic Delete key presses + `CGEventKeyboardSetUnicodeString` events
//!   (needs **Accessibility**).
//!
//! Synthetic events are tagged in `kCGEventSourceUserData` so our own tap ignores them.

#![allow(non_upper_case_globals, non_snake_case)]

use crate::controller::{Command, Controller, Input};
use layout_keeper_core::{layout, HotkeyCode, Key, Lang, Replacement};
use std::ffi::{c_char, c_void, CStr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFArrayRef = *const c_void;
type CFIndex = isize;
type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type CGEventTapCallBack = unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;
type Id = *mut c_void;
type Sel = *const c_void;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopCommonModes: CFStringRef;
    static kCFBooleanTrue: CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFArrayGetCount(a: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(a: CFArrayRef, i: CFIndex) -> CFTypeRef;
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, size: CFIndex, enc: u32) -> u8;
    fn CFMachPortCreateRunLoopSource(alloc: CFTypeRef, port: CFMachPortRef, order: CFIndex) -> *mut c_void;
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, src: *mut c_void, mode: CFStringRef);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, mask: u64, cb: CGEventTapCallBack, info: *mut c_void) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(e: CGEventRef, field: u32) -> i64;
    fn CGEventSetIntegerValueField(e: CGEventRef, field: u32, value: i64);
    fn CGEventGetFlags(e: CGEventRef) -> u64;
    fn CGEventSetFlags(e: CGEventRef, flags: u64);
    fn CGEventCreateKeyboardEvent(src: CFTypeRef, keycode: u16, down: bool) -> CGEventRef;
    fn CGEventKeyboardSetUnicodeString(e: CGEventRef, len: usize, chars: *const u16);
    fn CGEventPost(tap: u32, e: CGEventRef);
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    static kTISPropertyInputSourceLanguages: CFStringRef;
    static kTISPropertyInputSourceCategory: CFStringRef;
    static kTISCategoryKeyboardInputSource: CFStringRef;
    static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
    fn TISCopyCurrentKeyboardInputSource() -> CFTypeRef;
    fn TISCreateInputSourceList(props: CFTypeRef, include_all_installed: bool) -> CFArrayRef;
    fn TISGetInputSourceProperty(src: CFTypeRef, key: CFStringRef) -> CFTypeRef;
    fn TISSelectInputSource(src: CFTypeRef) -> i32;
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn class_addMethod(cls: Id, name: Sel, imp: *mut c_void, types: *const c_char) -> u8;
    fn class_getInstanceMethod(cls: Id, name: Sel) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(p: *mut c_void);
}

const kCFStringEncodingUTF8: u32 = 0x0800_0100;
const kCGSessionEventTap: u32 = 1;
const kCGHIDEventTap: u32 = 0;
const kCGHeadInsertEventTap: u32 = 0;
const kCGEventTapOptionDefault: u32 = 0;
const kCGEventTapOptionListenOnly: u32 = 1;
const kCGEventLeftMouseDown: u32 = 1;
const kCGEventRightMouseDown: u32 = 3;
const kCGEventKeyDown: u32 = 10;
const kCGEventFlagsChanged: u32 = 12;
const kCGEventOtherMouseDown: u32 = 25;
const kCGEventTapDisabledByTimeout: u32 = 0xFFFF_FFFE;
const kCGEventTapDisabledByUserInput: u32 = 0xFFFF_FFFF;
const kCGKeyboardEventKeycode: u32 = 9;
const kCGEventSourceUserData: u32 = 42;
const FLAG_CAPS: u64 = 0x0001_0000;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CTRL: u64 = 0x0004_0000;
const FLAG_ALT: u64 = 0x0008_0000;
const FLAG_CMD: u64 = 0x0010_0000;
const LK_MAGIC: i64 = 0x4C4B_5045;

const KC_RETURN: u16 = 36;
const KC_TAB: u16 = 48;
const KC_SPACE: u16 = 49;
const KC_DELETE: u16 = 51;
const KC_KP_ENTER: u16 = 76;
const KC_SHIFT_L: u16 = 56;
const KC_SHIFT_R: u16 = 60;
const KC_F15_PAUSE: u16 = 113; // F15 sits where Pause is on Apple extended keyboards

fn cfstring_to_string(s: CFStringRef) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let mut buf = [0 as c_char; 512];
    unsafe {
        (CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as CFIndex, kCFStringEncodingUTF8) != 0)
            .then(|| CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

fn sel(name: &CStr) -> Sel {
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `[obj sel]` returning an object.
unsafe fn msg0(obj: Id, s: Sel) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, s)
}

unsafe fn msg1(obj: Id, s: Sel, a: Id) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, s, a)
}

fn nsstring(s: &str) -> Id {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    unsafe {
        let cls = objc_getClass(c"NSString".as_ptr());
        let alloc = msg0(cls, sel(c"alloc"));
        let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(alloc, sel(c"initWithUTF8String:"), c.as_ptr())
    }
}

fn is_own_app(id: &str) -> bool {
    let a = id.to_ascii_lowercase();
    a == "app.layoutkeeper" || a.contains("layoutkeeper") || a.contains("layout-keeper")
}

pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[allow(dead_code)]
pub fn prompt_accessibility() -> bool {
    unsafe {
        let dict_cls = objc_getClass(c"NSDictionary".as_ptr());
        if dict_cls.is_null() {
            return AXIsProcessTrusted();
        }
        let f: unsafe extern "C" fn(Id, Sel, Id, Id) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let opts = f(
            dict_cls,
            sel(c"dictionaryWithObject:forKey:"),
            kCFBooleanTrue as Id,
            kAXTrustedCheckOptionPrompt as Id,
        );
        AXIsProcessTrustedWithOptions(opts as CFTypeRef)
    }
}

pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// `setApplicationIconImage` draws the bitmap as-is, so we clip it to a macOS squircle
/// first. The bundle `.icns` gets that mask from Dock automatically; a raw PNG does not.
fn apply_macos_dock_mask(rgba: &mut [u8], w: usize, h: usize) {
    let wf = w as f32;
    let hf = h as f32;
    for y in 0..h {
        for x in 0..w {
            let mut cover = 0.0_f32;
            for sy in 0..2 {
                for sx in 0..2 {
                    let px = x as f32 + (sx as f32 + 0.5) * 0.5;
                    let py = y as f32 + (sy as f32 + 0.5) * 0.5;
                    let nx = px / wf * 2.0 - 1.0;
                    let ny = py / hf * 2.0 - 1.0;
                    if nx.abs().powf(5.0) + ny.abs().powf(5.0) <= 1.0 {
                        cover += 0.25;
                    }
                }
            }
            let i = (y * w + x) * 4;
            rgba[i + 3] = (rgba[i + 3] as f32 * cover).round() as u8;
        }
    }
}

fn png_with_dock_mask(png: &[u8]) -> Option<Vec<u8>> {
    let icon = eframe::icon_data::from_png_bytes(png).ok()?;
    let w = icon.width as usize;
    let h = icon.height as usize;
    let mut rgba = icon.rgba;
    if rgba.len() != w.saturating_mul(h).saturating_mul(4) {
        return None;
    }
    apply_macos_dock_mask(&mut rgba, w, h);
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, icon.width, icon.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
        writer.finish().ok()?;
    }
    Some(out)
}

/// Replace the Dock / app switcher mark with the chosen logo, clipped to a squircle.
pub fn set_dock_icon(png: &[u8]) {
    let masked = png_with_dock_mask(png);
    let bytes = masked.as_deref().unwrap_or(png);
    on_main(|| unsafe {
        let pool = objc_autoreleasePoolPush();
        let data_cls = objc_getClass(c"NSData".as_ptr());
        let data_f: unsafe extern "C" fn(Id, Sel, *const u8, usize) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let data = data_f(
            data_cls,
            sel(c"dataWithBytes:length:"),
            bytes.as_ptr(),
            bytes.len(),
        );
        if !data.is_null() {
            let img_cls = objc_getClass(c"NSImage".as_ptr());
            let img = msg1(msg0(img_cls, sel(c"alloc")), sel(c"initWithData:"), data);
            if !img.is_null() {
                let app = msg0(objc_getClass(c"NSApplication".as_ptr()), sel(c"sharedApplication"));
                if !app.is_null() {
                    let _ = msg1(app, sel(c"setApplicationIconImage:"), img);
                }
            }
        }
        objc_autoreleasePoolPop(pool);
    });
}

unsafe extern "C" fn on_dock_reopen(_this: Id, _cmd: Sel, _app: Id, _has_visible: u8) -> u8 {
    crate::platform::request_show_gui();
    crate::tray::wake_gui();
    1
}

/// Dock click / `open` of a running Typed: show the hidden window instead of doing nothing.
pub fn install_dock_reopen_handler() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.load(Ordering::SeqCst) {
        return;
    }
    unsafe {
        let cls = objc_getClass(c"WinitApplicationDelegate".as_ptr());
        if cls.is_null() {
            return;
        }
        let sel_r = sel(c"applicationShouldHandleReopen:hasVisibleWindows:");
        if !class_getInstanceMethod(cls, sel_r).is_null() {
            DONE.store(true, Ordering::SeqCst);
            return;
        }
        if class_addMethod(
            cls,
            sel_r,
            on_dock_reopen as *mut c_void,
            c"B@:@B".as_ptr(),
        ) != 0
        {
            DONE.store(true, Ordering::SeqCst);
        }
    }
}

fn nsdata_to_vec(data: Id) -> Option<Vec<u8>> {
    if data.is_null() {
        return None;
    }
    unsafe {
        let len_f: unsafe extern "C" fn(Id, Sel) -> usize =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let len = len_f(data, sel(c"length"));
        let ptr = msg0(data, sel(c"bytes")) as *const u8;
        if ptr.is_null() || len == 0 {
            return None;
        }
        Some(std::slice::from_raw_parts(ptr, len).to_vec())
    }
}

/// App icon RGBA for a bundle identifier (`com.google.chrome`, ...).
pub fn app_icon_rgba(bundle_id: &str) -> Option<eframe::egui::ColorImage> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = (|| {
            let cls = objc_getClass(c"NSWorkspace".as_ptr());
            if cls.is_null() {
                return None;
            }
            let ws = msg0(cls, sel(c"sharedWorkspace"));
            if ws.is_null() {
                return None;
            }
            let path = msg1(ws, sel(c"absolutePathForAppBundleWithIdentifier:"), nsstring(bundle_id));
            if path.is_null() {
                return None;
            }
            let len_f: unsafe extern "C" fn(Id, Sel) -> usize =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            if len_f(path, sel(c"length")) == 0 {
                return None;
            }
            let icon = msg1(ws, sel(c"iconForFile:"), path);
            if icon.is_null() {
                return None;
            }
            let tiff = msg0(icon, sel(c"TIFFRepresentation"));
            if tiff.is_null() {
                return None;
            }
            let rep_cls = objc_getClass(c"NSBitmapImageRep".as_ptr());
            let rep = msg1(rep_cls, sel(c"imageRepWithData:"), tiff);
            if rep.is_null() {
                return None;
            }
            let dict_cls = objc_getClass(c"NSDictionary".as_ptr());
            let empty = msg0(dict_cls, sel(c"dictionary"));
            const PNG: usize = 4; // NSBitmapImageFileTypePNG
            let conv: unsafe extern "C" fn(Id, Sel, usize, Id) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let png = conv(rep, sel(c"representationUsingType:properties:"), PNG, empty);
            let bytes = nsdata_to_vec(png)?;
            let decoded = eframe::icon_data::from_png_bytes(&bytes).ok()?;
            Some(eframe::egui::ColorImage::from_rgba_unmultiplied(
                [decoded.width as usize, decoded.height as usize],
                &decoded.rgba,
            ))
        })();
        objc_autoreleasePoolPop(pool);
        result
    }
}

pub fn app_display_name(bundle_id: &str) -> Option<String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = (|| {
            let cls = objc_getClass(c"NSWorkspace".as_ptr());
            if cls.is_null() {
                return None;
            }
            let ws = msg0(cls, sel(c"sharedWorkspace"));
            let path = msg1(ws, sel(c"absolutePathForAppBundleWithIdentifier:"), nsstring(bundle_id));
            if path.is_null() {
                return None;
            }
            let fm_cls = objc_getClass(c"NSFileManager".as_ptr());
            let fm = msg0(fm_cls, sel(c"defaultManager"));
            let name = msg1(fm, sel(c"displayNameAtPath:"), path);
            cfstring_to_string(name as CFStringRef)
        })();
        objc_autoreleasePoolPop(pool);
        result.filter(|n| !n.is_empty() && n != bundle_id)
    }
}

fn frontmost_bundle_id() -> Option<String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let cls = objc_getClass(c"NSWorkspace".as_ptr());
        let mut out = None;
        if !cls.is_null() {
            let ws = msg0(cls, sel(c"sharedWorkspace"));
            let app = if ws.is_null() { std::ptr::null_mut() } else { msg0(ws, sel(c"frontmostApplication")) };
            if !app.is_null() {
                let bid = msg0(app, sel(c"bundleIdentifier"));
                // NSString is toll-free bridged with CFString.
                out = cfstring_to_string(bid as CFStringRef);
            }
        }
        objc_autoreleasePoolPop(pool);
        out
    }
}

/// TIS/HIToolbox asserts it is called on the main dispatch queue (macOS 26+).
/// The GUI focus poller runs off-thread, so hop there when needed.
fn on_main<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    extern "C" {
        fn pthread_main_np() -> i32;
        fn dispatch_sync_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
        static mut _dispatch_main_q: u8;
    }
    if unsafe { pthread_main_np() != 0 } {
        return f();
    }
    struct Job<F, R> {
        func: Option<F>,
        result: Option<R>,
    }
    extern "C" fn run<F, R>(ctx: *mut c_void)
    where
        F: FnOnce() -> R,
    {
        unsafe {
            let job = &mut *(ctx as *mut Job<F, R>);
            let func = job.func.take().expect("on_main job");
            job.result = Some(func());
        }
    }
    let mut job = Job {
        func: Some(f),
        result: None,
    };
    unsafe {
        dispatch_sync_f(
            std::ptr::addr_of_mut!(_dispatch_main_q) as *mut c_void,
            std::ptr::addr_of_mut!(job) as *mut c_void,
            run::<F, R>,
        );
    }
    job.result.expect("on_main result")
}

fn source_lang(src: CFTypeRef) -> Option<Lang> {
    unsafe {
        let langs = TISGetInputSourceProperty(src, kTISPropertyInputSourceLanguages);
        if langs.is_null() || CFArrayGetCount(langs) == 0 {
            return None;
        }
        let first = cfstring_to_string(CFArrayGetValueAtIndex(langs, 0))?;
        Lang::parse(first.split(['-', '_']).next().unwrap_or(""))
    }
}

fn current_lang() -> Option<Lang> {
    on_main(current_lang_on_main)
}

fn current_lang_on_main() -> Option<Lang> {
    unsafe {
        let src = TISCopyCurrentKeyboardInputSource();
        if src.is_null() {
            return None;
        }
        let l = source_lang(src);
        CFRelease(src);
        l
    }
}

fn switch_to(lang: Lang) -> bool {
    on_main(|| switch_to_on_main(lang))
}

fn switch_to_on_main(lang: Lang) -> bool {
    if current_lang_on_main() == Some(lang) {
        return true;
    }
    unsafe {
        let list = TISCreateInputSourceList(std::ptr::null(), false);
        if list.is_null() {
            return false;
        }
        let mut ok = false;
        for i in 0..CFArrayGetCount(list) {
            let src = CFArrayGetValueAtIndex(list, i);
            let cat = TISGetInputSourceProperty(src, kTISPropertyInputSourceCategory);
            let selectable = TISGetInputSourceProperty(src, kTISPropertyInputSourceIsSelectCapable);
            if cat.is_null() || CFEqual(cat, kTISCategoryKeyboardInputSource) == 0 {
                continue;
            }
            if selectable.is_null() || CFEqual(selectable, kCFBooleanTrue) == 0 {
                continue;
            }
            if source_lang(src) == Some(lang) {
                ok = TISSelectInputSource(src) == 0;
                break;
            }
        }
        CFRelease(list);
        if !ok {
            log::warn!("no selectable input source for {lang}; add one in System Settings → Keyboard → Input Sources");
        }
        ok
    }
}

/// US character of an ANSI virtual keycode (kVK_ANSI_*), unshifted.
fn us_base(kc: u16) -> Option<char> {
    const T: &[(u16, char)] = &[
        (0, 'a'), (1, 's'), (2, 'd'), (3, 'f'), (4, 'h'), (5, 'g'), (6, 'z'), (7, 'x'), (8, 'c'), (9, 'v'),
        (11, 'b'), (12, 'q'), (13, 'w'), (14, 'e'), (15, 'r'), (16, 'y'), (17, 't'), (18, '1'), (19, '2'),
        (20, '3'), (21, '4'), (22, '6'), (23, '5'), (24, '='), (25, '9'), (26, '7'), (27, '-'), (28, '8'),
        (29, '0'), (30, ']'), (31, 'o'), (32, 'u'), (33, '['), (34, 'i'), (35, 'p'), (37, 'l'), (38, 'j'),
        (39, '\''), (40, 'k'), (41, ';'), (42, '\\'), (43, ','), (44, '/'), (45, 'n'), (46, 'm'), (47, '.'),
        (50, '`'), (49, ' '),
    ];
    T.iter().find(|p| p.0 == kc).map(|p| p.1)
}

fn us_shifted(c: char) -> char {
    const S: &[(char, char)] = &[
        ('1', '!'), ('2', '@'), ('3', '#'), ('4', '$'), ('5', '%'), ('6', '^'), ('7', '&'), ('8', '*'), ('9', '('),
        ('0', ')'), ('-', '_'), ('=', '+'), ('[', '{'), (']', '}'), (';', ':'), ('\'', '"'), (',', '<'), ('.', '>'),
        ('/', '?'), ('`', '~'), ('\\', '|'),
    ];
    if c.is_ascii_lowercase() {
        return c.to_ascii_uppercase();
    }
    S.iter().find(|p| p.0 == c).map(|p| p.1).unwrap_or(c)
}

struct State {
    ctl: Controller,
    pending: Vec<Command>,
    flags: u64,
    lshift: bool,
    rshift: bool,
    tap: usize,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.lock().ok()?.as_mut().map(f)
}

fn try_with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.try_lock().ok()?.as_mut().map(f)
}

fn launched_from_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().contains("Typed.app"))
        .unwrap_or(false)
}

/// The tap could only be created listen-only: keys cannot be swallowed.
static TAP_LISTEN_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn install_tap(mask: u64) -> CFMachPortRef {
    unsafe {
        let mut tap = CGEventTapCreate(
            kCGSessionEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            mask,
            tap_cb,
            std::ptr::null_mut(),
        );
        if tap.is_null() {
            tap = CGEventTapCreate(
                kCGSessionEventTap,
                kCGHeadInsertEventTap,
                kCGEventTapOptionListenOnly,
                mask,
                tap_cb,
                std::ptr::null_mut(),
            );
            if !tap.is_null() {
                log::warn!("event tap is listen-only; cannot swallow Enter/Tab before send");
                TAP_LISTEN_ONLY.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if tap.is_null() {
            return std::ptr::null_mut();
        }
        let src = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        if src.is_null() {
            return std::ptr::null_mut();
        }
        // A CFRunLoopSource can live on only one loop. The GUI's eframe loop is the main
        // one, so key callbacks must land there — not on this background tick thread.
        CFRunLoopAddSource(CFRunLoopGetMain(), src, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        tap
    }
}

fn kc_to_hotkey_code(kc: u16) -> Option<HotkeyCode> {
    match kc {
        KC_F15_PAUSE => Some(HotkeyCode::Pause),
        KC_SPACE => Some(HotkeyCode::Space),
        KC_TAB => Some(HotkeyCode::Tab),
        KC_RETURN | KC_KP_ENTER => Some(HotkeyCode::Enter),
        0x35 => Some(HotkeyCode::Escape),
        KC_DELETE => Some(HotkeyCode::Backspace),
        _ => match us_base(kc) {
            Some(c) if c.is_ascii_alphabetic() => Some(HotkeyCode::Letter(c.to_ascii_uppercase())),
            Some(c) if c.is_ascii_digit() => Some(HotkeyCode::Digit(c as u8 - b'0')),
            _ => None,
        },
    }
}

fn handle_key(st: &mut State, kc: u16, flags: u64) -> bool {
    if !st.pending.is_empty() {
        st.pending.clear();
        st.ctl.replace_failed();
    }
    let mods = flags & (FLAG_CTRL | FLAG_ALT | FLAG_CMD) != 0;
    if let Some(code) = kc_to_hotkey_code(kc) {
        let ctrl = flags & FLAG_CTRL != 0;
        let alt = flags & FLAG_ALT != 0;
        let shift = flags & FLAG_SHIFT != 0;
        let meta = flags & FLAG_CMD != 0;
        if st.ctl.undo_hotkey().matches(code, ctrl, alt, shift, meta) {
            st.pending.extend(st.ctl.on_input(Input::Undo));
            return true;
        }
        if st.ctl.hotkey().matches_relaxed_shift(code, ctrl, alt, shift, meta) {
            st.pending.extend(st.ctl.on_input(Input::Hotkey));
            return true;
        }
        if st.ctl.summon_hotkey().matches(code, ctrl, alt, shift, meta) {
            st.pending.extend(st.ctl.on_input(Input::Summon));
            return true;
        }
    }
    let input = match kc {
        KC_DELETE => Input::Key { key: Key::Backspace, modifiers: mods },
        KC_RETURN | KC_KP_ENTER => Input::Key { key: Key::Boundary('\n'), modifiers: mods },
        KC_TAB => Input::Key { key: Key::Boundary('\t'), modifiers: mods },
        _ => match us_base(kc) {
            Some(base) => {
                let mut us = if flags & FLAG_SHIFT != 0 { us_shifted(base) } else { base };
                if flags & FLAG_CAPS != 0 && us.is_ascii_alphabetic() {
                    us = if us.is_ascii_uppercase() { us.to_ascii_lowercase() } else { us.to_ascii_uppercase() };
                }
                let c = match st.ctl.current_layout() {
                    Some(Lang::Ru) => layout::en_to_ru(us).unwrap_or(us),
                    _ => us,
                };
                // Option produces special characters on macOS: not word input.
                let key = if flags & FLAG_ALT != 0 && flags & FLAG_CTRL == 0 { Key::Reset } else { Key::Char(c) };
                Input::Key { key, modifiers: mods }
            }
            None => Input::Key { key: Key::Reset, modifiers: mods },
        },
    };
    let mut cmds = st.ctl.on_input(input);
    // A listen-only tap cannot swallow the Backspace that undid an autocorrection:
    // it has already erased one boundary character.
    if kc == KC_DELETE && TAP_LISTEN_ONLY.load(std::sync::atomic::Ordering::Relaxed) {
        for c in cmds.iter_mut() {
            if let Command::Replace(r) = c {
                r.delete = r.delete.saturating_sub(1);
            }
        }
    }
    // Enter/Tab that triggered a replacement, and a Backspace that undid one.
    let swallow = matches!(kc, KC_RETURN | KC_KP_ENTER | KC_TAB | KC_DELETE)
        && cmds.iter().any(|c| matches!(c, Command::Replace(_)));
    st.pending.extend(cmds);
    swallow
}

unsafe extern "C" fn tap_cb(_p: CGEventTapProxy, ty: u32, ev: CGEventRef, _i: *mut c_void) -> CGEventRef {
    let swallow = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        try_with_state(|st| {
            match ty {
                kCGEventTapDisabledByTimeout | kCGEventTapDisabledByUserInput => {
                    CGEventTapEnable(st.tap as CFMachPortRef, true);
                }
                kCGEventLeftMouseDown | kCGEventRightMouseDown | kCGEventOtherMouseDown => {
                    if st.pending.iter().any(|c| matches!(c, Command::Replace(_))) {
                        st.ctl.replace_failed();
                    }
                    st.pending.clear();
                    st.ctl.on_input(Input::Key { key: Key::Reset, modifiers: false });
                }
                kCGEventFlagsChanged | kCGEventKeyDown => {
                    if CGEventGetIntegerValueField(ev, kCGEventSourceUserData) == LK_MAGIC {
                        return false;
                    }
                    let kc = CGEventGetIntegerValueField(ev, kCGKeyboardEventKeycode) as u16;
                    let flags = CGEventGetFlags(ev);
                    st.flags = flags;
                    if ty == kCGEventFlagsChanged {
                        let shift_down = flags & FLAG_SHIFT != 0;
                        match kc {
                            KC_SHIFT_L => {
                                let was = st.lshift;
                                st.lshift = shift_down && !was;
                                if st.lshift && st.rshift {
                                    if st.ctl.hotkey().is_both_shifts() {
                                        let c = st.ctl.on_input(Input::Hotkey);
                                        st.pending.extend(c);
                                    } else if st.ctl.summon_hotkey().is_both_shifts() {
                                        let c = st.ctl.on_input(Input::Summon);
                                        st.pending.extend(c);
                                    }
                                }
                            }
                            KC_SHIFT_R => {
                                let was = st.rshift;
                                st.rshift = shift_down && !was;
                                if st.lshift && st.rshift {
                                    if st.ctl.hotkey().is_both_shifts() {
                                        let c = st.ctl.on_input(Input::Hotkey);
                                        st.pending.extend(c);
                                    } else if st.ctl.summon_hotkey().is_both_shifts() {
                                        let c = st.ctl.on_input(Input::Summon);
                                        st.pending.extend(c);
                                    }
                                }
                            }
                            _ => {}
                        }
                        if !shift_down {
                            st.lshift = false;
                            st.rshift = false;
                        }
                    } else {
                        return handle_key(st, kc, flags);
                    }
                }
                _ => {}
            }
            false
        })
        .unwrap_or(false)
    })).unwrap_or(false);
    if swallow { std::ptr::null_mut() } else { ev }
}

/// Tap `kc` with `flags` held (Cmd+C, Option+Shift+Left), tagged with LK_MAGIC.
unsafe fn post_chord(kc: u16, flags: u64) {
    for down in [true, false] {
        let e = CGEventCreateKeyboardEvent(std::ptr::null(), kc, down);
        if e.is_null() {
            return;
        }
        CGEventSetFlags(e, flags);
        CGEventSetIntegerValueField(e, kCGEventSourceUserData, LK_MAGIC);
        CGEventPost(kCGHIDEventTap, e);
        CFRelease(e as CFTypeRef);
    }
}

struct MacKeys;

impl crate::selection::Keys for MacKeys {
    fn modifiers_held(&self) -> bool {
        extern "C" {
            fn CGEventSourceFlagsState(state: i32) -> u64;
        }
        // kCGEventSourceStateHIDSystemState = 1: the physical keyboard.
        unsafe { CGEventSourceFlagsState(1) & (FLAG_SHIFT | FLAG_CTRL | FLAG_ALT | FLAG_CMD) != 0 }
    }
    fn copy(&self) {
        unsafe { post_chord(8, FLAG_CMD) } // kVK_ANSI_C
    }
    fn paste(&self) {
        unsafe { post_chord(9, FLAG_CMD) } // kVK_ANSI_V
    }
    fn select_word_left(&self) {
        unsafe { post_chord(123, FLAG_ALT | FLAG_SHIFT) } // kVK_LeftArrow
    }
    fn done(&self, target: Lang) {
        if switch_to(target) {
            with_state(|st| st.ctl.replace_succeeded(target));
        }
    }
}

unsafe fn post_key(kc: u16, text: Option<&[u16]>) -> bool {
    for down in [true, false] {
        let e = CGEventCreateKeyboardEvent(std::ptr::null(), kc, down);
        if e.is_null() {
            return false;
        }
        if let Some(t) = text {
            CGEventKeyboardSetUnicodeString(e, t.len(), t.as_ptr());
        }
        CGEventSetFlags(e, 0);
        CGEventSetIntegerValueField(e, kCGEventSourceUserData, LK_MAGIC);
        CGEventPost(kCGHIDEventTap, e);
        CFRelease(e as CFTypeRef);
    }
    true
}

fn replace(r: &Replacement) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("Accessibility permission not granted".into());
    }
    unsafe {
        for _ in 0..r.delete {
            if !post_key(KC_DELETE, None) {
                return Err("CGEventCreateKeyboardEvent failed".into());
            }
        }
        for c in r.insert.chars() {
            let ok = match c {
                '\n' => post_key(KC_RETURN, None),
                '\t' => post_key(KC_TAB, None),
                _ => {
                    let mut u = [0u16; 2];
                    post_key(0, Some(c.encode_utf16(&mut u)))
                }
            };
            if !ok {
                return Err("CGEventCreateKeyboardEvent failed mid-word".into());
            }
        }
    }
    switch_to(r.target);
    Ok(())
}

fn flush() {
    let cmds = with_state(|st| {
        let mut out = Vec::new();
        let mut rest = Vec::new();
        for c in std::mem::take(&mut st.pending) {
            match c {
                Command::SummonGui => out.push(c),
                other => rest.push(other),
            }
        }
        // Shift alone must not block replace; Ctrl/Alt/Cmd still wait (injection).
        if st.flags & (FLAG_CTRL | FLAG_ALT | FLAG_CMD) == 0 {
            out.extend(rest);
        } else {
            st.pending.extend(rest);
        }
        out
    })
    .unwrap_or_default();
    for c in cmds {
        let ok = match &c {
            Command::SwitchLayout(l) => switch_to(*l),
            Command::Replace(r) => match replace(r) {
                Ok(()) => {
                    with_state(|st| st.ctl.replace_succeeded(r.target));
                    true
                }
                Err(e) => { log::warn!("replacement skipped: {e}"); false }
            },
            Command::ConvertSelection => {
                if unsafe { AXIsProcessTrusted() } {
                    crate::selection::spawn(MacKeys);
                }
                true
            }
            Command::SummonGui => {
                // GUI and the key tap share this process. Spawning `layout-keeper-bin gui`
                // opened a second window (and a raw "exec" Dock icon) instead of showing
                // the hidden one.
                crate::platform::request_open_vault();
                crate::tray::wake_gui();
                true
            }
        };
        if !ok {
            with_state(|st| st.ctl.replace_failed());
        }
    }
}

fn hide_standalone_daemon_from_dock() {
    // The key loop now runs inside Typed.app. Hiding that process would hide the GUI too.
    // A no-arg double-click also has no "gui" argv even when main defaults to the window.
    let is_gui = std::env::args().any(|a| a == "gui" || a == "settings") || launched_from_app_bundle();
    if is_gui {
        return;
    }
    unsafe {
        let cls = objc_getClass(c"NSApplication".as_ptr());
        if cls.is_null() {
            return;
        }
        let app = msg0(cls, sel(c"sharedApplication"));
        if app.is_null() {
            return;
        }
        const PROHIBITED: isize = 2; // NSApplicationActivationPolicyProhibited
        let f: unsafe extern "C" fn(Id, Sel, isize) -> u8 =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let _ = f(app, sel(c"setActivationPolicy:"), PROHIBITED);
    }
}

pub fn run(ctl: Controller) -> Result<(), String> {
    hide_standalone_daemon_from_dock();
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("Accessibility permission missing: System Settings → Privacy & Security → Accessibility");
    }
    let mask: u64 = (1 << kCGEventKeyDown)
        | (1 << kCGEventFlagsChanged)
        | (1 << kCGEventLeftMouseDown)
        | (1 << kCGEventRightMouseDown)
        | (1 << kCGEventOtherMouseDown);
    let tap = install_tap(mask);
    let mut keys_ok = !tap.is_null();
    if !keys_ok {
        log::warn!(
            "cannot create event tap — grant Accessibility (System Settings → Privacy & Security → Accessibility).              Running per-app layout memory only."
        );
    }
    {
        let mut g = STATE.lock().map_err(|e| e.to_string())?;
        *g = Some(State {
            ctl,
            pending: vec![],
            flags: 0,
            lshift: false,
            rshift: false,
            tap: tap as usize,
        });
    }
    log::info!("running on macOS (event tap: {})", if keys_ok { "on" } else { "off" });
    let mut last_tick = Instant::now() - Duration::from_secs(1);
    let mut last_tap_retry = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(40));
        flush();
        if !keys_ok && last_tap_retry.elapsed() >= Duration::from_secs(2) {
            last_tap_retry = Instant::now();
            if unsafe { AXIsProcessTrusted() } {
                let new_tap = install_tap(mask);
                if !new_tap.is_null() {
                    with_state(|st| st.tap = new_tap as usize);
                    keys_ok = true;
                    log::info!("event tap enabled after Accessibility grant");
                }
            }
        }
        if last_tick.elapsed() >= Duration::from_millis(120) {
            last_tick = Instant::now();
            let app = frontmost_bundle_id().filter(|id| !is_own_app(id));
            let lang = current_lang();
            let cmds = with_state(|st| {
                let mut cmds = st.ctl.tick(app.as_deref(), lang);
                if st.pending.iter().any(|c| matches!(c, Command::Replace(_))) {
                    cmds.retain(|c| !matches!(c, Command::SwitchLayout(_)));
                }
                cmds
            })
            .unwrap_or_default();
            for c in cmds {
                if let Command::SwitchLayout(l) = c {
                    switch_to(l);
                }
            }
        }
    }
}

pub fn probe() -> (Option<String>, Option<Lang>) {
    on_main(|| (frontmost_bundle_id(), current_lang_on_main()))
}

/// Type `text` into the focused app (vault autofill). Unicode CGEvents tagged with `LK_MAGIC`
/// so the daemon's tap ignores them. Needs the Accessibility permission.
pub fn type_text(text: &str) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("нет разрешения «Универсальный доступ»".into());
    }
    for c in text.chars() {
        let ok = unsafe {
            if c == '\t' {
                post_key(KC_TAB, None)
            } else {
                let mut u = [0u16; 2];
                post_key(0, Some(c.encode_utf16(&mut u)))
            }
        };
        if !ok {
            return Err("CGEventCreateKeyboardEvent failed".into());
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    Ok(())
}


/// Remember Typed.app as a login item so the key loop starts at next user login.
/// Only writes `~/Library/LaunchAgents/app.layoutkeeper.plist`; does not launch
/// another copy of the app in the current session.
pub fn ensure_login_item() {
    let Some(bundle) = app_bundle_path() else {
        return;
    };
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let dir = std::path::PathBuf::from(home).join("Library/LaunchAgents");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("app.layoutkeeper.plist");
    let bundle = bundle.to_string_lossy();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>app.layoutkeeper</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/bin/open</string>
        <string>-j</string>
        <string>-g</string>
        <string>-a</string>
        <string>{bundle}</string>
        <string>--args</string>
        <string>gui</string>
        <string>--hidden</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
"#,
        bundle = xml_escape(&bundle),
    );
    if std::fs::read_to_string(&path).ok().as_deref() != Some(xml.as_str()) {
        let _ = std::fs::write(&path, xml);
    }
}

fn app_bundle_path() -> Option<std::path::PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    loop {
        if p.extension().is_some_and(|e| e == "app") {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
