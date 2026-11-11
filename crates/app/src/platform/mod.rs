//! OS adapters. Each exposes `run(controller)` (blocking event loop) and
//! `probe() -> (Option<app id>, Option<Lang>)` for `layout-keeper status`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{probe, type_text};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub mod win_admin;
#[cfg(target_os = "windows")]
pub use windows::{acquire_gui_mutex, ensure_daemon_running, probe, raise_settings_window, run, stop_daemon, type_text, wake_settings_window_async};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{
    accessibility_trusted, app_display_name, app_icon_rgba, ensure_login_item,
    install_dock_reopen_handler, open_accessibility_settings, probe, set_dock_icon, type_text,
};

#[cfg(not(target_os = "macos"))]
pub fn accessibility_trusted() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn prompt_accessibility() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() {}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon(_png: &[u8]) {}

#[cfg(not(target_os = "macos"))]
pub fn install_dock_reopen_handler() {}

#[cfg(not(target_os = "macos"))]
pub fn ensure_login_item() {}

#[cfg(not(target_os = "macos"))]
pub fn app_display_name(_id: &str) -> Option<String> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod unsupported {
    use crate::controller::Controller;
    use layout_keeper_core::Lang;
    pub fn run(_c: Controller) -> Result<(), String> {
        Err("this OS is not supported by the desktop daemon (see README / mobile/README.md)".into())
    }
    pub fn probe() -> (Option<String>, Option<Lang>) {
        (None, None)
    }
    pub fn type_text(_t: &str) -> Result<(), String> {
        Err("unsupported OS".into())
    }
}
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub use unsupported::{probe, run, type_text};

/// Flag file next to config.json: an already-running GUI consumes it and opens the vault tab.
fn vault_request_path() -> std::path::PathBuf {
    layout_keeper_core::Config::default_path().with_file_name("open-vault")
}

pub fn request_open_vault() {
    let p = vault_request_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&p, b"") {
        log::warn!("cannot request vault tab: {e}");
    }
}

pub fn take_open_vault_request() -> bool {
    let p = vault_request_path();
    if p.exists() {
        let _ = std::fs::remove_file(&p);
        true
    } else {
        false
    }
}

fn show_gui_request_path() -> std::path::PathBuf {
    layout_keeper_core::Config::default_path().with_file_name("show-gui")
}

/// Ask an already-running settings window to come out of the tray.
#[allow(dead_code)]
pub fn request_show_gui() {
    let p = show_gui_request_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&p, b"") {
        log::warn!("cannot request settings window: {e}");
    }
}

pub fn take_show_gui_request() -> bool {
    let p = show_gui_request_path();
    if p.exists() {
        let _ = std::fs::remove_file(&p);
        true
    } else {
        false
    }
}

pub fn gui_wake_pending() -> bool {
    vault_request_path().exists() || show_gui_request_path().exists()
}

#[cfg(target_os = "macos")]
pub fn acquire_gui_lock() -> Result<Option<std::fs::File>, String> {
    let path = layout_keeper_core::Config::default_path().with_file_name("gui.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    daemon_lock::acquire_at(&path)
}

/// Start `layout-keeper gui` from the same executable the daemon was launched from.
#[allow(dead_code)]
pub fn spawn_settings_gui() {
    spawn_settings_gui_inner(false);
}

/// Start settings already showing the vault tab (summon hotkey).
#[allow(dead_code)]
pub fn spawn_settings_gui_vault() {
    spawn_settings_gui_inner(true);
}

fn spawn_settings_gui_inner(vault: bool) {
    match std::env::current_exe() {
        Ok(exe) => {
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("gui");
            if vault {
                cmd.arg("--vault");
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            if let Err(e) = cmd.spawn() {
                log::warn!("cannot start settings window: {e}");
            }
        }
        Err(e) => log::warn!("cannot find own executable: {e}"),
    }
}

/// Spawn exactly the no-argument daemon used by target/release, never the GUI.
/// Shared by Linux and Windows; macOS starts the tap inside Typed.app instead.
#[cfg(not(target_os = "macos"))]
fn spawn_daemon() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(&exe);
    if let Some(parent) = exe.parent() { cmd.current_dir(parent); }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // Reap it if it ever exits while the settings window is still open.
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}

#[cfg(unix)]
mod daemon_lock {
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::path::Path;

    // Advisory lock is released by the OS on exit/crash. Keep the file itself;
    // unlinking it could let two processes lock different inodes.
    pub fn acquire_at(path: &Path) -> Result<Option<File>, String> {
        let file = OpenOptions::new().read(true).write(true).create(true)
            .truncate(false).open(path).map_err(|e| e.to_string())?;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(Some(file));
        }
        let e = std::io::Error::last_os_error();
        if e.kind() == std::io::ErrorKind::WouldBlock { Ok(None) } else { Err(e.to_string()) }
    }

    pub fn acquire() -> Result<Option<File>, String> {
        let path = layout_keeper_core::Config::default_path().with_file_name("daemon.lock");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        acquire_at(&path)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn daemon_lock_prevents_duplicates_and_releases_on_exit() {
            let path = std::env::temp_dir().join(format!("layout-keeper-lock-{}", std::process::id()));
            let guard = super::acquire_at(&path).unwrap().unwrap();
            assert!(super::acquire_at(&path).unwrap().is_none());
            drop(guard);
            assert!(super::acquire_at(&path).unwrap().is_some());
            std::fs::remove_file(path).unwrap();
        }
    }
}

#[cfg(unix)]
pub fn daemon_is_running() -> Result<bool, String> {
    Ok(daemon_lock::acquire()?.is_none())
}

#[cfg(target_os = "linux")]
pub fn ensure_daemon_running() -> Result<(), String> {
    if !daemon_is_running()? { spawn_daemon()?; }
    Ok(())
}

/// On macOS the keyboard tap must live in the Typed.app process. A separate
/// `layout-keeper-bin` helper does not inherit Accessibility and shows up in
/// the Dock as a bouncing generic icon.
#[cfg(target_os = "macos")]
pub fn ensure_daemon_running() -> Result<(), String> {
    stop_daemon();
    for _ in 0..20 {
        if !daemon_is_running()? {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    if daemon_is_running()? {
        stop_daemon_force();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    std::thread::Builder::new()
        .name("typed-keys".into())
        .spawn(|| {
            let path = layout_keeper_core::Config::default_path();
            match layout_keeper_core::Config::load_or_create(&path) {
                Ok(cfg) => {
                    if let Err(e) = run(crate::controller::Controller::new(cfg, Some(path))) {
                        log::error!("typed key loop: {e}");
                    }
                }
                Err(e) => log::error!("cannot load config for key loop: {e}"),
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(unix)]
fn process_command_line(pid: u32) -> String {
    let output = std::process::Command::new("ps")
        .args(["-ww", "-p", &pid.to_string(), "-o", "command="])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(unix)]
fn is_typed_gui_process(pid: u32) -> bool {
    let cmd = process_command_line(pid);
    cmd.split_whitespace().any(|a| a == "gui" || a == "settings")
}

#[cfg(unix)]
fn stop_daemon_signal(sig: i32) {
    let me = std::process::id();
    for name in ["layout-keeper", "layout-keeper-bin"] {
        let output = std::process::Command::new("pgrep").arg("-x").arg(name).output();
        let Ok(output) = output else {
            continue;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(pid) = line.trim().parse::<u32>() else {
                continue;
            };
            if pid == me || is_typed_gui_process(pid) {
                continue;
            }
            unsafe {
                libc::kill(pid as i32, sig);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn stop_daemon_force() {
    stop_daemon_signal(libc::SIGKILL);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn run(ctl: crate::controller::Controller) -> Result<(), String> {
    let Some(_guard) = daemon_lock::acquire()? else {
        log::info!("Typed daemon already running");
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    { linux::run(ctl) }
    #[cfg(target_os = "macos")]
    { macos::run(ctl) }
}

#[cfg(unix)]
pub fn stop_daemon() {
    stop_daemon_signal(libc::SIGTERM);
}

#[cfg(not(any(unix, windows)))]
pub fn ensure_daemon_running() -> Result<(), String> {
    Err("background daemon is unsupported on this OS".into())
}

#[cfg(not(any(unix, windows)))]
pub fn stop_daemon() {}
