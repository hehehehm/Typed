//! Physical-key layout maps: US QWERTY <-> Russian ЙЦУКЕН (Windows/macOS/X11 "ru" standard).
//!
//! Every pair is "the character the same physical key produces" in each layout,
//! so converting a string is a lossless re-interpretation of the keys pressed.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Input language / keyboard layout handled by LayoutKeeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Ru,
            Lang::Ru => Lang::En,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
        }
    }
    pub fn parse(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "eng" | "english" | "us" => Some(Lang::En),
            "ru" | "rus" | "russian" => Some(Lang::Ru),
            _ => None,
        }
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// (US char, RU char) produced by the same physical key, unshifted and shifted.
const PAIRS: &[(char, char)] = &[
    ('`', 'ё'), ('~', 'Ё'),
    ('q', 'й'), ('w', 'ц'), ('e', 'у'), ('r', 'к'), ('t', 'е'), ('y', 'н'),
    ('u', 'г'), ('i', 'ш'), ('o', 'щ'), ('p', 'з'), ('[', 'х'), (']', 'ъ'),
    ('a', 'ф'), ('s', 'ы'), ('d', 'в'), ('f', 'а'), ('g', 'п'), ('h', 'р'),
    ('j', 'о'), ('k', 'л'), ('l', 'д'), (';', 'ж'), ('\'', 'э'),
    ('z', 'я'), ('x', 'ч'), ('c', 'с'), ('v', 'м'), ('b', 'и'), ('n', 'т'),
    ('m', 'ь'), (',', 'б'), ('.', 'ю'), ('/', '.'),
    ('Q', 'Й'), ('W', 'Ц'), ('E', 'У'), ('R', 'К'), ('T', 'Е'), ('Y', 'Н'),
    ('U', 'Г'), ('I', 'Ш'), ('O', 'Щ'), ('P', 'З'), ('{', 'Х'), ('}', 'Ъ'),
    ('A', 'Ф'), ('S', 'Ы'), ('D', 'В'), ('F', 'А'), ('G', 'П'), ('H', 'Р'),
    ('J', 'О'), ('K', 'Л'), ('L', 'Д'), (':', 'Ж'), ('"', 'Э'),
    ('Z', 'Я'), ('X', 'Ч'), ('C', 'С'), ('V', 'М'), ('B', 'И'), ('N', 'Т'),
    ('M', 'Ь'), ('<', 'Б'), ('>', 'Ю'), ('?', ','),
    // Shifted digit row where the layouts differ.
    ('@', '"'), ('#', '№'), ('$', ';'), ('^', ':'), ('&', '?'),
];

/// Character produced in Russian layout by the key that gives `c` in US layout.
pub fn en_to_ru(c: char) -> Option<char> {
    PAIRS.iter().find(|p| p.0 == c).map(|p| p.1)
}

/// Character produced in US layout by the key that gives `c` in Russian layout.
pub fn ru_to_en(c: char) -> Option<char> {
    PAIRS.iter().find(|p| p.1 == c).map(|p| p.0)
}

/// Map one character typed in layout `from` to what the same key gives in the other layout.
/// Characters that are identical in both layouts (digits, space, `-`, `=` …) pass through.
pub fn convert_char(c: char, from: Lang) -> char {
    match from {
        Lang::En => en_to_ru(c),
        Lang::Ru => ru_to_en(c),
    }
    .unwrap_or(c)
}

/// Re-interpret the whole string as if it had been typed in the other layout.
pub fn convert(s: &str, from: Lang) -> String {
    s.chars().map(|c| convert_char(c, from)).collect()
}

/// Convert layout-dependent punctuation without turning literal punctuation into letters.
/// For example, EN `?` becomes RU `,`, but EN `,` stays `,` rather than becoming `б`.
pub fn convert_punctuation(s: &str, from: Lang) -> String {
    s.chars().map(|c| {
        let mapped = convert_char(c, from);
        if !c.is_alphabetic() && !mapped.is_alphabetic() { mapped } else { c }
    }).collect()
}

/// Flip a selected text to the other layout ("convert selection" hotkey).
/// The source layout is the script with more letters; words already in the other
/// script stay as they are. Returns the new text and the layout it is now in,
/// or `None` when there is nothing to convert.
pub fn convert_text(text: &str) -> Option<(String, Lang)> {
    let lat = text.chars().filter(|&c| is_latin_letter(c)).count();
    let cyr = text.chars().filter(|&c| is_cyrillic_letter(c)).count();
    let from = match (lat, cyr) {
        (0, 0) => return None,
        (l, c) if l >= c => Lang::En,
        _ => Lang::Ru,
    };
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if detect_script(word).map_or(true, |s| s == from) {
            out.push_str(&convert(word, from));
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_whitespace() {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    Some((out, from.other()))
}

pub fn is_latin_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
}

pub fn is_cyrillic_letter(c: char) -> bool {
    matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

/// Script of a token judged by its letters only; `None` for no letters or mixed scripts.
pub fn detect_script(s: &str) -> Option<Lang> {
    let lat = s.chars().filter(|&c| is_latin_letter(c)).count();
    let cyr = s.chars().filter(|&c| is_cyrillic_letter(c)).count();
    match (lat, cyr) {
        (0, 0) => None,
        (_, 0) => Some(Lang::En),
        (0, _) => Some(Lang::Ru),
        _ => None,
    }
}

/// True when `c` is (or, on the same key in the other layout, would be) a letter.
/// Such characters belong to a word even if they look like punctuation (`,` = `б`).
pub fn is_word_char(c: char) -> bool {
    if c.is_alphanumeric() {
        return true;
    }
    en_to_ru(c).is_some_and(|r| r.is_alphabetic()) || ru_to_en(c).is_some_and(|e| e.is_alphabetic())
}

/// For text injection by physical key: which US-layout character (key + shift)
/// produces `c` when layout `target` is active. `None` if no single key does.
pub fn us_key_for(c: char, target: Lang) -> Option<char> {
    match target {
        Lang::En => (c.is_ascii_graphic() || c == ' ').then_some(c),
        Lang::Ru => {
            if let Some(e) = ru_to_en(c) {
                Some(e)
            } else if en_to_ru(c).is_none() && (c.is_ascii_graphic() || c == ' ') {
                // Same character on the same key in both layouts (digits, '-', '=', '!' …).
                Some(c)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_is_bijective() {
        for (i, a) in PAIRS.iter().enumerate() {
            for b in &PAIRS[i + 1..] {
                assert_ne!(a.0, b.0, "duplicate US {:?}", a.0);
                assert_ne!(a.1, b.1, "duplicate RU {:?}", a.1);
            }
        }
    }

    #[test]
    fn zufsu_is_peace() {
        assert_eq!(convert("зуфсу", Lang::Ru), "peace");
        assert_eq!(convert("peace", Lang::En), "зуфсу");
    }

    #[test]
    fn examples_both_ways() {
        for (ru, en) in [("руддщ", "hello"), ("привет", "ghbdtn"), ("дела", "ltkf")] {
            assert_eq!(convert(ru, Lang::Ru), en);
            assert_eq!(convert(en, Lang::En), ru);
        }
    }

    #[test]
    fn case_and_punct() {
        assert_eq!(convert("Ghbdtn", Lang::En), "Привет");
        assert_eq!(convert("GHBDTN", Lang::En), "ПРИВЕТ");
        assert_eq!(convert("ytn, z", Lang::En), "нетб я");
        assert_eq!(convert("[jhjij", Lang::En), "хорошо");
        assert_eq!(convert("vj;yj", Lang::En), "можно");
    }

    #[test]
    fn script_detection() {
        assert_eq!(detect_script("peace"), Some(Lang::En));
        assert_eq!(detect_script("зуфсу"), Some(Lang::Ru));
        assert_eq!(detect_script("pзу"), None);
        assert_eq!(detect_script("123"), None);
    }

    #[test]
    fn injection_keys() {
        assert_eq!(us_key_for('п', Lang::Ru), Some('g'));
        assert_eq!(us_key_for(',', Lang::Ru), Some('?'));
        assert_eq!(us_key_for('1', Lang::Ru), Some('1'));
        assert_eq!(us_key_for('a', Lang::En), Some('a'));
        assert_eq!(us_key_for('ж', Lang::En), None);
    }
    #[test]
    fn punctuation_only_mapping() {
        assert_eq!(convert_punctuation("?/&#$^@", Lang::En), ",.?№;:\"");
        assert_eq!(convert_punctuation(",.! 123abc", Lang::En), ",.! 123abc");
        assert_eq!(convert_punctuation(",.?№;:\"", Lang::Ru), "?/&#$^@");
    }


    #[test]
    fn convert_selected_text() {
        assert_eq!(convert_text("ghbdtn? rfr ltkf"), Some(("привет, как дела".into(), Lang::Ru)));
        assert_eq!(convert_text("руддщ цщкдв"), Some(("hello world".into(), Lang::En)));
        // Words already in the other script are kept.
        assert_eq!(convert_text("ghbdtn Москва"), Some(("привет Москва".into(), Lang::Ru)));
        assert_eq!(convert_text("line1\nyt nfr\t"), Some(("дшту1\nне так\t".into(), Lang::Ru)));
        assert_eq!(convert_text("123 !!"), None);
    }

}
