
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::ptr;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_CANCELLED, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, OpenProcess, OpenProcessToken, WaitForSingleObject,
    INFINITE, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub const TASK_NAME: &str = "Typed";

pub fn is_typed_exe_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("typed.exe") || name.eq_ignore_ascii_case("layout-keeper.exe")
}
pub const FROM_TASK_ARG: &str = "--from-task";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn token_elevated(process: HANDLE) -> Result<bool, u32> {
    unsafe {
        let mut token: HANDLE = ptr::null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return Err(GetLastError());
        }
        let mut el = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut el as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let err = GetLastError();
        CloseHandle(token);
        if ok == 0 {
            return Err(err);
        }
        Ok(el.TokenIsElevated != 0)
    }
}

pub fn is_elevated() -> bool {
    token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}

pub fn other_instance_elevated() -> Option<bool> {
    let me = unsafe { GetCurrentProcessId() };
    let mut any = None;
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry = std::mem::zeroed::<PROCESSENTRY32W>();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if is_typed_exe_name(&name) && entry.th32ProcessID != me {
                    let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                    let el = if h.is_null() {
                        GetLastError() == ERROR_ACCESS_DENIED
                    } else {
                        let r = token_elevated(h);
                        CloseHandle(h);
                        match r {
                            Ok(e) => e,
                            Err(e) => e == ERROR_ACCESS_DENIED,
                        }
                    };
                    any = Some(any.unwrap_or(false) || el);
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    any
}

fn schtasks(args: &[&str]) -> Result<(), String> {
    let out = Command::new("schtasks.exe")
        .args(args)
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("schtasks: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if msg.is_empty() { format!("schtasks {}: код {:?}", args.first().unwrap_or(&""), out.status.code()) } else { msg })
    }
}

pub fn task_exists() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME]).is_ok()
}

pub fn task_command() -> Option<String> {
    let out = Command::new("schtasks.exe")
        .args(["/Query", "/TN", TASK_NAME, "/XML"])
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    command_from_xml(&decode(&out.stdout))
}

fn decode(b: &[u8]) -> String {
    let zeros = b.iter().filter(|&&c| c == 0).count();
    if b.len() >= 2 && zeros * 3 > b.len() {
        let units: Vec<u16> = b.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string()
    } else {
        String::from_utf8_lossy(b).into_owned()
    }
}

fn command_from_xml(xml: &str) -> Option<String> {
    let start = xml.find("<Command>")? + "<Command>".len();
    let end = start + xml[start..].find("</Command>")?;
    let c = xml[start..end].trim().trim_matches('"');
    Some(c.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\""))
}

pub fn task_points_here() -> bool {
    let (Some(cmd), Ok(exe)) = (task_command(), std::env::current_exe()) else { return false };
    cmd.eq_ignore_ascii_case(&exe.display().to_string())
}

pub fn run_task() -> Result<(), String> {
    schtasks(&["/Run", "/TN", TASK_NAME])
}

pub fn end_task() {
    let _ = schtasks(&["/End", "/TN", TASK_NAME]);
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn current_user() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    match std::env::var("USERDOMAIN") {
        Ok(d) if !d.is_empty() => format!("{d}\\{user}"),
        _ => user,
    }
}

fn task_xml(exe: &str, dir: &str, user: &str) -> String {
    let (exe, dir, user) = (xml_escape(exe), xml_escape(dir), xml_escape(user));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Typed: keyboard layout per app, started with administrator rights at sign-in.</Description></RegistrationInfo>
  <Triggers>
    <LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId><Delay>PT3S</Delay></LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec><Command>{exe}</Command><Arguments>run {FROM_TASK_ARG}</Arguments><WorkingDirectory>{dir}</WorkingDirectory></Exec>
  </Actions>
</Task>
"#
    )
}

pub fn create_task(user: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().map(|p| p.display().to_string()).unwrap_or_default();
    let xml = task_xml(&exe.display().to_string(), &dir, user);
    let path = std::env::temp_dir().join(format!("typed-task-{}.xml", std::process::id()));
    let mut bytes = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let p = path.display().to_string();
    let r = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &p, "/F"]);
    let _ = std::fs::remove_file(&path);
    r
}

pub fn delete_task() -> Result<(), String> {
    if !task_exists() {
        return Ok(());
    }
    schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])
}

pub fn hand_over_to_task(args: &[String]) -> bool {
    if args.iter().any(|a| a == FROM_TASK_ARG) || is_elevated() || !task_exists() {
        return false;
    }
    if !task_points_here() {
        log::warn!("run_as_admin: the «{TASK_NAME}» task starts another program; running without administrator rights");
        return false;
    }
    match run_task() {
        Ok(()) => {
            log::info!("run_as_admin: started the «{TASK_NAME}» task, this normal instance exits");
            true
        }
        Err(e) => {
            log::warn!("run_as_admin: cannot start the task ({e}); running without administrator rights");
            false
        }
    }
}

pub fn helper_main(args: &[String]) -> Result<(), String> {
    let on = match args.get(1).map(String::as_str) {
        Some("on") => true,
        Some("off") => false,
        Some("needs-refresh") => {
            return if task_exists() && !task_points_here() { Ok(()) } else { Err("the task is up to date or absent".into()) };
        }
        Some("refresh") => {
            if !task_exists() {
                return Ok(());
            }
            let user = args.iter().position(|a| a == "--user").and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(current_user);
            create_task(&user)?;
            return run_task();
        }
        _ => return Err("usage: typed admin-task on|off|refresh|needs-refresh [--user DOMAIN\\name] [--keep <pid>]".into()),
    };
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if on {
        let user = opt("--user").unwrap_or_else(current_user);
        let keep: Vec<u32> = opt("--keep").and_then(|p| p.parse().ok()).into_iter().collect();
        create_task(&user)?;
        super::windows::stop_daemon_except(&keep);
        run_task()
    } else {
        delete_task()
    }
}

pub fn run_helper_elevated(on: bool) -> Result<(), String> {
    if is_elevated() {
        let args: Vec<String> = ["admin-task", if on { "on" } else { "off" }, "--user", &current_user(), "--keep", &std::process::id().to_string()]
            .iter()
            .map(|s| s.to_string())
            .collect();
        return helper_main(&args);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let params = format!(
        "admin-task {} --user \"{}\" --keep {}",
        if on { "on" } else { "off" },
        current_user().replace('"', ""),
        std::process::id()
    );
    let (verb, file, params_w) = (wide("runas"), wide(&exe.display().to_string()), wide(&params));
    let dir = exe.parent().map(|p| wide(&p.display().to_string()));
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params_w.as_ptr();
        info.lpDirectory = dir.as_ref().map_or(ptr::null(), |d| d.as_ptr());
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            let e = GetLastError();
            return Err(if e == ERROR_CANCELLED { "cancelled".into() } else { format!("ShellExecuteEx: {e}") });
        }
        if info.hProcess.is_null() {
            return Err("helper did not start".into());
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        if code == 0 {
            Ok(())
        } else {
            Err(format!("код {code}"))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn task_xml_escapes_and_never_times_out() {
        let x = super::task_xml(r"C:\A&B\typed.exe", r"C:\A&B", r"PC\Даша");
        assert!(x.contains(r"C:\A&amp;B\typed.exe"));
        assert!(x.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(x.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(x.contains("run --from-task"));
        assert_eq!(super::command_from_xml(&x).as_deref(), Some(r"C:\A&B\typed.exe"));
        let utf16: Vec<u8> = "\u{feff}<Command>\"C:\\x\\typed.exe\"</Command>".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(super::command_from_xml(&super::decode(&utf16)).as_deref(), Some(r"C:\x\typed.exe"));
    }
}
