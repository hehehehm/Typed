//! Local 30-day tester window, counted from the moment this binary was compiled.
//! This is not a secure license: anyone with the sources or a hex editor can
//! remove the check. It stops a casual tester from keeping one build forever
//! and notices an obvious system-clock rollback.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TRIAL_DAYS: u64 = 30;
const TRIAL_SECS: u64 = TRIAL_DAYS * 24 * 60 * 60;
/// Allow a bit of NTP / timezone slop before treating a backwards clock as tampering.
const SKEW_SECS: u64 = 12 * 60 * 60;

const EXPIRED_RU: &str =
    "Пробный период Typed закончился. Эта сборка работала 30 дней с даты компиляции.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trial {
    Ok,
    Expired,
}

#[allow(dead_code)]
pub fn check() -> Result<(), String> {
    let now = unix_now().ok_or_else(|| EXPIRED_RU.to_string())?;
    let build = build_unix();
    let path = stamp_path();
    let last = read_stamp(&path);
    match evaluate(now, build, last) {
        Trial::Ok => {
            let _ = write_stamp(&path, now.max(last.unwrap_or(0)));
            Ok(())
        }
        Trial::Expired => Err(EXPIRED_RU.to_string()),
    }
}

#[allow(dead_code)]
pub fn notify(msg: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
        let text: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = "Typed\0".encode_utf16().collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "display dialog \"{escaped}\" with title \"Typed\" buttons {{\"OK\"}} default button 1"
        );
        let _ = std::process::Command::new("osascript").args(["-e", &script]).status();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = msg;
    }
}

fn build_unix() -> u64 {
    env!("TYPED_BUILD_UNIX").parse().unwrap_or(0)
}

fn unix_now() -> Option<u64> {
    SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

fn stamp_path() -> PathBuf {
    layout_keeper_core::Config::default_path().with_file_name("typed.chk")
}

pub fn evaluate(now: u64, build: u64, last: Option<u64>) -> Trial {
    if build == 0 || now < build {
        return Trial::Expired;
    }
    if now.saturating_sub(build) > TRIAL_SECS {
        return Trial::Expired;
    }
    if let Some(last) = last {
        if now + SKEW_SECS < last {
            return Trial::Expired;
        }
        if last.saturating_sub(build) > TRIAL_SECS {
            return Trial::Expired;
        }
    }
    Trial::Ok
}

fn xor_key(build: u64) -> [u8; 8] {
    let b = build.to_le_bytes();
    let mut key = [0x5A, 0x3C, 0x9E, 0x17, 0xC2, 0x88, 0x4B, 0xD1];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte ^= b[i];
    }
    key
}

fn encode_stamp(value: u64) -> [u8; 8] {
    let key = xor_key(build_unix());
    let raw = value.to_le_bytes();
    let mut out = [0u8; 8];
    for i in 0..8 {
        out[i] = raw[i] ^ key[i];
    }
    out
}

fn decode_stamp(bytes: &[u8]) -> Option<u64> {
    if bytes.len() != 8 {
        return None;
    }
    let key = xor_key(build_unix());
    let mut raw = [0u8; 8];
    for i in 0..8 {
        raw[i] = bytes[i] ^ key[i];
    }
    Some(u64::from_le_bytes(raw))
}

fn read_stamp(path: &Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    decode_stamp(&bytes)
}

fn write_stamp(path: &Path, value: u64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, encode_stamp(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD: u64 = 1_700_000_000;

    #[test]
    fn within_window_ok() {
        assert_eq!(evaluate(BUILD, BUILD, None), Trial::Ok);
        assert_eq!(evaluate(BUILD + TRIAL_SECS, BUILD, Some(BUILD)), Trial::Ok);
        assert_eq!(evaluate(BUILD + 10 * 24 * 60 * 60, BUILD, Some(BUILD + 100)), Trial::Ok);
    }

    #[test]
    fn after_thirty_days_expired() {
        assert_eq!(evaluate(BUILD + TRIAL_SECS + 1, BUILD, None), Trial::Expired);
    }

    #[test]
    fn before_build_expired() {
        assert_eq!(evaluate(BUILD - 1, BUILD, None), Trial::Expired);
        assert_eq!(evaluate(BUILD, 0, None), Trial::Expired);
    }

    #[test]
    fn clock_rollback_expired() {
        let last = BUILD + 20 * 24 * 60 * 60;
        assert_eq!(evaluate(last - SKEW_SECS - 1, BUILD, Some(last)), Trial::Expired);
    }

    #[test]
    fn small_clock_skew_ok() {
        let last = BUILD + 20 * 24 * 60 * 60;
        assert_eq!(evaluate(last - SKEW_SECS + 60, BUILD, Some(last)), Trial::Ok);
    }

    #[test]
    fn last_seen_past_window_stays_expired() {
        let last = BUILD + TRIAL_SECS + 60;
        assert_eq!(evaluate(BUILD + 5, BUILD, Some(last)), Trial::Expired);
    }

    #[test]
    fn stamp_roundtrip() {
        let path = std::env::temp_dir().join(format!("typed-chk-{}", std::process::id()));
        write_stamp(&path, 1_734_000_000).unwrap();
        assert_eq!(read_stamp(&path), Some(1_734_000_000));
        std::fs::remove_file(path).unwrap();
    }
}
