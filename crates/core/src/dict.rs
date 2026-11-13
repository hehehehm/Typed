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
/// Brand / product / company / service names (Wikidata CC0 + curated), since 0.3.65.
/// Known-only like the full-form sets: never offered as a correction.
static EN_BRANDS: &str = include_str!("../data/en_brands.txt");
static RU_BRANDS: &str = include_str!("../data/ru_brands.txt");

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
    /// Brand and product names (see [`Dictionary::with_brands`]).
    brands: HashSet<String>,
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
        Dictionary { lang, ranks, bigrams, sorted, known: None, brands: HashSet::new() }
    }

    /// Attach a full-form set built by `examples/build_wordset.rs`.
    pub fn with_known_set(mut self, fst_bytes: &'static [u8]) -> Self {
        self.known = fst::Set::new(fst_bytes).ok();
        self
    }

    /// Attach a brand-name list (`#` comment lines ignored). Brands count as full forms:
    /// real words that are never changed and never offered as fixes.
    pub fn with_brands(mut self, list: &str) -> Self {
        self.brands = list
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty() && !w.starts_with('#'))
            .map(Self::normalize)
            .collect();
        self
    }

    /// Number of brand names attached.
    pub fn brands_len(&self) -> usize {
        self.brands.len()
    }

    /// A brand / product name from the bundled list.
    pub fn is_brand(&self, word: &str) -> bool {
        !self.brands.is_empty() && FULL_FORMS.load(Ordering::Relaxed) && self.brands.contains(&Self::normalize(word))
    }

    /// Number of words in the full-form set (0 without one).
    pub fn known_len(&self) -> usize {
        self.known.as_ref().map_or(0, |s| s.len())
    }

    /// In the full-form set only (normalised or not). Rare forms live here: they are
    /// real words, but have no frequency rank and are never correction targets.
    pub fn in_full_forms(&self, word: &str) -> bool {
        if self.is_brand(word) {
            return true;
        }
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

    /// Ranked words starting with `prefix` (normalised), from the sorted list.
    pub fn starting_with<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, u32)> + 'a {
        let i = self.sorted.partition_point(|w| w.as_str() < prefix);
        self.sorted[i..]
            .iter()
            .take_while(move |w| w.starts_with(prefix))
            .filter_map(move |w| self.ranks.get(w).map(|&r| (w.as_str(), r)))
    }

    /// Words of the full-form set within `max` edits (Levenshtein plus swaps of two
    /// neighbour letters) of `word` (normalised letters), starting with one of `first`
    /// letters. A walk of the fst that stops as soon as a branch exceeds `max`, so it
    /// visits only a tiny part of the 3 M forms. At most `limit` results.
    pub fn full_forms_near(&self, word: &str, max: usize, first: &[char], limit: usize) -> Vec<String> {
        let Some(set) = &self.known else { return Vec::new() };
        if !FULL_FORMS.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let Some(key) = key_bytes(word) else { return Vec::new() };
        let firsts: Vec<u8> = first.iter().filter_map(|c| key_bytes(&c.to_string())).flatten().collect();
        let fst = set.as_fst();
        let n = key.len();
        let row0: Vec<usize> = (0..=n).collect();
        let mut out: Vec<Vec<u8>> = Vec::new();
        let mut path: Vec<u8> = Vec::new();
        let root = fst.root();
        for t in root.transitions() {
            if !firsts.contains(&t.inp) {
                continue;
            }
            near_walk(fst, fst.node(t.addr), t.inp, &key, max, &row0, None, 0, &mut path, &mut out, limit);
        }
        out.into_iter().map(|k| key_to_word(&k)).collect()
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

/// Embedded brand list for `lang` (for audits building their own dictionaries).
pub fn brands_list(lang: Lang) -> &'static str {
    match lang {
        Lang::En => EN_BRANDS,
        Lang::Ru => RU_BRANDS,
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
        Lang::En => EN.get_or_init(|| Dictionary::with_extras(Lang::En, EN_WORDS, EN_EXTRA).with_known_set(EN_KNOWN).with_brands(EN_BRANDS)),
        Lang::Ru => RU.get_or_init(|| Dictionary::with_extras(Lang::Ru, RU_WORDS, RU_EXTRA).with_known_set(RU_KNOWN).with_brands(RU_BRANDS)),
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
    fn brands_are_known_but_not_ranked() {
        for w in ["realme", "Realme", "tecno", "infinix", "redmi", "oneplus", "zscaler"] {
            assert!(get(Lang::En).is_word(w), "{w}");
            assert!(!get(Lang::En).contains(w), "{w} must not become a fix target");
        }
        for w in ["реалми", "сяоми", "хуавей", "хуавея", "айфона", "айфоне", "айфоном", "самсунга", "ютубе", "нейросеткой"] {
            assert!(get(Lang::Ru).is_word(w), "{w}");
        }
        assert!(get(Lang::En).brands_len() > 10_000);
        assert!(get(Lang::Ru).brands_len() > 3_000);
        // Short tokens are left out: they collide with mistyped words.
        assert!(!get(Lang::En).is_brand("kia") && !get(Lang::En).is_brand("bmw"));
        assert!(!get(Lang::En).is_brand("# brand"));
    }

    #[test]
    fn bigrams() {
        assert_eq!(get(Lang::En).unseen_bigrams("peace"), 0);
        assert!(get(Lang::En).unseen_bigrams("ghbdtn") > 0);
        assert!(get(Lang::Ru).unseen_bigrams("зуфсу") > 0);
    }
}

/// Inverse of [`key_bytes`].
pub fn key_to_word(k: &[u8]) -> String {
    k.iter()
        .map(|&b| if b >= 0xE0 { char::from_u32(b as u32 - 0xE0 + 'а' as u32).unwrap_or('?') } else { b as char })
        .collect()
}

/// One step of [`Dictionary::full_forms_near`]: extend the path with byte `b`, compute the
/// next edit-distance row (optimal string alignment), recurse while it can still match.
#[allow(clippy::too_many_arguments)]
fn near_walk(
    fst: &fst::raw::Fst<&'static [u8]>,
    node: fst::raw::Node<'_>,
    b: u8,
    key: &[u8],
    max: usize,
    prev: &[usize],
    prev2: Option<&[usize]>,
    prev_b: u8,
    path: &mut Vec<u8>,
    out: &mut Vec<Vec<u8>>,
    limit: usize,
) {
    if out.len() >= limit {
        return;
    }
    let n = key.len();
    let i = path.len() + 1; // length of the candidate prefix incl. b
    let mut row = vec![0usize; n + 1];
    row[0] = i;
    let mut row_min = row[0];
    for j in 1..=n {
        let cost = usize::from(key[j - 1] != b);
        let mut v = (prev[j] + 1).min(row[j - 1] + 1).min(prev[j - 1] + cost);
        if let Some(p2) = prev2 {
            if j > 1 && key[j - 1] == prev_b && key[j - 2] == b {
                v = v.min(p2[j - 2] + 1);
            }
        }
        row[j] = v;
        row_min = row_min.min(v);
    }
    if row_min > max {
        return;
    }
    path.push(b);
    if node.is_final() && row[n] <= max && path.as_slice() != key {
        out.push(path.clone());
    }
    for t in node.transitions() {
        near_walk(fst, fst.node(t.addr), t.inp, key, max, &row, Some(prev), b, path, out, limit);
        if out.len() >= limit {
            break;
        }
    }
    path.pop();
}
