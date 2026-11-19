//! Wider, scored typo search (since 0.3.72), tried when the strict rules in [`crate::typo`]
//! leave a word alone and the level is not [`Level::Cautious`].
//!
//! * Budget by length (in tenths of an edit): 3–4 letters one slip, 5–8 two, 9+ three
//!   ([`budget`]). Slips are weighted by how a hand makes them ([`wdist`]): a doubled or
//!   dropped double letter and two swapped neighbours are cheap, a neighbour key is one
//!   slip, a far key costs more than two (never in a one- or two-slip budget).
//! * Candidates: words of the 40 000-word list that keep the first letter (or swap the
//!   first two). Score = edit cost × [`EDIT_WEIGHT`] + ln(rank + [`RANK_OFFSET`]), minus a
//!   bonus for keeping the typed last two letters (the case ending).
//! * Ambiguity guard: the runner-up must score at least [`margin`] worse, otherwise the
//!   word is left alone (`тта` is not guessed into `там`).
//! * Same safety rules as the strict search: not just another ending of the same word,
//!   rare candidates (rank 20 000+) keep the typed last two letters, Capitalised / UPPER
//!   words (often names) need 5+ letters, a frequent target and a wider margin; 3–4 letter
//!   words only go to frequent words.
//!
//! [`layout_typo`]: a token typed in the wrong layout whose other-layout reading is a typo
//! (`ghbdtn` with a slip, `xnjj` → `что`).

use crate::dict::{self, Dictionary};
use crate::layout::{self, Lang};
use crate::morph;
use crate::typo::{adjacent_keys_raw, en_affix_only, en_end_letter_only, en_known_form, first_letter_kept, match_case, sound_alike, tail_ok, Level, MAX_RANK, TAIL_RANK};

/// Weight of one edit (in score units, natural log of frequency rank).
pub const EDIT_WEIGHT: f64 = 3.0;
/// Added to ranks before the log: the top words should not win by frequency alone.
pub const RANK_OFFSET: f64 = 100.0;
/// Score bonus for a candidate that keeps the typed last two letters.
pub const TAIL_BONUS: f64 = 0.8;

/// Edit budget for an `n`-letter word, in tenths of a slip.
pub fn budget(n: usize, level: Level) -> u32 {
    match (level, n) {
        (_, 0..=2) => 0,
        (Level::Cautious, _) => 0,
        (Level::Normal, 3) => 10,
        (Level::Normal, 4) => 12,
        (Level::Normal, 5..=6) => 20,
        (Level::Normal, 7..=8) => 22,
        (Level::Normal, _) => 32,
        (Level::Aggressive, 3) => 10,
        (Level::Aggressive, 4) => 16,
        (Level::Aggressive, 5..=6) => 22,
        (Level::Aggressive, 7) => 26,
        (Level::Aggressive, _) => 34,
    }
}

/// How much worse the runner-up must score.
pub fn margin(level: Level, capitalised: bool) -> f64 {
    let m = match level {
        Level::Aggressive => 1.1,
        _ => 1.8,
    };
    if capitalised { m + 0.8 } else { m }
}

/// Rank limit for targets of short typed words.
fn short_rank_limit(n: usize) -> u32 {
    match n {
        0..=3 => 2_000,
        4 => 8_000,
        _ => MAX_RANK,
    }
}

const TRANSPOSE: u32 = 8;
/// Letters only left out (`прграма` → `программа`) count without an unusual letter pair
/// when the target is this frequent.
pub const DROPPED_RANK: u32 = 10_000;

/// One word is the other plus letters at the end (`yandx` / `yan`, `cоn` / `const`):
/// another word, a name or a cut-off one, not a slip. A doubled last letter is a slip.
fn extra_or_cut_end(typed: &[char], cand: &[char]) -> bool {
    let (long, short) = if typed.len() > cand.len() { (typed, cand) } else { (cand, typed) };
    if long.len() == short.len() || !long.starts_with(short) {
        return false;
    }
    !long[short.len()..].iter().all(|&c| Some(&c) == short.last())
}

/// Cost of at most one slip (tenths).
pub const ONE_SLIP: u32 = 12;

/// Letter pairs precomputed per language: neighbour keys and sound-alike letters (the
/// DP asks millions of times; scanning the key rows each time was most of the cost).
struct Pairs {
    adjacent: Vec<bool>,
    sound: Vec<bool>,
}

const SLOTS: usize = 33;

fn slot(c: char, lang: Lang) -> Option<usize> {
    let base = match lang {
        Lang::Ru => 'а',
        Lang::En => 'a',
    } as u32;
    (c as u32).checked_sub(base).map(|i| i as usize).filter(|&i| i < SLOTS)
}

fn pairs(lang: Lang) -> &'static Pairs {
    static RU: std::sync::OnceLock<Pairs> = std::sync::OnceLock::new();
    static EN: std::sync::OnceLock<Pairs> = std::sync::OnceLock::new();
    let build = || {
        let base = match lang {
            Lang::Ru => 'а',
            Lang::En => 'a',
        } as u32;
        let ch = |i: usize| char::from_u32(base + i as u32).unwrap_or(' ');
        let mut p = Pairs { adjacent: vec![false; SLOTS * SLOTS], sound: vec![false; SLOTS * SLOTS] };
        for i in 0..SLOTS {
            for j in 0..SLOTS {
                p.adjacent[i * SLOTS + j] = adjacent_keys_raw(ch(i), ch(j), lang);
                p.sound[i * SLOTS + j] = sound_alike(ch(i), ch(j), lang);
            }
        }
        p
    };
    match lang {
        Lang::Ru => RU.get_or_init(build),
        Lang::En => EN.get_or_init(build),
    }
}

pub(crate) fn adjacent_fast(x: char, y: char, lang: Lang) -> bool {
    match (slot(x, lang), slot(y, lang)) {
        (Some(i), Some(j)) => pairs(lang).adjacent[i * SLOTS + j],
        _ => adjacent_keys_raw(x, y, lang),
    }
}

fn subst_cost(x: char, y: char, n: usize, lang: Lang) -> u32 {
    let (adj, sound) = match (slot(x, lang), slot(y, lang)) {
        (Some(i), Some(j)) => {
            let p = pairs(lang);
            (p.adjacent[i * SLOTS + j], p.sound[i * SLOTS + j])
        }
        _ => (adjacent_keys_raw(x, y, lang), sound_alike(x, y, lang)),
    };
    if adj {
        10
    } else if sound {
        if n > crate::typo::SHORT_SUBST_LEN { 11 } else { 15 }
    } else {
        24
    }
}

/// Typed `a[i]` is an extra letter.
fn del_cost(a: &[char], i: usize, lang: Lang) -> u32 {
    let c = a[i];
    let prev = i.checked_sub(1).map(|k| a[k]);
    let next = a.get(i + 1).copied();
    if prev == Some(c) || next == Some(c) {
        6
    } else if prev.is_some_and(|p| adjacent_fast(c, p, lang)) || next.is_some_and(|p| adjacent_fast(c, p, lang)) {
        9
    } else {
        13
    }
}

/// Candidate letter `b[j]` is missing from the typed word.
fn ins_cost(b: &[char], j: usize) -> u32 {
    let c = b[j];
    if (j > 0 && b[j - 1] == c) || b.get(j + 1) == Some(&c) {
        7
    } else {
        10
    }
}

/// Keyboard-weighted Damerau (OSA) distance in tenths of a slip, or `max + 1` past `max`.
pub fn wdist(a: &[char], b: &[char], max: u32, lang: Lang) -> u32 {
    let dels: Vec<u32> = (0..a.len()).map(|i| del_cost(a, i, lang)).collect();
    wdist_with(a, &dels, b, max, lang, &mut Scratch::default())
}

/// Reused rows for [`wdist_with`] (no allocation per candidate).
#[derive(Default)]
pub struct Scratch {
    prev2: Vec<u32>,
    prev: Vec<u32>,
    cur: Vec<u32>,
}

/// [`wdist`] with the typed word's deletion costs precomputed.
fn wdist_with(a: &[char], dels: &[u32], b: &[char], max: u32, lang: Lang, s: &mut Scratch) -> u32 {
    let (n, m) = (a.len(), b.len());
    for v in [&mut s.prev2, &mut s.prev, &mut s.cur] {
        v.clear();
        v.resize(m + 1, 0);
    }
    let (mut prev2, mut prev, mut cur) = (std::mem::take(&mut s.prev2), std::mem::take(&mut s.prev), std::mem::take(&mut s.cur));
    for j in 1..=m {
        prev[j] = prev[j - 1] + ins_cost(b, j - 1);
    }
    let mut result = None;
    for i in 1..=n {
        let d = dels[i - 1];
        cur[0] = prev[0] + d;
        let mut row_min = cur[0];
        for j in 1..=m {
            let sub = if a[i - 1] == b[j - 1] { 0 } else { subst_cost(a[i - 1], b[j - 1], n, lang) };
            let mut v = (prev[j] + d).min(cur[j - 1] + ins_cost(b, j - 1)).min(prev[j - 1] + sub);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] && a[i - 1] != a[i - 2] {
                v = v.min(prev2[j - 2] + TRANSPOSE);
            }
            cur[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            result = Some(max + 1);
            break;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    let r = result.unwrap_or(prev[m]);
    (s.prev2, s.prev, s.cur) = (prev2, prev, cur);
    r
}

#[derive(Debug, Clone)]
struct Scored {
    score: f64,
    word: String,
    ok: bool,
}

/// The scored search. `None` when nothing fits or the top two are too close.
pub fn suggest_wide(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool, level: Level) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n < 3 || !chars.iter().all(|c| c.is_alphabetic()) || layout::detect_script(word) != Some(lang) {
        return None;
    }
    let lowercase = chars.iter().all(|c| c.is_lowercase());
    let up = chars.iter().filter(|c| c.is_uppercase()).count();
    let natural = up == 0 || up == n || (up == 1 && chars[0].is_uppercase());
    if !natural || (!lowercase && n < 5) {
        return None;
    }
    let lower_s = Dictionary::normalize(word);
    if !assume_unknown {
        if d.is_word(word) {
            return None;
        }
        if lang == Lang::Ru && morph::is_known_form(d, &lower_s) {
            return None;
        }
        if lang == Lang::En && en_known_form(d, &lower_s) {
            return None;
        }
    }
    let max = budget(n, level);
    if max == 0 {
        return None;
    }
    let lower: Vec<char> = lower_s.chars().collect();
    let rank_limit = if lowercase { short_rank_limit(n) } else { TAIL_RANK };
    let first: String = lower[..1].iter().collect();
    let swapped: String = [lower[1], lower[0]].iter().collect();
    let pool = d.starting_with(&first).chain(d.starting_with(&swapped).filter(|_| lower[0] != lower[1]));
    let mut best: Option<Scored> = None;
    let mut second: Option<f64> = None;
    let tail2 = &lower[n - 2..];
    // Typos of two or more slips (and any fix of a Capitalised word) need a sign of a
    // slip in the typed word: a letter pair unusual in the language (`ghbdtn`-style
    // clusters, `тунт`). Rare real words and names are spelled with ordinary pairs.
    let odd = d.unseen_bigrams(&lower_s) >= 1;
    let dels: Vec<u32> = (0..n).map(|i| del_cost(&lower, i, lang)).collect();
    let mut scratch = Scratch::default();
    let mut cv: Vec<char> = Vec::with_capacity(n + 4);
    for (cand, r) in pool {
        if r >= MAX_RANK {
            continue;
        }
        cv.clear();
        cv.extend(cand.chars());
        let m = cv.len();
        if m < 2 || (m.abs_diff(n) as u32) * 6 > max || cand == lower_s {
            continue;
        }
        if !first_letter_kept(&lower, cand) {
            continue;
        }
        let cost = wdist_with(&lower, &dels, &cv, max, lang, &mut scratch);
        if cost > max || cost == 0 {
            continue;
        }
        // Candidates that may not win still compete: a two-letter word, another ending
        // of the same word, a rare word with another ending, a rare target of a short
        // word. They make the guess ambiguous (`нен`: `не` or `нее`).
        let ok = m >= 3
            && r < rank_limit
            && (r < TAIL_RANK || tail_ok(&lower, cand))
            && !(lang == Lang::Ru && morph::only_ending_differs(&lower_s, cand))
            && !(lang == Lang::En && (en_affix_only(&lower_s, cand) || en_end_letter_only(&lower_s, cand)))
            && (odd || (cost <= ONE_SLIP && lowercase) || (lowercase && m > n && r < DROPPED_RANK) || (lowercase && level == Level::Aggressive))
            && !extra_or_cut_end(&lower, &cv);
        if m < 3 && r >= crate::engine::SHORT_WORD_MAX_RANK {
            continue;
        }
        let mut score = cost as f64 / 10.0 * EDIT_WEIGHT + (r as f64 + RANK_OFFSET).ln();
        if cv.len() >= 2 && cv[cv.len() - 2..] == *tail2 {
            score -= TAIL_BONUS;
        }
        let s = Scored { score, word: cand.to_string(), ok };
        match &best {
            Some(b) if b.word == s.word => {}
            Some(b) if s.score < b.score => {
                second = Some(second.map_or(b.score, |x| x.min(b.score)));
                best = Some(s);
            }
            Some(_) => second = Some(second.map_or(s.score, |x| x.min(s.score))),
            None => best = Some(s),
        }
    }
    let best = best?;
    if !best.ok {
        return None;
    }
    if second.is_some_and(|s| s - best.score < margin(level, !lowercase)) {
        return None;
    }
    Some(match_case(word, &best.word))
}

/// Minimum letters for a wrong-layout + typo fix.
pub fn layout_typo_min_len(level: Level) -> usize {
    match level {
        Level::Aggressive => 3,
        _ => 4,
    }
}

/// `conv` (letters only, the token re-read in `dst`) is not a word of `dst` but a typo of
/// one: the fix. The caller checks the typed side (unknown, no typo fix of its own).
pub fn layout_typo(conv: &str, dst: Lang, level: Level) -> Option<String> {
    if level == Level::Cautious || conv.chars().count() < layout_typo_min_len(level) {
        return None;
    }
    if !conv.chars().all(char::is_alphabetic) || layout::detect_script(conv) != Some(dst) {
        return None;
    }
    let d = dict::get(dst);
    if d.is_word(conv) {
        return None;
    }
    crate::typo::suggest_level(conv, dst, level)
}

/// The typed side of a wrong-layout + typo fix looks foreign to its own language: it has
/// a letter pair unusual there (`xnjj`: `xn`), more than its other-layout reading has in
/// the other language. Unknown names and slang of the typed language usually don't, and
/// stay.
pub fn looks_foreign(typed_letters: &str, src: Lang) -> bool {
    let odd = dict::get(src).unseen_bigrams(typed_letters);
    let conv = layout::convert(typed_letters, src);
    let conv_letters: String = conv.chars().filter(|c| c.is_alphabetic()).collect();
    odd >= 1 && dict::get(src.other()).unseen_bigrams(&conv_letters) < odd
}

/// Audit helper: `t`, a real word of `lang` treated as an unknown one, read in the other
/// layout and typo-fixed there (what [`layout_typo`] would do to an unknown name).
#[doc(hidden)]
pub fn layout_typo_as_unknown(t: &str, lang: Lang) -> Option<String> {
    layout_typo_as_unknown_level(t, lang, Level::default())
}

#[doc(hidden)]
pub fn layout_typo_as_unknown_level(t: &str, lang: Lang, level: Level) -> Option<String> {
    if !looks_foreign(t, lang) {
        return None;
    }
    let conv = layout::convert(t, lang);
    layout_typo(&conv, lang.other(), level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typo::suggest;

    #[test]
    fn weighted_distance() {
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(wdist(&c("helllo"), &c("hello"), 30, Lang::En), 6);
        assert_eq!(wdist(&c("teh"), &c("the"), 30, Lang::En), 8);
        assert_eq!(wdist(&c("hrllo"), &c("hello"), 30, Lang::En), 10);
        assert!(wdist(&c("hzllo"), &c("hello"), 30, Lang::En) > 20);
    }

    #[test]
    fn short_words_when_unambiguous() {
        assert_eq!(suggest("teh", Lang::En).as_deref(), Some("the"));
        assert_eq!(suggest("adn", Lang::En).as_deref(), Some("and"));
        assert_eq!(suggest("чот", Lang::Ru).as_deref(), Some("что"));
        // Ambiguous or too far: left alone.
        assert_eq!(suggest("тта", Lang::Ru), None);
        // Capitalised 3–4 letter tokens are mostly names and abbreviations.
        assert_eq!(suggest("Teh", Lang::En), None);
    }

    #[test]
    fn more_slips_in_longer_words() {
        // Two slips from 5 letters.
        assert_eq!(suggest("превт", Lang::Ru).as_deref(), Some("привет"));
        assert_eq!(suggest("првте", Lang::Ru).as_deref(), Some("привет"));
        assert_eq!(suggest("сгоднч", Lang::Ru).as_deref(), Some("сегодня"));
        assert_eq!(suggest("wierd", Lang::En).as_deref(), Some("weird"));
        // The «double error» case: fixed only because the scored check clearly supports
        // it (one close frequent word, the runner-up far behind).
        assert_eq!(suggest("ввесия", Lang::Ru).as_deref(), Some("версия"));
        // The strict rules alone (Level::Cautious) leave all of these.
        for (w, l) in [("превт", Lang::Ru), ("сгоднч", Lang::Ru), ("wierd", Lang::En), ("ввесия", Lang::Ru)] {
            assert_eq!(crate::typo::suggest_level(w, l, Level::Cautious), None, "{w}");
        }
    }

    #[test]
    fn names_brands_and_code_stay() {
        for w in ["Ксюха", "Серёга", "Михалыч", "Шнуров", "Xiaomi", "Huawei", "GitHub", "Kubernetes", "serde", "tokio", "eprintln", "кринж", "рофл"] {
            let l = layout::detect_script(w).unwrap();
            assert_eq!(suggest(w, l), None, "{w}");
        }
        // Letters added at the end make another word or a name, not a slip.
        assert_eq!(suggest("Yandx", Lang::En), None);
    }

    #[test]
    fn layout_and_typo() {
        assert_eq!(layout_typo("чтоо", Lang::Ru, Level::Normal).as_deref(), Some("что"));
        assert_eq!(layout_typo("чтоо", Lang::Ru, Level::Cautious), None);
        // A real word is not "fixed".
        assert_eq!(layout_typo("привет", Lang::Ru, Level::Normal), None);
        // The typed side must look foreign: `xnjj` has `xn`, a real-looking English
        // name does not.
        assert!(looks_foreign("xnjj", Lang::En));
        assert!(!looks_foreign("bizon", Lang::En));
    }
}
