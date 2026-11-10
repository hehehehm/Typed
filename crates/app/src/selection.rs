#![cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
//! "Convert selection": the convert hotkey with no known last word flips whatever is
//! selected in the focused app, or else the word left of the caret.
//!
//! Runs on its own thread: the copy / paste keystrokes we inject must pass through our
//! own keyboard hook, which would stall if we waited on the hook thread.
//! Only the clipboard is used; nothing leaves the machine. The previous clipboard text is
//! put back afterwards (non-text clipboard contents are not preserved).

use layout_keeper_core::layout;
use layout_keeper_core::Lang;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Keystrokes the platform injects into the focused app.
pub trait Keys: Send + 'static {
    /// True while the user still holds Ctrl / Shift / Alt / Cmd of the hotkey.
    fn modifiers_held(&self) -> bool;
    fn copy(&self);
    fn paste(&self);
    /// Extend the selection one word to the left (Ctrl+Shift+Left / Option+Shift+Left).
    fn select_word_left(&self);
    /// Called with the new layout after a successful paste.
    fn done(&self, target: Lang);
}

static BUSY: AtomicBool = AtomicBool::new(false);

pub fn spawn<K: Keys>(keys: K) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&keys)));
        match r {
            Ok(Err(e)) => log::warn!("convert selection: {e}"),
            Err(_) => log::warn!("convert selection panicked"),
            Ok(Ok(())) => {}
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}

fn wait_modifiers_released(keys: &impl Keys) -> bool {
    let t = Instant::now();
    while keys.modifiers_held() {
        if t.elapsed() > Duration::from_millis(1500) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    true
}

/// Copy and wait for the clipboard to fill (it was cleared first).
fn copy_text(cb: &mut arboard::Clipboard, keys: &impl Keys) -> Option<String> {
    let _ = cb.clear();
    keys.copy();
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(400) {
        std::thread::sleep(Duration::from_millis(20));
        if let Ok(s) = cb.get_text() {
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// Editors such as VS Code copy the whole line when nothing is selected.
fn looks_like_line_copy(s: &str) -> bool {
    s.ends_with('\n') && s.matches('\n').count() == 1
}

fn run(keys: &impl Keys) -> Result<(), String> {
    if !wait_modifiers_released(keys) {
        return Err("hotkey modifiers still held".into());
    }
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let saved = cb.get_text().ok();
    let restore = |cb: &mut arboard::Clipboard| {
        match &saved {
            Some(s) => { let _ = cb.set_text(s.clone()); }
            None => { let _ = cb.clear(); }
        }
    };
    let mut text = copy_text(&mut cb, keys).filter(|s| !looks_like_line_copy(s));
    if text.is_none() {
        keys.select_word_left();
        std::thread::sleep(Duration::from_millis(40));
        text = copy_text(&mut cb, keys).filter(|s| !looks_like_line_copy(s));
    }
    let Some(text) = text else {
        restore(&mut cb);
        return Err("nothing selected".into());
    };
    let Some((out, target)) = layout::convert_text(&text) else {
        restore(&mut cb);
        return Err("selection has no letters".into());
    };
    cb.set_text(out).map_err(|e| { restore(&mut cb); e.to_string() })?;
    std::thread::sleep(Duration::from_millis(30));
    keys.paste();
    // Give the app time to read the clipboard before we put the old text back.
    std::thread::sleep(Duration::from_millis(300));
    restore(&mut cb);
    keys.done(target);
    log::debug!("converted selection of {} chars to {target}", text.chars().count());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_copy_detection() {
        assert!(looks_like_line_copy("let x = 1;\n"));
        assert!(!looks_like_line_copy("ghbdtn"));
        assert!(!looks_like_line_copy("a\nb\n"));
    }
}
