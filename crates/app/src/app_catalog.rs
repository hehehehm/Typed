
use crate::app_id::{basename, has_path, is_hidden_app};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_SUGGEST: usize = 8;
const MIN_QUERY: usize = 2;
const RESCAN: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppEntry {
    pub id: String,
    pub exe: String,
    pub display_name: String,
}

impl AppEntry {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        let id = id.into().trim().to_string();
        let exe = basename(&id).to_string();
        let display = display_name.into().trim().to_string();
        let display_name = if display.is_empty() {
            pretty_stem(&exe)
        } else {
            display
        };
        Self {
            id,
            exe,
            display_name,
        }
    }
}

pub struct AppCatalog {
    entries: Arc<Mutex<Vec<AppEntry>>>,
    rx: Receiver<Vec<AppEntry>>,
    tx_req: Sender<ScanReq>,
    frozen: bool,
    last_scan: Option<Instant>,
    started: bool,
}

enum ScanReq {
    Run,
}

impl Default for AppCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl AppCatalog {
    pub fn new() -> Self {
        let (tx_req, rx_req) = mpsc::channel();
        let (tx_res, rx) = mpsc::channel();
        let _ = thread::Builder::new()
            .name("typed-apps".into())
            .spawn(move || {
                while let Ok(ScanReq::Run) = rx_req.recv() {
                    let list = scan_installed();
                    if tx_res.send(list).is_err() {
                        break;
                    }
                }
            });
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            rx,
            tx_req,
            frozen: false,
            last_scan: None,
            started: false,
        }
    }

    #[cfg(test)]
    pub fn inject(&mut self, entries: Vec<AppEntry>) {
        self.frozen = true;
        self.started = true;
        self.last_scan = Some(Instant::now());
        if let Ok(mut g) = self.entries.lock() {
            *g = entries;
        }
    }

    pub fn ensure_scan(&mut self) {
        if self.frozen {
            return;
        }
        let stale = self
            .last_scan
            .map(|t| t.elapsed() >= RESCAN)
            .unwrap_or(true);
        if self.started && !stale {
            return;
        }
        if self.tx_req.send(ScanReq::Run).is_ok() {
            self.started = true;
            self.last_scan = Some(Instant::now());
        }
    }

    pub fn poll(&mut self, ctx: &eframe::egui::Context) {
        let mut got = false;
        while let Ok(list) = self.rx.try_recv() {
            if let Ok(mut g) = self.entries.lock() {
                *g = list;
            }
            got = true;
        }
        if got {
            ctx.request_repaint();
        }
    }

    pub fn snapshot(&self) -> Vec<AppEntry> {
        self.entries.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn resolve_path(&self, id: &str) -> Option<String> {
        if has_path(id) {
            return Some(id.to_string());
        }
        let exe = basename(id);
        if exe.is_empty() {
            return None;
        }
        let list = self.snapshot();
        list.into_iter()
            .filter(|e| e.exe.eq_ignore_ascii_case(exe) && has_path(&e.id))
            .map(|e| e.id)
            .next()
    }

    pub fn suggest(&self, query: &str, known: &[AppEntry]) -> Vec<AppEntry> {
        suggestions(query, &self.snapshot(), known)
    }
}

pub fn suggest_for(
    query: &str,
    catalog: &[AppEntry],
    known: &[AppEntry],
    limit: usize,
) -> Vec<(AppEntry, i32)> {
    let q = query.trim();
    if q.chars().count() < MIN_QUERY {
        return Vec::new();
    }
    let mut all: Vec<AppEntry> = catalog.to_vec();
    for k in known {
        merge_entry(&mut all, k.clone());
    }
    let mut scored: Vec<(AppEntry, i32)> = all
        .into_iter()
        .filter(|e| !is_hidden_app(&e.id) && !is_hidden_app(&e.exe))
        .filter_map(|e| score(q, &e).map(|s| (e, s)))
        .collect();
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            a.0.display_name
                .to_lowercase()
                .cmp(&b.0.display_name.to_lowercase())
        })
    });
    scored.dedup_by(|a, b| a.0.exe.eq_ignore_ascii_case(&b.0.exe));
    scored.truncate(limit.max(1).min(MAX_SUGGEST));
    scored
}

pub fn suggestions(query: &str, catalog: &[AppEntry], known: &[AppEntry]) -> Vec<AppEntry> {
    suggest_for(query, catalog, known, MAX_SUGGEST)
        .into_iter()
        .map(|(e, _)| e)
        .collect()
}

fn pretty_stem(exe: &str) -> String {
    let t = exe.trim();
    let stem = t
        .strip_suffix(".exe")
        .or_else(|| t.strip_suffix(".EXE"))
        .or_else(|| t.strip_suffix(".app"))
        .or_else(|| t.strip_suffix(".APP"))
        .unwrap_or(t);
    stem.to_string()
}

fn score(query: &str, e: &AppEntry) -> Option<i32> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return None;
    }
    let name = e.display_name.to_lowercase();
    let exe = e.exe.to_lowercase();
    let stem = pretty_stem(&exe).to_lowercase();
    let path = e.id.to_lowercase();

    if stem == q || name == q || exe == q {
        return Some(1000);
    }
    if stem.starts_with(&q) {
        return Some(920);
    }
    if exe.starts_with(&q) {
        return Some(900);
    }
    if name.starts_with(&q) {
        return Some(870);
    }
    if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|t| !t.is_empty() && t.starts_with(&q))
    {
        return Some(800);
    }
    if exe.contains(&q) || stem.contains(&q) {
        return Some(700);
    }
    if name.contains(&q) {
        return Some(650);
    }
    if path.contains(&q) && has_path(&e.id) {
        return Some(500);
    }
    if is_subseq(&q, &name) || is_subseq(&q, &stem) {
        return Some(400);
    }
    None
}

fn is_subseq(q: &str, s: &str) -> bool {
    let mut it = s.chars();
    for c in q.chars() {
        if !it.any(|x| x == c) {
            return false;
        }
    }
    true
}

fn merge_entry(into: &mut Vec<AppEntry>, e: AppEntry) {
    if e.id.is_empty() && e.exe.is_empty() {
        return;
    }
    let exe = if e.exe.is_empty() {
        basename(&e.id).to_string()
    } else {
        e.exe.clone()
    };
    if exe.is_empty() {
        return;
    }
    if let Some(old) = into.iter_mut().find(|o| o.exe.eq_ignore_ascii_case(&exe)) {
        if has_path(&e.id) && !has_path(&old.id) {
            old.id = e.id;
        }
        if display_better(&e.display_name, &old.display_name, &exe) {
            old.display_name = e.display_name;
        }
        return;
    }
    into.push(e);
}

fn display_better(new: &str, old: &str, exe: &str) -> bool {
    let pretty = pretty_stem(exe);
    let old_weak =
        old.is_empty() || old.eq_ignore_ascii_case(exe) || old.eq_ignore_ascii_case(&pretty);
    let new_weak =
        new.is_empty() || new.eq_ignore_ascii_case(exe) || new.eq_ignore_ascii_case(&pretty);
    (old_weak && !new_weak) || (!new_weak && new.len() > old.len())
}

fn scan_installed() -> Vec<AppEntry> {
    let mut out = Vec::new();
    #[cfg(target_os = "windows")]
    win::collect(&mut out);
    #[cfg(target_os = "macos")]
    mac::collect(&mut out);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    linux::collect(&mut out);
    out.retain(|e| !e.exe.is_empty() && !is_hidden_app(&e.id) && !is_hidden_app(&e.exe));
    out.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    out
}

pub fn parse_lnk_path(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 0x4c {
        return None;
    }
    let header_size = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    if header_size != 0x4c {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[0x14..0x18].try_into().ok()?);
    const HAS_ID_LIST: u32 = 1;
    const HAS_LINK_INFO: u32 = 2;
    let mut off = 0x4cusize;
    if flags & HAS_ID_LIST != 0 {
        if off + 2 > bytes.len() {
            return None;
        }
        let sz = u16::from_le_bytes(bytes[off..off + 2].try_into().ok()?) as usize;
        off = off.checked_add(2)?.checked_add(sz)?;
    }
    if flags & HAS_LINK_INFO == 0 {
        return None;
    }
    if off + 0x10 > bytes.len() {
        return None;
    }
    let info_size = u32::from_le_bytes(bytes[off..off + 4].try_into().ok()?) as usize;
    let header = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().ok()?) as usize;
    let info_flags = u32::from_le_bytes(bytes[off + 8..off + 12].try_into().ok()?);
    const VOLUME_AND_LOCAL: u32 = 1;
    if info_flags & VOLUME_AND_LOCAL == 0 {
        return None;
    }
    if off + 16 > bytes.len() {
        return None;
    }
    let local_off = u32::from_le_bytes(bytes[off + 16..off + 20].try_into().ok()?) as usize;
    let end = off.checked_add(info_size)?.min(bytes.len());
    if header >= 0x24 && off + 0x24 <= bytes.len() {
        let uni_off = u32::from_le_bytes(bytes[off + 28..off + 32].try_into().ok()?) as usize;
        if uni_off > 0 {
            if let Some(s) = read_widez(&bytes[off..end], uni_off) {
                if looks_like_exe(&s) || has_path(&s) {
                    return Some(s);
                }
            }
        }
    }
    let s = read_asz(&bytes[off..end], local_off)?;
    if looks_like_exe(&s) || has_path(&s) {
        Some(s)
    } else {
        None
    }
}

fn read_asz(block: &[u8], off: usize) -> Option<String> {
    if off >= block.len() {
        return None;
    }
    let slice = &block[off..];
    let n = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    let s = String::from_utf8_lossy(&slice[..n]).to_string();
    (!s.is_empty()).then_some(s)
}

fn read_widez(block: &[u8], off: usize) -> Option<String> {
    if off + 2 > block.len() {
        return None;
    }
    let mut u = Vec::new();
    let mut i = off;
    while i + 1 < block.len() {
        let c = u16::from_le_bytes(block[i..i + 2].try_into().ok()?);
        i += 2;
        if c == 0 {
            break;
        }
        u.push(c);
    }
    let s = String::from_utf16_lossy(&u);
    (!s.is_empty()).then_some(s)
}

fn looks_like_exe(s: &str) -> bool {
    let n = basename(s);
    let l = n.to_lowercase();
    l.ends_with(".exe") || l.ends_with(".app") || l.ends_with(".com")
}

fn push_exe(out: &mut Vec<AppEntry>, id: String, display: String) {
    if id.trim().is_empty() {
        return;
    }
    let exe = basename(&id);
    if exe.is_empty() || is_hidden_app(&id) || is_hidden_app(exe) {
        return;
    }
    let low = exe.to_lowercase();
    if low == "typed.exe" || low == "typed" || low == "layout-keeper.exe" {
        return;
    }
    if !looks_like_exe(&id) && has_path(&id) {
        return;
    }
    merge_entry(out, AppEntry::new(id, display));
}

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::{fs, ptr};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, MAX_PATH};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
        HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    pub fn collect(out: &mut Vec<AppEntry>) {
        unsafe {
            use windows_sys::Win32::System::Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
            };
            let hr = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
            running(out);
            start_menu(out);
            app_paths(out);
            uninstall(out);
            if hr == 0 {
                CoUninitialize();
            }
        }
    }

    fn running(out: &mut Vec<AppEntry>) {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap == INVALID_HANDLE_VALUE {
                return;
            }
            let mut pe: PROCESSENTRY32W = std::mem::zeroed();
            pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut pe) != 0 {
                loop {
                    if let Some(path) = process_path(pe.th32ProcessID) {
                        push_exe(out, path, String::new());
                    }
                    if Process32NextW(snap, &mut pe) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
    }

    fn process_path(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 || len == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    fn start_menu(out: &mut Vec<AppEntry>) {
        let mut roots = Vec::new();
        if let Ok(pd) = std::env::var("ProgramData") {
            roots.push(PathBuf::from(pd).join(r"Microsoft\Windows\Start Menu\Programs"));
        } else {
            roots.push(PathBuf::from(
                r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs",
            ));
        }
        if let Ok(ad) = std::env::var("AppData") {
            roots.push(PathBuf::from(ad).join(r"Microsoft\Windows\Start Menu\Programs"));
        }
        let mut lnks = Vec::new();
        for r in &roots {
            walk_lnks(r, &mut lnks, 8);
        }
        for p in lnks {
            let display = p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if display.is_empty() || looks_like_uninstaller(&display) {
                continue;
            }
            let target = resolve_lnk(&p);
            if let Some(t) = target {
                push_exe(out, t, display);
            }
        }
    }

    fn walk_lnks(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
        if depth == 0 || out.len() >= 4000 {
            return;
        }
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let ft = e.file_type().ok();
            if ft.map(|t| t.is_dir()).unwrap_or(false) {
                walk_lnks(&p, out, depth - 1);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("lnk"))
            {
                out.push(p);
            }
        }
    }

    fn resolve_lnk(path: &Path) -> Option<String> {
        if let Ok(bytes) = fs::read(path) {
            if let Some(t) = parse_lnk_path(&bytes) {
                return Some(t);
            }
        }
        com_resolve_lnk(path)
    }

    fn looks_like_uninstaller(name: &str) -> bool {
        let l = name.to_lowercase();
        l.contains("uninstall") || l.contains("удал") || l.starts_with("unins")
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..n])
    }

    fn app_paths(out: &mut Vec<AppEntry>) {
        const SUB: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths";
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            enum_app_paths(root, SUB, out);
        }
    }

    fn enum_app_paths(root: HKEY, sub: &str, out: &mut Vec<AppEntry>) {
        unsafe {
            let mut key: HKEY = ptr::null_mut();
            if RegOpenKeyExW(root, wide(sub).as_ptr(), 0, KEY_READ, &mut key) != 0 {
                return;
            }
            let mut i = 0u32;
            loop {
                let mut name = [0u16; 256];
                let mut nlen = name.len() as u32;
                let st = RegEnumKeyExW(
                    key,
                    i,
                    name.as_mut_ptr(),
                    &mut nlen,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                i += 1;
                if st != 0 {
                    break;
                }
                let exe_name = from_wide(&name);
                if exe_name.is_empty() {
                    continue;
                }
                let mut child: HKEY = ptr::null_mut();
                if RegOpenKeyExW(key, name.as_ptr(), 0, KEY_READ, &mut child) != 0 {
                    continue;
                }
                if let Some(path) = query_sz(child, "") {
                    push_exe(out, path, pretty_stem(&exe_name));
                }
                RegCloseKey(child);
            }
            RegCloseKey(key);
        }
    }

    fn uninstall(out: &mut Vec<AppEntry>) {
        const SUBS: &[&str] = &[
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ];
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            for sub in SUBS {
                enum_uninstall(root, sub, out);
            }
        }
    }

    fn enum_uninstall(root: HKEY, sub: &str, out: &mut Vec<AppEntry>) {
        unsafe {
            let mut key: HKEY = ptr::null_mut();
            if RegOpenKeyExW(root, wide(sub).as_ptr(), 0, KEY_READ, &mut key) != 0 {
                return;
            }
            let mut i = 0u32;
            loop {
                let mut name = [0u16; 256];
                let mut nlen = name.len() as u32;
                let st = RegEnumKeyExW(
                    key,
                    i,
                    name.as_mut_ptr(),
                    &mut nlen,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                i += 1;
                if st != 0 {
                    break;
                }
                let mut child: HKEY = ptr::null_mut();
                if RegOpenKeyExW(key, name.as_ptr(), 0, KEY_READ, &mut child) != 0 {
                    continue;
                }
                let display = query_sz(child, "DisplayName").unwrap_or_default();
                if let Some(icon) = query_sz(child, "DisplayIcon") {
                    if let Some(exe) = icon_to_exe(&icon) {
                        push_exe(out, exe, display);
                    }
                }
                RegCloseKey(child);
            }
            RegCloseKey(key);
        }
    }

    fn query_sz(key: HKEY, name: &str) -> Option<String> {
        unsafe {
            let name_w = wide(name);
            let name_p = if name.is_empty() {
                ptr::null()
            } else {
                name_w.as_ptr()
            };
            let mut ty = 0u32;
            let mut len = 0u32;
            if RegQueryValueExW(
                key,
                name_p,
                ptr::null_mut(),
                &mut ty,
                ptr::null_mut(),
                &mut len,
            ) != 0
                || len == 0
            {
                return None;
            }
            if ty != REG_SZ && ty != REG_EXPAND_SZ {
                return None;
            }
            let mut buf = vec![0u16; (len as usize / 2).saturating_add(1)];
            let mut len2 = (buf.len() * 2) as u32;
            if RegQueryValueExW(
                key,
                name_p,
                ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr() as *mut u8,
                &mut len2,
            ) != 0
            {
                return None;
            }
            let s = from_wide(&buf);
            let s = if ty == REG_EXPAND_SZ {
                expand_env(&s)
            } else {
                s
            };
            (!s.is_empty()).then_some(s)
        }
    }

    fn expand_env(s: &str) -> String {
        let src = wide(s);
        let mut buf = [0u16; 1024];
        unsafe {
            let n = ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32);
            if n > 1 && (n as usize) <= buf.len() {
                return from_wide(&buf);
            }
        }
        s.to_string()
    }

    fn icon_to_exe(icon: &str) -> Option<String> {
        let t = icon.trim().trim_matches('"');
        let t = t.split(',').next().unwrap_or(t).trim().trim_matches('"');
        if looks_like_exe(t) {
            Some(t.to_string())
        } else {
            None
        }
    }

    fn com_resolve_lnk(path: &Path) -> Option<String> {
        unsafe { com_resolve_lnk_inner(path) }
    }

    unsafe fn com_resolve_lnk_inner(path: &Path) -> Option<String> {
        use windows_sys::core::GUID;
        use windows_sys::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, STGM_READ};

        const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
        const IID_ISHELL_LINK_W: GUID = GUID::from_u128(0x000214f9_0000_0000_c000_000000000046);
        const IID_IPERSIST_FILE: GUID = GUID::from_u128(0x0000010b_0000_0000_c000_000000000046);

        #[repr(C)]
        #[allow(non_snake_case)]
        struct IUnknownVtbl {
            query_interface: unsafe extern "system" fn(
                *mut IUnknown,
                *const GUID,
                *mut *mut core::ffi::c_void,
            ) -> i32,
            add_ref: unsafe extern "system" fn(*mut IUnknown) -> u32,
            release: unsafe extern "system" fn(*mut IUnknown) -> u32,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct IUnknown {
            lpVtbl: *const IUnknownVtbl,
        }

        #[repr(C)]
        struct IPersistFileVtbl {
            query_interface: unsafe extern "system" fn(
                *mut IUnknown,
                *const GUID,
                *mut *mut core::ffi::c_void,
            ) -> i32,
            add_ref: unsafe extern "system" fn(*mut IUnknown) -> u32,
            release: unsafe extern "system" fn(*mut IUnknown) -> u32,
            get_class_id: unsafe extern "system" fn(*mut IUnknown, *mut GUID) -> i32,
            is_dirty: unsafe extern "system" fn(*mut IUnknown) -> i32,
            load: unsafe extern "system" fn(*mut IUnknown, *const u16, u32) -> i32,
            save: unsafe extern "system" fn(*mut IUnknown, *const u16, i32) -> i32,
            save_completed: unsafe extern "system" fn(*mut IUnknown, *const u16) -> i32,
            get_cur_file: unsafe extern "system" fn(*mut IUnknown, *mut *mut u16) -> i32,
        }

        #[repr(C)]
        struct IShellLinkWVtbl {
            query_interface: unsafe extern "system" fn(
                *mut IUnknown,
                *const GUID,
                *mut *mut core::ffi::c_void,
            ) -> i32,
            add_ref: unsafe extern "system" fn(*mut IUnknown) -> u32,
            release: unsafe extern "system" fn(*mut IUnknown) -> u32,
            get_path: unsafe extern "system" fn(
                *mut IUnknown,
                *mut u16,
                i32,
                *mut core::ffi::c_void,
                u32,
            ) -> i32,
        }

        let mut link: *mut IUnknown = ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_SHELL_LINK,
            ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_ISHELL_LINK_W,
            &mut link as *mut _ as *mut *mut core::ffi::c_void,
        );
        if hr < 0 || link.is_null() {
            return None;
        }
        let mut persist: *mut IUnknown = ptr::null_mut();
        let qi = ((*(*link).lpVtbl).query_interface)(
            link,
            &IID_IPERSIST_FILE,
            &mut persist as *mut _ as *mut *mut core::ffi::c_void,
        );
        if qi < 0 || persist.is_null() {
            ((*(*link).lpVtbl).release)(link);
            return None;
        }
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let persist_vt = (*persist).lpVtbl as *const IPersistFileVtbl;
        let loaded = ((*persist_vt).load)(persist, wide.as_ptr(), STGM_READ);
        let mut buf = [0u16; MAX_PATH as usize];
        let mut ok = None;
        if loaded >= 0 {
            let sl = (*link).lpVtbl as *const IShellLinkWVtbl;
            let r = ((*sl).get_path)(link, buf.as_mut_ptr(), buf.len() as i32, ptr::null_mut(), 0);
            if r >= 0 {
                let s = from_wide(&buf);
                if !s.is_empty() {
                    ok = Some(s);
                }
            }
        }
        ((*(*persist).lpVtbl).release)(persist);
        ((*(*link).lpVtbl).release)(link);
        ok
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::fs;

    pub fn collect(out: &mut Vec<AppEntry>) {
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        for root in roots {
            let Ok(rd) = fs::read_dir(&root) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name.starts_with('.') {
                    continue;
                }
                if p.extension().and_then(|s| s.to_str()) == Some("app")
                    || p.is_dir() && name.ends_with(".app")
                {
                    let display = name.trim_end_matches(".app").to_string();
                    push_exe(out, p.to_string_lossy().into_owned(), display);
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod linux {
    use super::*;
    use std::fs;

    pub fn collect(out: &mut Vec<AppEntry>) {
        running(out);
        desktops(out);
    }

    fn running(out: &mut Vec<AppEntry>) {
        let Ok(rd) = fs::read_dir("/proc") else {
            return;
        };
        for e in rd.flatten() {
            let name = e.file_name();
            if !name
                .to_str()
                .is_some_and(|s| s.chars().all(|c| c.is_ascii_digit()))
            {
                continue;
            }
            let link = e.path().join("exe");
            if let Ok(p) = fs::read_link(&link) {
                push_exe(out, p.to_string_lossy().into_owned(), String::new());
            }
        }
    }

    fn desktops(out: &mut Vec<AppEntry>) {
        let mut dirs = vec![
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join(".local/share/applications"));
        }
        for d in dirs {
            let Ok(rd) = fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("desktop") {
                    continue;
                }
                if let Some((id, display)) = parse_desktop(&p) {
                    push_exe(out, id, display);
                }
            }
        }
    }

    fn parse_desktop(path: &Path) -> Option<(String, String)> {
        let s = fs::read_to_string(path).ok()?;
        let mut name = None;
        let mut exec = None;
        let mut in_desktop = false;
        for line in s.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_desktop = line.eq_ignore_ascii_case("[desktop entry]");
                continue;
            }
            if !in_desktop {
                continue;
            }
            if let Some(v) = line.strip_prefix("Name=") {
                if name.is_none() {
                    name = Some(v.trim().to_string());
                }
            } else if let Some(v) = line.strip_prefix("Exec=") {
                exec = Some(v.trim().to_string());
            }
        }
        let exec = exec?;
        let bin = exec
            .split_whitespace()
            .find(|t| !t.starts_with('%') && !t.starts_with('-'))
            .unwrap_or(&exec)
            .trim_matches('"');
        if bin.is_empty() {
            return None;
        }
        Some((bin.to_string(), name.unwrap_or_default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, name: &str) -> AppEntry {
        AppEntry::new(id, name)
    }

    #[test]
    fn counter_finds_cs2_by_display_name() {
        let cat = [
            e(
                r"C:\Program Files\Adobe\Adobe Photoshop 2025\Photoshop.exe",
                "Adobe Photoshop 2025",
            ),
            e(
                r"C:\Program Files (x86)\Steam\steamapps\common\Counter-Strike Global Offensive\cs2.exe",
                "Counter-Strike 2",
            ),
            e(
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                "Google Chrome",
            ),
        ];
        let hits = suggestions("counter", &cat, &[]);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].exe.eq_ignore_ascii_case("cs2.exe"));
        assert!(hits[0].display_name.contains("Counter-Strike"));
    }

    #[test]
    fn photoshop_matches_exe_and_display() {
        let cat = [
            e(
                r"C:\Program Files\Adobe\Adobe Photoshop 2025\Photoshop.exe",
                "Adobe Photoshop 2025",
            ),
            e(
                r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
                "Windows PowerShell",
            ),
        ];
        let hits = suggestions("photoshop", &cat, &[]);
        assert_eq!(hits[0].exe, "Photoshop.exe");
        let hits = suggestions("photo", &cat, &[]);
        assert_eq!(hits[0].exe, "Photoshop.exe");
    }

    #[test]
    fn known_apps_are_merged_and_short_query_is_ignored() {
        let cat = [e(r"C:\a\chrome.exe", "Google Chrome")];
        let known = [e("code.exe", "Visual Studio Code")];
        assert!(suggestions("c", &cat, &known).is_empty());
        let hits = suggestions("code", &cat, &known);
        assert_eq!(hits[0].exe, "code.exe");
    }

    #[test]
    fn resolve_path_prefers_full_path() {
        let mut c = AppCatalog::new();
        c.inject(vec![e(
            r"C:\Users\me\AppData\Local\Telegram Desktop\Telegram.exe",
            "Telegram",
        )]);
        assert_eq!(
            c.resolve_path("telegram.exe").as_deref(),
            Some(r"C:\Users\me\AppData\Local\Telegram Desktop\Telegram.exe")
        );
        assert_eq!(c.resolve_path("unknown.exe"), None);
        assert_eq!(
            c.resolve_path(r"C:\already\full\path.exe").as_deref(),
            Some(r"C:\already\full\path.exe")
        );
    }

    #[test]
    fn parse_minimal_lnk_local_path() {
        let path = br"C:\Games\cs2.exe";
        let mut bytes = vec![0u8; 0x4c];
        bytes[0..4].copy_from_slice(&0x4cu32.to_le_bytes());
        bytes[0x14..0x18].copy_from_slice(&2u32.to_le_bytes());
        let mut info = vec![0u8; 0x1c + path.len() + 1];
        let size = info.len() as u32;
        info[0..4].copy_from_slice(&size.to_le_bytes());
        info[4..8].copy_from_slice(&0x1cu32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        info[16..20].copy_from_slice(&0x1cu32.to_le_bytes());
        info[0x1c..0x1c + path.len()].copy_from_slice(path);
        bytes.extend_from_slice(&info);
        assert_eq!(parse_lnk_path(&bytes).as_deref(), Some(r"C:\Games\cs2.exe"));
    }
}
