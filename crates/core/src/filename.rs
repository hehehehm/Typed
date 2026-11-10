//! File names typed in the wrong layout: `мфгдеюишт` is `vault.bin` typed with the
//! Russian layout on (`ю` is the `.` key). A token converts when it becomes
//! `name.ext` with a known extension and a name that reads as Latin.
//!
//! The other direction is a guard only: `vault.bin` typed correctly is never turned
//! into Russian.

use crate::dict;
use crate::layout::{self, Lang};
use crate::morph;

/// Common file extensions (lowercase).
pub const EXTENSIONS: &[&str] = &[
    "bin", "txt", "exe", "json", "md", "rs", "py", "js", "ts", "jsx", "tsx", "html", "htm", "css", "scss",
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "icns", "tif", "tiff", "heic", "svg", "webp", "psd", "fig",
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "rtf", "epub", "djvu",
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "dmg", "pkg", "deb", "rpm", "apk", "ipa", "iso", "img",
    "iss", "ps1", "bat", "cmd", "sh", "bash", "zsh", "toml", "lock", "log", "csv", "tsv", "xml", "yaml", "yml",
    "ini", "cfg", "conf", "env", "dll", "so", "dylib", "lib", "msi", "app", "sys", "dat", "db", "sqlite", "sql",
    "mp3", "mp4", "m4a", "wav", "flac", "ogg", "opus", "aac", "mov", "avi", "mkv", "webm", "srt",
    "c", "h", "cpp", "hpp", "cs", "java", "kt", "swift", "go", "rb", "php", "lua", "vue", "dart",
    "ipynb", "plist", "chk", "bak", "tmp", "key", "pem", "crt", "torrent", "ttf", "otf", "woff", "woff2",
];

/// Shortest extension that may trigger a conversion (one-letter ones are only guarded).
const MIN_EXT_CONVERT: usize = 2;

pub fn is_extension(ext: &str) -> bool {
    let l = ext.to_ascii_lowercase();
    EXTENSIONS.contains(&l.as_str())
}

/// Split off up to two trailing signs (`vault.bin,` / `vault.bin.`).
fn split_tail(chars: &[char]) -> (&[char], &[char]) {
    let mut k = 0;
    while k < 2 && k < chars.len() && !chars[chars.len() - 1 - k].is_alphanumeric() {
        k += 1;
    }
    chars.split_at(chars.len() - k)
}

/// Whether a Latin name part looks like a real file name. Returns `(plausible, has_word)`.
fn latin_name(name: &str) -> (bool, bool) {
    if name.is_empty()
        || !name.chars().any(|c| c.is_ascii_alphabetic())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        || name.starts_with(['.', '-', '_'])
    {
        return (false, false);
    }
    let en = dict::get(Lang::En);
    let mut has_word = false;
    for seg in name.split(|c: char| !c.is_ascii_alphabetic()).filter(|s| !s.is_empty()) {
        let known = en.is_word(seg) || is_extension(seg);
        if known && seg.len() >= 3 {
            has_word = true;
        }
        if !known && seg.len() > 2 && en.unseen_bigrams(seg) > 0 {
            return (false, false);
        }
    }
    (true, has_word)
}

fn is_russian_word(w: &str) -> bool {
    let d = dict::get(Lang::Ru);
    !w.is_empty() && (d.is_word(w) || morph::form_rank(d, w).is_some())
}

/// `мфгдеюишт` → `vault.bin`: a Russian-layout token that is a file name in Latin.
pub fn from_ru_layout(token: &str) -> Option<String> {
    if layout::detect_script(token) != Some(Lang::Ru) {
        return None;
    }
    let chars: Vec<char> = token.chars().collect();
    let (head, tail) = split_tail(&chars);
    // `.` typed on the Russian layout is a dot too; everything else is re-keyed.
    let conv: String = head.iter().map(|&c| if c == '.' { '.' } else { layout::convert_char(c, Lang::Ru) }).collect();
    let (name, ext) = conv.rsplit_once('.')?;
    if ext.len() < MIN_EXT_CONVERT || !is_extension(ext) {
        return None;
    }
    let (plausible, has_word) = latin_name(name);
    if !plausible {
        return None;
    }
    // The Russian text before the separator: a real Russian name (`отчет`) stays.
    let ru_name: String = head[..name.chars().count()].iter().collect();
    let ru_letters: String = ru_name.chars().filter(|c| c.is_alphabetic()).collect();
    if is_russian_word(&ru_letters) {
        return None;
    }
    let ru_core: String = head.iter().filter(|c| c.is_alphabetic()).collect();
    if is_russian_word(&ru_core) {
        return None;
    }
    let odd_russian = ru_name
        .split(|c: char| !c.is_alphabetic())
        .any(|s| dict::get(Lang::Ru).unseen_bigrams(s) > 0);
    if !(has_word || odd_russian) {
        return None;
    }
    let tail: String = tail.iter().collect();
    Some(conv + &tail)
}

/// `vault.bin` typed in English: a file name, never converted to Russian.
pub fn is_en_filename(token: &str) -> bool {
    if layout::detect_script(token) != Some(Lang::En) {
        return false;
    }
    let chars: Vec<char> = token.chars().collect();
    let (head, _) = split_tail(&chars);
    let head: String = head.iter().collect();
    let Some((name, ext)) = head.rsplit_once('.') else { return false };
    if !is_extension(ext) || !latin_name(name).0 {
        return false;
    }
    // `.` is `ю` on the Russian layout: a real Russian word typed on EN (`k.,jdm` →
    // любовь) must still convert, so only guard when the Russian reading is junk.
    let ru = layout::convert(&head, Lang::En);
    !(layout::detect_script(&ru) == Some(Lang::Ru) && ru.chars().all(char::is_alphabetic) && is_russian_word(&ru))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ru_layout_file_names() {
        assert_eq!(from_ru_layout("мфгдеюишт").as_deref(), Some("vault.bin"));
        assert_eq!(from_ru_layout("Куфвьуюьв").as_deref(), Some("Readme.md"));
        assert_eq!(from_ru_layout("сщташпюещьд").as_deref(), Some("config.toml"));
        assert_eq!(from_ru_layout("ензув-0ю3ю59юефкюпя").as_deref(), Some("typed-0.3.59.tar.gz"));
        assert_eq!(from_ru_layout("ензув.ефкюпя").as_deref(), Some("typed.tar.gz"));
        assert_eq!(from_ru_layout("мфгдеюишт,").as_deref(), Some("vault.bin,"));
        assert_eq!(from_ru_layout("дфнщгеэкы").as_deref(), None, "no separator");
    }

    #[test]
    fn russian_words_stay() {
        for w in ["люблю", "сюда", "плюс", "брюки", "уютно", "делаю", "июнь", "отчет.вщсч", "мфгде"] {
            assert_eq!(from_ru_layout(w), None, "{w}");
        }
    }

    #[test]
    fn en_file_names_guarded() {
        for w in ["vault.bin", "typed-0.3.59.tar.gz", "README.md", "main.rs", "vault.bin."] {
            assert!(is_en_filename(w), "{w}");
        }
        for w in ["k.,jdm", "hello", "ghbdtn/", "e.nyj"] {
            assert!(!is_en_filename(w), "{w}");
        }
    }
}
