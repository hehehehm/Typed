
use serde::{Deserialize, Serialize};
use std::fmt;

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
    ('@', '"'), ('#', '№'), ('$', ';'), ('^', ':'), ('&', '?'),
];

pub fn en_to_ru(c: char) -> Option<char> {
    PAIRS.iter().find(|p| p.0 == c).map(|p| p.1)
}

pub fn ru_to_en(c: char) -> Option<char> {
    PAIRS.iter().find(|p| p.1 == c).map(|p| p.0)
}

pub fn convert_char(c: char, from: Lang) -> char {
    match from {
        Lang::En => en_to_ru(c),
        Lang::Ru => ru_to_en(c),
    }
    .unwrap_or(c)
}

pub fn convert(s: &str, from: Lang) -> String {
    s.chars().map(|c| convert_char(c, from)).collect()
}

pub fn convert_punctuation(s: &str, from: Lang) -> String {
    s.chars().map(|c| {
        let mapped = convert_char(c, from);
        if !c.is_alphabetic() && !mapped.is_alphabetic() { mapped } else { c }
    }).collect()
}

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

pub fn is_word_char(c: char) -> bool {
    if c.is_alphanumeric() {
        return true;
    }
    en_to_ru(c).is_some_and(|r| r.is_alphabetic()) || ru_to_en(c).is_some_and(|e| e.is_alphabetic())
}

pub fn us_key_for(c: char, target: Lang) -> Option<char> {
    match target {
        Lang::En => (c.is_ascii_graphic() || c == ' ').then_some(c),
        Lang::Ru => {
            if let Some(e) = ru_to_en(c) {
                Some(e)
            } else if en_to_ru(c).is_none() && (c.is_ascii_graphic() || c == ' ') {
                Some(c)
            } else {
                None
            }
        }
    }
}

const PHYS_ROWS: [&str; 4] = [
    "`1234567890-=",
    "qwertyuiop[]",
    "asdfghjkl;'",
    "zxcvbnm,./",
];

const US_SHIFT: &[(char, char)] = &[
    ('`', '~'), ('1', '!'), ('2', '@'), ('3', '#'), ('4', '$'),
    ('5', '%'), ('6', '^'), ('7', '&'), ('8', '*'), ('9', '('),
    ('0', ')'), ('-', '_'), ('=', '+'),
    ('[', '{'), (']', '}'), (';', ':'), ('\'', '"'),
    (',', '<'), ('.', '>'), ('/', '?'),
];

fn unshift_us(us: char) -> (char, bool) {
    if us.is_ascii_uppercase() {
        return (us.to_ascii_lowercase(), true);
    }
    for &(lo, hi) in US_SHIFT {
        if us == hi {
            return (lo, true);
        }
    }
    (us, false)
}

fn apply_us_shift(phys: char, shift: bool) -> char {
    if !shift {
        return phys;
    }
    if phys.is_ascii_lowercase() {
        return phys.to_ascii_uppercase();
    }
    for &(lo, hi) in US_SHIFT {
        if phys == lo {
            return hi;
        }
    }
    phys
}

pub fn shift_phys(phys: char, dir: i32) -> Option<char> {
    if dir == 0 {
        return Some(phys);
    }
    for row in PHYS_ROWS {
        if let Some(i) = row.chars().position(|c| c == phys) {
            let j = i as i32 + dir;
            if j < 0 {
                return None;
            }
            return row.chars().nth(j as usize);
        }
    }
    None
}

pub fn shift_char(c: char, from: Lang, to: Lang, dir: i32) -> Option<char> {
    let us = us_key_for(c, from)?;
    let (phys, shift) = unshift_us(us);
    let phys2 = shift_phys(phys, dir)?;
    let us2 = apply_us_shift(phys2, shift);
    Some(match to {
        Lang::En => us2,
        Lang::Ru => en_to_ru(us2).unwrap_or(us2),
    })
}

pub fn shift_token(s: &str, from: Lang, to: Lang, dir: i32) -> Option<String> {
    if dir != 1 && dir != -1 {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    let mut n = 0;
    for c in s.chars() {
        if c.is_whitespace() {
            return None;
        }
        out.push(shift_char(c, from, to, dir)?);
        n += 1;
    }
    (n > 0).then_some(out)
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
        assert_eq!(convert_text("ghbdtn Москва"), Some(("привет Москва".into(), Lang::Ru)));
        assert_eq!(convert_text("line1\nyt nfr\t"), Some(("дшту1\nне так\t".into(), Lang::Ru)));
        assert_eq!(convert_text("123 !!"), None);
    }

    #[test]
    fn hand_shift_one_column() {
        assert_eq!(shift_token("привет", Lang::Ru, Lang::Ru, 1).as_deref(), Some("ротань"));
        assert_eq!(shift_token("ротань", Lang::Ru, Lang::Ru, -1).as_deref(), Some("привет"));
        assert_eq!(shift_token("hello", Lang::En, Lang::En, 1).as_deref(), Some("jr;;p"));
        assert_eq!(shift_token("jr;;p", Lang::En, Lang::En, -1).as_deref(), Some("hello"));
        assert_eq!(shift_token("hello", Lang::En, Lang::Ru, 1).as_deref(), Some("окжжз"));
        assert_eq!(shift_token("окжжз", Lang::Ru, Lang::En, -1).as_deref(), Some("hello"));
        assert_eq!(shift_token("привет", Lang::Ru, Lang::En, 1).as_deref(), Some("hjnfym"));
        assert_eq!(shift_token("hjnfym", Lang::En, Lang::Ru, -1).as_deref(), Some("привет"));
        assert_eq!(shift_token("фыва", Lang::Ru, Lang::Ru, -1), None);
        assert_eq!(shift_phys('q', -1), None);
        assert_eq!(shift_phys('p', 1), Some('['));
    }

}
