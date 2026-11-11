//! Typed (type daemon) desktop daemon + CLI.
//!
//! ```text
//! layout-keeper [run]                 start the background daemon (default)
//! layout-keeper gui                 settings window (edits config.json) + encrypted vault tab
//! layout-keeper vault status          whether vault.bin exists (never prints secrets)
//! layout-keeper status                config path, settings, focused app and layout now
//! layout-keeper set-app <id> <en|ru>  remember a layout for an app
//! layout-keeper remove-app <id>       forget an app
//! layout-keeper list-apps             show remembered apps
//! layout-keeper convert <text…>       print text re-typed in the other layout (offline helper)
//! layout-keeper check <word>          show the autocorrect decision for a word
//! layout-keeper config-path           print the config file path
//! ```
//! Logging: `RUST_LOG=debug layout-keeper` (typed text itself is never logged).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app_id;
mod brand;
mod controller;
mod gui;
mod icons;
mod logos;
mod platform;
mod selection;
mod tray;
mod trial;
mod vault_ui;

use controller::Controller;
use layout_keeper_core::{layout, Config, Engine, Hotkey, Lang};
use std::process::ExitCode;

fn usage() -> &'static str {
    "Typed — per-app keyboard layout memory and wrong-layout autocorrect (local only)

USAGE:
  layout-keeper [run]                 start the daemon (default)
  layout-keeper gui                   open the settings window (run the daemon separately)
  layout-keeper vault status          is there an encrypted vault file (manage it in `gui` → Сейф)
  layout-keeper status                show config path, settings, focused app/layout
  layout-keeper set-app <app-id> <en|ru>
  layout-keeper remove-app <app-id>
  layout-keeper list-apps
  layout-keeper set <autocorrect|auto-learn> <on|off>
  layout-keeper set hotkey <Combo>            e.g. Pause, ShiftShift, Ctrl+Shift+P
  layout-keeper set summon-hotkey <Combo>     e.g. Ctrl+Shift+V (opens settings on the vault tab)
  layout-keeper set undo-hotkey <Combo>       e.g. Ctrl+Alt+Z (undo the last autocorrection, never fix that word again)
  layout-keeper set undo-on-backspace on|off  Backspace right after an autocorrection undoes it
  layout-keeper convert <text...>     re-type text in the other layout (e.g. ghbdtn -> привет)
  layout-keeper check <word>          explain the autocorrect decision for a word
  layout-keeper config-path

ENV: LAYOUT_KEEPER_CONFIG=<file> overrides the config path; RUST_LOG=debug for verbose logs."
}

fn load() -> Result<(Config, std::path::PathBuf), String> {
    let path = Config::default_path();
    let cfg = Config::load_or_create(&path).map_err(|e| format!("cannot load config {}: {e}", path.display()))?;
    Ok((cfg, path))
}

fn on_off(s: &str) -> Result<bool, String> {
    match s.to_ascii_lowercase().as_str() {
        "on" | "true" | "1" | "yes" => Ok(true),
        "off" | "false" | "0" | "no" => Ok(false),
        _ => Err(format!("expected on/off, got '{s}'")),
    }
}

fn default_cmd() -> &'static str {
    // Double-clicking Typed.app launches layout-keeper-bin with no argv. That must
    // open the window, not a headless daemon (which would also hide itself from the Dock).
    #[cfg(target_os = "macos")]
    {
        if std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().contains("Typed.app"))
            .unwrap_or(false)
        {
            return "gui";
        }
    }
    "run"
}

fn real_main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start_hidden = args.iter().any(|a| a == "--hidden");
    let cmd = match args.first().map(|s| s.as_str()) {
        Some("--hidden") | Some("--vault") => default_cmd(),
        Some(c) => c,
        None => default_cmd(),
    };
    match cmd {
        "run" => {
            let (cfg, path) = load()?;
            eprintln!("Typed {} — config: {}", env!("CARGO_PKG_VERSION"), path.display());
            log::info!(
                "autocorrect={} auto_learn={} hotkey={} summon={} remembered_apps={}",
                cfg.autocorrect, cfg.auto_learn, cfg.hotkey, cfg.summon_hotkey, cfg.apps.len()
            );
            platform::run(Controller::new(cfg, Some(path)))
        }
        "gui" | "settings" => {
            let open_vault = args.iter().any(|a| a == "--vault" || a == "vault");
            gui::run(open_vault, start_hidden)
        }
        "vault" => match args.get(1).map(String::as_str) {
            Some("status") | None => {
                let p = layout_keeper_core::vault::default_path();
                let present = p.exists();
                println!("vault file:   {} ({})", p.display(), if present { "present" } else { "absent" });
                println!("encryption:   Argon2id + XChaCha20-Poly1305; unlock and manage it in `layout-keeper gui` → Сейф");
                Ok(())
            }
            Some(other) => Err(format!("unknown vault subcommand '{other}' (only `vault status`; secrets are never taken on the command line)")),
        },
        "status" => {
            let (cfg, path) = load()?;
            println!("config:       {}", path.display());
            println!("autocorrect:  {}", cfg.autocorrect);
            println!("auto-learn:   {}", cfg.auto_learn);
            println!("hotkey:       {}", cfg.hotkey);
            println!("summon:       {}", cfg.summon_hotkey);
            println!("undo:         {} (backspace undo: {}), never-correct words: {}", cfg.undo_hotkey, cfg.undo_on_backspace, cfg.never_correct.len());
            println!("apps:         {} remembered, {} exceptions", cfg.apps.len(), cfg.exception_apps.len());
            let (app, lang) = platform::probe();
            println!("focused app:  {}", app.as_deref().unwrap_or("(unknown)"));
            println!("layout now:   {}", lang.map(|l| l.code()).unwrap_or("(unknown)"));
            Ok(())
        }
        "set-app" => {
            let (id, l) = match (args.get(1), args.get(2)) {
                (Some(i), Some(l)) => (i, l),
                _ => return Err("usage: layout-keeper set-app <app-id> <en|ru>".into()),
            };
            let lang = Lang::parse(l).ok_or_else(|| format!("unknown layout '{l}' (en|ru)"))?;
            let (mut cfg, path) = load()?;
            cfg.set_layout(id, lang);
            cfg.save(&path).map_err(|e| e.to_string())?;
            println!("{id} -> {lang}");
            Ok(())
        }
        "remove-app" => {
            let id = args.get(1).ok_or("usage: layout-keeper remove-app <app-id>")?;
            let (mut cfg, path) = load()?;
            if cfg.remove_app(id) {
                cfg.save(&path).map_err(|e| e.to_string())?;
                println!("removed {id}");
            } else {
                println!("{id} was not remembered");
            }
            Ok(())
        }
        "list-apps" => {
            let (cfg, _) = load()?;
            if cfg.apps.is_empty() {
                println!("(no apps remembered yet)");
            }
            for (app, l) in &cfg.apps {
                println!("{l}  {app}");
            }
            Ok(())
        }
        "set" => {
            let (key, val) = match (args.get(1), args.get(2)) {
                (Some(k), Some(v)) => (k.as_str(), v.as_str()),
                _ => return Err("usage: layout-keeper set <autocorrect|auto-learn|hotkey|summon-hotkey|undo-hotkey|undo-on-backspace|fix-missing-spaces> <value>".into()),
            };
            let (mut cfg, path) = load()?;
            match key {
                "autocorrect" => cfg.autocorrect = on_off(val)?,
                "auto-learn" | "auto_learn" => cfg.auto_learn = on_off(val)?,
                "hotkey" => cfg.hotkey = val.parse::<Hotkey>()?,
                "summon-hotkey" | "summon_hotkey" => cfg.summon_hotkey = val.parse::<Hotkey>()?,
                "undo-hotkey" | "undo_hotkey" => cfg.undo_hotkey = val.parse::<Hotkey>()?,
                "undo-on-backspace" | "undo_on_backspace" => cfg.undo_on_backspace = on_off(val)?,
                "fix-missing-spaces" | "fix_missing_spaces" => cfg.fix_missing_spaces = on_off(val)?,
                _ => return Err(format!("unknown setting '{key}'")),
            }
            cfg.save(&path).map_err(|e| e.to_string())?;
            println!("{key} = {val}");
            Ok(())
        }
        "convert" => {
            let text = args[1..].join(" ");
            let from = layout::detect_script(&text).unwrap_or(Lang::En);
            println!("{}", layout::convert(&text, from));
            Ok(())
        }
        "check" => {
            let w = args.get(1).ok_or("usage: layout-keeper check <word>")?;
            let (cfg, _) = load()?;
            println!("{:?}", Engine::new(cfg.engine_options()).decide(w));
            Ok(())
        }
        "config-path" => {
            println!("{}", Config::default_path().display());
            Ok(())
        }
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        "-V" | "--version" => {
            println!("layout-keeper {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{}", usage())),
    }
}

#[cfg(windows)]
fn prepare_windows_console() {
    use std::os::windows::io::IntoRawHandle;
    use windows_sys::Win32::System::Console::{
        AttachConsole, FreeConsole, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    let gui = matches!(
        std::env::args().nth(1).as_deref(),
        Some("gui") | Some("settings")
    );
    if gui {
        // Start Menu / Explorer must not leave an empty Windows Terminal tab.
        unsafe {
            FreeConsole();
        }
        return;
    }
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            // Daemon launched from Explorer, VBS, or the settings window: stay silent.
            return;
        }
    }
    if let Ok(out) = std::fs::OpenOptions::new().read(true).write(true).open("CONOUT$") {
        let handle = out.into_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
            SetStdHandle(STD_ERROR_HANDLE, handle);
        }
    }
    if let Ok(inp) = std::fs::OpenOptions::new().read(true).write(true).open("CONIN$") {
        let handle = inp.into_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
        unsafe {
            SetStdHandle(STD_INPUT_HANDLE, handle);
        }
    }
}

fn main() -> ExitCode {
    #[cfg(windows)]
    prepare_windows_console();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
