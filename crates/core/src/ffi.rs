//! Minimal C ABI so a future Android IME (Kotlin/JNI) or iOS keyboard extension (Swift)
//! can reuse the decision logic. Pure functions only — no state, no I/O.

use crate::engine::{Decision, Engine, EngineOptions};
use crate::layout::{self, Lang};
use std::ffi::{c_char, CStr, CString};

unsafe fn to_str<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p).to_str().ok()
}

fn into_c(s: String) -> *mut c_char {
    CString::new(s).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

/// Re-interpret `text` as typed in the other layout (script auto-detected, EN if unclear).
/// Returns a newly allocated string; free it with [`lk_free`].
///
/// # Safety
/// `text` must be NULL or a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn lk_convert_layout(text: *const c_char) -> *mut c_char {
    let Some(t) = to_str(text) else { return std::ptr::null_mut() };
    let from = layout::detect_script(t).unwrap_or(Lang::En);
    into_c(layout::convert(t, from))
}

/// Autocorrect decision for a finished word: corrected text, or NULL to keep it.
///
/// # Safety
/// `word` must be NULL or a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn lk_autocorrect_word(word: *const c_char) -> *mut c_char {
    let Some(w) = to_str(word) else { return std::ptr::null_mut() };
    match Engine::new(EngineOptions::default()).decide(w) {
        Decision::Convert { text, .. } => into_c(text),
        Decision::Keep(_) => std::ptr::null_mut(),
    }
}

/// Free a string returned by this library.
///
/// # Safety
/// `s` must be NULL or a pointer previously returned by this library.
#[no_mangle]
pub unsafe extern "C" fn lk_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let input = CString::new("зуфсу").unwrap();
        unsafe {
            let out = lk_autocorrect_word(input.as_ptr());
            assert_eq!(CStr::from_ptr(out).to_str().unwrap(), "peace");
            lk_free(out);
            let out = lk_convert_layout(input.as_ptr());
            assert_eq!(CStr::from_ptr(out).to_str().unwrap(), "peace");
            lk_free(out);
            let keep = CString::new("peace").unwrap();
            assert!(lk_autocorrect_word(keep.as_ptr()).is_null());
        }
    }
}
