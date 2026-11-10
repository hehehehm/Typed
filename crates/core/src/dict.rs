//! Embedded word lists (vendored, offline) and letter-bigram statistics.
//!
//! Source: top 40 000 words of each language from hermitdave/FrequencyWords
//! (OpenSubtitles 2018 frequency lists, CC-BY-SA-4.0), filtered to pure
//! lowercase letters, `ё` folded to `е`, plus small curated extras
//! (`*_extra.txt`: chat slang, abbreviations, tech terms, rare/old words). See `data/README.md`.
//! Urban Dictionary content is deliberately NOT bundled.
//!
//! Since 0.3.64 each language also has a full-form FST set (`*_known.fst`, ~3.2 M RU /
//! 0.8 M EN forms from OpenCorpora, Hunspell, SCOWL, Wiktionary, wordfreq). It is only for
//! "is this a real word" ([`Dictionary::is_word`]); ranks and fix targets stay with the lists.

use crate::layout::Lang;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Process-wide switch for the full-form sets (on by default; audits compare both).
static FULL_FORMS: AtomicBool = AtomicBool::new(true);

/// Turn the full-form "known words" sets on or off (before/after audits).
pub fn set_full_forms_enabled(on: bool) {
    FULL_FORMS.store(on, Ordering::Relaxed);
}

static EN_WORDS: &str = include_str!("../data/en.txt");
static RU_WORDS: &str = include_str!("../data/ru.txt");
static EN_EXTRA: &str = include_str!("../data/en_extra.txt");
static RU_EXTRA: &str = include_str!("../data/ru_extra.txt");
/// Full-form "known words" sets (every case, number, gender, tense…), see `data/README.md`.
/// FST sets over [`key_bytes`]; used only to answer "is this a real word", never to
/// offer a correction target. Memory-mapped from the binary, no start-up cost.
static EN_KNOWN: &[u8] = include_bytes!("../data/en_known.fst");
static RU_KNOWN: &[u8] = include_bytes!("../data/ru_known.fst");

/// Rank given to curated extras that are not in the frequency list: frequent enough to
/// pass the short-word check in the engine (so `дщд` → `lol` works).
pub const EXTRA_RANK: u32 = 2_000;

/// Only the first this-many frequency-list words feed the bigram statistics, so growing
/// the list does not dilute the "unusual bigram" heuristic.
pub const BIGRAM_SOURCE_WORDS: usize = 20_000;

/// See [`Dictionary::unseen_bigrams`].
pub const MIN_BIGRAM_WORDS: u32 = 10;

/// Word list for one language with frequency ranks (0 = most frequent).
pub struct Dictionary {
    lang: Lang,
    ranks: HashMap<String, u32>,
    bigrams: HashSet<(char, char)>,
    /// All words sorted, for prefix lookups.
    sorted: Vec<String>,
    /// Full-form set without ranks (see [`Dictionary::is_word`]).
    known: Option<fst::Set<&'static [u8]>>,
}

/// One byte per letter for the full-form sets: `a`–`z` as ASCII, `а`–`я` as
/// 0xE0–0xFF (the cp1251 positions), `ё` folded into `е`. `None` for anything else.
pub fn key_bytes(word: &str) -> Option<Vec<u8>> {
    word.chars()
        .map(|c| match c {
            'a'..='z' => Some(c as u8),
            'ё' => Some(0xE5),
            'а'..='я' => Some((c as u32 - 'а' as u32 + 0xE0) as u8),
            _ => None,
        })
        .collect()
}

impl Dictionary {
    pub fn from_list(lang: Lang, list: &str) -> Self {
        Self::with_extras(lang, list, "")
    }

    /// Frequency list plus curated extras (`#` comment lines ignored). Extras get rank
    /// `min(list rank, EXTRA_RANK)` so short ones pass the engine's short-word check.
    pub fn with_extras(lang: Lang, list: &str, extras: &str) -> Self {
        let mut ranks = HashMap::new();
        let mut counts: HashMap<(char, char), u32> = HashMap::new();
        let count = |w: &str, counts: &mut HashMap<(char, char), u32>| {
            let chars: Vec<char> = w.chars().collect();
            for p in chars.windows(2) {
                *counts.entry((p[0], p[1])).or_default() += 1;
            }
        };
        for (i, w) in list.lines().map(str::trim).filter(|w| !w.is_empty() && !w.starts_with('#')).enumerate() {
            ranks.entry(w.to_string()).or_insert(i as u32);
            if i < BIGRAM_SOURCE_WORDS {
                count(w, &mut counts);
            }
        }
        for w in extras.lines().map(str::trim).filter(|w| !w.is_empty() && !w.starts_with('#')) {
            let w = Self::normalize(w);
            match ranks.get_mut(&w) {
                Some(r) => *r = (*r).min(EXTRA_RANK),
                None => {
                    count(&w, &mut counts);
                    ranks.insert(w, EXTRA_RANK);
                }
            }
        }
        // Bigrams appearing in fewer than MIN_BIGRAM_WORDS vocabulary words are treated as
        // "unusual" for the language (compounds and loanwords produce rare noise pairs).
        let bigrams = counts.into_iter().filter(|&(_, n)| n >= MIN_BIGRAM_WORDS).map(|(k, _)| k).collect();
        let mut sorted: Vec<String> = ranks.keys().cloned().collect();
        sorted.sort_unstable();
        Dictionary { lang, ranks, bigrams, sorted, known: None }
    }

    /// Attach a full-form set built by `examples/build_wordset.rs`.
    pub fn with_known_set(mut self, fst_bytes: &'static [u8]) -> Self {
        self.known = fst::Set::new(fst_bytes).ok();
        self
    }

    /// Number of words in the full-form set (0 without one).
    pub fn known_len(&self) -> usize {
        self.known.as_ref().map_or(0, |s| s.len())
    }

    /// In the full-form set only (normalised or not). Rare forms live here: they are
    /// real words, but have no frequency rank and are never correction targets.
    pub fn in_full_forms(&self, word: &str) -> bool {
        let Some(set) = &self.known else { return false };
        if !FULL_FORMS.load(Ordering::Relaxed) {
            return false;
        }
        key_bytes(&Self::normalize(word)).is_some_and(|k| set.contains(k))
    }

    /// A real word of this language: ranked list, curated extras or full-form set.
    /// Use for "do not touch this word" checks; use [`Dictionary::rank`] to pick fixes.
    pub fn is_word(&self, word: &str) -> bool {
        self.contains(word) || self.in_full_forms(word)
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }
    pub fn len(&self) -> usize {
        self.ranks.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ranks.is_empty()
    }

    /// Normalised lookup key: lowercase, `ё` → `е`.
    pub fn normalize(word: &str) -> String {
        word.chars().flat_map(char::to_lowercase).map(|c| if c == 'ё' { 'е' } else { c }).collect()
    }

    /// Frequency rank of the word, if known.
    pub fn rank(&self, word: &str) -> Option<u32> {
        self.ranks.get(&Self::normalize(word)).copied()
    }
    /// All words with their ranks (normalised form).
    pub fn iter(&self) -> impl Iterator<Item = (&str, u32)> {
        self.ranks.iter().map(|(w, &r)| (w.as_str(), r))
    }

    /// Rank of an already normalised word (no case folding, no allocation).
    pub fn rank_exact(&self, word: &str) -> Option<u32> {
        self.ranks.get(word).copied()
    }

    /// Some word starts with `prefix` (normalised).
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let i = self.sorted.partition_point(|w| w.as_str() < prefix);
        self.sorted.get(i).is_some_and(|w| w.starts_with(prefix))
    }

    pub fn contains(&self, word: &str) -> bool {
        self.rank(word).is_some()
    }

    /// Number of letter bigrams in `word` that are unusual in this language's vocabulary.
    pub fn unseen_bigrams(&self, word: &str) -> usize {
        let chars: Vec<char> = Self::normalize(word).chars().collect();
        chars.windows(2).filter(|p| !self.bigrams.contains(&(p[0], p[1]))).count()
    }
}

/// Embedded full-form FST bytes for `lang` (for audits building their own dictionaries).
pub fn full_forms_fst(lang: Lang) -> &'static [u8] {
    match lang {
        Lang::En => EN_KNOWN,
        Lang::Ru => RU_KNOWN,
    }
}

/// Ranked list plus extras only, without the full-form set (audits and tests).
pub fn ranked_only(lang: Lang) -> Dictionary {
    match lang {
        Lang::En => Dictionary::with_extras(Lang::En, EN_WORDS, EN_EXTRA),
        Lang::Ru => Dictionary::with_extras(Lang::Ru, RU_WORDS, RU_EXTRA),
    }
}

/// Lazily built, process-wide dictionary for `lang`.
pub fn get(lang: Lang) -> &'static Dictionary {
    static EN: OnceLock<Dictionary> = OnceLock::new();
    static RU: OnceLock<Dictionary> = OnceLock::new();
    match lang {
        Lang::En => EN.get_or_init(|| Dictionary::with_extras(Lang::En, EN_WORDS, EN_EXTRA).with_known_set(EN_KNOWN)),
        Lang::Ru => RU.get_or_init(|| Dictionary::with_extras(Lang::Ru, RU_WORDS, RU_EXTRA).with_known_set(RU_KNOWN)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert!(get(Lang::En).len() >= 5000);
        assert!(get(Lang::Ru).len() >= 5000);
    }

    #[test]
    fn lookups() {
        assert!(get(Lang::En).contains("peace"));
        assert!(get(Lang::En).contains("Hello"));
        assert!(get(Lang::Ru).contains("привет"));
        assert!(get(Lang::Ru).contains("ещё"));
        assert!(!get(Lang::Ru).contains("зуфсу"));
        assert!(!get(Lang::En).contains("ghbdtn"));
    }

    #[test]
    fn extras_present() {
        assert!(get(Lang::En).contains("lol"));
        assert!(get(Lang::En).contains("exe"));
        assert!(get(Lang::En).contains("cicd"));
        for w in ["graphql", "oauth", "unesco", "covid", "lidar"] { assert!(get(Lang::En).contains(w), "{w}"); }
        for w in ["мчс", "снилс", "ковид", "мкс", "егэ", "гост"] { assert!(get(Lang::Ru).contains(w), "{w}"); }
        assert!(get(Lang::Ru).contains("имхо"));
        assert!(get(Lang::En).rank("brb").unwrap() <= EXTRA_RANK);
        assert!(!get(Lang::En).contains("# curated"));
    }

    #[test]
    fn bigrams() {
        assert_eq!(get(Lang::En).unseen_bigrams("peace"), 0);
        assert!(get(Lang::En).unseen_bigrams("ghbdtn") > 0);
        assert!(get(Lang::Ru).unseen_bigrams("зуфсу") > 0);
    }
}
