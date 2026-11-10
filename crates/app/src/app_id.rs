//! App-id helpers: short names and filters for the remembered-apps list.
//!
//! System shell/hosts and one-shot installers are hidden from the main list
//! (they stay reachable behind a click-to-reveal toggle). Installers are also
//! skipped by auto-learn so they do not accumulate.

/// File name of an app id / path: `C:\a\b\powershell.exe` → `powershell.exe`, `firefox` → `firefox`.
pub fn basename(id: &str) -> &str {
    let t = id.trim_end_matches(['\\', '/']);
    match t.rfind(['\\', '/']) {
        Some(i) if i + 1 < t.len() => &t[i + 1..],
        _ if t.is_empty() => id,
        _ => t,
    }
}

/// True when the id looks like a full path (so a "show path" button makes sense).
pub fn has_path(id: &str) -> bool {
    basename(id) != id
}

fn slash_path(id: &str) -> String {
    id.to_lowercase().replace('/', "\\")
}

fn is_temp_path(id: &str) -> bool {
    let p = slash_path(id);
    p.contains("\\temp\\") || p.contains("\\tmp\\") || p.contains("\\windows\\installer\\")
}

fn is_windows_system_path(id: &str) -> bool {
    let p = slash_path(id);
    p.contains("\\windows\\systemapps\\")
        || p.contains("\\windows\\immersivecontrolpanel\\")
        || p.contains("\\windows\\servicing\\")
}

/// One-shot installers, unpackers and files that live in a temp folder.
/// These should not stay in the remembered-apps list.
pub fn is_ephemeral_app(id: &str) -> bool {
    if is_temp_path(id) {
        return true;
    }
    let name = basename(id).to_lowercase();
    if name.ends_with(".tmp") || name.starts_with("unins") {
        return true;
    }
    match name.as_str() {
        "msiexec.exe" | "trustedinstaller.exe" | "tiworker.exe" | "setuphost.exe"
        | "setupprep.exe" | "installagent.exe" | "dxsetup.exe" => return true,
        _ => {}
    }
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name.as_str());
    stem.contains("setup")
        || stem.contains("installer")
        || stem.contains("install_")
        || stem.contains("vcredist")
        || stem.contains("vc_redist")
}

/// Shell / OS UI entries are tucked away in the remembered-apps list only.
/// Match names, never directories: user terminals in System32 stay visible.
pub fn is_system_app(id: &str) -> bool {
    if is_windows_system_path(id) {
        return true;
    }
    let name = basename(id).to_lowercase();
    matches!(
        name.as_str(),
        "explorer.exe"
            | "searchhost.exe"
            | "searchapp.exe"
            | "searchui.exe"
            | "searchprotocolhost.exe"
            | "searchfilterhost.exe"
            | "searchindexer.exe"
            | "startmenuexperiencehost.exe"
            | "shellexperiencehost.exe"
            | "textinputhost.exe"
            | "tabtip.exe"
            | "tabtip32.exe"
            | "applicationframehost.exe"
            | "systemsettings.exe"
            | "systemsettingsbroker.exe"
            | "systemsettingsadminflows.exe"
            | "runtimebroker.exe"
            | "sihost.exe"
            | "dwm.exe"
            | "lockapp.exe"
            | "logonui.exe"
            | "ctfmon.exe"
            | "widgets.exe"
            | "widgetservice.exe"
            | "widgetboard.exe"
            | "phoneexperiencehost.exe"
            | "rundll32.exe"
            | "consent.exe"
            | "windowsinternal.composableshell.experiences.textinput.inputapp.exe"
            | "backgroundtaskhost.exe"
            | "dllhost.exe"
            | "svchost.exe"
            | "taskhostw.exe"
            | "taskhost.exe"
            | "conhost.exe"
            | "openconsole.exe"
            | "csrss.exe"
            | "winlogon.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "smss.exe"
            | "fontdrvhost.exe"
            | "wudfhost.exe"
            | "unsecapp.exe"
            | "wmiprvse.exe"
            | "pickerhost.exe"
            | "openwith.exe"
            | "launchwinapp.exe"
            | "useroobebroker.exe"
            | "useraccountbroker.exe"
            | "dataexchangehost.exe"
            | "shellhost.exe"
            | "werfault.exe"
            | "werfaultsecure.exe"
            | "wermgr.exe"
            | "msiexec.exe"
            | "trustedinstaller.exe"
            | "tiworker.exe"
            | "usoclient.exe"
            | "mousocoreworker.exe"
            | "compattelrunner.exe"
            | "setuphost.exe"
            | "setupprep.exe"
            | "securityhealthsystray.exe"
            | "securityhealthservice.exe"
            | "securityhealthhost.exe"
            | "msmpeng.exe"
            | "nissrv.exe"
            | "smartscreen.exe"
            | "control.exe"
            | "screenclippinghost.exe"
            | "gamebarpresencewriter.exe"
            | "crossdeviceresume.exe"
            | "webexperiencehost.exe"
            | "webexperiencehostapp.exe"
            | "shellappruntime.exe"
            | "atbroker.exe"
            | "utilman.exe"
            | "osk.exe"
            | "filepicker.exe"
            | "appvshnotify.exe"
            | "splwow64.exe"
            | "com.apple.finder"
            | "com.apple.dock"
            | "com.apple.systemuiserver"
            | "com.apple.controlcenter"
            | "finder"
            | "dock"
            | "systemuiserver"
            | "controlcenter"
            | "gnome-shell"
            | "plasmashell"
            | "xfce4-panel"
    )
}

/// Hidden from the main remembered-apps list (still visible behind a toggle).
pub fn is_hidden_app(id: &str) -> bool {
    is_system_app(id) || is_ephemeral_app(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_apps_match_names_not_directories() {
        for id in [
            "explorer.exe",
            r"C:\Windows\explorer.exe",
            r"C:\Windows\EXPLORER.EXE",
            "conhost.exe",
            r"C:\Windows\System32\conhost.exe",
            "SearchHost.exe",
            "msiexec.exe",
            "RuntimeBroker.exe",
            r"C:\Windows\SystemApps\Microsoft.Windows.Search_cw5n1h2txyewy\SearchApp.exe",
            "com.apple.Finder",
            "com.apple.Dock",
            "com.apple.SystemUIServer",
            "com.apple.controlcenter",
            "/usr/bin/gnome-shell",
            "plasmashell",
            "xfce4-panel",
            "windowsinternal.composableshell.experiences.textinput.inputapp.exe",
        ] {
            assert!(is_system_app(id), "{id}");
        }
        for id in [
            "powershell.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            r"C:\Windows\System32\cmd.exe",
            "WindowsTerminal.exe",
            "firefox",
            "MobaXterm.exe",
            "notepad.exe",
            "SnippingTool.exe",
            "mstsc.exe",
            "com.apple.Safari",
            "/home/explorer.exe/firefox",
            "myexplorer.exe",
        ] {
            assert!(!is_system_app(id), "{id}");
        }
    }

    #[test]
    fn ephemeral_installers_and_temp_files() {
        for id in [
            "typed-setup-0.3.15.tmp",
            r"C:\Users\heheh\AppData\Local\Temp\typed-setup-0.3.15.tmp",
            "layoutkeeper-setup-0.3.5.tmp",
            "layoutkeeper-setup-0.3.8.tmp",
            "innosetup-6.7.1.tmp",
            "unins.tmp",
            "unins000.exe",
            r"C:\Users\x\AppData\Local\Temp\is-ABC12.tmp\setup.exe",
            r"C:\Windows\Installer\foo.exe",
            "Something-Installer.exe",
            "VC_redist.x64.exe",
            "msiexec.exe",
        ] {
            assert!(is_ephemeral_app(id), "{id}");
            assert!(is_hidden_app(id), "{id}");
        }
        for id in [
            "firefox.exe",
            "discord.exe",
            "compil32.exe",
            "openvpn-gui.exe",
            "notepad.exe",
            "WindowsTerminal.exe",
            "amneziawg.exe",
        ] {
            assert!(!is_ephemeral_app(id), "{id}");
            assert!(!is_hidden_app(id), "{id}");
        }
    }

    #[test]
    fn basenames() {
        assert_eq!(
            basename(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            "powershell.exe"
        );
        assert_eq!(basename("firefox"), "firefox");
        assert_eq!(basename("/usr/bin/code"), "code");
        assert_eq!(basename("com.apple.Safari"), "com.apple.Safari");
        assert!(has_path(r"c:\a\b.exe"));
        assert!(!has_path("telegram"));
    }
}
