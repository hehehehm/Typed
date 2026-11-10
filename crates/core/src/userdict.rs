//! User-defined words and abbreviations (`custom_words` in the config).
//!
//! They are kept outside the process-wide built-in dictionaries so the daemon can refresh
//! them on every config reload. In the engine they take priority over built-in words:
//! if the converted form is a user word, the token is converted even when the typed form
//! is a known word (add `exe` → typing `учу` gives `exe`).

use crate::dict::Dictionary;
use crate::layout::{self, Lang};
use std::collections::HashSet;

/// Why a custom word was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordError {
    Empty,
    Phrase,
    NotLetters,
    MixedScript,
}

impl WordError {
    /// Short Russian status message for the GUI.
    pub fn message_ru(self) -> &'static str {
        match self {
            WordError::Empty => "Пустое слово",
            WordError::Phrase => "Только одно слово: словосочетания пока не поддерживаются",
            WordError::NotLetters => "Нужна хотя бы одна буква",
            WordError::MixedScript => "Слово должно быть целиком латиницей или целиком кириллицей",
        }
    }
}

/// Validate a custom word and return its language (Latin → EN, Cyrillic → RU).
pub fn classify(word: &str) -> Result<Lang, WordError> {
    let w = word.trim();
    if w.is_empty() {
        return Err(WordError::Empty);
    }
    if w.chars().any(char::is_whitespace) {
        return Err(WordError::Phrase);
    }
    // Digits and signs are fine (mp3, c++, ci/cd); the language comes from the letters.
    if !w.chars().any(|c| c.is_alphabetic()) {
        return Err(WordError::NotLetters);
    }
    if w.chars().any(|c| c.is_alphabetic() && !layout::is_latin_letter(c) && !layout::is_cyrillic_letter(c)) {
        return Err(WordError::MixedScript);
    }
    layout::detect_script(w).ok_or(WordError::MixedScript)
}

/// Normalised per-language set of user words.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserWords {
    en: HashSet<String>,
    ru: HashSet<String>,
}

impl UserWords {
    /// Build from config entries; invalid entries are skipped silently.
    pub fn from_words<S: AsRef<str>>(words: &[S]) -> Self {
        let mut u = UserWords::default();
        for w in words {
            if let Ok(lang) = classify(w.as_ref()) {
                let key = Dictionary::normalize(w.as_ref().trim());
                match lang {
                    Lang::En => u.en.insert(key),
                    Lang::Ru => u.ru.insert(key),
                };
            }
        }
        u
    }

    pub fn contains(&self, word: &str, lang: Lang) -> bool {
        let set = match lang {
            Lang::En => &self.en,
            Lang::Ru => &self.ru,
        };
        !set.is_empty() && set.contains(&Dictionary::normalize(word))
    }

    pub fn len(&self) -> usize {
        self.en.len() + self.ru.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Case-insensitive duplicate check used by the GUI.
pub fn contains_word(list: &[String], word: &str) -> bool {
    let n = Dictionary::normalize(word.trim());
    list.iter().any(|w| Dictionary::normalize(w.trim()) == n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_words() {
        assert_eq!(classify("exe"), Ok(Lang::En));
        assert_eq!(classify(" Имхо "), Ok(Lang::Ru));
        assert_eq!(classify(""), Err(WordError::Empty));
        assert_eq!(classify("two words"), Err(WordError::Phrase));
        assert_eq!(classify("123"), Err(WordError::NotLetters));
        assert_eq!(classify("mp3"), Ok(Lang::En));
        assert_eq!(classify("c++"), Ok(Lang::En));
        assert_eq!(classify("ci/cd"), Ok(Lang::En));
        assert_eq!(classify("!?"), Err(WordError::NotLetters));
        assert_eq!(classify("exeшник"), Err(WordError::MixedScript));
    }

    #[test]
    fn sets_and_dedupe() {
        let u = UserWords::from_words(&["EXE", "Ёлка", "bad word", "42", "mp3"]);
        assert_eq!(u.len(), 3);
        assert!(u.contains("MP3", Lang::En));
        assert!(u.contains("exe", Lang::En));
        assert!(u.contains("елка", Lang::Ru));
        assert!(!u.contains("exe", Lang::Ru));
        assert!(contains_word(&["Exe".to_string()], "eXe"));
    }
}
