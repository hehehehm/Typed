#![cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]

use layout_keeper_core::layout;
use layout_keeper_core::Lang;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub trait Keys: Send + 'static {
    fn modifiers_held(&self) -> bool;
    fn copy(&self);
    fn paste(&self);
    fn select_word_left(&self);
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
